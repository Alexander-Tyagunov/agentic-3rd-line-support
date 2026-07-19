//! Shared GCP bearer-token acquisition (Cloud Run metadata server, or a
//! GOOGLE_ACCESS_TOKEN override for local dev).

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

const METADATA_TOKEN_URL: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

pub async fn token(http: &reqwest::Client) -> Result<String> {
    if let Ok(tok) = std::env::var("GOOGLE_ACCESS_TOKEN") {
        if !tok.is_empty() {
            return Ok(tok);
        }
    }
    let resp = http
        .get(METADATA_TOKEN_URL)
        .header("Metadata-Flavor", "Google")
        .send()
        .await
        .context("metadata token request")?
        .error_for_status()?;
    let body: Value = resp.json().await?;
    body.get("access_token")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("no access_token in metadata response"))
}

const METADATA_ID_URL: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/identity";

/// Mint an OIDC identity token for `audience` (a Cloud Run service URL) so the
/// backend can invoke another authenticated service (e.g. the monitoring sweep).
pub async fn identity_token(http: &reqwest::Client, audience: &str) -> Result<String> {
    let resp = http
        .get(METADATA_ID_URL)
        .query(&[("audience", audience), ("format", "full")])
        .header("Metadata-Flavor", "Google")
        .send()
        .await
        .context("metadata identity request")?
        .error_for_status()?;
    Ok(resp.text().await?)
}
