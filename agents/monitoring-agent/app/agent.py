"""The monitoring agent loop, built on Vertex AI Gemini function calling."""

from __future__ import annotations

import json
from datetime import datetime, timezone
from functools import lru_cache
from typing import Any

from google import genai
from google.genai import types

from . import health, tools
from .config import Config
from .grounding import load_grounding

_MAX_TURNS = 12

_SYSTEM = """\
You are a 3rd-line monitoring agent for a small e-commerce backend on Cloud Run.
Inspect recent logs and surface FINDING-worthy problems — both known patterns and
emergent/novel ones — then emit one structured finding per distinct problem.

Tools:
- query_logs(filter, minutes): Cloud Logging (LQL). Always include resource.type and
  severity. Start broad, then drill in. Keep minutes <= {lookback}.
- emit_finding(...): publish ONE finding per distinct problem, deduplicated by a stable
  signature such as <pattern>:<service>:<event>.

Grounding — known risky patterns and the queries that surface them:
{grounding}

Rules:
- Correlate when needed. An ORPHANED TRANSACTION is a payment.captured with no
  order.created on the same trace within the window — verify before emitting.
- Only emit ACTIONABLE findings. Do not emit for normal healthy traffic.
- Be economical: a few targeted queries, then emit. Finish with a one-line summary
  ("no findings" if nothing is wrong).
"""


@lru_cache(maxsize=1)
def _client(project_id: str, location: str) -> genai.Client:
    return genai.Client(vertexai=True, project=project_id, location=location)


def _system_prompt(cfg: Config) -> str:
    return _SYSTEM.format(lookback=cfg.lookback_minutes, grounding=load_grounding(cfg))


def run_sweep(
    cfg: Config,
    lane: str = "agentic",
    hint: str | None = None,
    trigger: str = "scheduler",
) -> dict[str, Any]:
    """Run one sweep (synchronous; call via asyncio.to_thread from the endpoint).

    Records a run doc (success/error + timing) for the console's Runs view.
    """
    started = datetime.now(timezone.utc).isoformat()
    try:
        return _run_sweep(cfg, lane, hint, trigger, started)
    except Exception as exc:  # record the failure, then re-raise
        health.record_run(
            cfg.project_id, "error", trigger, error=str(exc), started_at=started
        )
        raise


def _run_sweep(
    cfg: Config, lane: str, hint: str | None, trigger: str, started: str
) -> dict[str, Any]:
    client = _client(cfg.project_id, cfg.location)
    config = types.GenerateContentConfig(
        system_instruction=_system_prompt(cfg),
        tools=[types.Tool(function_declarations=tools.DECLARATIONS)],
        tool_config=types.ToolConfig(
            function_calling_config=types.FunctionCallingConfig(mode="AUTO")
        ),
        temperature=0.0,
        # Disable "thinking" — these are tool-calling agents; thinking budget can
        # otherwise consume the whole output and return an empty candidate.
        thinking_config=types.ThinkingConfig(thinking_budget=0),
        max_output_tokens=2048,
    )

    prompt = f"Sweep the last {cfg.lookback_minutes} minutes for problems. Lane: {lane}."
    if hint:
        prompt += f"\nA deterministic alert fired — use this as a starting point:\n{hint}"
    contents: list[types.Content] = [
        types.Content(role="user", parts=[types.Part(text=prompt)])
    ]

    emitted = 0
    summary = ""
    for turn in range(_MAX_TURNS):
        response = client.models.generate_content(
            model=cfg.model, contents=contents, config=config
        )
        candidate = response.candidates[0] if response.candidates else None
        parts = (candidate.content.parts if candidate and candidate.content else None) or []
        calls = [p.function_call for p in parts if getattr(p, "function_call", None)]

        if not calls:
            summary = " ".join(p.text for p in parts if getattr(p, "text", None)).strip()
            print(json.dumps({"agent": "monitoring", "turn": turn, "done": True}), flush=True)
            break

        print(
            json.dumps({"agent": "monitoring", "turn": turn, "tools": [c.name for c in calls]}),
            flush=True,
        )
        contents.append(candidate.content)
        responses: list[types.Part] = []
        for call in calls:
            args = dict(call.args or {})
            if call.name == "query_logs":
                result = tools.query_logs(cfg, args)
            elif call.name == "emit_finding":
                result = tools.emit_finding(cfg, lane, args)
                emitted += 1
            else:
                result = f"unknown tool {call.name}"
            responses.append(
                types.Part.from_function_response(name=call.name, response={"result": result})
            )
        contents.append(types.Content(role="tool", parts=responses))

    health.record_run(
        cfg.project_id,
        "success",
        trigger,
        summary=(summary or f"{emitted} finding(s) emitted"),
        count=emitted,
        started_at=started,
    )
    return {"lane": lane, "findings_emitted": emitted, "summary": summary}
