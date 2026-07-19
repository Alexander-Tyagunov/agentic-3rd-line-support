# The message backbone: three primary topics + dead-letter topics, each with an
# authenticated (OIDC) push subscription into the owning Cloud Run service.

# ---- Topics ----
resource "google_pubsub_topic" "log_alerts" {
  project = var.project_id
  name    = "${local.prefix}-log-alerts"
  labels  = local.labels
}

resource "google_pubsub_topic" "findings" {
  project = var.project_id
  name    = "${local.prefix}-findings"
  labels  = local.labels
}

resource "google_pubsub_topic" "tickets" {
  project = var.project_id
  name    = "${local.prefix}-tickets"
  labels  = local.labels
}

resource "google_pubsub_topic" "findings_dlq" {
  project = var.project_id
  name    = "${local.prefix}-findings-dlq"
  labels  = local.labels
}

resource "google_pubsub_topic" "tickets_dlq" {
  project = var.project_id
  name    = "${local.prefix}-tickets-dlq"
  labels  = local.labels
}

# ---- Publisher grants (topic-scoped least privilege) ----
resource "google_pubsub_topic_iam_member" "monitoring_publishes_findings" {
  project = var.project_id
  topic   = google_pubsub_topic.findings.name
  role    = "roles/pubsub.publisher"
  member  = module.monitoring_agent.service_account_member
}

resource "google_pubsub_topic_iam_member" "triage_publishes_tickets" {
  project = var.project_id
  topic   = google_pubsub_topic.tickets.name
  role    = "roles/pubsub.publisher"
  member  = module.triage_agent.service_account_member
}

# ---- Push subscriptions ----
# log-alerts -> monitoring agent (enrich a known alert with context)
resource "google_pubsub_subscription" "log_alerts_push" {
  project = var.project_id
  name    = "${local.prefix}-log-alerts-sub"
  topic   = google_pubsub_topic.log_alerts.id

  ack_deadline_seconds = 60

  push_config {
    push_endpoint = "${module.monitoring_agent.uri}/pubsub/alerts"
    oidc_token {
      service_account_email = google_service_account.pubsub_push.email
      audience              = module.monitoring_agent.uri
    }
  }

  retry_policy {
    minimum_backoff = "10s"
    maximum_backoff = "600s"
  }
}

# findings -> triage agent
resource "google_pubsub_subscription" "findings_push" {
  project = var.project_id
  name    = "${local.prefix}-findings-sub"
  topic   = google_pubsub_topic.findings.id

  ack_deadline_seconds = 120

  push_config {
    push_endpoint = "${module.triage_agent.uri}/pubsub/findings"
    oidc_token {
      service_account_email = google_service_account.pubsub_push.email
      audience              = module.triage_agent.uri
    }
  }

  dead_letter_policy {
    dead_letter_topic     = google_pubsub_topic.findings_dlq.id
    max_delivery_attempts = 5
  }

  retry_policy {
    minimum_backoff = "10s"
    maximum_backoff = "600s"
  }
}

# tickets -> ticket backend
resource "google_pubsub_subscription" "tickets_push" {
  project = var.project_id
  name    = "${local.prefix}-tickets-sub"
  topic   = google_pubsub_topic.tickets.id

  ack_deadline_seconds = 60

  push_config {
    push_endpoint = "${module.ticket_backend.uri}/pubsub/tickets"
    oidc_token {
      service_account_email = google_service_account.pubsub_push.email
      audience              = module.ticket_backend.uri
    }
  }

  dead_letter_policy {
    dead_letter_topic     = google_pubsub_topic.tickets_dlq.id
    max_delivery_attempts = 5
  }

  retry_policy {
    minimum_backoff = "10s"
    maximum_backoff = "600s"
  }
}

# ---- Dead-letter wiring: Pub/Sub service agent must publish to DLQs and
#      subscribe to the source subscriptions. ----
resource "google_pubsub_topic_iam_member" "dlq_publisher" {
  for_each = {
    findings = google_pubsub_topic.findings_dlq.name
    tickets  = google_pubsub_topic.tickets_dlq.name
  }
  project = var.project_id
  topic   = each.value
  role    = "roles/pubsub.publisher"
  member  = local.pubsub_agent
}

resource "google_pubsub_subscription_iam_member" "dlq_subscriber" {
  for_each = {
    findings = google_pubsub_subscription.findings_push.name
    tickets  = google_pubsub_subscription.tickets_push.name
  }
  project      = var.project_id
  subscription = each.value
  role         = "roles/pubsub.subscriber"
  member       = local.pubsub_agent
}
