//! Ops endpoints: Cloud Run service status + scale (min instances) via the
//! Cloud Run Admin v2 REST API. Lets you see + scale the fleet from the console.

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth;
use crate::error::AppError;
use crate::state::AppState;

const SERVICES: &[&str] = &[
    "a3l-synthetic-shop",
    "a3l-monitoring-agent",
    "a3l-triage-agent",
    "a3l-ticket-backend",
];

fn svc_url(project: &str, region: &str, name: &str) -> String {
    format!("https://run.googleapis.com/v2/projects/{project}/locations/{region}/services/{name}")
}

pub async fn status(State(st): State<AppState>) -> Result<Json<Value>, AppError> {
    let token = auth::token(&st.http).await?;
    let mut out = Vec::new();
    for s in SERVICES {
        let v: Value = match st
            .http
            .get(svc_url(&st.cfg.project_id, &st.cfg.region, s))
            .bearer_auth(&token)
            .send()
            .await
        {
            Ok(r) => r.json().await.unwrap_or(Value::Null),
            Err(_) => Value::Null,
        };
        out.push(json!({
            "service": s,
            "min_instances": v.pointer("/template/scaling/minInstanceCount").and_then(Value::as_i64).unwrap_or(0),
            "state": v.pointer("/terminalCondition/state").and_then(Value::as_str).unwrap_or("UNKNOWN"),
            "uri": v.get("uri").and_then(Value::as_str).unwrap_or_default(),
        }));
    }
    Ok(Json(json!(out)))
}

#[derive(Deserialize)]
pub struct ScaleReq {
    pub service: String,
    pub min_instances: i64,
}

/// Read the service, set template.scaling.minInstanceCount, PATCH it back
/// (creates a new revision with the new floor).
pub async fn scale(
    State(st): State<AppState>,
    Json(req): Json<ScaleReq>,
) -> Result<Json<Value>, AppError> {
    if !SERVICES.contains(&req.service.as_str()) {
        return Err(anyhow::anyhow!("unknown service").into());
    }
    let token = auth::token(&st.http).await?;
    let url = svc_url(&st.cfg.project_id, &st.cfg.region, &req.service);

    let mut svc: Value = st
        .http
        .get(&url)
        .bearer_auth(&token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    if let Some(tmpl) = svc.pointer_mut("/template").and_then(Value::as_object_mut) {
        tmpl.insert(
            "scaling".to_owned(),
            json!({ "minInstanceCount": req.min_instances }),
        );
    }

    let resp = st
        .http
        .patch(format!("{url}?updateMask=template"))
        .bearer_auth(&token)
        .json(&svc)
        .send()
        .await?;
    let code = resp.status();
    if !code.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("scale PATCH {code}: {body}").into());
    }

    Ok(Json(
        json!({ "service": req.service, "min_instances": req.min_instances }),
    ))
}
