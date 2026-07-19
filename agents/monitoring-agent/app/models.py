"""The `finding` data contract (see docs/architecture.md §3)."""

from __future__ import annotations

from pydantic import BaseModel, Field


class Evidence(BaseModel):
    log_query: str = ""
    sample_trace_ids: list[str] = Field(default_factory=list)
    count: int = 0
    window: str = ""


class Finding(BaseModel):
    finding_id: str
    detected_at: str
    source_lane: str  # "agentic" | "deterministic"
    service: str
    severity: str  # WARNING | ERROR | CRITICAL
    title: str
    summary: str
    signature: str  # stable dedup key, e.g. "orphaned_txn:checkout:payment.captured"
    evidence: Evidence
