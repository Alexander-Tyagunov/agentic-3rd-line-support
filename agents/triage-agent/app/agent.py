"""The triage agent loop, built on Vertex AI Gemini function calling."""

from __future__ import annotations

import json
from functools import lru_cache
from typing import Any

from google import genai
from google.genai import types

from . import tools
from .config import Config
from .grounding import load_grounding

_MAX_TURNS = 10

_SYSTEM = """\
You are a 3rd-line triage agent. You receive ONE finding and must resolve it into
exactly one outcome, recorded via a tool. Never drop a finding silently.

Decision procedure:
1. If the finding is clearly noise / not actionable -> call ignore_finding.
2. Otherwise call find_duplicate(signature).
   - If found with status open, declined, or wontfix -> call close_as_duplicate
     (a valid event, closed with no action — the bug is already registered).
   - If found with status merged (previously fixed) and it is recurring -> treat as
     a regression and call create_ticket.
   - If not found -> call create_ticket.
3. For create_ticket, write a COMPLETE bug report a human could act on without
   re-reading the logs:
   - `description`: what is happening and its impact.
   - `steps_gherkin`: reproduction in Gherkin — `Scenario:` + Given/When/Then.
   - `expected_state` vs `current_state`: correct behavior vs the actual broken behavior.
   - `actual_log` + `log_timestamp`: a representative real log line from the evidence
     and its timestamp.
   - `root_cause_hypothesis` and `potential_resolution`: cite the relevant runbook.
   - `justification`: why it's actionable and why this S1-S4 severity (use the rubric).

Use the `signature` exactly as provided by the finding so dedup stays consistent.

Grounding (service catalog, runbooks, severity rubric):
{grounding}

Finish with ONE short sentence (20 words max) stating the action you took —
e.g. "Created S2 ticket for the orphaned-transaction finding." Do not repeat
words or phrases, and do not restate your plan.
"""


@lru_cache(maxsize=1)
def _client(project_id: str, location: str) -> genai.Client:
    return genai.Client(vertexai=True, project=project_id, location=location)


def _system_prompt(cfg: Config) -> str:
    return _SYSTEM.format(grounding=load_grounding(cfg))


def _dispatch(cfg: Config, name: str, args: dict[str, Any]) -> tuple[str, str | None]:
    """Return (result_text, outcome) for a tool call."""
    if name == "create_ticket":
        return tools.create_ticket(cfg, args), "ticketed"
    if name == "close_as_duplicate":
        return tools.close_as_duplicate(cfg, args), "duplicate_closed"
    if name == "ignore_finding":
        return tools.ignore_finding(cfg, args), "ignored"
    if name == "find_duplicate":
        return tools.find_duplicate(cfg, args), None
    return f"unknown tool {name}", None


def triage_finding(cfg: Config, finding_json: str) -> dict[str, Any]:
    """Triage one finding (synchronous; call via asyncio.to_thread)."""
    client = _client(cfg.project_id, cfg.location)
    config = types.GenerateContentConfig(
        system_instruction=_system_prompt(cfg),
        tools=[types.Tool(function_declarations=tools.DECLARATIONS)],
        tool_config=types.ToolConfig(
            function_calling_config=types.FunctionCallingConfig(mode="AUTO")
        ),
        temperature=0.0,
        # Disable "thinking" — tool-calling agent; thinking budget can otherwise
        # consume the whole output and return an empty candidate.
        thinking_config=types.ThinkingConfig(thinking_budget=0),
        max_output_tokens=2048,
    )

    prompt = f"Triage this finding and record exactly one outcome:\n{finding_json}"
    contents: list[types.Content] = [
        types.Content(role="user", parts=[types.Part(text=prompt)])
    ]

    outcome = "none"
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
            break

        print(
            json.dumps({"agent": "triage", "turn": turn, "tools": [c.name for c in calls]}),
            flush=True,
        )
        contents.append(candidate.content)
        responses: list[types.Part] = []
        for call in calls:
            result, call_outcome = _dispatch(cfg, call.name, dict(call.args or {}))
            if call_outcome:
                outcome = call_outcome
            responses.append(
                types.Part.from_function_response(name=call.name, response={"result": result})
            )
        contents.append(types.Content(role="tool", parts=responses))

    return {"outcome": outcome, "summary": summary}
