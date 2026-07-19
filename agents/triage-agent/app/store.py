"""Firestore access for the known-issue registry, the event ledger, and heartbeats.

Synchronous Google client; call these via `asyncio.to_thread` from async tools.
Collections (see docs/architecture.md §11-12): known_issues, events, health.
"""

from __future__ import annotations

import re
from datetime import datetime, timezone
from functools import lru_cache
from typing import Any

from google.cloud import firestore


@lru_cache(maxsize=4)
def _client(project_id: str) -> firestore.Client:
    return firestore.Client(project=project_id)


def _sig_id(signature: str) -> str:
    """Firestore-safe document id derived from a signature."""
    sid = re.sub(r"[^A-Za-z0-9_.-]", "_", signature).strip("_")
    return (sid or "unknown")[:1500]


def find_known_issue(project_id: str, signature: str) -> dict[str, Any] | None:
    snap = _client(project_id).collection("known_issues").document(_sig_id(signature)).get()
    return snap.to_dict() if snap.exists else None


def register_known_issue(
    project_id: str, signature: str, ticket_id: str, service: str, severity: str
) -> None:
    doc = _client(project_id).collection("known_issues").document(_sig_id(signature))
    doc.set(
        {
            "signature": signature,
            "canonical_ticket_id": ticket_id,
            "status": "open",
            "service": service,
            "severity": severity,
            "occurrence_count": 1,
            "first_seen": firestore.SERVER_TIMESTAMP,
            "last_seen": firestore.SERVER_TIMESTAMP,
        }
    )


def bump_known_issue(project_id: str, signature: str) -> None:
    doc = _client(project_id).collection("known_issues").document(_sig_id(signature))
    doc.update(
        {"occurrence_count": firestore.Increment(1), "last_seen": firestore.SERVER_TIMESTAMP}
    )


def record_event(project_id: str, event: dict[str, Any]) -> None:
    _client(project_id).collection("events").add({**event, "at": firestore.SERVER_TIMESTAMP})


def record_heartbeat(project_id: str, component: str, detail: dict[str, Any] | None = None) -> None:
    _client(project_id).collection("health").document(component).set(
        {"component": component, "last_seen": firestore.SERVER_TIMESTAMP, "detail": detail or {}},
        merge=True,
    )


def record_run(
    project_id: str,
    status: str,
    summary: str = "",
    error: str = "",
    detail: str = "",
    started_at: str = "",
) -> None:
    """Append a run record (one per finding processed) for the console Runs view."""
    _client(project_id).collection("runs").add(
        {
            "agent": "triage",
            "status": status,
            "trigger": "pubsub",
            "summary": summary,
            "error": error,
            "detail": detail,
            "count": 0,
            "started_at": started_at,
            "finished_at": datetime.now(timezone.utc).isoformat(),
        }
    )
