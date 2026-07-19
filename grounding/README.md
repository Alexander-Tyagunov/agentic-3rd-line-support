# Grounding

The knowledge the **triage agent** reasons over when turning a raw *finding*
into a well-formed *ticket*. It is deliberately **versioned Markdown in the
repo** so changes are diffable and reviewable like code.

| File | Purpose |
|------|---------|
| [`service-catalog.md`](service-catalog.md) | Services, owners, dependencies, SLOs |
| [`runbooks.md`](runbooks.md) | Known failure modes and how to reason about them |
| [`severity-rubric.md`](severity-rubric.md) | The S1–S4 scoring the triage agent applies |
| [`risky-patterns.md`](risky-patterns.md) | Named patterns + the Logging Query Language that surfaces them |

The monitoring agent uses `risky-patterns.md` for its query templates; the triage
agent uses all four. Keep entries short and specific — this is context the model
reads on every run, so signal-to-noise matters.
