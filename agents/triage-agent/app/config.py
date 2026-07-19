"""Runtime configuration, read from the environment (set by Terraform on Cloud Run)."""

from __future__ import annotations

import os
from dataclasses import dataclass


@dataclass(frozen=True)
class Config:
    project_id: str
    location: str  # Vertex AI location for Gemini
    model: str  # e.g. "gemini-2.5-flash"
    tickets_topic: str
    grounding_dir: str


def load_config() -> Config:
    return Config(
        project_id=os.environ.get("PROJECT_ID", ""),
        location=os.environ.get("GEMINI_LOCATION", "global"),
        model=os.environ.get("GEMINI_MODEL", "gemini-3.5-flash"),
        tickets_topic=os.environ.get("TICKETS_TOPIC", "a3l-tickets"),
        grounding_dir=os.environ.get("GROUNDING_DIR", "grounding"),
    )
