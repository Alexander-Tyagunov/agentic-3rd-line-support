# Coding Agent

The third agent: it turns an approved bug ticket into a **pull request**, and
stops there — a human reviews and merges. This is the only step that changes
production code, so it is the hardest human gate in the system.

## How it's triggered

1. In the console, a human clicks **Approve & fix** on a `proposed` ticket (or
   `AUTO_APPROVE=true` for a fully-autonomous demo).
2. The ticket backend opens a **GitHub Issue** labeled `agent-bug` (rendered from
   the ticket: service, severity, root-cause hypothesis, suggested fix, evidence).
3. The `issues` event triggers the workflow below → Gemini implements the fix →
   opens a PR → **human review → merge**.

## Path A — Gemini CLI GitHub Action (primary)

Defined in [`.github/workflows/gemini.yml`](../../.github/workflows/gemini.yml),
using [`google-github-actions/run-gemini-cli`](https://github.com/google-github-actions/run-gemini-cli).

- **Trigger:** an `agent-bug` issue, or a human `@gemini-cli` comment (bot comments
  are filtered to prevent loops).
- **Auth:** Gemini via **Vertex AI** using **Workload Identity Federation** — no
  keys. `use_vertex_ai: true` + the WIF inputs; the impersonated service account is
  scoped to **`roles/aiplatform.user`**.
- **Model:** `gemini-2.5-pro` (coding benefits from the stronger model).
- **Context:** the CLI loads [`GEMINI.md`](../../GEMINI.md) (which points at `CLAUDE.md`).
- **Guards:** `concurrency`, `timeout-minutes`, and the action's own turn limits.

### Required repo secrets (from `terraform output`)
| Secret | Source |
|--------|--------|
| `GCP_WORKLOAD_IDENTITY_PROVIDER` | `github_workload_identity_provider` |
| `GCP_SERVICE_ACCOUNT` | `github_actions_service_account` |
| `GCP_PROJECT_ID` | your project id |

Plus the `agent-bug` label — see [`docs/setup.md`](../../docs/setup.md) §8.

### The demo target
`apps/synthetic-shop/src/inventory.rs` contains a planted `inventory_oversell`
bug: `reserve()` decrements stock **before** checking availability, so it can go
negative. The expected fix is check-before-decrement (`ReserveError::Insufficient`)
plus a regression test.

## Path B — self-hosted Gemini job (taught alternative)

To close the loop **entirely inside GCP** without a GitHub Action, run a Cloud Run
**Job** that subscribes to the `tickets` topic and drives the fix with `google-genai`
+ git/`gh`:

```python
# Pull one ticket, implement it, open a PR — the "build the harness" variant.
from google import genai
from google.genai import types

client = genai.Client(vertexai=True, project=PROJECT, location="global")
# 1. git clone the repo into a workspace (git + gh preconfigured on the image)
# 2. drive Gemini with function tools for read_file / write_file / run_command,
#    looping over response.function_calls until the fix + test are in place
# 3. Bash: git checkout -b fix/<ticket_id>; git commit -am ...; git push
# 4. Bash: gh pr create --fill   (a human still reviews + merges)
```

Path A is minimal infra and the documented product path; Path B keeps everything in
GCP and exposes the loop internals. Both stop at a PR.

## Why never auto-merge

Detection, triage, and drafting a fix are cheap and reversible, so the agents run
autonomously there. Shipping code is not — so the PR review is a hard gate. This
mirrors how leading teams run autonomous remediation: propose a reviewable change,
never push straight to production.
