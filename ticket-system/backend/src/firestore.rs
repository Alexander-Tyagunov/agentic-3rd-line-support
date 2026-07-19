//! Minimal Firestore access over the REST API.
//!
//! No third-party Firestore client — just `reqwest` + the documents REST API, so
//! the auth (a bearer token from the Cloud Run metadata server, or a
//! `GOOGLE_ACCESS_TOKEN` override for local dev) and the query are fully visible.
//! Handles Firestore's typed-value JSON in `to_fields` / `fields_to_json`.

use anyhow::{anyhow, Context, Result};
use base64::Engine as _;
use serde_json::{json, Map, Value};

const METADATA_TOKEN_URL: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

#[derive(Clone)]
pub struct Fs {
    http: reqwest::Client,
    project: String,
}

impl Fs {
    pub fn new(http: reqwest::Client, project: impl Into<String>) -> Self {
        Self {
            http,
            project: project.into(),
        }
    }

    fn base(&self) -> String {
        format!(
            "https://firestore.googleapis.com/v1/projects/{}/databases/(default)/documents",
            self.project
        )
    }

    async fn token(&self) -> Result<String> {
        if let Ok(tok) = std::env::var("GOOGLE_ACCESS_TOKEN") {
            if !tok.is_empty() {
                return Ok(tok);
            }
        }
        let resp = self
            .http
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

    /// List documents in a collection (up to 200), each converted to a plain JSON object.
    pub async fn list(&self, collection: &str) -> Result<Vec<Value>> {
        let token = self.token().await?;
        let url = format!("{}/{}?pageSize=200", self.base(), collection);
        let resp = self.http.get(url).bearer_auth(token).send().await?;
        if !resp.status().is_success() {
            return Ok(Vec::new()); // empty / missing collection
        }
        let body: Value = resp.json().await?;
        let docs = body
            .get("documents")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(docs
            .iter()
            .map(|d| fields_to_json(d.get("fields")))
            .collect())
    }

    /// Fetch a single document, or `None` if it does not exist.
    pub async fn get(&self, collection: &str, doc_id: &str) -> Result<Option<Value>> {
        let token = self.token().await?;
        let url = format!("{}/{}/{}", self.base(), collection, doc_id);
        let resp = self.http.get(url).bearer_auth(token).send().await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let body: Value = resp.error_for_status()?.json().await?;
        Ok(Some(fields_to_json(body.get("fields"))))
    }

    /// Upsert a whole document from a JSON object.
    pub async fn upsert(&self, collection: &str, doc_id: &str, obj: &Value) -> Result<()> {
        let token = self.token().await?;
        let url = format!("{}/{}/{}", self.base(), collection, doc_id);
        self.http
            .patch(url)
            .bearer_auth(token)
            .json(&json!({ "fields": to_fields(obj) }))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Delete every document in a collection (used by the reset endpoint). Returns count.
    pub async fn delete_all(&self, collection: &str) -> Result<usize> {
        let token = self.token().await?;
        let url = format!("{}/{}?pageSize=300", self.base(), collection);
        let resp = self.http.get(url).bearer_auth(&token).send().await?;
        if !resp.status().is_success() {
            return Ok(0);
        }
        let body: Value = resp.json().await?;
        let docs = body
            .get("documents")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut n = 0usize;
        for d in &docs {
            if let Some(name) = d.get("name").and_then(Value::as_str) {
                let del = format!("https://firestore.googleapis.com/v1/{name}");
                if let Ok(r) = self.http.delete(del).bearer_auth(&token).send().await {
                    if r.status().is_success() {
                        n += 1;
                    }
                }
            }
        }
        Ok(n)
    }

    /// Create a document with a server-assigned id (like Python's `collection.add`).
    pub async fn create(&self, collection: &str, obj: &Value) -> Result<()> {
        let token = self.token().await?;
        let url = format!("{}/{}", self.base(), collection);
        self.http
            .post(url)
            .bearer_auth(token)
            .json(&json!({ "fields": to_fields(obj) }))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// Patch only the fields named in `mask`.
    pub async fn patch_fields(
        &self,
        collection: &str,
        doc_id: &str,
        obj: &Value,
        mask: &[&str],
    ) -> Result<()> {
        let token = self.token().await?;
        let query: String = mask
            .iter()
            .map(|f| format!("updateMask.fieldPaths={f}"))
            .collect::<Vec<_>>()
            .join("&");
        let url = format!("{}/{}/{}?{}", self.base(), collection, doc_id, query);
        self.http
            .patch(url)
            .bearer_auth(token)
            .json(&json!({ "fields": to_fields(obj) }))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

/// Decode a Pub/Sub push envelope's base64 `message.data`.
pub fn decode_push_data(envelope: &Value) -> Option<Vec<u8>> {
    let data = envelope
        .get("message")
        .and_then(|m| m.get("data"))
        .and_then(Value::as_str)?;
    base64::engine::general_purpose::STANDARD.decode(data).ok()
}

// ---- Firestore typed-value <-> plain JSON ----

fn to_fields(obj: &Value) -> Value {
    let mut map = Map::new();
    if let Value::Object(m) = obj {
        for (key, val) in m {
            map.insert(key.clone(), json_to_fs(val));
        }
    }
    Value::Object(map)
}

fn json_to_fs(v: &Value) -> Value {
    match v {
        Value::Null => json!({ "nullValue": null }),
        Value::Bool(b) => json!({ "booleanValue": b }),
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                json!({ "integerValue": n.to_string() })
            } else {
                json!({ "doubleValue": n.as_f64() })
            }
        }
        Value::String(s) => json!({ "stringValue": s }),
        Value::Array(a) => {
            json!({ "arrayValue": { "values": a.iter().map(json_to_fs).collect::<Vec<_>>() } })
        }
        Value::Object(_) => json!({ "mapValue": { "fields": to_fields(v) } }),
    }
}

fn fields_to_json(fields: Option<&Value>) -> Value {
    let mut obj = Map::new();
    if let Some(Value::Object(m)) = fields {
        for (key, val) in m {
            obj.insert(key.clone(), fs_to_json(val));
        }
    }
    Value::Object(obj)
}

fn fs_to_json(v: &Value) -> Value {
    if let Some(s) = v.get("stringValue") {
        return s.clone();
    }
    if let Some(i) = v.get("integerValue") {
        return i
            .as_str()
            .and_then(|x| x.parse::<i64>().ok())
            .map_or(Value::Null, |n| json!(n));
    }
    if let Some(d) = v.get("doubleValue") {
        return d.clone();
    }
    if let Some(b) = v.get("booleanValue") {
        return b.clone();
    }
    if v.get("nullValue").is_some() {
        return Value::Null;
    }
    if let Some(ts) = v.get("timestampValue") {
        return ts.clone();
    }
    if let Some(m) = v.get("mapValue") {
        return fields_to_json(m.get("fields"));
    }
    if let Some(a) = v.get("arrayValue") {
        let values = a
            .get("values")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        return Value::Array(values.iter().map(fs_to_json).collect());
    }
    Value::Null
}
