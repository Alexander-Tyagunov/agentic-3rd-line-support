# Continuous deployment: one Cloud Build trigger per service, each PATH-FILTERED
# so a merge to main only rebuilds + redeploys the app whose folder changed.
# Editing the UI never redeploys the agents, and vice versa.
#
# Gated behind enable_cloudbuild_deploy because it needs a ONE-TIME manual step:
# connect the repo to Cloud Build via the "Google Cloud Build" GitHub App
# (Console -> Cloud Build -> Triggers -> Connect repository). See docs/setup.md §13.
#
# NOTE: an org policy on this project forbids the default Cloud Build SA, so builds
# run as a dedicated user-managed SA created below (and cloudbuild.yaml uses
# CLOUD_LOGGING_ONLY, which a user-managed SA requires).

locals {
  # service key => how to build + deploy it, and which paths should trigger it.
  # `context`/`dockerfile` are relative to the repo root (the build workspace).
  ci_services = {
    synthetic-shop = {
      service    = module.synthetic_shop.name
      image      = "synthetic-shop"
      context    = "apps/synthetic-shop"
      dockerfile = "apps/synthetic-shop/Dockerfile"
      paths      = ["apps/synthetic-shop/**"]
    }
    monitoring-agent = {
      service    = module.monitoring_agent.name
      image      = "monitoring-agent"
      context    = "." # bundles grounding/, so context is the repo root
      dockerfile = "agents/monitoring-agent/Dockerfile"
      paths      = ["agents/monitoring-agent/**", "grounding/**"]
    }
    triage-agent = {
      service    = module.triage_agent.name
      image      = "triage-agent"
      context    = "."
      dockerfile = "agents/triage-agent/Dockerfile"
      paths      = ["agents/triage-agent/**", "grounding/**"]
    }
    ticket-backend = {
      service    = module.ticket_backend.name
      image      = "ticket-backend"
      context    = "ticket-system" # backend + WASM UI + shared crate are one image
      dockerfile = "ticket-system/Dockerfile"
      paths      = ["ticket-system/**"]
    }
  }

  # The four runtime SAs the build SA must be able to actAs when deploying.
  ci_runtime_sas = {
    shop       = module.synthetic_shop.service_account_email
    monitoring = module.monitoring_agent.service_account_email
    triage     = module.triage_agent.service_account_email
    backend    = module.ticket_backend.service_account_email
  }
}

# Dedicated build+deploy identity (org policy requires a user-managed SA).
resource "google_service_account" "cloudbuild" {
  count        = var.enable_cloudbuild_deploy ? 1 : 0
  project      = var.project_id
  account_id   = "${local.prefix}-cloudbuild"
  display_name = "Cloud Build CD — build + deploy"
}

resource "google_cloudbuild_trigger" "deploy" {
  for_each = var.enable_cloudbuild_deploy ? local.ci_services : {}

  project     = var.project_id
  name        = "${local.prefix}-deploy-${each.key}"
  description = "Build + deploy ${each.value.service} when files under its path change on main"

  github {
    owner = var.github_owner
    name  = var.github_repo
    push {
      branch = "^main$"
    }
  }

  # Only fire when files under these globs changed in the push.
  included_files = each.value.paths

  filename        = "cloudbuild.yaml"
  service_account = google_service_account.cloudbuild[0].id

  substitutions = {
    _SERVICE    = each.value.service
    _IMAGE      = each.value.image
    _CONTEXT    = each.value.context
    _DOCKERFILE = each.value.dockerfile
    _REGION     = var.region
  }

  depends_on = [google_project_service.enabled]
}

# --- Permissions for the dedicated build SA ---

# Run builds (source, logs, artifact push) and push images explicitly.
resource "google_project_iam_member" "cloudbuild_builder" {
  count   = var.enable_cloudbuild_deploy ? 1 : 0
  project = var.project_id
  role    = "roles/cloudbuild.builds.builder"
  member  = "serviceAccount:${google_service_account.cloudbuild[0].email}"
}
resource "google_project_iam_member" "cloudbuild_ar_writer" {
  count   = var.enable_cloudbuild_deploy ? 1 : 0
  project = var.project_id
  role    = "roles/artifactregistry.writer" # push (includes pull for the deploy preflight)
  member  = "serviceAccount:${google_service_account.cloudbuild[0].email}"
}

# Deploy / update Cloud Run services.
resource "google_project_iam_member" "cloudbuild_run_developer" {
  count   = var.enable_cloudbuild_deploy ? 1 : 0
  project = var.project_id
  role    = "roles/run.developer"
  member  = "serviceAccount:${google_service_account.cloudbuild[0].email}"
}

# Deploying a service creates a revision that runs as that service's runtime SA,
# which requires the deployer to actAs it — granted per-SA (least privilege).
resource "google_service_account_iam_member" "cloudbuild_actas_runtime" {
  for_each = var.enable_cloudbuild_deploy ? local.ci_runtime_sas : {}

  service_account_id = "projects/${var.project_id}/serviceAccounts/${each.value}"
  role               = "roles/iam.serviceAccountUser"
  member             = "serviceAccount:${google_service_account.cloudbuild[0].email}"
}

# The Cloud Build service agent must be able to mint tokens for the user-managed
# build SA (required whenever a trigger runs as a user-specified SA).
resource "google_service_account_iam_member" "cloudbuild_agent_token_creator" {
  count              = var.enable_cloudbuild_deploy ? 1 : 0
  service_account_id = google_service_account.cloudbuild[0].name
  role               = "roles/iam.serviceAccountTokenCreator"
  member             = "serviceAccount:service-${local.project_number}@gcp-sa-cloudbuild.iam.gserviceaccount.com"
}
