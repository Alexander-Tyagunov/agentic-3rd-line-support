# Reusable "Cloud Run service + dedicated least-privilege SA + invoker IAM".
# Used for the synthetic app, both agents, and the ticket backend.

resource "google_service_account" "this" {
  project      = var.project_id
  account_id   = var.service_account_id
  display_name = "SA for Cloud Run service ${var.name}"
}

resource "google_cloud_run_v2_service" "this" {
  name     = var.name
  project  = var.project_id
  location = var.location
  ingress  = var.ingress

  # Educational, ephemeral deployment — allow `terraform destroy` to remove it.
  deletion_protection = false

  template {
    service_account = google_service_account.this.email

    scaling {
      min_instance_count = var.min_instances
      max_instance_count = var.max_instances
    }

    containers {
      image = var.image

      resources {
        limits = {
          cpu    = var.cpu
          memory = var.memory
        }
        cpu_idle = var.cpu_idle
      }

      dynamic "env" {
        for_each = var.env
        content {
          name  = env.key
          value = env.value
        }
      }

      dynamic "env" {
        for_each = var.secret_env
        content {
          name = env.value.name
          value_source {
            secret_key_ref {
              secret  = env.value.secret
              version = env.value.version
            }
          }
        }
      }
    }
  }

  labels = var.labels

  lifecycle {
    ignore_changes = [
      # Cloud Build (CD) owns image rollouts: it deploys immutable :SHA images.
      # Don't let terraform revert to the var.image (:latest) placeholder, and
      # don't churn on the client metadata `gcloud run deploy` stamps per revision.
      template[0].containers[0].image,
      client,
      client_version,
    ]
  }
}

resource "google_cloud_run_v2_service_iam_member" "invokers" {
  for_each = toset(var.invokers)

  project  = var.project_id
  location = var.location
  name     = google_cloud_run_v2_service.this.name
  role     = "roles/run.invoker"
  member   = each.value
}
