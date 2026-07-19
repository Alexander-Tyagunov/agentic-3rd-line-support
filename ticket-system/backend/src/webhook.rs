//! GitHub webhook → close the feedback loop.
//!
//! When the coding agent's PR merges (its "Fixes #N" closes the issue) or the
//! team closes the issue as not-planned, GitHub calls this endpoint. We update
//! the ticket lifecycle, the `known_issues` status, and append a ledger event —
//! so the triage agent treats future duplicates correctly (a merged fix vs a
//! declined/won't-fix decision).

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;
use ticket_shared::Ticket;

use crate::error::AppError;
use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

/// Verify the `X-Hub-Signature-256` HMAC. Empty secret => unsecured demo mode.
fn verify(secret: &str, sig_header: Option<&str>, body: &[u8]) -> bool {
    if secret.is_empty() {
        return true;
    }
    let Some(hex_sig) = sig_header.and_then(|h| h.strip_prefix("sha256=")) else {
        return false;
    };
    let Ok(sig_bytes) = hex::decode(hex_sig) else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&sig_bytes).is_ok()
}

/// Firestore doc id for a signature — must match the triage agent's `_sig_id`.
fn sig_id(signature: &str) -> String {
    let mapped: String = signature
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = mapped.trim_matches('_');
    let base = if trimmed.is_empty() {
        "unknown"
    } else {
        trimmed
    };
    base.chars().take(1500).collect()
}

pub async fn github_webhook(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, AppError> {
    let sig = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok());
    if !verify(&st.cfg.github_webhook_secret, sig, &body) {
        return Ok(StatusCode::UNAUTHORIZED);
    }

    let event = headers
        .get("x-github-event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if event == "ping" {
        return Ok(StatusCode::OK);
    }

    let payload: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

    let action = payload.get("action").and_then(Value::as_str).unwrap_or("");

    // Pull request lifecycle — links the PR to its ticket so the console can show
    // "agent working" → the PR, and records the merge/decline outcome.
    if event == "pull_request" {
        let pr = payload.get("pull_request").cloned().unwrap_or(Value::Null);
        let pr_url = pr
            .get("html_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let refs = [
            pr.get("body").and_then(Value::as_str).unwrap_or_default(),
            pr.get("title").and_then(Value::as_str).unwrap_or_default(),
        ];
        let issue_no = refs.iter().find_map(|t| referenced_issue(t));
        let merged = pr.get("merged").and_then(Value::as_bool).unwrap_or(false);
        match action {
            "opened" | "reopened" | "ready_for_review" => {
                apply_pr(&st, issue_no, &pr_url, "pr_opened", None, "pr_opened").await?;
            }
            "closed" if merged => {
                apply_pr(
                    &st,
                    issue_no,
                    &pr_url,
                    "merged",
                    Some("merged"),
                    "pr_merged",
                )
                .await?;
            }
            "closed" => {
                apply_pr(
                    &st,
                    issue_no,
                    &pr_url,
                    "declined",
                    Some("declined"),
                    "pr_declined",
                )
                .await?;
            }
            _ => {}
        }
    }

    // Fallback: an issue closed directly (no PR webhook, or closed by hand).
    if event == "issues" && action == "closed" {
        let issue = payload.get("issue").cloned().unwrap_or(Value::Null);
        let url = issue
            .get("html_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let reason = issue
            .get("state_reason")
            .and_then(Value::as_str)
            .unwrap_or("");
        let (ticket_status, ki_status, outcome) = match reason {
            "not_planned" => ("declined", "declined", "pr_declined"),
            _ => ("merged", "merged", "pr_merged"), // completed / null => treat as fixed
        };
        apply_outcome(&st, &url, ticket_status, ki_status, outcome).await?;
    }

    Ok(StatusCode::OK)
}

/// Extract the first issue number a PR references ("Fixes #12", "Closes #7", …).
fn referenced_issue(text: &str) -> Option<u64> {
    let lower = text.to_lowercase();
    for kw in [
        "fixes #",
        "fix #",
        "closes #",
        "close #",
        "resolves #",
        "resolve #",
    ] {
        if let Some(pos) = lower.find(kw) {
            let digits: String = lower[pos + kw.len()..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(n) = digits.parse::<u64>() {
                return Some(n);
            }
        }
    }
    None
}

/// Update the ticket a PR belongs to (found via the issue number it references),
/// set its PR url + status, mirror the known-issue status, and log a ledger event.
async fn apply_pr(
    st: &AppState,
    issue_no: Option<u64>,
    pr_url: &str,
    ticket_status: &str,
    ki_status: Option<&str>,
    outcome: &str,
) -> Result<(), AppError> {
    let Some(n) = issue_no else {
        return Ok(()); // couldn't tie the PR to an issue — nothing to do
    };
    let suffix = format!("/{n}");
    let tickets = st.fs.list("tickets").await?;
    let found = tickets
        .into_iter()
        .filter_map(|d| serde_json::from_value::<Ticket>(d).ok())
        .find(|t| {
            t.github_issue_url
                .as_deref()
                .is_some_and(|u| u.ends_with(&suffix))
        });
    let Some(ticket) = found else {
        return Ok(());
    };

    st.fs
        .patch_fields(
            "tickets",
            &ticket.ticket_id,
            &json!({ "status": ticket_status, "github_pr_url": pr_url }),
            &["status", "github_pr_url"],
        )
        .await?;
    if let Some(ki) = ki_status {
        st.fs
            .patch_fields(
                "known_issues",
                &sig_id(&ticket.signature),
                &json!({ "status": ki }),
                &["status"],
            )
            .await?;
    }
    let _ = st
        .fs
        .create(
            "events",
            &json!({
                "outcome": outcome,
                "ticket_id": ticket.ticket_id,
                "signature": ticket.signature,
                "service": ticket.service,
                "reason": format!("Pull request {pr_url}"),
                "at": chrono::Utc::now().to_rfc3339(),
            }),
        )
        .await;
    Ok(())
}

async fn apply_outcome(
    st: &AppState,
    issue_url: &str,
    ticket_status: &str,
    ki_status: &str,
    outcome: &str,
) -> Result<(), AppError> {
    if issue_url.is_empty() {
        return Ok(());
    }
    let tickets = st.fs.list("tickets").await?;
    let found = tickets
        .into_iter()
        .filter_map(|d| serde_json::from_value::<Ticket>(d).ok())
        .find(|t| t.github_issue_url.as_deref() == Some(issue_url));
    let Some(ticket) = found else {
        return Ok(()); // no matching ticket — nothing to do
    };

    st.fs
        .patch_fields(
            "tickets",
            &ticket.ticket_id,
            &json!({ "status": ticket_status }),
            &["status"],
        )
        .await?;
    st.fs
        .patch_fields(
            "known_issues",
            &sig_id(&ticket.signature),
            &json!({ "status": ki_status }),
            &["status"],
        )
        .await?;
    let _ = st
        .fs
        .create(
            "events",
            &json!({
                "outcome": outcome,
                "ticket_id": ticket.ticket_id,
                "signature": ticket.signature,
                "service": ticket.service,
                "at": chrono::Utc::now().to_rfc3339(),
            }),
        )
        .await;
    Ok(())
}
