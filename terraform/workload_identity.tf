# Workload Identity Federation so the Gemini CLI GitHub Action can authenticate
# to Vertex AI WITHOUT a downloadable key. The GitHub OIDC token is exchanged for
# short-lived credentials to impersonate a dedicated, minimally-scoped SA.

resource "google_iam_workload_identity_pool" "github" {
  project                   = var.project_id
  workload_identity_pool_id = "${local.prefix}-github-pool"
  display_name              = "GitHub Actions pool"
  description               = "OIDC federation for the coding agent"
}

resource "google_iam_workload_identity_pool_provider" "github" {
  project                            = var.project_id
  workload_identity_pool_id          = google_iam_workload_identity_pool.github.workload_identity_pool_id
  workload_identity_pool_provider_id = "github-oidc"
  display_name                       = "GitHub OIDC"

  attribute_mapping = {
    "google.subject"             = "assertion.sub"
    "attribute.repository"       = "assertion.repository"
    "attribute.repository_owner" = "assertion.repository_owner"
  }

  # Only tokens from this repo owner may use the pool.
  attribute_condition = "assertion.repository_owner == '${var.github_owner}'"

  oidc {
    issuer_uri = "https://token.actions.githubusercontent.com"
  }
}

# The SA the Action impersonates — Vertex AI User only.
resource "google_service_account" "github_actions" {
  project      = var.project_id
  account_id   = "${local.prefix}-gh-actions"
  display_name = "GitHub Actions (coding agent) — Vertex user"
}

resource "google_project_iam_member" "github_actions_vertex" {
  project = var.project_id
  role    = "roles/aiplatform.user"
  member  = "serviceAccount:${google_service_account.github_actions.email}"
}

# Allow the repo's federated identity to impersonate the SA.
resource "google_service_account_iam_member" "github_actions_wif" {
  service_account_id = google_service_account.github_actions.name
  role               = "roles/iam.workloadIdentityUser"
  member             = "principalSet://iam.googleapis.com/${google_iam_workload_identity_pool.github.name}/attribute.repository/${var.github_owner}/${var.github_repo}"
}
