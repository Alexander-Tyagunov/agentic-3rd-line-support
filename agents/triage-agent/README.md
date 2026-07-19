# Triage Agent (Python · Vertex Gemini)

Consumes *findings* (Pub/Sub push), reasons over the [`grounding/`](../../grounding/)
knowledge base, deduplicates against a Firestore index, and emits a well-formed
*ticket* to the `tickets` topic — or drops the finding as noise/duplicate.

## Trigger
- `POST /pubsub/findings` — authenticated (OIDC) Pub/Sub push on the `findings` topic.

## What it decides
1. **Real?** Is this actionable or noise?
2. **Duplicate?** Check Firestore for an open ticket with the same `signature`.
3. **Severity** via [`grounding/severity-rubric.md`](../../grounding/severity-rubric.md).
4. **Root cause hypothesis + suggested fix**, citing the relevant runbook.

## Output — `ticket`
See the schema in [`docs/architecture.md`](../../docs/architecture.md#3-data-contracts).
Published to `$TICKETS_TOPIC`.

## Model access (Vertex Gemini)
Same as the monitoring agent — `google-genai` on Vertex via ADC. Set
`GEMINI_LOCATION` + `GEMINI_MODEL` (default `gemini-3.5-flash`, v3+).

## Config (env)
| Var | Meaning |
|-----|---------|
| `PROJECT_ID` | GCP project |
| `TICKETS_TOPIC` | topic to publish tickets to |
| `GEMINI_LOCATION` / `GEMINI_MODEL` | Vertex location + Gemini model (v3+) |

## Idempotency (every finding is recorded)

The agent resolves each finding into exactly one **ledger event** — nothing is
dropped silently:

- **`ticketed`** — new + actionable → `create_ticket` (publish + register `known_issue`).
- **`duplicate_closed`** — signature already registered → `close_as_duplicate`
  (recorded, closed with no action, occurrence count incremented).
- **`ignored`** — noise → `ignore_finding` with a reason.

These `events`, plus `known_issues`, are what the management console reads.

## Build & deploy

Build from the **repo root** (bundles `grounding/`):

```bash
REPO=$(cd terraform && terraform output -raw artifact_registry)
gcloud builds submit . --config=agents/triage-agent/cloudbuild.yaml \
  --substitutions=_IMAGE="$REPO/triage-agent:latest"
# then set triage_agent_image in terraform.tfvars and `terraform apply`
```

> **Status:** implemented on **Vertex Gemini** (`gemini-3.5-flash`, v3+); `ruff` clean
> and **verified live** — processed a pushed finding, doing `find_duplicate` →
> `create_ticket`, then `find_duplicate` → `close_as_duplicate` (dedup working).
> Tools: `find_duplicate`, `create_ticket`, `close_as_duplicate`, `ignore_finding`.
