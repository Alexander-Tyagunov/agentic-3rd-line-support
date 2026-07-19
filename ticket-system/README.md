# Ticket System (Rust)

The product-of-record for tickets and the **human review gate** before code
changes. A Cargo workspace of three crates so the backend and the WASM UI share
one set of serde types — the payoff of keeping this in Rust.

```
ticket-system/
├── shared/    # serde types: Ticket, Finding, Severity, TicketStatus (used by both)
├── backend/   # axum: consumes `tickets` (Pub/Sub push) → Firestore; serves the UI + REST
└── ui/        # Leptos + WASM: lists/inspects tickets; "Approve & fix"
```

## Flow

1. Triage agent publishes a `ticket` to the `tickets` topic.
2. `backend` receives it via `POST /pubsub/tickets` (OIDC) and stores it in
   Firestore with status `proposed`.
3. A human opens the UI (served by `backend`), reviews `proposed` tickets, and
   clicks **Approve & fix**.
4. `backend` creates a **GitHub Issue** labeled `agent-bug` (using the token from
   Secret Manager) → the coding-agent workflow implements it and opens a PR.
   - Set `AUTO_APPROVE=true` to skip the human step for a fully-autonomous demo.

## Endpoints (backend)
| Method | Path | Purpose |
|--------|------|---------|
| `GET`  | `/health` | Liveness (Cloud Run's frontend reserves `/healthz`) |
| `GET`  | `/api/tickets` | List tickets (Review + History views) |
| `GET`  | `/api/events` | The event ledger (monitored/ticketed/deduped/ignored) |
| `GET`  | `/api/known-issues` | Dedup registry (signature → canonical ticket, occurrences) |
| `GET`  | `/api/health` | Component heartbeats |
| `GET`  | `/api/runs` | Agent run history (monitoring + triage: success/fail, timing) |
| `GET`  | `/api/meta` | Deployment identity for UI deep-links (project/owner/repo/region) |
| `POST` | `/api/tickets/{id}/approve` | Approve → open a GitHub Issue (`agent-bug`); idempotent |
| `POST` | `/api/tickets/{id}/retry-coding` | Restart the coding agent (re-fire the `agent-bug` label) |
| `POST` | `/api/agents/monitoring/run` | "Run sweep now" — OIDC-invoke the monitoring agent's `/sweep` |
| `POST` | `/api/simulate` | Proxy to the synthetic app's `/simulate` (Simulate tab) |
| `POST` | `/api/admin/reset` | Scoped reset (`?scope=all\|tickets\|events\|known_issues\|health\|runs\|queue`) |
| `GET`  | `/api/ops` | Cloud Run fleet status (per-service min-instances + state) |
| `POST` | `/api/ops/scale` | Scale a service (min instances) |
| `POST` | `/pubsub/tickets` | Pub/Sub push handler (persists tickets; auto-approves if `AUTO_APPROVE=true`) |
| `POST` | `/webhook/github` | GitHub webhook: PR/issue outcome → ticket lifecycle + `known_issues` status + ledger event (HMAC-verified when `GITHUB_WEBHOOK_SECRET` is set) |
| `GET`  | `/*` | Serves the WASM UI bundle (`UI_DIST`, default `dist`) |

Also writes a `ticket-backend` heartbeat to `health/` every 60s for the Health view.

Firestore access is via the REST API (`reqwest` + a metadata-server bearer token,
or `GOOGLE_ACCESS_TOKEN` for local dev) — see `backend/src/firestore.rs`.

## Build & run
```bash
# Verify (no trunk needed):
cargo check                                         # host: shared + backend
cargo clippy -p ticket-backend -- -D warnings
cargo check --target wasm32-unknown-unknown -p ticket-ui   # the CSR UI

# UI (WASM) — requires trunk: cargo install trunk
cd ui && trunk build --release        # emits ui/dist/

# Backend serves the UI bundle + the API:
cd ../backend && UI_DIST=../ui/dist PROJECT_ID=<proj> cargo run
# Local dev with hot reload instead: `cd ui && trunk serve` (proxies /api to :8080)
```

## Config (env, backend)
| Var | Meaning |
|-----|---------|
| `PROJECT_ID` | GCP project (Firestore) |
| `REGION` | Cloud Run region (for the Ops + Run-sweep endpoints) |
| `SHOP_URL` | synthetic app base URL (Simulate proxy) |
| `MONITORING_URL` | monitoring agent base URL ("Run sweep now") |
| `GITHUB_OWNER` / `GITHUB_REPO` | where to open Issues |
| `GITHUB_TOKEN` | from Secret Manager (mounted by Terraform) |
| `GITHUB_WEBHOOK_SECRET` | HMAC secret for `/webhook/github` (optional) |
| `AUTO_APPROVE` | `true` = skip the human gate (demo) |

> **Status:** all three crates **implemented & verified** — `shared/` types,
> `backend/` (axum; `cargo check` + `clippy -D warnings` clean), and `ui/`
> (Leptos/WASM; `cargo check --target wasm32-unknown-unknown` clean). The UI has
> seven views — **Events** (filterable, duplicates link to their ticket),
> **Tickets** (lifecycle-aware actions: Approve → issue → PR links + Retry),
> **Known issues**, **Health**, **Runs** (agent success/fail + logs), **Simulate**,
> **Ops** (scale the fleet) — plus a footer and Eastern-time timestamps throughout.
