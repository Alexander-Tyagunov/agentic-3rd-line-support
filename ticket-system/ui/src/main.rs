//! Console UI (Leptos CSR / WASM) — the one pane of glass over the loop.
//! Tabs: Events ledger, Tickets (+ full report detail), Known issues, Health,
//! Runs (agent run history), Simulate (drive the demo + reset), Ops (scale).
//! All timestamps are shown in Eastern time (America/New_York).

use gloo_net::http::Request;
use leptos::prelude::*;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ticket_shared::{Health, KnownIssue, LedgerEvent, Run, Ticket};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;

const REPO_URL: &str = "https://github.com/Alexander-Tyagunov/agentic-3rd-line-support";
const CODING_WORKFLOW: &str = "gemini.yml";

fn main() {
    leptos::mount::mount_to_body(App);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Events,
    Tickets,
    KnownIssues,
    Health,
    Runs,
    Simulate,
    Ops,
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct OpsSvc {
    #[serde(default)]
    service: String,
    #[serde(default)]
    min_instances: i64,
    #[serde(default)]
    state: String,
    #[serde(default)]
    uri: String,
}

/// Only the deployment identity the UI needs for deep-links. The `/api/meta`
/// endpoint returns more; serde ignores the rest.
#[derive(Clone, Default, Deserialize)]
struct Meta {
    #[serde(default)]
    project_id: String,
}

const SCENARIOS: &[(&str, &str)] = &[
    ("obvious_txn_error", "Payment errors (500 burst)"),
    ("orphaned_txn", "Orphaned transaction"),
    ("non_obvious_anomaly", "Latency creep (subtle)"),
    ("db_pool_exhaustion", "DB pool exhaustion"),
    ("inventory_oversell", "Inventory oversell (bug)"),
    ("logging_error", "Exception + stack trace"),
    ("panic", "Unhandled panic (5xx)"),
];

const EVENT_FILTERS: &[(&str, &str)] = &[
    ("all", "All"),
    ("ticketed", "Ticketed"),
    ("duplicate_closed", "Duplicates"),
    ("ignored", "Ignored"),
];

// ---- helpers ----

/// Format an RFC3339 UTC timestamp in Eastern time. Falls back to the raw string
/// if it can't be parsed (empty stays empty).
fn to_est(iso: &str) -> String {
    if iso.is_empty() {
        return String::new();
    }
    let d = js_sys::Date::new(&JsValue::from_str(iso));
    if d.get_time().is_nan() {
        return iso.to_owned();
    }
    let opts = js_sys::Object::new();
    let set = |k: &str, v: &str| {
        let _ = js_sys::Reflect::set(&opts, &JsValue::from_str(k), &JsValue::from_str(v));
    };
    set("timeZone", "America/New_York");
    set("dateStyle", "medium");
    set("timeStyle", "medium");
    let s = String::from(d.to_locale_string("en-US", &opts));
    format!("{s} ET")
}

fn est_opt(v: &Option<String>) -> String {
    to_est(v.as_deref().unwrap_or_default())
}

/// Cloud Logging (Logs Explorer) deep link for a Cloud Run service.
fn logs_url(project: &str, service: &str) -> String {
    format!(
        "https://console.cloud.google.com/logs/query;query=resource.type%3D%22cloud_run_revision%22%20resource.labels.service_name%3D%22{service}%22?project={project}"
    )
}

fn agent_service(agent: &str) -> String {
    match agent {
        "monitoring" => "a3l-monitoring-agent".to_owned(),
        "triage" => "a3l-triage-agent".to_owned(),
        other => format!("a3l-{other}"),
    }
}

// ---- data loading ----

fn load<T>(url: &'static str, sig: RwSignal<Vec<T>>)
where
    T: DeserializeOwned + Send + Sync + 'static,
{
    spawn_local(async move {
        if let Ok(resp) = Request::get(url).send().await {
            if let Ok(items) = resp.json::<Vec<T>>().await {
                sig.set(items);
            }
        }
    });
}

fn load_one<T>(url: &'static str, sig: RwSignal<T>)
where
    T: DeserializeOwned + Send + Sync + 'static,
{
    spawn_local(async move {
        if let Ok(resp) = Request::get(url).send().await {
            if let Ok(item) = resp.json::<T>().await {
                sig.set(item);
            }
        }
    });
}

fn post(url: String, body: Option<Value>, status: RwSignal<String>, after: impl Fn() + 'static) {
    status.set("working…".into());
    spawn_local(async move {
        let req = Request::post(&url);
        let sent = match body {
            Some(v) => match req.json(&v) {
                Ok(r) => r.send().await,
                Err(e) => Err(e),
            },
            None => req.send().await,
        };
        status.set(match &sent {
            Ok(r) if r.ok() => "done ✓".into(),
            _ => "failed ✗".into(),
        });
        after();
    });
}

// ---- app ----

#[component]
fn App() -> impl IntoView {
    let tickets = RwSignal::new(Vec::<Ticket>::new());
    let events = RwSignal::new(Vec::<LedgerEvent>::new());
    let known = RwSignal::new(Vec::<KnownIssue>::new());
    let health = RwSignal::new(Vec::<Health>::new());
    let ops = RwSignal::new(Vec::<OpsSvc>::new());
    let runs = RwSignal::new(Vec::<Run>::new());
    let meta = RwSignal::new(Meta::default());
    let tab = RwSignal::new(Tab::Events);
    let selected = RwSignal::new(Option::<Ticket>::None);
    let status = RwSignal::new(String::new());
    let count = RwSignal::new(5u32);
    let ev_filter = RwSignal::new(String::from("all"));
    let busy = RwSignal::new(false);

    let refresh = move || {
        load::<Ticket>("/api/tickets", tickets);
        load::<LedgerEvent>("/api/events", events);
        load::<KnownIssue>("/api/known-issues", known);
        load::<Health>("/api/health", health);
        load::<OpsSvc>("/api/ops", ops);
        load::<Run>("/api/runs", runs);
    };
    load_one::<Meta>("/api/meta", meta);
    refresh();

    let nav = move |t: Tab, label: &str| {
        let cls = move || if tab.get() == t { "active" } else { "" };
        view! { <button class=cls on:click=move |_| { selected.set(None); tab.set(t); }>{label.to_string()}</button> }
    };

    view! {
        <header>
            <div class="topbar">
                <div class="brand">
                    <div class="logo"></div>
                    <div class="brand-text">
                        <h1>"3rd-line support "<span class="accent-pill">"console"</span></h1>
                        <div class="pipeline">
                            <span class="step">"Detect"</span><span class="arrow">"→"</span>
                            <span class="step">"Triage"</span><span class="arrow">"→"</span>
                            <span class="step">"Ticket"</span><span class="arrow">"→"</span>
                            <span class="step">"Fix"</span>
                            <span class="arrow">"·"</span>
                            <span class="meta">"Gemini on Google Cloud"</span>
                        </div>
                    </div>
                </div>
                <button class="refresh-btn" on:click=move |_| refresh()>"⟳ Refresh"</button>
            </div>
            <nav>
                {nav(Tab::Events, "Events")}
                {nav(Tab::Tickets, "Tickets")}
                {nav(Tab::KnownIssues, "Known issues")}
                {nav(Tab::Health, "Health")}
                {nav(Tab::Runs, "Runs")}
                {nav(Tab::Simulate, "Simulate")}
                {nav(Tab::Ops, "Ops")}
            </nav>
        </header>
        <main>
            <div class="status-line">{move || status.get()}</div>
            {move || match tab.get() {
                Tab::Events => events_view(events, ev_filter, tab, selected, tickets, runs, status).into_any(),
                Tab::Tickets => tickets_view(tickets, selected, status, busy).into_any(),
                Tab::KnownIssues => known_view(known).into_any(),
                Tab::Health => health_view(health).into_any(),
                Tab::Runs => runs_view(runs, meta, status).into_any(),
                Tab::Simulate => simulate_view(count, status, tickets, events, known, runs).into_any(),
                Tab::Ops => ops_view(ops, status).into_any(),
            }}
        </main>
        <footer>
            <span class="foot-brand">"3rd-line support"</span>
            " · built by "
            <a href=REPO_URL target="_blank" rel="noopener">"Alex Tyagunov"</a>
            " · "
            <a href=REPO_URL target="_blank" rel="noopener">"GitHub ↗"</a>
        </footer>
    }
}

fn sev_class(s: &str) -> String {
    format!("badge sev-{s}")
}

fn run_badge(status: &str) -> String {
    format!("badge run-{status}")
}

fn run_monitoring(status: RwSignal<String>, runs: RwSignal<Vec<Run>>) {
    post(
        "/api/agents/monitoring/run".into(),
        None,
        status,
        move || load::<Run>("/api/runs", runs),
    );
}

// ---- Events ----

fn open_ticket(
    tid: String,
    tickets: RwSignal<Vec<Ticket>>,
    selected: RwSignal<Option<Ticket>>,
    tab: RwSignal<Tab>,
) {
    selected.set(tickets.get().into_iter().find(|t| t.ticket_id == tid));
    tab.set(Tab::Tickets);
}

#[allow(clippy::too_many_arguments)]
fn events_view(
    events: RwSignal<Vec<LedgerEvent>>,
    ev_filter: RwSignal<String>,
    tab: RwSignal<Tab>,
    selected: RwSignal<Option<Ticket>>,
    tickets: RwSignal<Vec<Ticket>>,
    runs: RwSignal<Vec<Run>>,
    status: RwSignal<String>,
) -> impl IntoView {
    view! {
        <h2>"Event ledger"</h2>
        <p class="hint">"Every finding the triage agent saw. Nothing is dropped — duplicates are recorded and auto-closed against the ticket they repeat."</p>
        <div class="run-strip">
            {move || match runs.get().into_iter().find(|r| r.agent == "monitoring") {
                Some(r) => view! {
                    <span class="muted">"Monitoring — last run "{est_opt(&r.finished_at)}" · "</span>
                    <span class=run_badge(&r.status)>{r.status.clone()}</span>
                }.into_any(),
                None => view! { <span class="muted">"Monitoring — no runs yet"</span> }.into_any(),
            }}
            <span class="spacer"></span>
            <button class="btn" on:click=move |_| run_monitoring(status, runs)>"Run sweep now"</button>
        </div>
        <div class="filters">
            {EVENT_FILTERS.iter().map(|(key, label)| {
                let key = *key;
                let cls = move || if ev_filter.get() == key { "chip active" } else { "chip" };
                view! {
                    <button class=cls on:click=move |_| ev_filter.set(key.to_string())>
                        {*label}
                        <span class="n">{move || events.get().iter().filter(|e| key == "all" || e.outcome == key).count()}</span>
                    </button>
                }
            }).collect_view()}
        </div>
        <div class="card">
            <table>
                <thead><tr><th>"outcome"</th><th>"when"</th><th>"signature"</th><th>"service"</th><th>"ticket"</th><th>"reason"</th></tr></thead>
                <tbody>
                    {move || {
                        let f = ev_filter.get();
                        let is_all = f == "all";
                        events.get().into_iter()
                            .filter(|e| is_all || e.outcome == f)
                            .map(|e| {
                                let cls = format!("badge out-{}", e.outcome);
                                let tid = e.ticket_id.clone().unwrap_or_default();
                                let ticket_cell = if tid.is_empty() {
                                    view! { <span class="muted">"—"</span> }.into_any()
                                } else {
                                    let go = tid.clone();
                                    view! {
                                        <a class="tlink" title="Open this ticket"
                                           on:click=move |_| open_ticket(go.clone(), tickets, selected, tab)>
                                            {tid.clone()}
                                        </a>
                                    }.into_any()
                                };
                                view! {
                                    <tr>
                                        <td><span class=cls>{e.outcome}</span></td>
                                        <td class="muted nowrap">{est_opt(&e.at)}</td>
                                        <td><code>{e.signature}</code></td>
                                        <td>{e.service.unwrap_or_default()}</td>
                                        <td>{ticket_cell}</td>
                                        <td class="muted reason">{e.reason.unwrap_or_default()}</td>
                                    </tr>
                                }
                            }).collect_view()
                    }}
                </tbody>
            </table>
        </div>
    }
}

// ---- Tickets ----

fn tickets_view(
    tickets: RwSignal<Vec<Ticket>>,
    selected: RwSignal<Option<Ticket>>,
    status: RwSignal<String>,
    busy: RwSignal<bool>,
) -> impl IntoView {
    view! {
        {move || match selected.get() {
            Some(t) => ticket_detail(t, tickets, selected, status, busy).into_any(),
            None => ticket_table(tickets, selected, status, busy).into_any(),
        }}
    }
}

fn ticket_table(
    tickets: RwSignal<Vec<Ticket>>,
    selected: RwSignal<Option<Ticket>>,
    status: RwSignal<String>,
    busy: RwSignal<bool>,
) -> impl IntoView {
    view! {
        <h2>"Tickets & history"</h2>
        <div class="card">
            <table>
                <thead><tr><th>"id"</th><th>"sev"</th><th>"service"</th><th>"title"</th><th>"status"</th><th>"action"</th></tr></thead>
                <tbody>
                    {move || tickets.get().into_iter().map(|t| {
                        let open = t.clone();
                        let st_cls = format!("badge st-{}", t.status);
                        view! {
                            <tr class="clickable" on:click=move |_| selected.set(Some(open.clone()))>
                                <td><code>{t.ticket_id.clone()}</code></td>
                                <td><span class=sev_class(&t.severity)>{t.severity.clone()}</span></td>
                                <td>{t.service.clone()}</td>
                                <td>{t.title.clone()}</td>
                                <td><span class=st_cls>{t.status.clone()}</span></td>
                                <td on:click=|ev| ev.stop_propagation()>
                                    {ticket_actions(&t, tickets, selected, status, busy)}
                                </td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}

/// Status-aware action cell: Approve while proposed; afterwards links to the
/// issue, the coding agent's CI runs, the PR once it exists, and a Retry button
/// to restart the coding step (e.g. after a transient failure).
fn ticket_actions(
    t: &Ticket,
    tickets: RwSignal<Vec<Ticket>>,
    selected: RwSignal<Option<Ticket>>,
    status: RwSignal<String>,
    busy: RwSignal<bool>,
) -> AnyView {
    let issue = t.github_issue_url.clone().unwrap_or_default();
    let pr = t.github_pr_url.clone().unwrap_or_default();
    let id = t.ticket_id.clone();

    let issue_link = (!issue.is_empty()).then(|| {
        let n = issue.rsplit('/').next().unwrap_or_default().to_string();
        view! { <a class="btn ghost" href=issue.clone() target="_blank" rel="noopener" on:click=|e| e.stop_propagation()>{format!("Issue #{n} ↗")}</a> }
    });
    let pr_link = (!pr.is_empty()).then(|| {
        let n = pr.rsplit('/').next().unwrap_or_default().to_string();
        view! { <a class="btn ghost" href=pr.clone() target="_blank" rel="noopener" on:click=|e| e.stop_propagation()>{format!("PR #{n} ↗")}</a> }
    });
    let ci_link = view! {
        <a class="btn ghost" href=format!("{REPO_URL}/actions/workflows/{CODING_WORKFLOW}")
           target="_blank" rel="noopener" on:click=|e| e.stop_propagation()>"CI runs ↗"</a>
    };
    let retry_id = id.clone();
    let retry = view! {
        <button class="btn" title="Re-trigger the coding agent for this issue"
            on:click=move |e| { e.stop_propagation(); retry_coding(retry_id.clone(), status); }>"↻ Retry"</button>
    };

    match t.status.as_str() {
        "proposed" | "approved" | "" => view! {
            <button class="btn primary" disabled=move || busy.get()
                on:click=move |ev| { ev.stop_propagation(); approve(id.clone(), tickets, selected, status, busy); }>
                "Approve & fix"
            </button>
        }
        .into_any(),
        "issue_created" => view! {
            <div class="actions">
                <span class="badge agent"><span class="pulse"></span>"agent working"</span>
                {issue_link}{ci_link}{retry}
            </div>
        }
        .into_any(),
        "pr_opened" => view! {
            <div class="actions"><span class="badge st-pr_opened">"PR open"</span>{issue_link}{pr_link}{ci_link}{retry}</div>
        }
        .into_any(),
        "merged" => view! {
            <div class="actions"><span class="badge st-merged">"merged"</span>{issue_link}{pr_link}{ci_link}</div>
        }
        .into_any(),
        "declined" => view! {
            <div class="actions"><span class="badge st-declined">"declined"</span>{issue_link}{pr_link}{ci_link}{retry}</div>
        }
        .into_any(),
        _ => view! { <span class="muted">"—"</span> }.into_any(),
    }
}

fn ticket_detail(
    t: Ticket,
    tickets: RwSignal<Vec<Ticket>>,
    selected: RwSignal<Option<Ticket>>,
    status: RwSignal<String>,
    busy: RwSignal<bool>,
) -> impl IntoView {
    view! {
        <div class="row" style="margin-bottom:12px">
            <button class="btn" on:click=move |_| selected.set(None)>"← Back"</button>
            {ticket_actions(&t, tickets, selected, status, busy)}
        </div>
        <div class="detail">
            <h2>{t.title.clone()}</h2>
            <div class="kv">
                <span class=sev_class(&t.severity)>{t.severity.clone()}</span>
                <span class=format!("badge st-{}", t.status)>{t.status.clone()}</span>
                <span class="muted">{t.service.clone()}</span>
                <code>{t.ticket_id.clone()}</code>
            </div>
            <section><h4>"Description"</h4><p>{t.description.clone()}</p></section>
            <section><h4>"Steps to reproduce (Gherkin)"</h4><pre>{t.steps_gherkin.clone()}</pre></section>
            <section class="row" style="gap:24px; align-items:flex-start">
                <div style="flex:1"><h4>"Expected state"</h4><p>{t.expected_state.clone()}</p></div>
                <div style="flex:1"><h4>"Current state"</h4><p>{t.current_state.clone()}</p></div>
            </section>
            <section><h4>"Actual log"</h4><pre>{t.actual_log.clone()}</pre><p class="muted">"at " {to_est(&t.log_timestamp)}</p></section>
            <section><h4>"Root cause hypothesis"</h4><p>{t.root_cause_hypothesis.clone()}</p></section>
            <section><h4>"Potential resolution"</h4><p>{t.potential_resolution.clone()}</p></section>
            <section><h4>"Justification"</h4><p>{t.justification.clone()}</p></section>
            <section><h4>"Signature"</h4><code>{t.signature.clone()}</code></section>
        </div>
    }
}

fn approve(
    id: String,
    tickets: RwSignal<Vec<Ticket>>,
    selected: RwSignal<Option<Ticket>>,
    status: RwSignal<String>,
    busy: RwSignal<bool>,
) {
    if busy.get() {
        return;
    }
    busy.set(true);
    let url = format!("/api/tickets/{id}/approve");
    post(url, None, status, move || {
        busy.set(false);
        selected.set(None);
        load::<Ticket>("/api/tickets", tickets);
    });
}

fn retry_coding(id: String, status: RwSignal<String>) {
    post(
        format!("/api/tickets/{id}/retry-coding"),
        None,
        status,
        || {},
    );
}

// ---- Known issues ----

fn known_view(known: RwSignal<Vec<KnownIssue>>) -> impl IntoView {
    view! {
        <h2>"Known issues (dedup registry)"</h2>
        <div class="card">
            <table>
                <thead><tr><th>"signature"</th><th>"status"</th><th>"service"</th><th>"sev"</th><th>"seen"</th><th>"canonical ticket"</th></tr></thead>
                <tbody>
                    {move || known.get().into_iter().map(|k| {
                        let st_cls = format!("badge st-{}", k.status);
                        let sev_cls = sev_class(&k.severity);
                        view! {
                            <tr>
                                <td><code>{k.signature}</code></td>
                                <td><span class=st_cls>{k.status}</span></td>
                                <td>{k.service}</td>
                                <td><span class=sev_cls>{k.severity}</span></td>
                                <td>{k.occurrence_count}</td>
                                <td><code>{k.canonical_ticket_id}</code></td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}

// ---- Health ----

fn health_view(health: RwSignal<Vec<Health>>) -> impl IntoView {
    view! {
        <h2>"Component health"</h2>
        <div class="grid">
            {move || health.get().into_iter().map(|h| view! {
                <div class="tile">
                    <div class="kv"><span class="dot ok"></span><h3 style="margin:0">{h.component}</h3></div>
                    <p class="muted" style="margin-top:8px">"last seen: " {est_opt(&h.last_seen)}</p>
                </div>
            }).collect_view()}
        </div>
    }
}

// ---- Runs ----

fn runs_view(
    runs: RwSignal<Vec<Run>>,
    meta: RwSignal<Meta>,
    status: RwSignal<String>,
) -> impl IntoView {
    view! {
        <h2>"Agent runs"</h2>
        <p class="hint">
            "Every monitoring sweep and triage invocation, newest first — success/fail, when (ET), and a link to the full logs. Coding-agent runs live in "
            <a href=format!("{REPO_URL}/actions/workflows/{CODING_WORKFLOW}") target="_blank" rel="noopener">"GitHub Actions ↗"</a>"."
        </p>
        <div class="run-strip">
            <span class="spacer"></span>
            <button class="btn" on:click=move |_| run_monitoring(status, runs)>"Run monitoring sweep now"</button>
        </div>
        <div class="card">
            <table>
                <thead><tr><th>"agent"</th><th>"status"</th><th>"trigger"</th><th>"when"</th><th>"summary"</th><th>"logs"</th></tr></thead>
                <tbody>
                    {move || {
                        let project = meta.get().project_id;
                        let items = runs.get();
                        if items.is_empty() {
                            return view! { <tr><td colspan="6" class="muted" style="padding:18px">"No runs yet. Trigger a sweep or simulate an issue."</td></tr> }.into_any();
                        }
                        items.into_iter().map(|r| {
                            let detail = if r.status == "error" { r.error.clone() } else if !r.summary.is_empty() { r.summary.clone() } else { r.detail.clone() };
                            let logs = logs_url(&project, &agent_service(&r.agent));
                            view! {
                                <tr>
                                    <td><code>{r.agent.clone()}</code></td>
                                    <td><span class=run_badge(&r.status)>{r.status.clone()}</span></td>
                                    <td class="muted">{r.trigger.clone()}</td>
                                    <td class="muted nowrap">{est_opt(&r.finished_at)}</td>
                                    <td class="muted reason">{detail}</td>
                                    <td><a class="tlink" href=logs target="_blank" rel="noopener">"logs ↗"</a></td>
                                </tr>
                            }
                        }).collect_view().into_any()
                    }}
                </tbody>
            </table>
        </div>
    }
}

// ---- Simulate ----

fn simulate_view(
    count: RwSignal<u32>,
    status: RwSignal<String>,
    tickets: RwSignal<Vec<Ticket>>,
    events: RwSignal<Vec<LedgerEvent>>,
    known: RwSignal<Vec<KnownIssue>>,
    runs: RwSignal<Vec<Run>>,
) -> impl IntoView {
    let reload = move || {
        load::<Ticket>("/api/tickets", tickets);
        load::<LedgerEvent>("/api/events", events);
        load::<KnownIssue>("/api/known-issues", known);
        load::<Run>("/api/runs", runs);
    };
    let do_reset = move |scope: &'static str| {
        post(
            format!("/api/admin/reset?scope={scope}"),
            None,
            status,
            reload,
        );
    };

    view! {
        <h2>"Simulate a production issue"</h2>
        <p class="hint">"Injects logs into the synthetic app. The monitoring sweep runs on a schedule; give it ~1 min (or hit Run sweep now on Events), then check Events / Tickets / Runs."</p>
        <div class="row" style="margin-bottom:14px">
            <span class="muted">"count"</span>
            <input type="number" min="1" prop:value=move || count.get().to_string()
                on:input=move |ev| count.set(event_target_value(&ev).parse().unwrap_or(1)) />
        </div>
        <div class="grid">
            {SCENARIOS.iter().map(|(id, label)| {
                let id = *id;
                view! {
                    <div class="tile">
                        <h3>{*label}</h3>
                        <p><code>{id}</code></p>
                        <button class="btn primary" on:click=move |_| {
                            post("/api/simulate".into(), Some(json!({"scenario": id, "count": count.get()})), status, reload);
                        }>"Inject"</button>
                    </div>
                }
            }).collect_view()}
        </div>

        <h2 style="margin-top:28px">"Reset state (clean slate)"</h2>
        <p class="hint">"Wipe Firestore state and/or purge the queues before a lecture."</p>
        <div class="row">
            <button class="btn danger" on:click=move |_| do_reset("all")>"Reset everything"</button>
            <button class="btn" on:click=move |_| do_reset("tickets")>"Tickets only"</button>
            <button class="btn" on:click=move |_| do_reset("events")>"Events only"</button>
            <button class="btn" on:click=move |_| do_reset("known_issues")>"Known issues only"</button>
            <button class="btn" on:click=move |_| do_reset("runs")>"Runs only"</button>
            <button class="btn" on:click=move |_| do_reset("queue")>"Purge queues"</button>
        </div>
    }
}

// ---- Ops ----

fn state_help(s: &str) -> &'static str {
    match s {
        "CONDITION_SUCCEEDED" | "Ready" | "Active" => "Deployed and serving traffic.",
        "CONDITION_RECONCILING" | "Reconciling" => "A new revision is rolling out.",
        "CONDITION_FAILED" => "The latest revision failed to deploy.",
        _ => "Cloud Run terminal condition of the latest revision.",
    }
}

fn ops_view(ops: RwSignal<Vec<OpsSvc>>, status: RwSignal<String>) -> impl IntoView {
    view! {
        <h2>"Service fleet"</h2>
        <p class="hint">
            "Min instances is the warm floor. Scaling a service to 0 removes the warm instance to save cost — it still cold-starts on the next request, so it is "
            <b>"not switched off"</b>"."
        </p>
        <p class="hint">
            "To idle everything before a demo, scale "<code>"a3l-synthetic-shop"</code>
            " to 0 — it is the only always-on service (the log flood). The console wakes itself the moment you open it."
        </p>
        <div class="card">
            <table>
                <thead><tr>
                    <th>"service"</th>
                    <th class="help" title="Cloud Run terminal condition of the latest revision (e.g. CONDITION_SUCCEEDED = deployed and serving traffic).">"state"</th>
                    <th>"min instances"</th>
                    <th>"action"</th>
                </tr></thead>
                <tbody>
                    {move || ops.get().into_iter().map(|s| {
                        let name0 = s.service.clone();
                        let name1 = s.service.clone();
                        let is_console = s.service.ends_with("ticket-backend");
                        let svc_cls = if is_console { "svc-console" } else { "" };
                        let dot = if s.state == "CONDITION_SUCCEEDED" || s.state == "Ready" { "dot ok" } else { "dot warn" };
                        view! {
                            <tr>
                                <td class=svc_cls>
                                    <span class=dot></span>" "<code>{s.service.clone()}</code>
                                    {is_console.then(|| view! { <span class="tag">"this console"</span> })}
                                </td>
                                <td class="muted help" title=state_help(&s.state)>{s.state.clone()}</td>
                                <td>{s.min_instances}</td>
                                <td class="row">
                                    <button class="btn" on:click=move |_| scale(name0.clone(), 0, ops, status)>"Scale to 0"</button>
                                    <button class="btn" on:click=move |_| scale(name1.clone(), 1, ops, status)>"Scale to 1"</button>
                                </td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}

fn scale(service: String, min: i64, ops: RwSignal<Vec<OpsSvc>>, status: RwSignal<String>) {
    post(
        "/api/ops/scale".into(),
        Some(json!({ "service": service, "min_instances": min })),
        status,
        move || load::<OpsSvc>("/api/ops", ops),
    );
}
