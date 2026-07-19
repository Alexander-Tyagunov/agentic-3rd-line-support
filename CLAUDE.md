# Project guidance for coding agents

This file guides the **coding agent** (Gemini CLI GitHub Action) and any human
or agent contributor. Keep changes small, well-tested, and reviewable.

## What this repo is

An educational reference implementation of an autonomous "3rd-line support" loop
on GCP. See [`docs/architecture.md`](docs/architecture.md) before making changes.

## Ground rules

- **Never** commit secrets. Secret *values* live in Secret Manager / GitHub
  Secrets, never in `*.tf`, `*.tfvars`, or source. Only `*.tfvars.example` is
  committed.
- **Scope narrowly.** When implementing an `agent-bug` issue, change only what
  the ticket describes. Prefer the smallest fix that addresses the root cause.
- **Explain the root cause** in the PR description, referencing the ticket and
  the log evidence it was built from.
- Keep the PR self-contained; do not refactor unrelated code.

## Language standards

### Rust (`apps/`, `ticket-system/`)
- `cargo fmt` and `cargo clippy -- -D warnings` must pass.
- Prefer `Result<_, _>` with `thiserror`/`anyhow`; no `.unwrap()` in service paths.
- Structured logging via `tracing`; one JSON object per event to stdout.

### Python (`agents/`)
- `ruff` clean; type hints on public functions; `python >= 3.10`.
- Use the Vertex AI Gemini SDK (`google-genai`) with function calling; keep tool
  surfaces small and well-documented (the agent-computer interface matters).
- No hardcoded model IDs scattered around — read from config/env.

## Testing / verification

- Rust: `cargo test` in the changed crate.
- Python: `pytest` in the changed agent.
- If you cannot run a check, say so explicitly in the PR rather than claiming it passed.

## The known planted bug

`apps/synthetic-shop` intentionally contains an `inventory_oversell` logic bug
used to demo the coding agent. If a ticket targets it, fix the overselling logic
(check available stock **before** decrementing) and add a regression test.
