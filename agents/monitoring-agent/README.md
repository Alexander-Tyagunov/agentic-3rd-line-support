# Monitoring Agent (Python · Vertex Gemini)

Detects **known and emergent** risky patterns in Cloud Logging and emits a
structured *finding* to the `findings` Pub/Sub topic.

## Triggers
- `POST /sweep` — Cloud Scheduler calls this every ~10 min. The agent queries a
  window of logs (LQL templates from [`grounding/risky-patterns.md`](../../grounding/risky-patterns.md))
  and looks for emergent clusters, correlation violations (orphaned txns), and
  novel error signatures.
- `POST /pubsub/alerts` — a Pub/Sub push from a deterministic alert; the agent
  enriches the known alert with context before emitting the finding.

## Output — `finding`
See the schema in [`docs/architecture.md`](../../docs/architecture.md#3-data-contracts).
Published to `$FINDINGS_TOPIC`.

## Model access (Vertex Gemini)
Runs with its service account (`roles/aiplatform.user`) — `google-genai` uses ADC
via `genai.Client(vertexai=True, project, location)`, no keys. Set:
```
GEMINI_LOCATION=global
GEMINI_MODEL=gemini-3.1-flash-lite   # v3+ only
```

## Run locally
```bash
uv sync
uv run uvicorn app.main:app --port 8080
curl -X POST localhost:8080/sweep
```

## Config (env)
| Var | Meaning |
|-----|---------|
| `PROJECT_ID` | GCP project |
| `FINDINGS_TOPIC` | topic to publish findings to |
| `GEMINI_LOCATION` / `GEMINI_MODEL` | Vertex location + Gemini model (v3+) |
| `LOG_LOOKBACK_MINUTES` | sweep window size |

## Build & deploy

Build from the **repo root** (so `grounding/` is bundled in):

```bash
REPO=$(cd terraform && terraform output -raw artifact_registry)
gcloud builds submit . --config=agents/monitoring-agent/cloudbuild.yaml \
  --substitutions=_IMAGE="$REPO/monitoring-agent:latest"
# then set monitoring_agent_image in terraform.tfvars and `terraform apply`
```

> **Status:** implemented on **Vertex Gemini** (`gemini-3.1-flash-lite`, v3+); `ruff`
> clean and **verified live** — a scheduled sweep emitted a finding end-to-end.
> Tools: `query_logs` (read) + `emit_finding` (publish). Distroless runtime, headless.
