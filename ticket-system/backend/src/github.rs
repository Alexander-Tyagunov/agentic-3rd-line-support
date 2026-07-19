//! Open a GitHub Issue (the bridge from an approved ticket to the coding agent).

use anyhow::{Context, Result};
use serde_json::{json, Value};

pub async fn create_issue(
    http: &reqwest::Client,
    owner: &str,
    repo: &str,
    token: &str,
    title: &str,
    body: &str,
    labels: &[&str],
) -> Result<String> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/issues");
    let resp = http
        .post(url)
        .header("User-Agent", "agentic-3rd-line-support")
        .header("Accept", "application/vnd.github+json")
        .bearer_auth(token)
        .json(&json!({ "title": title, "body": body, "labels": labels }))
        .send()
        .await
        .context("github create issue")?
        .error_for_status()?;
    let body: Value = resp.json().await?;
    Ok(body
        .get("html_url")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned())
}

/// Re-trigger the coding agent for an issue by removing then re-adding its label.
/// Re-adding fires `issues.labeled`, which the coding workflow runs on — so this
/// restarts the fix/PR step (e.g. after a transient failure). The remove is
/// best-effort (the label may already be absent).
pub async fn retrigger_label(
    http: &reqwest::Client,
    owner: &str,
    repo: &str,
    token: &str,
    issue_number: &str,
    label: &str,
) -> Result<()> {
    let base = format!("https://api.github.com/repos/{owner}/{repo}/issues/{issue_number}/labels");
    let _ = http
        .delete(format!("{base}/{label}"))
        .header("User-Agent", "agentic-3rd-line-support")
        .header("Accept", "application/vnd.github+json")
        .bearer_auth(token)
        .send()
        .await;
    http.post(&base)
        .header("User-Agent", "agentic-3rd-line-support")
        .header("Accept", "application/vnd.github+json")
        .bearer_auth(token)
        .json(&json!({ "labels": [label] }))
        .send()
        .await
        .context("github add label")?
        .error_for_status()?;
    Ok(())
}
