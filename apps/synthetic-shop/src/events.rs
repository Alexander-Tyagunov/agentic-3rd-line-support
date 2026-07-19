//! Business-event emission in the shape Cloud Logging promotes.
//!
//! This service's *whole purpose* is to emit production-like structured logs, so
//! it writes one JSON object per line straight to stdout (via `writeln!`) instead
//! of routing through `tracing`. The Cloud Logging agent ingests each line and
//! promotes `severity`, `message`, and the `logging.googleapis.com/*` fields;
//! everything else stays queryable under `jsonPayload` (e.g. `jsonPayload.event`).
//! The `print_stdout`/`print_stderr` clippy denials are honored by using
//! `writeln!` on a locked handle rather than the `println!` macro.

use std::io::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::inventory::Inventory;

/// Monotonic counter used to make ids unique within a process.
static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Demo failure scenarios injectable via `POST /simulate`.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scenario {
    /// Burst of payment failures (HTTP 5xx) — the deterministic alert catches this.
    ObviousTxnError,
    /// An exception with a stack trace on `message`.
    LoggingError,
    /// `payment.captured` with no matching `order.created` (correlation defect).
    OrphanedTxn,
    /// Latency creep / elevated retries — only the agentic sweep catches this.
    NonObviousAnomaly,
    /// Connection-pool timeouts.
    DbPoolExhaustion,
    /// Sold below zero stock — the planted code bug the coding agent fixes.
    InventoryOversell,
    /// A rare unhandled 5xx spike (logged, not actually crashed).
    Panic,
}

fn next_seq() -> u64 {
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

/// Pseudo-unique lowercase-hex id of `len` chars (<= 32). Good enough for
/// synthetic trace/span correlation; not cryptographic.
fn hex_id(len: usize) -> String {
    let seed = nanos() ^ (u128::from(next_seq()) << 64);
    let mut s = format!("{seed:032x}");
    s.truncate(len);
    s
}

/// A fresh 32-char trace id. Reuse one across events to correlate them.
pub fn new_trace() -> String {
    hex_id(32)
}

fn project_id() -> String {
    std::env::var("PROJECT_ID").unwrap_or_else(|_| "local".to_owned())
}

fn service() -> String {
    std::env::var("SERVICE_NAME").unwrap_or_else(|_| "checkout".to_owned())
}

/// Deterministic-ish jitter in `[base, base + spread)`.
fn jitter(base: u64, spread: u64) -> u64 {
    if spread == 0 {
        return base;
    }
    base + (nanos() as u64 % spread)
}

fn write_line(v: &Value) {
    // If stdout is broken there is nowhere useful to report it; drop the error.
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{v}");
}

/// Emit one structured log line. `extra` should be a JSON object; its keys are
/// merged at the top level (i.e. into `jsonPayload`).
pub fn log_event(severity: &str, event: &str, message: &str, trace_id: &str, extra: Value) {
    let svc = service();
    let request_id = format!("req-{:x}", next_seq());

    let mut obj = json!({
        "severity": severity,
        "message": message,
        "event": event,
        "service": svc,
        "logging.googleapis.com/trace": format!("projects/{}/traces/{}", project_id(), trace_id),
        "logging.googleapis.com/spanId": hex_id(16),
        "logging.googleapis.com/labels": {
            "service": svc,
            "request_id": request_id,
            "event": event,
        },
    });

    if let (Some(map), Some(extra_map)) = (obj.as_object_mut(), extra.as_object()) {
        for (k, val) in extra_map {
            map.insert(k.clone(), val.clone());
        }
    }

    write_line(&obj);
}

/// One tick of baseline healthy traffic.
pub fn healthy_tick() {
    match next_seq() % 6 {
        0 => log_event(
            "INFO",
            "browse",
            "catalog viewed",
            &new_trace(),
            json!({ "latency_ms": jitter(30, 60) }),
        ),
        1 => log_event(
            "INFO",
            "add_to_cart",
            "item added to cart",
            &new_trace(),
            json!({ "sku": "SKU-42" }),
        ),
        2 => log_event(
            "INFO",
            "auth.login",
            "user login",
            &new_trace(),
            json!({ "method": "password" }),
        ),
        _ => purchase_flow(),
    }
}

/// A correct purchase: checkout -> payment.captured -> order.created, all sharing
/// one trace. `orphaned_txn` deliberately omits the final `order.created`.
fn purchase_flow() {
    let t = new_trace();
    log_event(
        "INFO",
        "checkout",
        "checkout started",
        &t,
        json!({ "latency_ms": jitter(80, 300) }),
    );
    log_event(
        "INFO",
        "payment.captured",
        "payment captured",
        &t,
        json!({ "amount_cents": 4999, "currency": "USD" }),
    );
    log_event(
        "INFO",
        "order.created",
        "order created",
        &t,
        json!({ "order_id": format!("ord-{:x}", next_seq()) }),
    );
}

/// Inject `count` events for `scenario`.
pub fn run_scenario(scenario: Scenario, count: u32) {
    for _ in 0..count {
        match scenario {
            Scenario::ObviousTxnError => log_event(
                "ERROR",
                "payment.failed",
                "charge failed: gateway timeout",
                &new_trace(),
                json!({
                    "reason": "gateway_timeout",
                    "httpRequest": { "requestMethod": "POST", "status": 504 },
                }),
            ),
            Scenario::LoggingError => log_event(
                "ERROR",
                "app.exception",
                "unhandled exception in checkout handler",
                &new_trace(),
                json!({
                    "stack": "Exception: index out of bounds\n  at checkout.rs:42\n  at handler.rs:17",
                }),
            ),
            Scenario::OrphanedTxn => {
                // Capture with NO following order.created for this trace.
                let t = new_trace();
                log_event(
                    "INFO",
                    "payment.captured",
                    "payment captured",
                    &t,
                    json!({ "amount_cents": 4999, "currency": "USD" }),
                );
            }
            Scenario::NonObviousAnomaly => {
                let t = new_trace();
                log_event(
                    "WARNING",
                    "checkout",
                    "checkout slow (elevated retries)",
                    &t,
                    json!({ "latency_ms": jitter(1800, 1200), "attempt": 2, "retry": true }),
                );
            }
            Scenario::DbPoolExhaustion => log_event(
                "ERROR",
                "db.error",
                "pool timeout: no connection available",
                &new_trace(),
                json!({ "pool": "orders-db", "wait_ms": jitter(5000, 2000) }),
            ),
            Scenario::InventoryOversell => {
                // Exercise the planted bug: reserving beyond stock should fail, but
                // the buggy `reserve` lets `available` go negative — an oversell.
                let mut inv = Inventory::new(3);
                let requested = 5;
                match inv.reserve(requested) {
                    Ok(remaining) if remaining < 0 => log_event(
                        "ERROR",
                        "inventory.oversold",
                        "sold below zero stock",
                        &new_trace(),
                        json!({ "sku": "SKU-42", "available": remaining, "requested": requested }),
                    ),
                    Ok(remaining) => log_event(
                        "INFO",
                        "inventory.reserved",
                        "stock reserved",
                        &new_trace(),
                        json!({ "sku": "SKU-42", "available": remaining, "requested": requested }),
                    ),
                    Err(_) => log_event(
                        "WARNING",
                        "inventory.rejected",
                        "reservation rejected: insufficient stock",
                        &new_trace(),
                        json!({ "sku": "SKU-42", "requested": requested }),
                    ),
                }
            }
            Scenario::Panic => log_event(
                "CRITICAL",
                "app.panic",
                "unhandled panic: checkout total on empty cart",
                &new_trace(),
                json!({
                    "httpRequest": { "requestMethod": "POST", "status": 500 },
                    "stack": "panicked at 'called `Option::unwrap()` on a `None` value'",
                }),
            ),
        }
    }
}
