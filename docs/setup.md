# Setup & installation

<samp>[Business value](business-value.md) &nbsp;·&nbsp; [Architecture](architecture.md) &nbsp;·&nbsp; **Setup** &nbsp;·&nbsp; [Terraform](terraform.md) &nbsp;·&nbsp; [Deep-dive](article.md) &nbsp;·&nbsp; [↩ README](../README.md)</samp>

> Everything needed to roll this out to **your own GCP project**, end to end, then
> tear it down. Commands use a `PROJECT_ID` variable so you can copy-paste.

> [!NOTE]
> The maintainer's project (`agentic-3rd-line-support`, 217601243322) already has
> its APIs enabled via the command in [step 4](#4-enable-the-gcp-apis). If you are
> reproducing this elsewhere, run every step.

<details open>
<summary><b>Contents</b></summary>

| Prep | Provision | Deploy |
|------|-----------|--------|
| [Prerequisites](#prerequisites) | [4 · Enable APIs](#4-enable-the-gcp-apis) | [9 · Build & deploy](#9-build--deploy-the-services) |
| [1 · Clone](#1-clone) | [5 · Gemini on Vertex](#5-gemini-on-vertex-ai-no-model-garden-step) | [10 · Run the demo](#10-run-the-demo) |
| [2 · Project + billing](#2-gcp-project--billing) | [6 · GitHub credentials](#6-github-credentials) | [11 · Teardown](#11-teardown) |
| [3 · Authenticate](#3-authenticate-gcloud) | [7 · Terraform](#7-terraform) · [8 · Post-apply wiring](#8-post-apply-wiring) | [12 · Troubleshooting](#12-troubleshooting) · [13 · CI/CD](#13--continuous-deployment-cloud-build) |

**Reproduce exactly:** [Appendix A — the as-executed runbook](#appendix-a--the-exact-commands-used-for-the-reference-deployment) &nbsp;·&nbsp; [Appendix B — gotchas & fixes](#appendix-b--gotchas-encountered-and-how-they-were-fixed) &nbsp;·&nbsp; see also [Terraform, explained](terraform.md)

</details>

---

## Prerequisites

Tools (install what you don't have):

| Tool | Used for | Install |
|------|----------|---------|
| `gcloud` | GCP auth + API enablement | https://cloud.google.com/sdk/docs/install |
| `terraform` >= 1.6 | infrastructure | https://developer.hashicorp.com/terraform/install |
| `gh` | set repo secrets, labels | https://cli.github.com/ |
| `cargo` (Rust) | build the synthetic app + ticket system | https://rustup.rs/ |
| `trunk` | build the WASM UI | `cargo install trunk` |
| `uv` (or pip) | build the Python agents | https://docs.astral.sh/uv/ |
| Docker *(optional)* | local image builds (or use Cloud Build) | https://docs.docker.com/get-docker/ |

You also need a **GCP project with billing enabled** and a **GitHub repo** to
hold this code (this one).

```bash
export PROJECT_ID="your-project-id"      # e.g. agentic-3rd-line-support
export REGION="us-central1"
export GEMINI_LOCATION="global"          # Vertex location for Gemini
export GH_OWNER="your-github-username"
export GH_REPO="agentic-3rd-line-support"
```

---

## 1. Clone

```bash
git clone "https://github.com/$GH_OWNER/$GH_REPO.git" && cd "$GH_REPO"
```

## 2. GCP project + billing

Create a project (skip if you have one) and link billing:

```bash
gcloud projects create "$PROJECT_ID"                       # optional
gcloud billing accounts list                               # find your billing account id
gcloud billing projects link "$PROJECT_ID" --billing-account=XXXXXX-XXXXXX-XXXXXX
```

## 3. Authenticate gcloud

```bash
gcloud auth login                                          # user login
gcloud auth application-default login                      # ADC (Terraform uses this)
gcloud config set project "$PROJECT_ID"
```

> [!TIP]
> If you juggle multiple accounts, add `--account=you@example.com` to each command
> instead of switching the active account.

## 4. Enable the GCP APIs

This is the exact command run for the reference project:

```bash
gcloud services enable \
  run.googleapis.com \
  cloudbuild.googleapis.com \
  pubsub.googleapis.com \
  logging.googleapis.com \
  monitoring.googleapis.com \
  secretmanager.googleapis.com \
  artifactregistry.googleapis.com \
  firestore.googleapis.com \
  cloudscheduler.googleapis.com \
  aiplatform.googleapis.com \
  iam.googleapis.com \
  iamcredentials.googleapis.com \
  sts.googleapis.com \
  cloudresourcemanager.googleapis.com \
  cloudbilling.googleapis.com \
  billingbudgets.googleapis.com \
  --project="$PROJECT_ID"
```

> [!NOTE]
> Terraform also declares these in `terraform/apis.tf`, so a later `apply` is a
> no-op for anything already enabled — enabling now just makes the first apply
> faster.

## 5. Gemini on Vertex AI (no Model Garden step)

The agents call **Gemini**, a **first-party** Vertex model — no Model Garden
enablement or EULA is required (unlike partner models such as Claude). Enabling
`aiplatform.googleapis.com` in step 4 is enough. Models used (all **v3+**):
`gemini-3.1-flash-lite` (monitoring), `gemini-3.5-flash` (triage + coding).

Verify access (uses ADC; returns 200 with a small JSON):

```bash
curl -sS -X POST \
  -H "Authorization: Bearer $(gcloud auth application-default print-access-token)" \
  -H "Content-Type: application/json" \
  "https://aiplatform.googleapis.com/v1/projects/${PROJECT_ID}/locations/global/publishers/google/models/gemini-3.5-flash:generateContent" \
  -d '{"contents":[{"role":"user","parts":[{"text":"ping"}]}]}' | head -c 400
```

> [!IMPORTANT]
> First-party Gemini needs no partner-model policy exceptions. If a call 404s,
> confirm the model id is **v3+** and available in your location (try `global` or
> `us-central1`).

## 6. GitHub credentials

There are **two independent** GitHub credentials — don't confuse them.

### 6a. Token for the ticket backend (opens Issues)

The ticket backend calls the GitHub REST API to create an Issue when a ticket is
approved. Get a token scoped to *just this repo*:

- **Fine-grained PAT (recommended)** — GitHub → **Settings → Developer settings →
  Personal access tokens → Fine-grained tokens → Generate new token**.
  - *Resource owner:* your account.
  - *Repository access:* **Only select repositories → this repo**.
  - *Permissions → Repository:* **Issues: Read and write** (Metadata: Read-only is added automatically).
  - Copy the token (`github_pat_…`).
- **Classic PAT (simpler)** — same menu → *Tokens (classic)* → scope **`repo`**.

Provide it to Terraform via an env var (never commit it):

```bash
export TF_VAR_github_token="github_pat_xxx"
```

### 6b. Credential for the coding agent (Gemini CLI GitHub Action)

The Action authenticates to **Gemini via Vertex** (no keys — Workload Identity
Federation) and to **GitHub** via a token:

- **Default** — the built-in `GITHUB_TOKEN` is enough to open PRs.
- **Recommended (tighter control)** — create a GitHub App
  (https://github.com/settings/apps/new) with Contents / Issues / Pull requests:
  Read & write, install it on the repo, and add `APP_ID` + `APP_PRIVATE_KEY` repo
  secrets. See `.github/workflows/gemini.yml`.

The GCP side (Workload Identity Federation + project id) is created by Terraform;
wire its outputs into repo secrets in [step 8](#8-post-apply-wiring).

## 7. Terraform

```bash
cd terraform
cp terraform.tfvars.example terraform.tfvars
# edit terraform.tfvars: project_id, region, github_owner, github_repo (gemini_* have v3 defaults)
terraform init
terraform plan       # review
terraform apply
```

> [!IMPORTANT]
> Secrets never live in `.tf`/`.tfvars`: the GitHub token comes from
> `TF_VAR_github_token` (step 6a) and lands in Secret Manager; model auth is via
> service accounts, so there is no model key.

## 8. Post-apply wiring

Push the WIF outputs into the repo as Actions secrets, and create the trigger label:

```bash
# still in terraform/
gh secret set GCP_WORKLOAD_IDENTITY_PROVIDER -R "$GH_OWNER/$GH_REPO" \
  -b "$(terraform output -raw github_workload_identity_provider)"
gh secret set GCP_SERVICE_ACCOUNT -R "$GH_OWNER/$GH_REPO" \
  -b "$(terraform output -raw github_actions_service_account)"
gh secret set GCP_PROJECT_ID -R "$GH_OWNER/$GH_REPO" -b "$PROJECT_ID"

gh label create agent-bug -R "$GH_OWNER/$GH_REPO" \
  -c B60205 -d "Auto-filed by the triage agent" || true
```

<details>
<summary><b>Optional — GitHub webhook (PR/issue feedback loop)</b></summary>

<br/>

So merged/declined PRs flow back into the ticket lifecycle and the dedup registry
(a merged fix vs a won't-fix changes how the next duplicate is handled), point a
webhook at the console backend:

```bash
BACKEND=$(cd terraform && terraform output -raw ticket_ui_url)   # serves /webhook/github

# Pick a secret, store it for the backend, and re-apply so it's mounted:
export TF_VAR_github_webhook_secret="$(openssl rand -hex 20)"
(cd terraform && terraform apply)

gh api -X POST "repos/$GH_OWNER/$GH_REPO/hooks" \
  -f "name=web" -F "active=true" \
  -f "events[]=issues" -f "events[]=pull_request" \
  -f "config[url]=$BACKEND/webhook/github" \
  -f "config[content_type]=json" \
  -f "config[secret]=$TF_VAR_github_webhook_secret"
```

> [!WARNING]
> The backend verifies the `X-Hub-Signature-256` HMAC only when
> `GITHUB_WEBHOOK_SECRET` is set. If it is empty it runs **unsecured — accepting
> all posts** — which is fine for a local demo but not for anything exposed.

</details>

## 9. Build & deploy the services

Images default to a placeholder until you push real ones:

```bash
REPO=$(cd terraform && terraform output -raw artifact_registry)
gcloud auth configure-docker "${REGION}-docker.pkg.dev"

# Synthetic shop (ready today):
gcloud builds submit apps/synthetic-shop --tag "$REPO/synthetic-shop:latest"
gcloud run deploy a3l-synthetic-shop --image "$REPO/synthetic-shop:latest" \
  --region "$REGION" --no-cpu-throttling --min-instances 1 --allow-unauthenticated

# Agents + ticket backend: build the same way, then either re-`terraform apply`
# with the *_image variables set, or `gcloud run deploy`.
```

> [!NOTE]
> Re-pushing an unchanged `:latest` tag won't make Terraform redeploy (the image
> string is identical). Force a new revision with `gcloud run deploy --image ...`,
> or change an env var.

## 10. Run the demo

```bash
URL=$(gcloud run services describe a3l-synthetic-shop --region "$REGION" --format='value(status.url)')
curl "$URL/health"
curl -X POST "$URL/simulate" -H 'content-type: application/json' \
  -d '{"scenario":"orphaned_txn","count":10}'
```

Then watch: Logs Explorer → the monitoring agent produces a finding → triage
files a ticket → the ticket UI shows it → approve → a PR appears.

Scenarios: `obvious_txn_error`, `logging_error`, `orphaned_txn`,
`non_obvious_anomaly`, `db_pool_exhaustion`, `inventory_oversell`, `panic`.

## 11. Teardown

```bash
cd terraform
terraform destroy
```

Optionally disable the APIs and delete the Artifact Registry images. The synthetic
app's `min-instances 1` is the only steady cost while running.

---

## 12. Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| **Vertex 404 / model not found** | model id not v3+ or not in your location | confirm the Gemini model id is **v3+** and available in `GEMINI_LOCATION` (try `global` or `us-central1`) |
| **`allUsers` invoker rejected** | org policy `iam.allowedPolicyMemberDomains` blocks public members | set `synthetic_allow_unauthenticated=false` and call with an identity token |
| **Budget errors on apply** | no permission on the billing account | leave `billing_account=""` to skip the budget, or grant yourself `roles/billing.costsManager` |
| **Coding agent doesn't run** | missing label / secrets / App install | confirm the GitHub App is installed, the `agent-bug` label exists, and the `GCP_*` secrets are set |
| **`GET /healthz` returns a Google 404** | Cloud Run's frontend reserves the exact path `/healthz` | the services expose **`/health`** instead — any other path reaches the container normally (verified: `/randomxyz` → the app's own 404) |
| **Console scale button 403s** | console SA can't pull the image during the revision preflight | grant the ticket-backend SA `roles/artifactregistry.reader` on the images repo (Terraform does this) |

---

## 13 · Continuous deployment (Cloud Build)

Turn on **per-service, path-filtered** CD so a merge to `main` (or a direct push)
automatically builds and redeploys **only** the app whose folder changed — no more
manual `gcloud builds submit` / `gcloud run deploy`.

**How it's wired** — `terraform/cloudbuild.tf` creates one trigger per service,
each with `included_files` scoped to its folder, all running the root
[`cloudbuild.yaml`](../cloudbuild.yaml). Each build tags the image with the commit
`:SHA` (and `:latest`) and deploys the immutable `:SHA` — so a UI change rebuilds
only `a3l-ticket-backend`, an `agents/triage-agent/**` (or `grounding/**`) change
rebuilds only `a3l-triage-agent`, and so on.

| Trigger | Fires on changes to |
|---------|---------------------|
| `a3l-deploy-synthetic-shop` | `apps/synthetic-shop/**` |
| `a3l-deploy-monitoring-agent` | `agents/monitoring-agent/**`, `grounding/**` |
| `a3l-deploy-triage-agent` | `agents/triage-agent/**`, `grounding/**` |
| `a3l-deploy-ticket-backend` | `ticket-system/**` |

### 13a · One-time: connect the repo to Cloud Build

Cloud Build needs the repo connected through the **"Google Cloud Build" GitHub
App** (this replaces storing a GitHub token — the App handles auth):

1. Console → **Cloud Build → Triggers** → set region **Global** → **Connect
   repository**.
2. Choose **GitHub (Cloud Build GitHub App)**, authenticate, install/authorize the
   App on `your-org/agentic-3rd-line-support`, and select the repo.
3. Stop before "Create trigger" — Terraform creates the triggers.

### 13b · Enable and apply

```bash
cd terraform
# in terraform.tfvars:
#   enable_cloudbuild_deploy = true
zsh -ic '
  export GOOGLE_OAUTH_ACCESS_TOKEN="$(gcloud auth print-access-token --account='"$GCP_ACCOUNT"')"
  export TF_VAR_github_token="$GH_AGENTS_AUTOMATION"
  export TF_VAR_github_webhook_secret=""
  terraform apply
'
```

This creates a dedicated build SA (`a3l-cloudbuild`), the four triggers (each
wired to that SA), and all the IAM: `roles/cloudbuild.builds.builder` +
`roles/artifactregistry.writer` (build + push), `roles/run.developer` + per-SA
`roles/iam.serviceAccountUser` (deploy + actAs each runtime SA), and
`roles/iam.serviceAccountTokenCreator` for the Cloud Build service agent on that
SA.

> [!IMPORTANT]
> An **org policy on this project forbids the default Cloud Build SA**, so the
> triggers must run as a user-managed SA — that's why `a3l-cloudbuild` exists and
> why `cloudbuild.yaml` sets `options.logging: CLOUD_LOGGING_ONLY` (a user-managed
> SA can't write to the default logs bucket). If you build the triggers by hand in
> the console instead (§13d), pick `a3l-cloudbuild` in the **Service account**
> field.

### 13c · Test it

Push a change to `main` (direct push or a merged PR) touching **one** app folder,
then watch Console → Cloud Build → **History**: exactly one trigger fires, and only
that service gets a new revision.

```bash
# e.g. a UI-only change should redeploy ONLY a3l-ticket-backend
echo "<!-- ci test -->" >> ticket-system/ui/index.html
git commit -am "test: trigger UI deploy" && git push origin main
```

> [!IMPORTANT]
> Once CD is on, **Cloud Build owns image rollouts** — do not also deploy by hand.
> Terraform's `agent-service` module `ignore_changes` on the container image keeps a
> later `terraform apply` from reverting a CI-deployed `:SHA` back to `:latest`.

> [!NOTE]
> The first build per service takes a few minutes (Rust). Builds show under Cloud
> Build → History; a failed deploy step is almost always a missing IAM grant on the
> build SA — re-check 13b.

### 13d · Building the triggers by hand in the console

`terraform apply` (13b) is the recommended path — it creates all four triggers +
the SA + IAM in one shot. If you'd rather use the **Create trigger** form, first
create the build SA + IAM once (an org policy makes the *Service account* field
mandatory):

```bash
PROJECT_ID=agentic-3rd-line-support; NUM=217601243322
SA=a3l-cloudbuild@$PROJECT_ID.iam.gserviceaccount.com
gcloud iam service-accounts create a3l-cloudbuild --display-name="Cloud Build CD" --project=$PROJECT_ID
for R in roles/cloudbuild.builds.builder roles/artifactregistry.writer roles/run.developer; do
  gcloud projects add-iam-policy-binding $PROJECT_ID --member="serviceAccount:$SA" --role="$R"
done
for RT in a3l-synthetic-shop a3l-monitoring-agent a3l-triage-agent a3l-ticket-backend; do
  gcloud iam service-accounts add-iam-policy-binding $RT@$PROJECT_ID.iam.gserviceaccount.com \
    --member="serviceAccount:$SA" --role="roles/iam.serviceAccountUser"
done
gcloud iam service-accounts add-iam-policy-binding $SA \
  --member="serviceAccount:service-$NUM@gcp-sa-cloudbuild.iam.gserviceaccount.com" \
  --role="roles/iam.serviceAccountTokenCreator"
```

Then create **one trigger per service** with these values. Fields not listed keep
the form defaults shown in the screenshots (Event = *Push to a branch*, Source =
*Cloud Build repositories*, *1st gen*, your connected repo).

**Same for every trigger:**

| Field | Value |
|-------|-------|
| Region | `global` |
| Branch | `^main$` |
| Configuration → Type | **Cloud Build configuration file (yaml or json)** |
| Configuration → Location | Repository |
| Cloud Build configuration file location | `cloudbuild.yaml` |
| Service account | `a3l-cloudbuild@…` |

**Per service** (Name, the *Included files filter* under "Show included and ignored
files filters", and the *Substitution variables* under Advanced):

| Name | Included files | `_SERVICE` | `_IMAGE` | `_CONTEXT` | `_DOCKERFILE` |
|------|----------------|-----------|---------|-----------|--------------|
| `a3l-deploy-synthetic-shop` | `apps/synthetic-shop/**` | `a3l-synthetic-shop` | `synthetic-shop` | `apps/synthetic-shop` | `apps/synthetic-shop/Dockerfile` |
| `a3l-deploy-monitoring-agent` | `agents/monitoring-agent/**`, `grounding/**` | `a3l-monitoring-agent` | `monitoring-agent` | `.` | `agents/monitoring-agent/Dockerfile` |
| `a3l-deploy-triage-agent` | `agents/triage-agent/**`, `grounding/**` | `a3l-triage-agent` | `triage-agent` | `.` | `agents/triage-agent/Dockerfile` |
| `a3l-deploy-ticket-backend` | `ticket-system/**` | `a3l-ticket-backend` | `ticket-backend` | `ticket-system` | `ticket-system/Dockerfile` |

Also add `_REGION` = `us-central1` to each trigger's substitutions. Leave "Require
approval" unchecked so pushes deploy automatically.

> [!WARNING]
> Use **either** Terraform (13b) **or** the manual form — not both, or you'll get
> duplicate triggers. If you built them by hand, keep `enable_cloudbuild_deploy =
> false` so Terraform doesn't create a second set.

### 13e · Coding-agent review gate — the agent never approves

The coding agent **opens** PRs but must **never approve or merge** them — that's the
second human gate. Two layers enforce it:

- **Code (always on):** the workflow (`.github/workflows/gemini.yml`) has no
  approve/merge step, and the prompt tells the agent to edit files only.
- **Server-enforced (once the repo is public or on GitHub Pro):** branch protection
  on `main` requiring a human **code-owner** approval. `CODEOWNERS` (`* @<you>`) is
  already committed, so the bot's review can never satisfy the requirement.

```bash
gh api -X PUT "repos/$GH_OWNER/$GH_REPO/branches/main/protection" --input - <<'JSON'
{ "required_status_checks": null, "enforce_admins": false,
  "required_pull_request_reviews": { "required_approving_review_count": 1,
    "require_code_owner_reviews": true, "dismiss_stale_reviews": true },
  "restrictions": null }
JSON
```

> [!NOTE]
> GitHub bundles "create **and** approve PRs" into one Actions toggle, and any
> PR-write token can approve — so branch protection is the only way to *guarantee*
> the agent can't approve. It's unavailable on a private Free-plan repo (403:
> "Upgrade to Pro or make public"); until then the code-level gate above holds.

---

## Appendix A — the exact commands used for the reference deployment

> [!NOTE]
> This is the **real, ordered sequence** used to stand up the reference project
> (`agentic-3rd-line-support`, project number `217601243322`), including the fixes
> made mid-flight. Steps 1–3 above are folded in here as the concrete commands that
> were run. Substitute your own values; nothing below contains a secret value.

### A.0 · Environment

```bash
export PROJECT_ID="agentic-3rd-line-support"
export REGION="us-central1"
export GCP_ACCOUNT="you@example.com"     # the account that owns the project
export GH_OWNER="your-github-username"
export GH_REPO="agentic-3rd-line-support"
export REPO="${REGION}-docker.pkg.dev/${PROJECT_ID}/a3l-images"   # Artifact Registry
```

> [!TIP]
> The reference machine keeps **more than one `gcloud` account/config active at
> once**, so *every* command below pins `--account="$GCP_ACCOUNT"` (and usually
> `--project="$PROJECT_ID"`) rather than relying on the active config. Do the same
> if you juggle accounts — it prevents commands silently hitting the wrong project.

### A.1 · Enable the APIs

The exact command from [step 4](#4-enable-the-gcp-apis), run once against the
project (all 16 services). Terraform re-declares them, so this just makes the first
apply faster.

### A.2 · Pre-provision the Monitoring service agent

The first `terraform apply` failed on the `logging.tf` Pub/Sub publisher grant
because the Monitoring **service agent** didn't exist yet. Force it into existence
first:

```bash
gcloud beta services identity create \
  --service=monitoring.googleapis.com \
  --project="$PROJECT_ID" --account="$GCP_ACCOUNT"
```

### A.3 · First `terraform apply` (infrastructure)

```bash
cd terraform
cp terraform.tfvars.example terraform.tfvars
# edit: project_id, region, github_owner, github_repo (gemini_* keep their v3 defaults)

terraform init
terraform apply        # reviewed the plan, then approved
```

Three fixes were made **during** this first apply and are now baked into the
config — you get them for free, but this is what they were:

1. **Billing budget 403 → provider quota project.** The `google_billing_budget`
   call was rejected under user credentials until
   `user_project_override = true` + `billing_project = var.project_id` were added to
   both providers (`providers.tf`).
2. **Budget dropped for the reference run.** Even with the quota project, creating
   the budget needs billing-account-level permission, so the reference deployment
   runs with `billing_account = ""` (the budget resource is `count`-gated off).
3. **Alert policy `notification_rate_limit` removed.** That field is only valid on
   *log-based* alert policies; on the metric-threshold policy in `logging.tf` it
   failed. The policy now sets only `auto_close`.

> [!IMPORTANT]
> Firestore is created by this apply (`firestore.tf`, Native mode, `us-central1`).
> There is exactly one `(default)` database per project.

### A.4 · Build the images (Cloud Build)

Every image is **distroless** (Rust → `gcr.io/distroless/cc-debian12:nonroot`;
Python → `gcr.io/distroless/python3-debian12` built from a `python:3.11-slim`
stage; the console is a 3-stage build: `trunk` compiles the Leptos/WASM UI → cargo
builds the axum backend → distroless runtime). Build each into Artifact Registry:

```bash
gcloud builds submit apps/synthetic-shop      --tag "$REPO/synthetic-shop:latest"  --account="$GCP_ACCOUNT" --project="$PROJECT_ID"
gcloud builds submit agents/monitoring-agent  --tag "$REPO/monitoring-agent:latest" --account="$GCP_ACCOUNT" --project="$PROJECT_ID"
gcloud builds submit agents/triage-agent      --tag "$REPO/triage-agent:latest"    --account="$GCP_ACCOUNT" --project="$PROJECT_ID"
gcloud builds submit ticket-system            --tag "$REPO/ticket-backend:latest"  --account="$GCP_ACCOUNT" --project="$PROJECT_ID"
```

> [!NOTE]
> The Python agent Dockerfiles use the **repo root** as build context (they import
> shared code), so those two `builds submit` commands are pointed at the agent
> directory but the Dockerfile copies from the wider context as needed — check each
> `Dockerfile` if you fork the layout.

### A.5 · Point Terraform at the real images and re-apply

Set the four `*_image` variables to the `:latest` tags (in `terraform.tfvars`),
then apply. This apply — and every later one — bridges credentials for a single
command so it uses `$GCP_ACCOUNT`'s token without touching your ADC, and injects
the GitHub token from the shell (its value is never printed or written to disk):

```bash
zsh -ic '
  export GOOGLE_OAUTH_ACCESS_TOKEN="$(gcloud auth print-access-token --account='"$GCP_ACCOUNT"')"
  export TF_VAR_github_token="$GH_AGENTS_AUTOMATION"     # your token, from shell env
  export TF_VAR_github_webhook_secret=""                # empty until you enable the webhook
  terraform apply
'
```

> [!WARNING]
> **`:latest` is a moving tag, so Terraform won't redeploy on a rebuild.** If you
> `gcloud builds submit` a new `:latest` but the `*_image` string is unchanged,
> `terraform apply` sees **no diff** and keeps the old revision. Force a new
> revision explicitly:
> ```bash
> gcloud run deploy a3l-ticket-backend --image "$REPO/ticket-backend:latest" \
>   --region="$REGION" --account="$GCP_ACCOUNT" --project="$PROJECT_ID"
> ```
> (or change any env var to make Terraform roll a revision).

### A.6 · Post-apply wiring

Run [step 8](#8-post-apply-wiring) verbatim — push the WIF outputs into the repo as
`GCP_*` secrets and create the `agent-bug` label. (The reference maintainer set the
repo secrets by hand in the GitHub UI; the CLI form in step 8 is equivalent.)

### A.7 · Verify end to end

```bash
BE="$(cd terraform && terraform output -raw ticket_ui_url)"
SHOP="$(cd terraform && terraform output -raw synthetic_shop_url)"

curl "$SHOP/health"                                       # NOT /healthz — see Appendix B
curl -X POST "$SHOP/simulate" -H 'content-type: application/json' \
  -d '{"scenario":"db_pool_exhaustion","count":10}'

# Watch the loop land in the console's API:
curl -s "$BE/api/tickets"      # a rich ticket appears (description, Gherkin, etc.)
curl -s "$BE/api/events"       # ledger: ticketed / duplicate_closed / ignored
curl -s "$BE/api/known-issues" # dedup registry
curl -s "$BE/api/health"       # heartbeats: {component, last_seen} per service
curl -s "$BE/api/ops"          # Cloud Run status for all four services
```

### A.8 · Console upgrade cycle (rich tickets, redesigned UI, Simulate, Ops)

When the console features were added, the loop was: edit code → verify locally →
rebuild the affected images → `terraform apply` (for IAM/env changes) → force-redeploy:

```bash
# local gates before shipping
cargo fmt -p ticket-backend -- --check && cargo clippy -p ticket-backend -- -D warnings
cargo check -p ticket-backend

# rebuild + force new revisions (because the tag is :latest)
gcloud builds submit ticket-system --tag "$REPO/ticket-backend:latest" --account="$GCP_ACCOUNT" --project="$PROJECT_ID"
gcloud run deploy a3l-ticket-backend --image "$REPO/ticket-backend:latest" --region="$REGION" --account="$GCP_ACCOUNT" --project="$PROJECT_ID"
```

### A.9 · The scale-button fix (as executed this iteration)

The Ops "scale" button returned **403**. The diagnosis and fix, in order:

```bash
# 1. Confirm the request shape is fine by running it as an owner — it succeeds:
TOK="$(gcloud auth print-access-token --account=$GCP_ACCOUNT)"
curl -s -X PATCH \
  "https://run.googleapis.com/v2/projects/$PROJECT_ID/locations/$REGION/services/a3l-monitoring-agent?updateMask=template" \
  -H "Authorization: Bearer $TOK" -H "content-type: application/json" \
  -d @patched-service.json -w '%{http_code}\n'          # -> 200 as owner

# 2. Make the backend surface the GCP error body (stop swallowing it in ops.rs),
#    rebuild + redeploy, then hit the endpoint — the REAL error is:
#      "Permission 'artifactregistry.repositories.downloadArtifacts' denied
#       on .../repositories/a3l-images"
```

Root cause: creating a revision makes Cloud Run preflight-verify the **caller** can
pull the image; the console SA had no Artifact Registry read. The fix is the
repo-scoped `roles/artifactregistry.reader` grant now in `iam.tf`
(`ticket_ar_reader`), applied and verified:

```bash
# apply the IAM fix (creds bridged as in A.5), then:
BE="$(cd terraform && terraform output -raw ticket_ui_url)"
curl -s -X POST "$BE/api/ops/scale" -H 'content-type: application/json' \
  -d '{"service":"a3l-monitoring-agent","min_instances":0}' -w '  %{http_code}\n'   # -> 200
```

See **[Terraform, explained → the scale-button saga](terraform.md#iamtf--least-privilege-and-the-scale-button-saga)**
for the full reasoning (and why `run.developer` + `serviceAccountUser` alone were
not enough).

### A.10 · Teardown

```bash
cd terraform
zsh -ic 'export GOOGLE_OAUTH_ACCESS_TOKEN="$(gcloud auth print-access-token --account='"$GCP_ACCOUNT"')"; terraform destroy'
```

---

## Appendix B — gotchas encountered, and how they were fixed

Every one of these cost real time on the reference build. They're fixed in the
committed config/code; this table is so you recognise them if they resurface.

| # | Symptom | Root cause | Fix (now in the repo) |
|---|---------|-----------|-----------------------|
| 1 | Budget resource **403** on first apply | user creds need a quota project for the Budgets API | `user_project_override = true` + `billing_project` in `providers.tf` |
| 2 | Budget still fails | creating a budget needs billing-account-level permission | run with `billing_account = ""` (budget `count`-gated off) |
| 3 | Apply fails: Monitoring **service agent "does not exist"** | the service agent isn't created until the API is first used | `gcloud beta services identity create --service=monitoring.googleapis.com` (Appendix A.2) |
| 4 | Alert policy rejected | `notification_rate_limit` is valid only on *log-based* policies | removed it; the metric policy sets only `auto_close` (`logging.tf`) |
| 5 | `GET /healthz` returns a **Google 404** | Cloud Run's front end reserves the exact path `/healthz` | every service exposes **`/health`** instead |
| 6 | Rebuilt image, but nothing redeploys | `*_image` is a moving `:latest` tag → no Terraform diff | `gcloud run deploy --image …:latest` to force a revision |
| 7 | Agents return an **empty candidate** on turn 0 | tool calls weren't read from the right place + "thinking" on | read `candidate.content.parts[].function_call`; set `ThinkingConfig(thinking_budget=0)`; **Gemini v3+ only** |
| 8 | Ops **scale 403**, misattributed to `run`/`actAs` | revision preflight needs the caller to pull the image | repo-scoped `roles/artifactregistry.reader` for the console SA (`iam.tf`) |
| 9 | Diagnosing #8 was hard | the backend swallowed the GCP error via `.error_for_status()` | `ops.rs` now surfaces the response body on a non-2xx |
| 10 | Can't reproduce-as-the-SA during diagnosis | `iamcredentials` `generateAccessToken` 404s on this consumer project; SA impersonation also blocked | diagnose against the live endpoint instead; don't rely on impersonation here |

> [!NOTE]
> Items 8–10 are from the most recent iteration; the fix is least-privilege
> (per-SA `actAs` + repo-scoped Artifact Registry read), documented in
> [Terraform, explained](terraform.md#iamtf--least-privilege-and-the-scale-button-saga).
