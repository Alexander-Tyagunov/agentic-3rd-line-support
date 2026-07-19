"""FastAPI entrypoint for the triage agent.

POST /pubsub/findings — authenticated Pub/Sub push on the `findings` topic.
The Gemini agent loop is synchronous, so it runs in a worker thread. Returns 2xx
only on success, so a failure is retried (and dead-lettered) rather than lost.
"""

from __future__ import annotations

import asyncio
import base64
from datetime import datetime, timezone
from typing import Any

from fastapi import FastAPI, HTTPException, Request

from . import store
from .agent import triage_finding
from .config import load_config

app = FastAPI(title="triage-agent")
cfg = load_config()


@app.get("/health")  # Cloud Run's frontend reserves "/healthz"
async def health_check() -> dict[str, str]:
    return {"status": "ok"}


def _decode_push(envelope: dict[str, Any] | None) -> str | None:
    message = (envelope or {}).get("message") or {}
    data = message.get("data")
    if not data:
        return None
    try:
        return base64.b64decode(data).decode("utf-8")
    except (ValueError, TypeError):
        return None


@app.post("/pubsub/findings")
async def findings(request: Request) -> dict[str, Any]:
    envelope = await request.json()
    finding_json = _decode_push(envelope)
    if not finding_json:
        return {"status": "ignored", "reason": "empty message"}

    started = datetime.now(timezone.utc).isoformat()
    try:
        await asyncio.to_thread(store.record_heartbeat, cfg.project_id, "triage-agent")
        result = await asyncio.to_thread(triage_finding, cfg, finding_json)
    except Exception as exc:  # 5xx -> Pub/Sub retries -> DLQ after max attempts
        await asyncio.to_thread(
            store.record_run, cfg.project_id, "error", error=str(exc), started_at=started
        )
        raise HTTPException(status_code=500, detail=str(exc)) from exc
    await asyncio.to_thread(
        store.record_run,
        cfg.project_id,
        "success",
        summary=result.get("summary", ""),
        detail=result.get("outcome", ""),
        started_at=started,
    )
    return {"status": "ok", **result}
