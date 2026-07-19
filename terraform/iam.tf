# Service accounts that aren't owned by a single Cloud Run service, plus the
# project-level role grants for the per-service SAs (which the module creates).
# Principle: one SA per workload, each with the narrowest useful role set.

# OIDC identity used by Pub/Sub push subscriptions to invoke Cloud Run.
resource "google_service_account" "pubsub_push" {
  project      = var.project_id
  account_id   = "${local.prefix}-pubsub-push"
  display_name = "Pub/Sub push OIDC identity"
}

# Identity Cloud Scheduler uses to invoke the monitoring agent's sweep endpoint.
resource "google_service_account" "scheduler" {
  project      = var.project_id
  account_id   = "${local.prefix}-scheduler"
  display_name = "Cloud Scheduler invoker"
}

# --- Synthetic app: only needs to write logs ---
resource "google_project_iam_member" "synthetic_log_writer" {
  project = var.project_id
  role    = "roles/logging.logWriter"
  member  = module.synthetic_shop.service_account_member
}

# --- Monitoring agent: read logs + call Vertex (publish rights are topic-scoped, see pubsub.tf) ---
resource "google_project_iam_member" "monitoring_log_viewer" {
  project = var.project_id
  role    = "roles/logging.viewer"
  member  = module.monitoring_agent.service_account_member
}
resource "google_project_iam_member" "monitoring_vertex" {
  project = var.project_id
  role    = "roles/aiplatform.user"
  member  = module.monitoring_agent.service_account_member
}

# Health heartbeats (its own + synthetic-shop liveness) go to Firestore.
resource "google_project_iam_member" "monitoring_firestore" {
  project = var.project_id
  role    = "roles/datastore.user"
  member  = module.monitoring_agent.service_account_member
}

# --- Triage agent: Vertex + Firestore (dedup index); publish rights topic-scoped ---
resource "google_project_iam_member" "triage_vertex" {
  project = var.project_id
  role    = "roles/aiplatform.user"
  member  = module.triage_agent.service_account_member
}
resource "google_project_iam_member" "triage_firestore" {
  project = var.project_id
  role    = "roles/datastore.user"
  member  = module.triage_agent.service_account_member
}

# --- Ticket backend: Firestore (ticket store) ---
resource "google_project_iam_member" "ticket_firestore" {
  project = var.project_id
  role    = "roles/datastore.user"
  member  = module.ticket_backend.service_account_member
}

# Console admin/ops controls (reset queues, scale services from the UI).
# Broad for an educational demo — scope down for anything real.
resource "google_project_iam_member" "ticket_pubsub_editor" {
  project = var.project_id
  role    = "roles/pubsub.editor" # seek/purge subscriptions
  member  = module.ticket_backend.service_account_member
}
resource "google_project_iam_member" "ticket_run_developer" {
  project = var.project_id
  role    = "roles/run.developer" # read + scale Cloud Run services
  member  = module.ticket_backend.service_account_member
}

# Scaling a Cloud Run service creates a new revision, which requires the caller
# to actAs that revision's runtime SA. Granted per-SA (least privilege) rather
# than project-wide, on exactly the runtime SAs the console can scale.
resource "google_service_account_iam_member" "ticket_actas" {
  for_each = {
    shop       = module.synthetic_shop.service_account_email
    monitoring = module.monitoring_agent.service_account_email
    triage     = module.triage_agent.service_account_email
    backend    = module.ticket_backend.service_account_email
  }
  service_account_id = "projects/${var.project_id}/serviceAccounts/${each.value}"
  role               = "roles/iam.serviceAccountUser"
  member             = module.ticket_backend.service_account_member
}

# Creating a revision makes Cloud Run preflight-verify the CALLER can pull the
# revision's image, so the console SA needs read access to the image repo.
# (This — not actAs — is what returns 403 on scale; reads skip the check.)
resource "google_artifact_registry_repository_iam_member" "ticket_ar_reader" {
  project    = var.project_id
  location   = google_artifact_registry_repository.images.location
  repository = google_artifact_registry_repository.images.repository_id
  role       = "roles/artifactregistry.reader"
  member     = module.ticket_backend.service_account_member
}

# The console invokes the monitoring agent's /sweep for its "Run sweep now"
# button (OIDC), so its SA needs run.invoker on that service. (Standalone member
# rather than in the module's invokers list, to avoid a dependency cycle:
# ticket_backend already consumes monitoring_agent.uri.)
resource "google_cloud_run_v2_service_iam_member" "backend_invokes_monitoring" {
  project  = var.project_id
  location = var.region
  name     = module.monitoring_agent.name
  role     = "roles/run.invoker"
  member   = module.ticket_backend.service_account_member
}

# Pub/Sub's service agent must be able to mint OIDC tokens for authenticated push.
resource "google_project_iam_member" "pubsub_token_creator" {
  project = var.project_id
  role    = "roles/iam.serviceAccountTokenCreator"
  member  = local.pubsub_agent
}
