output "synthetic_shop_url" {
  description = "Hit POST {url}/simulate to inject demo scenarios."
  value       = module.synthetic_shop.uri
}

output "monitoring_agent_url" {
  value = module.monitoring_agent.uri
}

output "triage_agent_url" {
  value = module.triage_agent.uri
}

output "ticket_ui_url" {
  description = "The Rust/WASM ticket review UI."
  value       = module.ticket_backend.uri
}

output "topics" {
  value = {
    log_alerts   = google_pubsub_topic.log_alerts.name
    findings     = google_pubsub_topic.findings.name
    tickets      = google_pubsub_topic.tickets.name
    findings_dlq = google_pubsub_topic.findings_dlq.name
    tickets_dlq  = google_pubsub_topic.tickets_dlq.name
  }
}

output "artifact_registry" {
  description = "Docker repo to push images to."
  value       = "${var.region}-docker.pkg.dev/${var.project_id}/${google_artifact_registry_repository.images.repository_id}"
}

output "github_actions_service_account" {
  description = "Set as the GCP_SERVICE_ACCOUNT GitHub secret for the coding agent."
  value       = google_service_account.github_actions.email
}

output "github_workload_identity_provider" {
  description = "Set as the GCP_WORKLOAD_IDENTITY_PROVIDER GitHub secret for the coding agent."
  value       = google_iam_workload_identity_pool_provider.github.name
}
