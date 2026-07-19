# Firestore (Native mode) backs the triage dedup index and the ticket store.
# One default database per project. Deletion policy is DELETE so `terraform
# destroy` can remove it (educational/ephemeral).

resource "google_firestore_database" "default" {
  project     = var.project_id
  name        = "(default)"
  location_id = var.region # must be a valid Firestore location (region or multi-region like "nam5")
  type        = "FIRESTORE_NATIVE"

  delete_protection_state = "DELETE_PROTECTION_DISABLED"
  deletion_policy         = "DELETE"

  depends_on = [google_project_service.enabled]
}
