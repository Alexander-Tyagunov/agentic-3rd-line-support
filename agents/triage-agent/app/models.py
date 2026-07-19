"""The `ticket` data contract (see docs/architecture.md §3)."""

from __future__ import annotations

from typing import Any

from pydantic import BaseModel, Field


class Evidence(BaseModel):
    log_query: str = ""
    sample_trace_ids: list[str] = Field(default_factory=list)
    count: int = 0
    window: str = ""


class Ticket(BaseModel):
    ticket_id: str
    created_at: str
    finding_ids: list[str] = Field(default_factory=list)
    status: str = "proposed"  # lifecycle: see docs/architecture.md §11
    severity: str  # S1..S4
    service: str
    title: str
    root_cause_hypothesis: str = ""
    suggested_fix: str = ""
    grounding_refs: list[str] = Field(default_factory=list)
    evidence: Evidence = Field(default_factory=Evidence)
    signature: str
    # ---- Full report ----
    description: str = ""
    steps_gherkin: str = ""
    expected_state: str = ""
    current_state: str = ""
    actual_log: str = ""
    log_timestamp: str = ""
    potential_resolution: str = ""
    justification: str = ""
    dedup: dict[str, Any] = Field(default_factory=lambda: {"is_duplicate": False})
