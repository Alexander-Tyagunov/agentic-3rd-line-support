# Synthetic Shop (Rust)

A fake e-commerce service whose only job is to produce **realistic, business-
meaningful logs** for the rest of the pipeline to reason over. It is intentionally
simple — think of it as executable pseudocode for "a service that logs like a real one."

## What it does

1. **Floods logs continuously.** A background task emits structured JSON log lines
   (`browse`, `add_to_cart`, `checkout`, `payment.captured`, `order.created`,
   `auth.login`) at `LOG_FLOOD_RATE_PER_SEC`, so Cloud Logging always has traffic.
2. **Injects failures on demand** via one endpoint, so you can drive the demo live.

## Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| `GET`  | `/health` | Liveness (Cloud Run's frontend reserves `/healthz`) |
| `POST` | `/simulate` | Inject a scenario (see below) |

### `POST /simulate`

```jsonc
// request
{ "scenario": "orphaned_txn", "count": 10 }
```

| `scenario` | What it emits | Detection lane |
|------------|---------------|----------------|
| `obvious_txn_error` | burst of `payment.failed` (HTTP 500 / severity ERROR) | deterministic alert |
| `logging_error` | an exception with a stack trace on `message` | deterministic / agentic |
| `orphaned_txn` | `payment.captured` with **no** matching `order.created` | agentic (correlation) |
| `non_obvious_anomaly` | gradual latency creep / elevated retry rate | agentic only |
| `db_pool_exhaustion` | connection-pool timeout errors | deterministic |
| `inventory_oversell` | `inventory.oversold` — the planted **code bug** | agentic → coding agent |
| `panic` | a rare unhandled 5xx spike | deterministic |

## Log format

One JSON object per line to stdout, using the fields Cloud Logging promotes
(`severity`, `message`, `logging.googleapis.com/trace`, `.../labels`,
`httpRequest`). Example:

```json
{"severity":"ERROR","message":"charge failed: gateway timeout","logging.googleapis.com/trace":"projects/PROJECT_ID/traces/06796866738c859f2f19b7cfb3214824","logging.googleapis.com/labels":{"service":"payments","request_id":"a1b2c3"},"event":"payment.failed","reason":"gateway_timeout","httpRequest":{"requestMethod":"POST","status":504}}
```

## Run locally

```bash
cargo run
# in another shell:
curl localhost:8080/health
curl -X POST localhost:8080/simulate -H 'content-type: application/json' \
  -d '{"scenario":"orphaned_txn","count":10}'
```

## Deploy to Cloud Run

```bash
REPO=$(cd ../../terraform && terraform output -raw artifact_registry)
gcloud builds submit --tag "$REPO/synthetic-shop:latest"
gcloud run deploy a3l-synthetic-shop --image "$REPO/synthetic-shop:latest" \
  --region "$REGION" --no-cpu-throttling --min-instances 1
```

## Config (env)

| Var | Default | Meaning |
|-----|---------|---------|
| `PORT` | `8080` | Cloud Run injects this |
| `LOG_FLOOD_RATE_PER_SEC` | `20` | Baseline healthy-traffic log rate |
| `SERVICE_NAME` | `checkout` | Value put in the `service` label |

> **Status:** implemented & verified (clippy clean, runs locally; endpoints and
> Cloud Logging-shaped output confirmed).
