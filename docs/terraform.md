# Terraform, explained

<samp>[Business value](business-value.md) &nbsp;·&nbsp; [Architecture](architecture.md) &nbsp;·&nbsp; [Setup](setup.md) &nbsp;·&nbsp; **Terraform** &nbsp;·&nbsp; [Deep-dive](article.md) &nbsp;·&nbsp; [↩ README](../README.md)</samp>

> A file-by-file walkthrough of `terraform/` — **what** each resource is and
> **why** it's shaped the way it is. Several choices here are scars from actually
> standing this up; those are called out inline so you don't rediscover them.

<details open>
<summary><b>Contents</b></summary>

- [Design principles](#design-principles)
- [File map](#file-map)
- [`versions.tf` — pins & remote state](#versionstf--pins--remote-state)
- [`providers.tf` — the quota-project fix](#providerstf--the-quota-project-fix)
- [`variables.tf` — the knobs](#variablestf--the-knobs)
- [`apis.tf` — enable everything, disable nothing](#apistf--enable-everything-disable-nothing)
- [`modules/agent-service` — the reusable service pattern](#modulesagent-service--the-reusable-service-pattern)
- [`cloud_run.tf` — the four services + image repo](#cloud_runtf--the-four-services--image-repo)
- [`pubsub.tf` — the message backbone](#pubsubtf--the-message-backbone)
- [`logging.tf` — the deterministic detection lane](#loggingtf--the-deterministic-detection-lane)
- [`iam.tf` — least privilege, and the scale-button saga](#iamtf--least-privilege-and-the-scale-button-saga)
- [`secrets.tf` — containers, not values](#secretstf--containers-not-values)
- [`workload_identity.tf` — keyless CI](#workload_identitytf--keyless-ci)
- [`scheduler.tf` — the proactive sweep](#schedulertf--the-proactive-sweep)
- [`firestore.tf` — state store](#firestoretf--state-store)
- [`cloudbuild.tf` — continuous deployment](#cloudbuildtf--continuous-deployment)
- [`monitoring.tf` — the (optional) budget](#monitoringtf--the-optional-budget)
- [`outputs.tf` — what you wire up next](#outputstf--what-you-wire-up-next)
- [Service-agent prerequisites](#service-agent-prerequisites)
- [Applying, credentials, and redeploys](#applying-credentials-and-redeploys)

</details>

---

## Design principles

Five rules the whole configuration follows:

1. **One service account per workload.** Every Cloud Run service gets its own SA
   with the narrowest useful role set. No shared "app" identity.
2. **Least-privilege, scoped where possible.** Publish rights are *topic*-scoped;
   secret access is *secret*-scoped; `actAs` is *SA*-scoped. Project-wide grants
   are used only where an API genuinely requires it, and flagged as such.
3. **`plan` works before any image exists.** Every service image defaults to the
   public `hello` container, so you can `plan`/`apply` the infrastructure first
   and swap real images in later.
4. **A bare `apply` never breaks itself.** Anything that needs an out-of-band
   value (secrets) is created *conditionally* — the resource that consumes it is
   only wired up when a value is present.
5. **Ephemeral by design.** `deletion_protection = false`, Firestore
   `deletion_policy = DELETE`, `disable_on_destroy = false` — the whole thing is
   meant to be `terraform destroy`ed to near-zero cost.

---

## File map

| File | Responsibility |
|------|----------------|
| `versions.tf` | Terraform + provider version pins; commented remote-state backend |
| `providers.tf` | `google` / `google-beta` providers, `locals`, the project data source |
| `variables.tf` | Every input knob (project, region, models, images, GitHub, budget) |
| `apis.tf` | Enables the 16 required Google APIs |
| `modules/agent-service/` | Reusable "Cloud Run service + dedicated SA + invoker IAM" |
| `cloud_run.tf` | The Artifact Registry repo + the four services (via the module) |
| `pubsub.tf` | Topics, dead-letter topics, OIDC push subscriptions, publisher grants |
| `logging.tf` | Log-based metrics + alert policy → Pub/Sub (the deterministic lane) |
| `iam.tf` | Shared SAs (push/scheduler) + per-workload role grants |
| `secrets.tf` | Secret Manager containers for the GitHub token + webhook HMAC |
| `workload_identity.tf` | WIF pool/provider + the coding-agent SA (keyless CI) |
| `scheduler.tf` | Cloud Scheduler job that pokes the monitoring sweep |
| `firestore.tf` | The Native-mode database |
| `cloudbuild.tf` | Per-service, path-filtered Cloud Build CD triggers + build-SA IAM (gated) |
| `monitoring.tf` | Optional billing budget (gated) |
| `outputs.tf` | URLs, topic names, image repo, WIF outputs for GitHub secrets |

---

## `versions.tf` — pins & remote state

- Terraform **>= 1.6**, `hashicorp/google` and `hashicorp/google-beta` **~> 6.0**.
  `google-beta` is required because a couple of resources (and historically the
  service-agent handling) use beta fields.
- The **GCS remote-state backend is present but commented out**. State on a
  throwaway educational project is fine locally; for anything shared, create a
  bucket, uncomment the block, and `terraform init -migrate-state`.

---

## `providers.tf` — the quota-project fix

```hcl
provider "google" {
  project               = var.project_id
  region                = var.region
  user_project_override = true          # <-- important
  billing_project       = var.project_id
}
```

> [!IMPORTANT]
> `user_project_override = true` + `billing_project` is **not** boilerplate — it
> was added to fix a real failure. When Terraform runs with **user credentials**
> (an `application-default login` token, or a bridged access token), some APIs —
> notably **Billing Budgets** — refuse the request unless a *quota project* is
> attached to it. Without these two lines the budget resource returned a 403 at
> apply time. Setting them makes the provider send the project as the quota/billing
> project on every call.

`locals` here also computes two things used elsewhere:

- `local.project_number` — needed to construct **Google-managed service-agent**
  identities.
- `local.pubsub_agent` / `local.monitoring_agent` — the service-agent emails for
  Pub/Sub and Cloud Monitoring, which must be granted narrow publish rights (see
  [`pubsub.tf`](#pubsubtf--the-message-backbone) and
  [`logging.tf`](#loggingtf--the-deterministic-detection-lane)).

---

## `variables.tf` — the knobs

The ones worth understanding:

| Variable | Default | Why it's like this |
|----------|---------|--------------------|
| `name_prefix` | `a3l` | Kept short so generated SA `account_id`s stay within GCP's 6–30 char limit |
| `gemini_location` | `global` | Vertex location for Gemini; `global` has the widest v3 availability |
| `gemini_model_monitoring` | `gemini-3.1-flash-lite` | Cheap, frequent sweeps. **Hard rule: v3+ only** |
| `gemini_model_triage` | `gemini-3.5-flash` | Grounded reasoning needs more capability. **v3+ only** |
| `*_image` (×4) | public `hello` image | Lets `plan`/`apply` run before real images exist; CI/`gcloud` override per deploy |
| `synthetic_min_instances` | `1` | The log flooder must stay warm to keep emitting — the one steady cost |
| `synthetic_allow_unauthenticated` | `true` | So you can `curl /simulate` in the demo; set `false` behind an org policy |
| `ticket_ui_allow_unauthenticated` | `true` | Public console for the demo; use IAP for anything real |
| `github_token` | `""` (sensitive) | **Never** put the value here — inject via `TF_VAR_github_token`; empty ⇒ the secret container is made but no version |
| `github_webhook_secret` | `""` (sensitive) | Empty ⇒ webhook runs unsecured (demo only); set to enable HMAC verification |
| `billing_account` | `""` | Empty ⇒ **skip the budget entirely** (see the budget note below) |
| `enable_cloudbuild_deploy` | `false` | The Cloud Build path needs a one-time GitHub App install; off by default |

> [!NOTE]
> The **v3+ model gate** is a hard project rule: never drop these defaults to
> `gemini-2.5-*` or lower. The agent loop was specifically debugged against v3 —
> older models returned empty candidates on the first turn.

---

## `apis.tf` — enable everything, disable nothing

Enables all 16 APIs (`run`, `cloudbuild`, `pubsub`, `logging`, `monitoring`,
`secretmanager`, `artifactregistry`, `firestore`, `cloudscheduler`, `aiplatform`,
`iam`, `iamcredentials`, `sts`, `cloudresourcemanager`, `cloudbilling`,
`billingbudgets`) via a single `for_each`.

> [!TIP]
> `disable_on_destroy = false` is deliberate: a `terraform destroy` should **not**
> try to turn APIs off, because disabling an API that other (out-of-band)
> resources still use can fail and wedge the destroy.

`aiplatform.googleapis.com` is the only thing needed for **Gemini** — it's a
first-party Vertex model, so there is no Model Garden enablement or partner EULA
(unlike Claude-on-Vertex, which the project used before migrating).

---

## `modules/agent-service` — the reusable service pattern

Every compute workload (synthetic app, both agents, ticket backend) is one
instance of this module, which bundles three things that always travel together:

1. **A dedicated service account** (`google_service_account.this`).
2. **A Cloud Run v2 service** running as that SA.
3. **Invoker IAM** (`google_cloud_run_v2_service_iam_member` for each member in
   `var.invokers`).

Design details worth noting:

- **`cpu_idle`** — defaults to `true` (CPU throttled between requests). The
  synthetic app overrides it to `false` because its background log-flooder needs
  CPU *between* requests; the agents and backend are request-driven and stay
  throttled (cheaper).
- **`scaling { min_instance_count / max_instance_count }`** — `min` defaults to 0
  (scale to zero) for everything except the synthetic app.
- **`secret_env`** — a `dynamic "env"` block turns a list of
  `{name, secret, version}` into Cloud Run `value_source.secret_key_ref` mounts.
  The caller passes an **empty list** when no secret value exists, so a bare apply
  never mounts a missing secret.
- **`deletion_protection = false`** — required so `terraform destroy` can remove
  the service (Cloud Run v2 defaults this to on).
- **Outputs** — `uri`, `name`, `service_account_email`, and a ready-made
  `service_account_member` (`serviceAccount:<email>`) string that the IAM files
  consume.

---

## `cloud_run.tf` — the four services + image repo

First an **Artifact Registry** Docker repo (`a3l-images`) that all images push to.
Then the four module instances:

| # | Service | Min | `cpu_idle` | Invokers | Notable env |
|---|---------|:---:|:----------:|----------|-------------|
| 0 | `a3l-synthetic-shop` | 1 | **false** | `allUsers` (demo) | `LOG_FLOOD_RATE_PER_SEC`, `SERVICE_NAME=checkout` |
| 1 | `a3l-monitoring-agent` | 0 | true | pubsub-push SA, scheduler SA | `FINDINGS_TOPIC`, `GEMINI_LOCATION/MODEL`, `LOG_LOOKBACK_MINUTES` |
| 2 | `a3l-triage-agent` | 0 | true | pubsub-push SA | `TICKETS_TOPIC`, `GEMINI_LOCATION/MODEL` |
| 3 | `a3l-ticket-backend` | 0 | true | pubsub-push SA + `allUsers` (demo) | `REGION`, `SHOP_URL`, `GITHUB_OWNER/REPO` + conditional secrets |

Key points:

- The monitoring agent is invokable by **two** identities — the scheduler (for the
  `/sweep`) and the Pub/Sub push SA (for `/pubsub/alerts` enrichment).
- The ticket backend gets **`REGION` and `SHOP_URL`** in its env because the
  console's Ops and Simulate features call the Cloud Run Admin API and proxy to the
  shop — it needs to know the region and the shop URL at runtime.
- Its **`secret_env` is built with `concat` + conditionals** — `GITHUB_TOKEN` and
  `GITHUB_WEBHOOK_SECRET` are only mounted if their variables are non-empty. This
  is what keeps a first, tokenless `apply` from failing.

---

## `pubsub.tf` — the message backbone

Three primary topics — `log-alerts`, `findings`, `tickets` — plus two dead-letter
topics (`findings-dlq`, `tickets-dlq`). Each primary topic has an **authenticated
push subscription** into the service that owns it.

- **OIDC push, not pull.** Every subscription carries an `oidc_token` minted for
  the shared `pubsub-push` SA, with `audience` set to the target service URL. That
  SA is granted `run.invoker` on exactly one service (in `cloud_run.tf`), so a
  subscription can only ever invoke its intended target.
- **Publisher grants are topic-scoped.** The monitoring agent gets
  `pubsub.publisher` on `findings` *only*; triage on `tickets` *only*. Neither can
  publish anywhere else.
- **Dead-letter wiring needs the Pub/Sub service agent.** `findings` and `tickets`
  subscriptions declare a `dead_letter_policy` (5 attempts → DLQ). For that to
  work, Google's Pub/Sub **service agent** must be able to publish to the DLQ
  topics and subscribe to the source subscriptions — granted here via
  `local.pubsub_agent`.
- **Backoff/ack** — 10s→600s retry backoff; ack deadlines sized per stage (60s for
  alerts/tickets, 120s for findings since triage does more work).

> [!NOTE]
> The `log-alerts` subscription has **no** dead-letter policy — it carries
> best-effort enrichment notifications, not the primary data path, so dropping one
> is acceptable.

---

## `logging.tf` — the deterministic detection lane

This is the "known patterns, no LLM" half of detection:

- **`payment_errors`** — a DELTA counter metric over
  `jsonPayload.event="payment.failed" severity>=ERROR`, with `service` extracted as
  a label.
- **`checkout_latency_ms`** — a DISTRIBUTION metric extracting
  `jsonPayload.latency_ms`, with exponential buckets (feeds the
  `non_obvious_anomaly` scenario).
- **A Pub/Sub notification channel** pointing at the `log-alerts` topic, plus a
  grant so the **Monitoring service agent** may publish to it.
- **An alert policy** — "payment errors > 5 in 5m" — that fires into that channel,
  which pushes into the monitoring agent for enrichment.

> [!WARNING]
> `alert_strategy` only sets `auto_close = 1800s`. It intentionally does **not**
> set `notification_rate_limit` — that field is valid *only* on log-based alert
> policies, and including it on this metric-threshold policy failed at apply. If
> you copy this pattern, don't re-add it here.

> [!IMPORTANT]
> The Monitoring service agent (`service-<num>@gcp-sa-monitoring…`) may **not exist
> yet** on a fresh project, which makes the publisher grant fail. See
> [Service-agent prerequisites](#service-agent-prerequisites) for the one-line fix.

---

## `iam.tf` — least privilege, and the scale-button saga

Two shared SAs live here: `pubsub-push` (OIDC push identity) and `scheduler`
(sweep invoker). The rest are per-workload grants:

| Workload | Roles | Scope |
|----------|-------|-------|
| synthetic app | `logging.logWriter` | project |
| monitoring agent | `logging.viewer`, `aiplatform.user`, `datastore.user` | project |
| triage agent | `aiplatform.user`, `datastore.user` | project |
| ticket backend | `datastore.user` | project |
| Pub/Sub service agent | `iam.serviceAccountTokenCreator` | project (to mint push OIDC tokens) |

The console's **Ops / admin** features needed more, and getting them right took
real debugging — documented so the reasoning survives:

- **`pubsub.editor`** — the console's scoped "reset queue" seeks/purges
  subscriptions.
- **`run.developer`** — read Cloud Run status *and* update services (the Ops
  scale buttons).
- **`iam.serviceAccountUser`, granted per-SA** (`ticket_actas`, a `for_each` over
  the four runtime SAs) — scaling a service **creates a new revision**, and Cloud
  Run requires the caller to `actAs` that revision's runtime SA. This is granted on
  exactly the four SAs the console can scale, **not** project-wide.
- **`artifactregistry.reader`, repo-scoped** (`ticket_ar_reader`) — the fix for a
  403 that cost hours.

> [!IMPORTANT]
> **The scale-button 403, and why it was misleading.** The Ops "scale" call
> (`PATCH …/services/<name>?updateMask=template`) kept returning **403** even
> though the backend SA had `run.developer` (which *includes* `run.services.update`)
> **and** `serviceAccountUser`. The real denied permission — only visible once the
> backend stopped swallowing the response body — was
> **`artifactregistry.repositories.downloadArtifacts`**. Creating a revision makes
> Cloud Run **preflight-verify that the caller can pull the image**; the console SA
> had no Artifact Registry read access. `GET`/status requests skip that check,
> which is exactly why *reads worked and writes 403'd*. The fix is the repo-scoped
> `roles/artifactregistry.reader` grant. (A broad project-level
> `serviceAccountUser` that had been added while chasing the wrong hypothesis was
> then removed in favour of the per-SA grants.)

---

## `secrets.tf` — containers, not values

Two Secret Manager secrets: the **GitHub token** (backend opens Issues) and the
optional **webhook HMAC secret**.

- Terraform creates the secret **container** and the **accessor IAM** (backend SA
  gets `secretmanager.secretAccessor`), but the **value** is supplied out of band:
  `export TF_VAR_github_token=…` before apply, or `gcloud secrets versions add`
  later.
- Each `secret_version` is **conditional** (`count = var.x != "" ? 1 : 0`) so a
  tokenless apply just makes empty containers — matching the conditional
  `secret_env` mounts in `cloud_run.tf`.

> [!NOTE]
> This is the whole reason there is **no model key** anywhere: Gemini is reached
> via Vertex + a service account, so the GitHub token is the *only* application
> secret in the cloud path.

---

## `workload_identity.tf` — keyless CI

Lets the **Gemini CLI GitHub Action** authenticate to Vertex with **no
downloadable key**:

- A **Workload Identity Pool** + **OIDC provider** trusting GitHub's issuer
  (`token.actions.githubusercontent.com`).
- An **`attribute_condition`** that only accepts tokens whose
  `repository_owner` matches `var.github_owner` — so only your org's workflows can
  use the pool.
- A dedicated **`gh-actions` SA** with `aiplatform.user` *only*, which the repo's
  federated identity (`principalSet://…/attribute.repository/<owner>/<repo>`) may
  impersonate via `workloadIdentityUser`.

The pool provider name and SA email are surfaced as outputs to set as the
`GCP_WORKLOAD_IDENTITY_PROVIDER` / `GCP_SERVICE_ACCOUNT` repo secrets.

---

## `scheduler.tf` — the proactive sweep

A Cloud Scheduler job hits `POST {monitoring_agent}/sweep` every 10 minutes
(`*/10 * * * *`, UTC) with an **OIDC token** minted for the `scheduler` SA
(audience = the agent URL). This is what drives *emergent* detection independent of
any alert firing.

---

## `firestore.tf` — state store

One **Native-mode** database (the `(default)` database, one per project) backing
the triage dedup index, the ticket store, the event ledger, and health heartbeats
(collections `known_issues`, `tickets`, `events`, `health`).

- `location_id = var.region` — must be a valid Firestore location (a region or a
  multi-region like `nam5`).
- `deletion_policy = DELETE` + protection disabled so `terraform destroy` removes
  it.

---

## `cloudbuild.tf` — continuous deployment

Gated on `enable_cloudbuild_deploy` (default `false`). When on, it creates **one
Cloud Build trigger per service** — each **path-filtered** so a merge to `main`
only rebuilds and redeploys the app whose folder changed (editing the UI never
redeploys the agents, and vice versa):

| Service | Rebuilds when these change (`included_files`) | Build context |
|---------|-----------------------------------------------|---------------|
| `a3l-synthetic-shop` | `apps/synthetic-shop/**` | `apps/synthetic-shop` |
| `a3l-monitoring-agent` | `agents/monitoring-agent/**`, `grounding/**` | repo root (bundles `grounding/`) |
| `a3l-triage-agent` | `agents/triage-agent/**`, `grounding/**` | repo root |
| `a3l-ticket-backend` | `ticket-system/**` (backend + WASM UI + shared) | `ticket-system` |

All four triggers run the same root [`cloudbuild.yaml`](../cloudbuild.yaml) with
different substitutions; it builds the image tagged with the **commit SHA** (and
`:latest`), pushes both, and `gcloud run deploy`s the **SHA** tag — an immutable
reference, so there's no moving-`:latest` ambiguity and no manual force needed.

Builds run as a **dedicated user-managed SA** (`a3l-cloudbuild`) — an org policy on
this project forbids the default Cloud Build SA, so the triggers set
`service_account` explicitly (and `cloudbuild.yaml` uses `CLOUD_LOGGING_ONLY`,
which a user-managed SA requires). This file grants that SA
`roles/cloudbuild.builds.builder` + `roles/artifactregistry.writer` (build + push),
`roles/run.developer` + per-SA `roles/iam.serviceAccountUser` (deploy + actAs each
runtime SA), and grants the **Cloud Build service agent**
`roles/iam.serviceAccountTokenCreator` on it (required to run a build as a
user-specified SA).

> [!IMPORTANT]
> **One-time manual step:** connect the repo to Cloud Build via the "Google Cloud
> Build" GitHub App (Console → Cloud Build → Triggers → Connect repository). The
> App handles auth, so — unlike a 2nd-gen connection — no GitHub token or OAuth
> secret is stored. Full runbook in [setup.md §13](setup.md#13--continuous-deployment-cloud-build).

> [!NOTE]
> Once CD is on, **Cloud Build owns image rollouts.** The `agent-service` module
> sets `ignore_changes` on the container image (and the `gcloud` client metadata)
> so a later `terraform apply` won't revert a CI-deployed `:SHA` image back to the
> `:latest` placeholder.

---

## `monitoring.tf` — the (optional) budget

A `google_billing_budget` with 50% / 90% / 100% threshold alerts, **gated on
`billing_account != ""`**.

> [!WARNING]
> The reference deployment runs with `billing_account = ""` (budget skipped). The
> Budgets API rejected the request unless the caller holds billing-account-level
> permissions *and* a quota project is attached — the latter is what
> `user_project_override` in `providers.tf` addresses, but the account-level
> permission still has to exist. If you own the billing account, set it and get the
> alerts; otherwise leave it empty and rely on `terraform destroy`.

---

## `outputs.tf` — what you wire up next

| Output | Use |
|--------|-----|
| `synthetic_shop_url` | `POST {url}/simulate` to inject scenarios |
| `ticket_ui_url` | the console (also serves `/webhook/github`) |
| `monitoring_agent_url` / `triage_agent_url` | agent endpoints |
| `topics` | the five topic names |
| `artifact_registry` | the Docker repo to push images to |
| `github_actions_service_account` | → `GCP_SERVICE_ACCOUNT` repo secret |
| `github_workload_identity_provider` | → `GCP_WORKLOAD_IDENTITY_PROVIDER` repo secret |

---

## Service-agent prerequisites

Two Google-managed **service agents** are referenced by IAM grants. They're created
by Google automatically the first time the relevant API is *used* — which can be
*after* Terraform tries to grant them a role, causing a "service account does not
exist" apply error. On a fresh project, force them to exist first:

```bash
# Monitoring service agent (needed by logging.tf's publisher grant)
gcloud beta services identity create \
  --service=monitoring.googleapis.com --project="$PROJECT_ID"

# The Pub/Sub service agent is normally created when the API is enabled; if a
# grant in pubsub.tf 404s, the same command for pubsub.googleapis.com fixes it.
```

> [!IMPORTANT]
> This exact step was required on the reference project — the first apply failed on
> the `logging.tf` publisher grant until the Monitoring service agent was
> provisioned.

---

## Applying, credentials, and redeploys

The mechanics that this project actually uses (full command-level detail lives in
**[setup.md → the exact reference runbook](setup.md#appendix-a--the-exact-commands-used-for-the-reference-deployment)**):

- **Credentials.** Terraform uses ADC by default. When you need it to act as a
  *specific* account without disturbing your active `gcloud` config, bridge a token
  in for one command:
  ```bash
  export GOOGLE_OAUTH_ACCESS_TOKEN="$(gcloud auth print-access-token --account=you@example.com)"
  export TF_VAR_github_token="$YOUR_TOKEN_ENV_VAR"   # value never printed/committed
  terraform apply
  ```
- **Images.** With CD enabled, **Cloud Build owns deploys** — it pushes an
  immutable `:SHA` image and rolls the revision itself, and the module's
  `ignore_changes` stops `terraform apply` from reverting it. Without CD, deploy by
  setting `*_image` + applying, or `gcloud run deploy` — note a re-pushed `:latest`
  alone is a no-op for Terraform (identical string, no diff), which is exactly why
  CD deploys by `:SHA` instead.
- **`/health`, not `/healthz`.** Cloud Run's front end reserves the exact path
  `/healthz` and returns a Google 404 that never reaches the container — every
  service exposes `/health` instead. (Not a Terraform concern, but it bites during
  post-apply verification.)
- **Teardown.** `terraform destroy` removes everything; the synthetic app's
  `min-instances 1` is the only steady cost while running.
