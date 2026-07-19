provider "google" {
  project = var.project_id
  region  = var.region
  # User credentials need a quota project for some APIs (e.g. billing budgets).
  user_project_override = true
  billing_project       = var.project_id
}

provider "google-beta" {
  project               = var.project_id
  region                = var.region
  user_project_override = true
  billing_project       = var.project_id
}

data "google_project" "this" {
  project_id = var.project_id
}

locals {
  prefix         = var.name_prefix
  project_number = data.google_project.this.number

  labels = merge(
    {
      project    = "agentic-3rd-line-support"
      managed_by = "terraform"
    },
    var.labels,
  )

  # Google-managed service agents we must grant narrow permissions to.
  pubsub_agent     = "serviceAccount:service-${data.google_project.this.number}@gcp-sa-pubsub.iam.gserviceaccount.com"
  monitoring_agent = "serviceAccount:service-${data.google_project.this.number}@gcp-sa-monitoring.iam.gserviceaccount.com"
}
