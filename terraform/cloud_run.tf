# The four Cloud Run services, all via the reusable agent-service module.
# Images default to the public "hello" container so `plan`/`apply` work before
# real images exist; CI (or a manual `gcloud run deploy`) swaps in real images.

resource "google_artifact_registry_repository" "images" {
  project       = var.project_id
  location      = var.region
  repository_id = "${local.prefix}-images"
  format        = "DOCKER"
  description   = "Container images for agentic-3rd-line-support"

  depends_on = [google_project_service.enabled]
}

# 0 · Synthetic shop — floods logs; must keep a warm instance + CPU always on.
module "synthetic_shop" {
  source = "./modules/agent-service"

  name               = "${local.prefix}-synthetic-shop"
  service_account_id = "${local.prefix}-synthetic-shop"
  project_id         = var.project_id
  location           = var.region
  image              = var.synthetic_shop_image

  min_instances = var.synthetic_min_instances
  cpu_idle      = false # background flooder needs CPU between requests
  invokers      = var.synthetic_allow_unauthenticated ? ["allUsers"] : []

  env = {
    LOG_FLOOD_RATE_PER_SEC = "20"
    SERVICE_NAME           = "checkout"
  }

  labels     = local.labels
  depends_on = [google_project_service.enabled]
}

# 1 · Monitoring agent — invoked by Scheduler (sweep) and by Pub/Sub (alerts).
module "monitoring_agent" {
  source = "./modules/agent-service"

  name               = "${local.prefix}-monitoring-agent"
  service_account_id = "${local.prefix}-monitoring-agent"
  project_id         = var.project_id
  location           = var.region
  image              = var.monitoring_agent_image

  invokers = [
    "serviceAccount:${google_service_account.pubsub_push.email}",
    "serviceAccount:${google_service_account.scheduler.email}",
  ]

  env = {
    PROJECT_ID           = var.project_id
    FINDINGS_TOPIC       = google_pubsub_topic.findings.name
    GEMINI_LOCATION      = var.gemini_location
    GEMINI_MODEL         = var.gemini_model_monitoring
    LOG_LOOKBACK_MINUTES = "15"
  }

  labels     = local.labels
  depends_on = [google_project_service.enabled]
}

# 2 · Triage agent — invoked by Pub/Sub push on findings.
module "triage_agent" {
  source = "./modules/agent-service"

  name               = "${local.prefix}-triage-agent"
  service_account_id = "${local.prefix}-triage-agent"
  project_id         = var.project_id
  location           = var.region
  image              = var.triage_agent_image

  invokers = ["serviceAccount:${google_service_account.pubsub_push.email}"]

  env = {
    PROJECT_ID      = var.project_id
    TICKETS_TOPIC   = google_pubsub_topic.tickets.name
    GEMINI_LOCATION = var.gemini_location
    GEMINI_MODEL    = var.gemini_model_triage
  }

  labels     = local.labels
  depends_on = [google_project_service.enabled]
}

# 3 · Ticket backend — consumes tickets (push), serves the WASM UI, opens Issues.
module "ticket_backend" {
  source = "./modules/agent-service"

  name               = "${local.prefix}-ticket-backend"
  service_account_id = "${local.prefix}-ticket-backend"
  project_id         = var.project_id
  location           = var.region
  image              = var.ticket_backend_image

  invokers = concat(
    ["serviceAccount:${google_service_account.pubsub_push.email}"],
    var.ticket_ui_allow_unauthenticated ? ["allUsers"] : [],
  )

  env = {
    PROJECT_ID     = var.project_id
    REGION         = var.region
    SHOP_URL       = module.synthetic_shop.uri
    MONITORING_URL = module.monitoring_agent.uri # console "Run sweep now"
    GITHUB_OWNER   = var.github_owner
    GITHUB_REPO    = var.github_repo
  }

  # Only mount secrets that have a value (avoids a broken deploy on a bare apply).
  secret_env = concat(
    var.github_token != "" ? [{
      name    = "GITHUB_TOKEN"
      secret  = google_secret_manager_secret.github_token.secret_id
      version = "latest"
    }] : [],
    var.github_webhook_secret != "" ? [{
      name    = "GITHUB_WEBHOOK_SECRET"
      secret  = google_secret_manager_secret.github_webhook_secret.secret_id
      version = "latest"
    }] : [],
  )

  labels     = local.labels
  depends_on = [google_project_service.enabled]
}
