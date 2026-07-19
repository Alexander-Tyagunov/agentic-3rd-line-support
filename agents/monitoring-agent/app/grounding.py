"""Load the risky-pattern grounding the agent reasons over.

Prefers the versioned Markdown in `grounding/` (bundled into the image); falls
back to a compact embedded default so the agent still works if it is missing.
"""

from __future__ import annotations

import os

from .config import Config

_DEFAULT = """\
## payment_error_spike (known)
resource.type="cloud_run_revision" jsonPayload.event="payment.failed" severity>="ERROR"

## db_pool_exhaustion (known)
resource.type="cloud_run_revision" jsonPayload.message=~"pool timeout|no connection available"

## orphaned_transaction (emergent, correlation)
Find `jsonPayload.event="payment.captured"`, then per trace confirm there is NO
`jsonPayload.event="order.created"` with the same trace in the window.

## latency_creep (emergent, trend)
resource.type="cloud_run_revision" jsonPayload.event="checkout" jsonPayload.latency_ms>0
Flag sustained upward drift even while within SLO.

## inventory_oversell (emergent, invariant)
resource.type="cloud_run_revision" jsonPayload.event="inventory.oversold"

## new_error_signature (emergent, novelty)
Cluster severity>=ERROR messages; flag signatures not seen in the prior window.
"""


def load_grounding(cfg: Config) -> str:
    path = os.path.join(cfg.grounding_dir, "risky-patterns.md")
    try:
        with open(path, encoding="utf-8") as fh:
            return fh.read()
    except OSError:
        return _DEFAULT
