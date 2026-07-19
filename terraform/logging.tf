# Deterministic detection lane: log-based metrics + an alert policy that
# publishes to the log-alerts Pub/Sub topic (which pushes into the monitoring
# agent for enrichment). These catch the patterns we can name in advance.

# Counter metric: payment failures emitted by the synthetic app.
resource "google_logging_metric" "payment_errors" {
  project = var.project_id
  name    = "${local.prefix}_payment_errors"
  filter  = <<-EOT
    resource.type="cloud_run_revision"
    jsonPayload.event="payment.failed"
    severity>="ERROR"
  EOT

  metric_descriptor {
    metric_kind = "DELTA"
    value_type  = "INT64"
    unit        = "1"
    labels {
      key         = "service"
      value_type  = "STRING"
      description = "Originating service"
    }
  }

  label_extractors = {
    service = "EXTRACT(jsonPayload.service)"
  }
}

# Distribution metric: checkout latency (for the "non_obvious_anomaly" scenario).
resource "google_logging_metric" "checkout_latency" {
  project = var.project_id
  name    = "${local.prefix}_checkout_latency_ms"
  filter  = <<-EOT
    resource.type="cloud_run_revision"
    jsonPayload.event="checkout"
    jsonPayload.latency_ms>0
  EOT

  metric_descriptor {
    metric_kind = "DELTA"
    value_type  = "DISTRIBUTION"
    unit        = "ms"
  }

  value_extractor = "EXTRACT(jsonPayload.latency_ms)"

  bucket_options {
    exponential_buckets {
      num_finite_buckets = 16
      growth_factor      = 2
      scale              = 1
    }
  }
}

# Pub/Sub notification channel for alerts.
resource "google_monitoring_notification_channel" "pubsub_alerts" {
  project      = var.project_id
  display_name = "Agentic 3rd-line — log alerts to Pub/Sub"
  type         = "pubsub"

  labels = {
    topic = google_pubsub_topic.log_alerts.id
  }
}

# The Monitoring service agent must be allowed to publish to the alerts topic.
resource "google_pubsub_topic_iam_member" "monitoring_agent_publishes_alerts" {
  project = var.project_id
  topic   = google_pubsub_topic.log_alerts.name
  role    = "roles/pubsub.publisher"
  member  = local.monitoring_agent
}

# Alert when payment errors spike (a known pattern).
resource "google_monitoring_alert_policy" "payment_error_rate" {
  project      = var.project_id
  display_name = "Payment errors > 5 in 5m"
  combiner     = "OR"

  conditions {
    display_name = "payment.failed rate"
    condition_threshold {
      filter          = "metric.type=\"logging.googleapis.com/user/${google_logging_metric.payment_errors.name}\" AND resource.type=\"cloud_run_revision\""
      comparison      = "COMPARISON_GT"
      threshold_value = 5
      duration        = "0s"

      aggregations {
        alignment_period   = "300s"
        per_series_aligner = "ALIGN_SUM"
      }

      trigger {
        count = 1
      }
    }
  }

  notification_channels = [google_monitoring_notification_channel.pubsub_alerts.id]

  # notification_rate_limit is only valid on log-based alert policies, not
  # metric-threshold ones, so this policy only sets auto_close.
  alert_strategy {
    auto_close = "1800s"
  }
}
