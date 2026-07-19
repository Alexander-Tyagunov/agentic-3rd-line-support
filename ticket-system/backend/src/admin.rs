//! Demo-driver endpoints: inject scenarios into the synthetic app, and reset
//! state for a clean slate before a lecture.

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct SimulateReq {
    pub scenario: String,
    #[serde(default = "one")]
    pub count: u32,
}

fn one() -> u32 {
    1
}

/// Proxy to the synthetic app's /simulate so the demo is driven from the console.
pub async fn simulate(
    State(st): State<AppState>,
    Json(req): Json<SimulateReq>,
) -> Result<Json<Value>, AppError> {
    if st.cfg.shop_url.is_empty() {
        return Err(anyhow::anyhow!("SHOP_URL not set").into());
    }
    let url = format!("{}/simulate", st.cfg.shop_url.trim_end_matches('/'));
    let resp = st
        .http
        .post(url)
        .json(&json!({ "scenario": req.scenario, "count": req.count }))
        .send()
        .await?
        .error_for_status()?;
    let body: Value = resp
        .json()
        .await
        .unwrap_or_else(|_| json!({ "status": "ok" }));
    Ok(Json(body))
}

#[derive(Deserialize)]
pub struct ResetReq {
    #[serde(default)]
    pub scope: String, // all | tickets | events | known_issues | health | queue
}

/// Reset state. `scope=all` wipes tickets+events+known_issues and purges queues.
pub async fn reset(
    State(st): State<AppState>,
    Query(q): Query<ResetReq>,
) -> Result<Json<Value>, AppError> {
    let scope = if q.scope.is_empty() {
        "all"
    } else {
        q.scope.as_str()
    };
    let collections: &[&str] = match scope {
        "all" => &["events", "tickets", "known_issues", "runs"],
        "tickets" => &["tickets"],
        "events" => &["events"],
        "known_issues" => &["known_issues"],
        "health" => &["health"],
        "runs" => &["runs"],
        _ => &[],
    };

    let mut deleted = serde_json::Map::new();
    for c in collections {
        let n = st.fs.delete_all(c).await?;
        deleted.insert((*c).to_owned(), json!(n));
    }
    if scope == "all" || scope == "queue" {
        purge_queues(&st).await;
        deleted.insert("queues".to_owned(), json!("purged"));
    }
    Ok(Json(json!({ "scope": scope, "deleted": deleted })))
}

/// Purge Pub/Sub backlog by seeking each subscription to "now".
async fn purge_queues(st: &AppState) {
    let Ok(token) = auth::token(&st.http).await else {
        return;
    };
    let now = chrono::Utc::now().to_rfc3339();
    for sub in ["a3l-findings-sub", "a3l-tickets-sub", "a3l-log-alerts-sub"] {
        let url = format!(
            "https://pubsub.googleapis.com/v1/projects/{}/subscriptions/{}:seek",
            st.cfg.project_id, sub
        );
        let _ = st
            .http
            .post(url)
            .bearer_auth(&token)
            .json(&json!({ "time": now }))
            .send()
            .await;
    }
}
