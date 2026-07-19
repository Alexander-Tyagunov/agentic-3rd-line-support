"""Health heartbeats + shop liveness for the console's Health view.

Writes `health/{component}` in Firestore. The synthetic shop isn't wired to
Firestore (it's a pure log emitter), so its liveness is derived from real log
activity — more honest than a self-reported heartbeat.
"""

from __future__ import annotations

from datetime import datetime, timedelta, timezone
from functools import lru_cache
from typing import Any

from google.cloud import firestore
from google.cloud import logging as gcloud_logging


@lru_cache(maxsize=1)
def _fs(project_id: str) -> firestore.Client:
    return firestore.Client(project=project_id)


@lru_cache(maxsize=1)
def _log(project_id: str) -> gcloud_logging.Client:
    return gcloud_logging.Client(project=project_id)


def record_heartbeat(project_id: str, component: str, detail: dict[str, Any] | None = None) -> None:
    _fs(project_id).collection("health").document(component).set(
        {"component": component, "last_seen": firestore.SERVER_TIMESTAMP, "detail": detail or {}},
        merge=True,
    )


def record_run(
    project_id: str,
    status: str,
    trigger: str,
    summary: str = "",
    error: str = "",
    count: int = 0,
    started_at: str = "",
) -> None:
    """Append a run record for the console's Runs view (success/fail + timing)."""
    _fs(project_id).collection("runs").add(
        {
            "agent": "monitoring",
            "status": status,
            "trigger": trigger,
            "summary": summary,
            "error": error,
            "count": count,
            "started_at": started_at,
            "finished_at": datetime.now(timezone.utc).isoformat(),
        }
    )


def shop_active(project_id: str, minutes: int) -> bool:
    """True if the synthetic shop emitted any business event in the window."""
    since = (datetime.now(timezone.utc) - timedelta(minutes=minutes)).strftime(
        "%Y-%m-%dT%H:%M:%SZ"
    )
    flt = f'resource.type="cloud_run_revision" AND jsonPayload.event:* AND timestamp>="{since}"'
    for _ in _log(project_id).list_entries(filter_=flt, max_results=1):
        return True
    return False
