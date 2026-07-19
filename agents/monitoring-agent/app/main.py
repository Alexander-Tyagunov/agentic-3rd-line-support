"""FastAPI entrypoint for the monitoring agent.

- POST /sweep         — Cloud Scheduler invokes this periodically (emergent patterns).
- POST /pubsub/alerts — Pub/Sub push from a deterministic alert (enrich, then emit).

The Gemini agent loop is synchronous, so it's run in a worker thread to keep the
event loop free.
"""

from __future__ import annotations

import asyncio
import base64
from typing import Any

from fastapi import BackgroundTasks, FastAPI, Request

from . import health
from .agent import run_sweep
from .config import load_config

app = FastAPI(title="monitoring-agent")
cfg = load_config()


@app.get("/health")  # Cloud Run's frontend reserves "/healthz"
async def health_check() -> dict[str, str]:
    return {"status": "ok"}


@app.post("/sweep")
async def sweep(request: Request) -> dict[str, Any]:
    """Scheduled sweep — runs inline (Scheduler allows a long attempt deadline).

    The console's "Run sweep now" button passes ?trigger=manual so the run is
    labelled accordingly in the Runs view; Cloud Scheduler omits it (=scheduler).
    """
    trigger = request.query_params.get("trigger", "scheduler")
    await asyncio.to_thread(health.record_heartbeat, cfg.project_id, "monitoring-agent")
    if await asyncio.to_thread(health.shop_active, cfg.project_id, cfg.lookback_minutes):
        await asyncio.to_thread(
            health.record_heartbeat, cfg.project_id, "synthetic-shop", {"source": "log-activity"}
        )
    return await asyncio.to_thread(run_sweep, cfg, "agentic", None, trigger)


def _decode_push(envelope: dict[str, Any] | None) -> str | None:
    message = (envelope or {}).get("message") or {}
    data = message.get("data")
    if not data:
        return None
    try:
        return base64.b64decode(data).decode("utf-8")
    except (ValueError, TypeError):
        return None


@app.post("/pubsub/alerts")
async def alerts(request: Request, background: BackgroundTasks) -> dict[str, str]:
    """Ack fast, then enrich in the background (the LLM sweep can exceed the push deadline)."""
    envelope = await request.json()
    hint = _decode_push(envelope)
    background.add_task(run_sweep, cfg, "deterministic", hint, "alert")
    return {"status": "accepted"}
