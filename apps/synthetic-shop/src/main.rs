//! Synthetic Shop — a fake e-commerce service whose job is to produce realistic,
//! business-meaningful logs for the rest of the pipeline.
//!
//! - A background task floods baseline "healthy" traffic logs continuously.
//! - `POST /simulate` injects a specific failure scenario on demand for the demo.
//!
//! See README.md and grounding/risky-patterns.md for the scenario catalog.

mod events;
mod inventory;

use axum::{
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::events::{healthy_tick, run_scenario, Scenario};

#[derive(Debug, Deserialize)]
struct SimulateRequest {
    scenario: Scenario,
    #[serde(default = "default_count")]
    count: u32,
}

const fn default_count() -> u32 {
    1
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);

    let rate: u64 = std::env::var("LOG_FLOOD_RATE_PER_SEC")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20)
        .max(1);

    // Start the baseline log flooder.
    tokio::spawn(flooder(rate));

    let app = Router::new()
        .route("/health", get(healthz)) // NB: Cloud Run's frontend reserves "/healthz"
        .route("/simulate", post(simulate));

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz() -> &'static str {
    "ok"
}

async fn simulate(Json(req): Json<SimulateRequest>) -> Json<Value> {
    let count = req.count.clamp(1, 1000);
    run_scenario(req.scenario, count);
    Json(json!({ "injected": count, "scenario": format!("{:?}", req.scenario) }))
}

// cancel-safe: the loop only sleeps and writes stdout lines; if the task is
// dropped it loses at most one in-flight line and leaves no partial external state.
async fn flooder(rate_per_sec: u64) {
    let period = std::time::Duration::from_millis(1000 / rate_per_sec.max(1));
    loop {
        healthy_tick();
        tokio::time::sleep(period).await;
    }
}
