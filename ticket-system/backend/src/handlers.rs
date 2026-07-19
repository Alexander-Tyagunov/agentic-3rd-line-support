//! HTTP handlers: read the ledger/tickets/health for the UI, persist pushed
//! tickets, and open a GitHub Issue when a ticket is approved.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use ticket_shared::{Health, KnownIssue, LedgerEvent, Run, Ticket};

use crate::error::AppError;
use crate::firestore::decode_push_data;
use crate::github;
use crate::state::AppState;

pub async fn healthz() -> &'static str {
    "ok"
}

fn parse_all<T: DeserializeOwned>(docs: Vec<Value>) -> Vec<T> {
    docs.into_iter()
        .filter_map(|d| serde_json::from_value(d).ok())
        .collect()
}

pub async fn list_tickets(State(st): State<AppState>) -> Result<Json<Vec<Ticket>>, AppError> {
    let mut items: Vec<Ticket> = parse_all(st.fs.list("tickets").await?);
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(Json(items))
}

pub async fn list_events(State(st): State<AppState>) -> Result<Json<Vec<LedgerEvent>>, AppError> {
    let mut items: Vec<LedgerEvent> = parse_all(st.fs.list("events").await?);
    items.sort_by(|a, b| b.at.cmp(&a.at));
    Ok(Json(items))
}

pub async fn list_known_issues(
    State(st): State<AppState>,
) -> Result<Json<Vec<KnownIssue>>, AppError> {
    let mut items: Vec<KnownIssue> = parse_all(st.fs.list("known_issues").await?);
    items.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
    Ok(Json(items))
}

pub async fn list_health(State(st): State<AppState>) -> Result<Json<Vec<Health>>, AppError> {
    Ok(Json(parse_all(st.fs.list("health").await?)))
}

pub async fn list_runs(State(st): State<AppState>) -> Result<Json<Vec<Run>>, AppError> {
    let mut items: Vec<Run> = parse_all(st.fs.list("runs").await?);
    items.sort_by(|a, b| b.finished_at.cmp(&a.finished_at));
    Ok(Json(items))
}

/// Small bits of deployment identity the UI needs to build Cloud Logging + GitHub
/// Actions deep-links (no secrets).
pub async fn meta(State(st): State<AppState>) -> Json<Value> {
    Json(json!({
        "project_id": st.cfg.project_id,
        "region": st.cfg.region,
        "github_owner": st.cfg.github_owner,
        "github_repo": st.cfg.github_repo,
    }))
}

pub async fn approve_ticket(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let doc = st
        .fs
        .get("tickets", &id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("ticket {id} not found"))?;
    let ticket: Ticket = serde_json::from_value(doc)?;

    // Idempotency: if an issue was already opened for this ticket, return it
    // instead of filing a duplicate (guards against a double-click / retry).
    if let Some(existing) = ticket.github_issue_url.as_deref().filter(|u| !u.is_empty()) {
        return Ok(Json(
            json!({ "status": ticket.status, "github_issue_url": existing }),
        ));
    }

    let url = open_issue(&st, &ticket).await?;
    let patch = json!({ "status": "issue_created", "github_issue_url": url });
    st.fs
        .patch_fields("tickets", &id, &patch, &["status", "github_issue_url"])
        .await?;
    Ok(Json(patch))
}

pub async fn pubsub_tickets(
    State(st): State<AppState>,
    Json(envelope): Json<Value>,
) -> Result<StatusCode, AppError> {
    let Some(bytes) = decode_push_data(&envelope) else {
        return Ok(StatusCode::OK); // malformed/empty: ack so it isn't redelivered forever
    };
    let ticket: Ticket = serde_json::from_slice(&bytes)?;
    st.fs
        .upsert(
            "tickets",
            &ticket.ticket_id,
            &serde_json::to_value(&ticket)?,
        )
        .await?;

    if st.cfg.auto_approve {
        if let Ok(url) = open_issue(&st, &ticket).await {
            let patch = json!({ "status": "issue_created", "github_issue_url": url });
            let _ = st
                .fs
                .patch_fields(
                    "tickets",
                    &ticket.ticket_id,
                    &patch,
                    &["status", "github_issue_url"],
                )
                .await;
        }
    }
    Ok(StatusCode::OK)
}

async fn open_issue(st: &AppState, ticket: &Ticket) -> Result<String, AppError> {
    if st.cfg.github_token.is_empty() {
        return Err(anyhow::anyhow!("GITHUB_TOKEN not set").into());
    }
    let body = format!(
        "**Service:** {} · **Severity:** {}\n\n\
         ### Description\n{}\n\n\
         ### Steps to reproduce\n```gherkin\n{}\n```\n\n\
         ### Expected state\n{}\n\n### Current state\n{}\n\n\
         ### Evidence\n`{}`\n\n_at {}_\n\n\
         ### Root cause hypothesis\n{}\n\n### Potential resolution\n{}\n\n\
         ### Justification\n{}\n\n\
         ---\n_Signature: `{}` · filed from ticket {} by the triage agent._",
        ticket.service,
        ticket.severity,
        ticket.description,
        ticket.steps_gherkin,
        ticket.expected_state,
        ticket.current_state,
        ticket.actual_log,
        ticket.log_timestamp,
        ticket.root_cause_hypothesis,
        ticket.potential_resolution,
        ticket.justification,
        ticket.signature,
        ticket.ticket_id,
    );
    let url = github::create_issue(
        &st.http,
        &st.cfg.github_owner,
        &st.cfg.github_repo,
        &st.cfg.github_token,
        &ticket.title,
        &body,
        &["agent-bug"],
    )
    .await?;
    Ok(url)
}
