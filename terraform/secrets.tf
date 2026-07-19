# Secret Manager holds the ONLY application secret in the cloud path: the GitHub
# token the ticket backend uses to open Issues. (Model access is via Vertex + a
# service account, so there is no model API key to store.)
#
# This file creates the secret *container* and IAM. The secret VALUE is supplied
# out-of-band — preferably `export TF_VAR_github_token=...` before apply, or added
# later with `gcloud secrets versions add`. Never commit the value.

resource "google_secret_manager_secret" "github_token" {
  project   = var.project_id
  secret_id = "${local.prefix}-github-token"

  replication {
    auto {}
  }

  labels = local.labels
}

# Only create a version if a value was provided (keeps a bare `apply` clean).
resource "google_secret_manager_secret_version" "github_token" {
  count       = var.github_token != "" ? 1 : 0
  secret      = google_secret_manager_secret.github_token.id
  secret_data = var.github_token
}

# The ticket backend may read the token.
resource "google_secret_manager_secret_iam_member" "ticket_backend_access" {
  project   = var.project_id
  secret_id = google_secret_manager_secret.github_token.secret_id
  role      = "roles/secretmanager.secretAccessor"
  member    = module.ticket_backend.service_account_member
}

# Optional GitHub webhook HMAC secret (PR/issue feedback loop) — same handling.
resource "google_secret_manager_secret" "github_webhook_secret" {
  project   = var.project_id
  secret_id = "${local.prefix}-github-webhook-secret"

  replication {
    auto {}
  }

  labels = local.labels
}

resource "google_secret_manager_secret_version" "github_webhook_secret" {
  count       = var.github_webhook_secret != "" ? 1 : 0
  secret      = google_secret_manager_secret.github_webhook_secret.id
  secret_data = var.github_webhook_secret
}

resource "google_secret_manager_secret_iam_member" "ticket_backend_webhook_access" {
  project   = var.project_id
  secret_id = google_secret_manager_secret.github_webhook_secret.secret_id
  role      = "roles/secretmanager.secretAccessor"
  member    = module.ticket_backend.service_account_member
}
