//! Console backend: consumes the tickets topic (Pub/Sub push), persists tickets
//! to Firestore, serves the WASM UI, exposes read APIs for the ledger/health, and
//! opens a GitHub Issue when a ticket is approved.

mod admin;
mod agents;
mod auth;
mod config;
mod error;
mod firestore;
mod github;
mod handlers;
mod ops;
mod state;
mod webhook;

use axum::routing::{get, post};
use axum::Router;
use tower_http::services::ServeDir;

use crate::config::Config;
use crate::firestore::Fs;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().json().with_target(false).init();

    let cfg = Config::from_env();
    let http = reqwest::Client::builder().build()?;
    let fs = Fs::new(http.clone(), cfg.project_id.clone());
    let ui_dist = cfg.ui_dist.clone();
    let port = cfg.port;
    let state = AppState { fs, cfg, http };

    // Liveness heartbeat for the console's Health view.
    {
        let fs = state.fs.clone();
        tokio::spawn(async move {
            loop {
                let doc = serde_json::json!({
                    "component": "ticket-backend",
                    "last_seen": chrono::Utc::now().to_rfc3339(),
                });
                let _ = fs.upsert("health", "ticket-backend", &doc).await;
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            }
        });
    }

    let app = Router::new()
        .route("/health", get(handlers::healthz)) // Cloud Run frontend reserves "/healthz"
        .route("/api/tickets", get(handlers::list_tickets))
        .route("/api/events", get(handlers::list_events))
        .route("/api/known-issues", get(handlers::list_known_issues))
        .route("/api/health", get(handlers::list_health))
        .route("/api/runs", get(handlers::list_runs))
        .route("/api/meta", get(handlers::meta))
        .route("/api/tickets/:id/approve", post(handlers::approve_ticket))
        .route("/api/tickets/:id/retry-coding", post(agents::retry_coding))
        .route("/api/agents/monitoring/run", post(agents::run_monitoring))
        .route("/pubsub/tickets", post(handlers::pubsub_tickets))
        .route("/webhook/github", post(webhook::github_webhook))
        .route("/api/simulate", post(admin::simulate))
        .route("/api/admin/reset", post(admin::reset))
        .route("/api/ops", get(ops::status))
        .route("/api/ops/scale", post(ops::scale))
        .fallback_service(ServeDir::new(ui_dist))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!(port, "ticket-backend listening");
    axum::serve(listener, app).await?;
    Ok(())
}
