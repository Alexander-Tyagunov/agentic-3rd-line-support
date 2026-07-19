//! Agent control endpoints: manually run the monitoring sweep, and restart the
//! coding agent for a ticket (re-trigger its GitHub Action). This gives the
//! console "restart this step" buttons for the two steps a human drives.

use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Value};
use ticket_shared::Ticket;

use crate::auth;
use crate::error::AppError;
use crate::github;
use crate::state::AppState;

/// Kick the monitoring agent's `/sweep` now (OIDC-authenticated), labelled as a
/// manual run so it shows up distinctly in the Runs view.
pub async fn run_monitoring(State(st): State<AppState>) -> Result<Json<Value>, AppError> {
    let base = st.cfg.monitoring_url.trim_end_matches('/');
    if base.is_empty() {
        return Err(anyhow::anyhow!("MONITORING_URL not set").into());
    }
    let token = auth::identity_token(&st.http, base).await?;
    let resp = st
        .http
        .post(format!("{base}/sweep?trigger=manual"))
        .bearer_auth(token)
        .send()
        .await?;
    let code = resp.status();
    if !code.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("sweep {code}: {body}").into());
    }
    Ok(Json(json!({ "status": "triggered" })))
}

/// Restart the coding agent for a ticket by re-firing its issue's `agent-bug`
/// label (e.g. after the agent failed to open a PR due to a transient outage).
pub async fn retry_coding(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let doc = st
        .fs
        .get("tickets", &id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("ticket {id} not found"))?;
    let ticket: Ticket = serde_json::from_value(doc)?;
    let issue = ticket.github_issue_url.clone().unwrap_or_default();
    if issue.is_empty() {
        return Err(anyhow::anyhow!("ticket has no GitHub issue yet — approve it first").into());
    }
    let issue_number = issue.rsplit('/').next().unwrap_or_default();
    github::retrigger_label(
        &st.http,
        &st.cfg.github_owner,
        &st.cfg.github_repo,
        &st.cfg.github_token,
        issue_number,
        "agent-bug",
    )
    .await?;
    Ok(Json(json!({ "status": "retriggered", "issue": issue })))
}
