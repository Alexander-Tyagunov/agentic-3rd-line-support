"""Load the grounding the triage agent reasons over (catalog, runbooks, rubric)."""

from __future__ import annotations

import os

from .config import Config

_FILES = ["service-catalog.md", "runbooks.md", "severity-rubric.md"]

_DEFAULT = """\
Severity: S1 outage/silent money-or-data loss; S2 serious degradation or a clear
correctness bug; S3 minor/contained; S4 cosmetic. Round up when unsure.
Payments invariant: every payment.captured must be followed by order.created on the
same trace within 60s; a violation is an orphaned transaction (S1/S2).
Inventory must never go below zero; inventory.oversold is a code defect (S2).
"""


def load_grounding(cfg: Config) -> str:
    parts: list[str] = []
    for name in _FILES:
        try:
            with open(os.path.join(cfg.grounding_dir, name), encoding="utf-8") as fh:
                parts.append(f"# {name}\n{fh.read()}")
        except OSError:
            continue
    return "\n\n".join(parts) if parts else _DEFAULT
