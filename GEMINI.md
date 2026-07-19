# Project guidance for the coding agent

The Gemini CLI loads this file as context. The full guidance lives in
[`CLAUDE.md`](CLAUDE.md) — read it. Key rules:

- **Never commit secrets.** Values live in Secret Manager / GitHub Secrets.
- **Scope narrowly.** Implement only what the `agent-bug` ticket describes; smallest
  fix that addresses the root cause. No unrelated refactors.
- **Prove it.** Add or update a test that fails before your fix and passes after.
- **Explain the root cause** in the PR, referencing the ticket and log evidence.
- **Standards:** Rust — `cargo fmt` + `cargo clippy -- -D warnings`; Python — `ruff`
  clean, type hints. Never claim a check passed if you couldn't run it.

The planted bug lives in `apps/synthetic-shop/src/inventory.rs` (`reserve()`
decrements stock before checking availability). The fix: check-before-decrement,
return `ReserveError::Insufficient`, and add a regression test.
