"""Tools the monitoring agent can call: read logs, emit findings.

Declared as Gemini FunctionDeclarations; the bodies are plain synchronous
functions (the Google Cloud clients are blocking) dispatched by the agent loop.
The interface is deliberately tiny: one read tool, one write tool.
"""

from __future__ import annotations

import json
from datetime import datetime, timedelta, timezone
from functools import lru_cache
from typing import Any
from uuid import uuid4

from google.cloud import logging as gcloud_logging
from google.cloud import pubsub_v1
from google.genai import types

from .config import Config
from .models import Evidence, Finding

_MAX_ENTRIES = 200


@lru_cache(maxsize=1)
def _publisher() -> pubsub_v1.PublisherClient:
    return pubsub_v1.PublisherClient()


@lru_cache(maxsize=4)
def _log_client(project_id: str) -> gcloud_logging.Client:
    return gcloud_logging.Client(project=project_id)


def _fetch_entries(project_id: str, filter_str: str) -> list[dict[str, Any]]:
    client = _log_client(project_id)
    rows: list[dict[str, Any]] = []
    for entry in client.list_entries(
        filter_=filter_str, order_by=gcloud_logging.DESCENDING, max_results=_MAX_ENTRIES
    ):
        payload = (
            entry.payload if isinstance(entry.payload, dict) else {"message": str(entry.payload)}
        )
        rows.append(
            {
                "severity": str(entry.severity) if entry.severity else "DEFAULT",
                "timestamp": entry.timestamp.isoformat() if entry.timestamp else None,
                "trace": entry.trace,
                "event": payload.get("event"),
                "service": payload.get("service"),
                "message": payload.get("message"),
                "latency_ms": payload.get("latency_ms"),
                "reason": payload.get("reason"),
            }
        )
    return rows


# ---- Function declarations (the model sees these) ----

QUERY_LOGS = types.FunctionDeclaration(
    name="query_logs",
    description=(
        "Run a Cloud Logging (LQL) filter over a recent window and return matching "
        "entries as JSON. Always include resource.type and severity in the filter."
    ),
    parameters_json_schema={
        "type": "object",
        "properties": {
            "filter": {"type": "string"},
            "minutes": {"type": "integer"},
        },
        "required": ["filter"],
    },
)

EMIT_FINDING = types.FunctionDeclaration(
    name="emit_finding",
    description=(
        "Publish ONE finding per distinct, actionable problem. Deduplicate with a "
        "stable signature such as pattern:service:event."
    ),
    parameters_json_schema={
        "type": "object",
        "properties": {
            "service": {"type": "string"},
            "severity": {"type": "string", "enum": ["WARNING", "ERROR", "CRITICAL"]},
            "title": {"type": "string"},
            "summary": {"type": "string"},
            "signature": {"type": "string"},
            "log_query": {"type": "string"},
            "sample_trace_ids": {"type": "array", "items": {"type": "string"}},
            "count": {"type": "integer"},
            "window": {"type": "string"},
        },
        "required": ["service", "severity", "title", "summary", "signature"],
    },
)

DECLARATIONS = [QUERY_LOGS, EMIT_FINDING]


# ---- Executors (the agent loop dispatches to these) ----

def query_logs(cfg: Config, args: dict[str, Any]) -> str:
    minutes = int(args.get("minutes") or cfg.lookback_minutes)
    minutes = max(1, min(minutes, cfg.lookback_minutes))
    since = (datetime.now(timezone.utc) - timedelta(minutes=minutes)).strftime("%Y-%m-%dT%H:%M:%SZ")
    filter_str = f'{str(args.get("filter", "")).strip()} AND timestamp>="{since}"'
    try:
        rows = _fetch_entries(cfg.project_id, filter_str)
    except Exception as exc:  # surface to the model
        return json.dumps({"error": f"query_logs failed: {exc}"})
    return json.dumps({"count": len(rows), "entries": rows}, default=str)


def emit_finding(cfg: Config, lane: str, args: dict[str, Any]) -> str:
    try:
        now = datetime.now(timezone.utc)
        finding = Finding(
            finding_id=f"fnd_{now.strftime('%Y%m%dT%H%M%SZ')}_{uuid4().hex[:4]}",
            detected_at=now.isoformat(),
            source_lane=lane,
            service=str(args["service"]),
            severity=str(args["severity"]),
            title=str(args["title"]),
            summary=str(args["summary"]),
            signature=str(args["signature"]),
            evidence=Evidence(
                log_query=str(args.get("log_query", "")),
                sample_trace_ids=list(args.get("sample_trace_ids", [])),
                count=int(args.get("count", 0)),
                window=str(args.get("window", "")),
            ),
        )
        publisher = _publisher()
        topic_path = publisher.topic_path(cfg.project_id, cfg.findings_topic)
        publisher.publish(topic_path, finding.model_dump_json().encode("utf-8")).result(timeout=30)
    except Exception as exc:
        return f"emit_finding failed: {exc}"
    return f"published {finding.finding_id} ({finding.severity})"
