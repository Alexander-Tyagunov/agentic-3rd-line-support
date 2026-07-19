# The proactive sweep: Cloud Scheduler pokes the monitoring agent on an interval
# so it looks for EMERGENT patterns (not just alert-driven known ones).

resource "google_cloud_scheduler_job" "monitoring_sweep" {
  project          = var.project_id
  region           = var.region
  name             = "${local.prefix}-monitoring-sweep"
  description      = "Periodic log sweep for emergent risky patterns"
  schedule         = "*/10 * * * *"
  time_zone        = "Etc/UTC"
  attempt_deadline = "320s"

  http_target {
    http_method = "POST"
    uri         = "${module.monitoring_agent.uri}/sweep"

    oidc_token {
      service_account_email = google_service_account.scheduler.email
      audience              = module.monitoring_agent.uri
    }
  }

  depends_on = [google_project_service.enabled]
}
