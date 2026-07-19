output "uri" {
  value       = google_cloud_run_v2_service.this.uri
  description = "Public URL of the Cloud Run service."
}

output "name" {
  value = google_cloud_run_v2_service.this.name
}

output "service_account_email" {
  value       = google_service_account.this.email
  description = "Email of the dedicated service account."
}

output "service_account_member" {
  value       = "serviceAccount:${google_service_account.this.email}"
  description = "IAM member string for the service account."
}
