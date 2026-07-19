variable "project_id" {
  type        = string
  description = "GCP project ID (e.g. agentic-3rd-line-support)."
}

variable "region" {
  type        = string
  description = "Primary region for Cloud Run, Pub/Sub, Scheduler, etc."
  default     = "us-central1"
}

variable "gemini_location" {
  type        = string
  description = "Vertex AI location for Gemini (e.g. \"global\" or a region like us-central1)."
  default     = "global"
}

variable "gemini_model_monitoring" {
  type        = string
  description = "Gemini model for the monitoring agent (frequent, cheap sweeps). Must be v3+."
  default     = "gemini-3.1-flash-lite"
}

variable "gemini_model_triage" {
  type        = string
  description = "Gemini model for the triage agent (grounded reasoning). Must be v3+."
  default     = "gemini-3.5-flash"
}

variable "name_prefix" {
  type        = string
  description = "Short prefix for resource names (<= 6 chars keeps SA account_ids in range)."
  default     = "a3l"
}

variable "labels" {
  type        = map(string)
  description = "Extra labels merged onto all resources."
  default     = {}
}

# ---- Container images (CI overrides these; defaults let `plan` run early) ----
variable "synthetic_shop_image" {
  type    = string
  default = "us-docker.pkg.dev/cloudrun/container/hello"
}
variable "monitoring_agent_image" {
  type    = string
  default = "us-docker.pkg.dev/cloudrun/container/hello"
}
variable "triage_agent_image" {
  type    = string
  default = "us-docker.pkg.dev/cloudrun/container/hello"
}
variable "ticket_backend_image" {
  type    = string
  default = "us-docker.pkg.dev/cloudrun/container/hello"
}

# ---- Synthetic app behavior ----
variable "synthetic_min_instances" {
  type        = number
  description = "Keep >= 1 so the app can continuously flood logs. Set 0 (+ a scheduler ping) to cut cost."
  default     = 1
}

variable "synthetic_allow_unauthenticated" {
  type        = bool
  description = "Allow public access so you can curl /simulate during the demo."
  default     = true
}

variable "ticket_ui_allow_unauthenticated" {
  type        = bool
  description = "Allow public access to the ticket review UI (demo convenience; use IAP for anything real)."
  default     = true
}

# ---- GitHub ----
variable "github_owner" {
  type        = string
  description = "GitHub org/user that owns the repo (for WIF + coding agent + optional Cloud Build)."
}

variable "github_repo" {
  type        = string
  description = "Repository name (without owner)."
}

variable "github_token" {
  type        = string
  description = "GitHub token used by the ticket backend to create Issues. Prefer setting via env: TF_VAR_github_token. Leave empty to create the secret container without a value and add the version out-of-band."
  default     = ""
  sensitive   = true
}

variable "github_webhook_secret" {
  type        = string
  description = "HMAC secret for GitHub webhook verification (PR/issue feedback loop). Prefer TF_VAR_github_webhook_secret. Empty = webhook runs unsecured (demo only)."
  default     = ""
  sensitive   = true
}

# ---- Continuous deployment (Cloud Build) ----
variable "enable_cloudbuild_deploy" {
  type        = bool
  description = "Create the per-service, path-filtered Cloud Build triggers that build + deploy on push to main. Requires connecting the repo to Cloud Build via the GitHub App first (one-time, in the console)."
  default     = false
}

# ---- Cost guard ----
variable "billing_account" {
  type        = string
  description = "Billing account id (XXXXXX-XXXXXX-XXXXXX) for the budget alert. Empty = skip budget."
  default     = ""
}

variable "budget_amount_usd" {
  type        = number
  description = "Monthly budget threshold in USD for the alert."
  default     = 50
}
