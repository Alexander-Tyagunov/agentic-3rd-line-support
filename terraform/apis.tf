# Enable every API the stack needs. disable_on_destroy = false so a destroy
# doesn't try to turn APIs off (which can fail if other resources still use them).

locals {
  services = [
    "run.googleapis.com",
    "cloudbuild.googleapis.com",
    "pubsub.googleapis.com",
    "logging.googleapis.com",
    "monitoring.googleapis.com",
    "secretmanager.googleapis.com",
    "artifactregistry.googleapis.com",
    "firestore.googleapis.com",
    "cloudscheduler.googleapis.com",
    "aiplatform.googleapis.com", # Vertex AI (Gemini)
    "iam.googleapis.com",
    "iamcredentials.googleapis.com", # Workload Identity Federation
    "sts.googleapis.com",            # Security Token Service (WIF)
    "cloudresourcemanager.googleapis.com",
    "cloudbilling.googleapis.com", # budget alert
    "billingbudgets.googleapis.com",
  ]
}

resource "google_project_service" "enabled" {
  for_each = toset(local.services)

  project            = var.project_id
  service            = each.value
  disable_on_destroy = false
}
