"""Tools for the triage agent, declared for Gemini function calling.

The interface encodes the idempotency policy (docs/architecture.md §11): every
finding ends as exactly one ledger event — ticketed, duplicate_closed, or ignored.
Bodies are synchronous (Firestore/Pub/Sub clients are blocking).
"""

from __future__ import annotations

import json
from datetime import datetime, timezone
from functools import lru_cache
from typing import Any
from uuid import uuid4

from google.cloud import pubsub_v1
from google.genai import types

from . import store
from .config import Config
from .models import Evidence, Ticket


@lru_cache(maxsize=1)
def _publisher() -> pubsub_v1.PublisherClient:
    return pubsub_v1.PublisherClient()


FIND_DUPLICATE = types.FunctionDeclaration(
    name="find_duplicate",
    description="Check whether a finding signature already has a registered known issue.",
    parameters_json_schema={
        "type": "object",
        "properties": {"signature": {"type": "string"}},
        "required": ["signature"],
    },
)

CREATE_TICKET = types.FunctionDeclaration(
    name="create_ticket",
    description=(
        "Create a NEW ticket for a new, actionable problem: publishes it to the "
        "tickets topic, registers the known issue, and records a `ticketed` event."
    ),
    parameters_json_schema={
        "type": "object",
        "properties": {
            "service": {"type": "string"},
            "severity": {"type": "string", "enum": ["S1", "S2", "S3", "S4"]},
            "title": {"type": "string"},
            "description": {"type": "string", "description": "Full description of the problem."},
            "steps_gherkin": {
                "type": "string",
                "description": "Steps to reproduce in Gherkin (Given/When/Then, Scenario:).",
            },
            "expected_state": {"type": "string", "description": "Correct behavior."},
            "current_state": {"type": "string", "description": "Actual broken behavior."},
            "actual_log": {"type": "string", "description": "A representative log line."},
            "log_timestamp": {"type": "string", "description": "Timestamp of that line."},
            "root_cause_hypothesis": {"type": "string"},
            "potential_resolution": {"type": "string", "description": "Concrete fix direction."},
            "justification": {"type": "string", "description": "Why actionable + this severity."},
            "suggested_fix": {"type": "string"},
            "signature": {"type": "string"},
            "finding_id": {"type": "string"},
            "log_query": {"type": "string"},
            "sample_trace_ids": {"type": "array", "items": {"type": "string"}},
            "count": {"type": "integer"},
            "window": {"type": "string"},
            "grounding_refs": {"type": "array", "items": {"type": "string"}},
        },
        "required": [
            "service", "severity", "title", "signature", "finding_id",
            "description", "steps_gherkin", "expected_state", "current_state",
            "potential_resolution", "justification",
        ],
    },
)

CLOSE_AS_DUPLICATE = types.FunctionDeclaration(
    name="close_as_duplicate",
    description=(
        "Record a finding as a duplicate of an already-registered issue and close it "
        "with no action (increments the known issue's occurrence count)."
    ),
    parameters_json_schema={
        "type": "object",
        "properties": {
            "signature": {"type": "string"},
            "finding_id": {"type": "string"},
            "reason": {"type": "string"},
        },
        "required": ["signature", "finding_id"],
    },
)

IGNORE_FINDING = types.FunctionDeclaration(
    name="ignore_finding",
    description="Record a non-actionable finding (noise) as `ignored` with a reason.",
    parameters_json_schema={
        "type": "object",
        "properties": {
            "finding_id": {"type": "string"},
            "signature": {"type": "string"},
            "reason": {"type": "string"},
        },
        "required": ["finding_id"],
    },
)

DECLARATIONS = [FIND_DUPLICATE, CREATE_TICKET, CLOSE_AS_DUPLICATE, IGNORE_FINDING]


def find_duplicate(cfg: Config, args: dict[str, Any]) -> str:
    ki = store.find_known_issue(cfg.project_id, str(args.get("signature", "")))
    if not ki:
        return json.dumps({"found": False})
    return json.dumps(
        {
            "found": True,
            "canonical_ticket_id": ki.get("canonical_ticket_id"),
            "status": ki.get("status"),
            "occurrence_count": ki.get("occurrence_count"),
        },
        default=str,
    )


def create_ticket(cfg: Config, args: dict[str, Any]) -> str:
    try:
        now = datetime.now(timezone.utc)
        ticket_id = f"tkt_{now.strftime('%Y%m%dT%H%M%SZ')}_{uuid4().hex[:4]}"
        ticket = Ticket(
            ticket_id=ticket_id,
            created_at=now.isoformat(),
            finding_ids=[str(args["finding_id"])],
            status="proposed",
            severity=str(args["severity"]),
            service=str(args["service"]),
            title=str(args["title"]),
            root_cause_hypothesis=str(args.get("root_cause_hypothesis", "")),
            suggested_fix=str(args.get("suggested_fix", "")),
            grounding_refs=list(args.get("grounding_refs", [])),
            evidence=Evidence(
                log_query=str(args.get("log_query", "")),
                sample_trace_ids=list(args.get("sample_trace_ids", [])),
                count=int(args.get("count", 0)),
                window=str(args.get("window", "")),
            ),
            signature=str(args["signature"]),
            description=str(args.get("description", "")),
            steps_gherkin=str(args.get("steps_gherkin", "")),
            expected_state=str(args.get("expected_state", "")),
            current_state=str(args.get("current_state", "")),
            actual_log=str(args.get("actual_log", "")),
            log_timestamp=str(args.get("log_timestamp", "")),
            potential_resolution=str(args.get("potential_resolution", "")),
            justification=str(args.get("justification", "")),
        )
        publisher = _publisher()
        topic_path = publisher.topic_path(cfg.project_id, cfg.tickets_topic)
        publisher.publish(topic_path, ticket.model_dump_json().encode("utf-8")).result(timeout=30)
        store.register_known_issue(
            cfg.project_id, ticket.signature, ticket_id, ticket.service, ticket.severity
        )
        store.record_event(
            cfg.project_id,
            {
                "outcome": "ticketed",
                "finding_id": str(args["finding_id"]),
                "signature": ticket.signature,
                "service": ticket.service,
                "ticket_id": ticket_id,
            },
        )
    except Exception as exc:
        return f"create_ticket failed: {exc}"
    return f"created {ticket_id} ({ticket.severity})"


def close_as_duplicate(cfg: Config, args: dict[str, Any]) -> str:
    signature = str(args.get("signature", ""))
    ki = store.find_known_issue(cfg.project_id, signature)
    if not ki:
        return "no known issue for that signature — use create_ticket instead"
    try:
        store.bump_known_issue(cfg.project_id, signature)
        store.record_event(
            cfg.project_id,
            {
                "outcome": "duplicate_closed",
                "finding_id": str(args.get("finding_id", "")),
                "signature": signature,
                "service": ki.get("service"),
                "ticket_id": ki.get("canonical_ticket_id"),
                "reason": str(args.get("reason", "")),
            },
        )
    except Exception as exc:
        return f"close_as_duplicate failed: {exc}"
    return f"closed as duplicate of {ki.get('canonical_ticket_id')}"


def ignore_finding(cfg: Config, args: dict[str, Any]) -> str:
    try:
        store.record_event(
            cfg.project_id,
            {
                "outcome": "ignored",
                "finding_id": str(args.get("finding_id", "")),
                "signature": str(args.get("signature", "")),
                "reason": str(args.get("reason", "")),
            },
        )
    except Exception as exc:
        return f"ignore_finding failed: {exc}"
    return "ignored"
