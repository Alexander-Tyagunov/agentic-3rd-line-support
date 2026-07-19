//! Types shared by the console backend (axum) and the WASM UI (Leptos).
//! Compiling these once and using them on both sides is why the console is Rust.
//! Fields use serde defaults so a document written by the Python agents (or a
//! partially-populated Firestore doc) always deserializes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Evidence {
    #[serde(default)]
    pub log_query: String,
    #[serde(default)]
    pub sample_trace_ids: Vec<String>,
    #[serde(default)]
    pub count: i64,
    #[serde(default)]
    pub window: String,
}

/// A bug ticket. `status` lifecycle: proposed → approved → issue_created →
/// pr_opened → (merged | declined), plus duplicate_closed. See architecture §11.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ticket {
    #[serde(default)]
    pub ticket_id: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub finding_ids: Vec<String>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub service: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub root_cause_hypothesis: String,
    #[serde(default)]
    pub suggested_fix: String,
    #[serde(default)]
    pub grounding_refs: Vec<String>,
    #[serde(default)]
    pub evidence: Evidence,
    #[serde(default)]
    pub signature: String,
    // ---- Full report (filled by triage) ----
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub steps_gherkin: String,
    #[serde(default)]
    pub expected_state: String,
    #[serde(default)]
    pub current_state: String,
    #[serde(default)]
    pub actual_log: String,
    #[serde(default)]
    pub log_timestamp: String,
    #[serde(default)]
    pub potential_resolution: String,
    #[serde(default)]
    pub justification: String,
    #[serde(default)]
    pub github_issue_url: Option<String>,
    /// Set by the PR webhook once the coding agent opens a pull request.
    #[serde(default)]
    pub github_pr_url: Option<String>,
}

/// One entry in the event ledger. `outcome`: ticketed | duplicate_closed | ignored.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LedgerEvent {
    #[serde(default)]
    pub outcome: String,
    #[serde(default)]
    pub finding_id: String,
    #[serde(default)]
    pub signature: String,
    #[serde(default)]
    pub service: Option<String>,
    #[serde(default)]
    pub ticket_id: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub at: Option<String>,
}

/// The dedup registry: one per signature.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KnownIssue {
    #[serde(default)]
    pub signature: String,
    #[serde(default)]
    pub canonical_ticket_id: String,
    #[serde(default)]
    pub status: String, // open | merged | declined | wontfix
    #[serde(default)]
    pub service: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub occurrence_count: i64,
    #[serde(default)]
    pub first_seen: Option<String>,
    #[serde(default)]
    pub last_seen: Option<String>,
}

/// One agent invocation, for the console's Runs view. Written by the monitoring
/// and triage agents; `status` is success | error.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Run {
    #[serde(default)]
    pub agent: String, // monitoring | triage
    #[serde(default)]
    pub status: String, // success | error
    #[serde(default)]
    pub trigger: String, // scheduler | manual | alert | pubsub
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub count: i64,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
}

/// A component heartbeat.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Health {
    #[serde(default)]
    pub component: String,
    #[serde(default)]
    pub last_seen: Option<String>,
}
