use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Instant;
use tokio::sync::mpsc;

// ── CLI output format ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    Text,
    Json,
    Csv,
}

use crate::connector::{self, ConnectorConfig, Row, load_rows_from_json};

// ── TOML schema ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct Campaign {
    pub campaign: Meta,
    #[serde(default)]
    pub params: Vec<CampaignParam>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub env_file: Option<String>,
    #[serde(default)]
    pub connectors: Vec<ConnectorConfig>,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub outputs: Vec<OutputConfig>,
    #[serde(default)]
    pub continue_on_error: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CampaignParam {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OutputConfig {
    pub from_step: String,
    pub path: String,
    #[serde(default)]
    pub select: Option<String>,
    #[serde(default)]
    pub include_vars: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Meta {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StepCondition {
    pub var: String,
    #[serde(default)]
    pub eq: Option<String>,
    #[serde(default)]
    pub ne: Option<String>,
    #[serde(default)]
    pub exists: Option<bool>,
    #[serde(default)]
    pub lt: Option<f64>,
    #[serde(default)]
    pub lte: Option<f64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AccumulateConfig {
    pub var: String,
    pub from: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Step {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default = "default_http")]
    pub kind: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub wait_ms: u64,
    #[serde(default)]
    pub env: Option<String>,
    #[serde(default)]
    pub extract: HashMap<String, String>,
    #[serde(default)]
    pub assert: Vec<Assertion>,
    #[serde(default)]
    pub transforms: Vec<Transform>,
    #[serde(default)]
    pub continue_on_error: Option<bool>,
    #[serde(default)]
    pub foreach: Option<String>,
    #[serde(default)]
    pub when: Option<StepCondition>,
    // FileLoader fields (kind = "file")
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub file_output: Option<String>,
    #[serde(default)]
    pub file_encoding: Option<String>, // "base64" | "text" | "hex"  (default: "base64")
    // Multipart form-data parts (HTTP step)
    #[serde(default)]
    pub multipart_parts: Vec<MultipartPart>,
    // GraphQL step fields (kind = "graphql")
    #[serde(default)]
    pub graphql_query: Option<String>,
    #[serde(default)]
    pub graphql_variables: HashMap<String, String>,
    // Loop step fields (kind = "loop")
    #[serde(default)]
    pub until: Option<StepCondition>,
    #[serde(default)]
    pub accumulate: Option<AccumulateConfig>,
    // Poll step fields (kind = "poll")
    #[serde(default = "default_poll_interval")]
    pub interval_ms: u64,
    #[serde(default = "default_poll_timeout")]
    pub timeout_secs: u64,
    // Search step fields (kind = "search")
    #[serde(default)]
    pub search: Option<SearchConfig>,
    // Set step fields (kind = "set")
    #[serde(default)]
    pub vars: HashMap<String, String>,
    // JQ step fields (kind = "jq")
    #[serde(default)]
    pub jq_input: Option<String>,
    #[serde(default)]
    pub jq_expression: Option<String>,
    #[serde(default)]
    pub jq_output: Option<String>,
    #[serde(default)]
    pub jq_raw: bool,
    #[serde(default)]
    pub jq_args: HashMap<String, String>,
    // Parallel step fields (kind = "parallel")
    #[serde(default)]
    pub parallel_steps: Vec<String>,
    // Notify step fields (kind = "notify")
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SearchConfig {
    pub input: String,
    pub path: String,
    #[serde(rename = "match")]
    pub pattern: String,
    pub output: String,
    #[serde(default)]
    pub first_only: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MultipartPart {
    pub name: String,
    pub value: String, // plain text (supports {{VAR}}), or "@/path/to/file" for binary
    #[serde(default)]
    pub content_type: Option<String>,
}

fn default_http() -> String { "http".into() }
fn default_poll_interval() -> u64 { 1000 }
fn default_poll_timeout()  -> u64 { 60   }

#[derive(Debug, Deserialize, Clone)]
pub struct Assertion {
    pub on: String,
    #[serde(default)]
    pub eq: Option<Value>,
    #[serde(default)]
    pub ne: Option<Value>,
    #[serde(default)]
    pub lt: Option<f64>,
    #[serde(default)]
    pub lte: Option<f64>,
    #[serde(default)]
    pub gt: Option<f64>,
    #[serde(default)]
    pub gte: Option<f64>,
    #[serde(rename = "in", default)]
    pub in_: Vec<Value>,
    #[serde(default)]
    pub exists: Option<bool>,
    #[serde(default)]
    pub contains: Option<String>,
    #[serde(default)]
    pub matches: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Transform {
    #[serde(rename = "type")]
    pub kind: String,
    pub input: String,
    pub output: String,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default = "default_group")]
    pub group: usize,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub delimiter: Option<String>,
    #[serde(default)]
    pub index: usize,
}

fn default_group() -> usize { 1 }

// ── result types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct StepResult {
    pub name: String,
    pub method: String,
    pub url: String,
    pub status: Option<u16>,
    pub duration_ms: u64,
    pub success: bool,
    pub skipped: bool,
    pub non_blocking: bool,
    pub error: Option<String>,
    pub extracted: HashMap<String, String>,
    /// All assertion results: (description, passed).
    pub assertion_results: Vec<(String, bool)>,
    /// Raw JSON body — used by output connectors; None for transform/seed steps.
    pub body_json: Option<Value>,
    pub graphql: bool,
    /// Resolved request snapshot for TUI "Load in Request tab" feature.
    pub request_headers: Vec<(String, String)>,
    pub request_body: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IterationResult {
    pub row_index: Option<usize>,
    pub row_vars: Row,
    pub steps: Vec<StepResult>,
}

impl IterationResult {
    pub fn ok_count(&self)   -> usize { self.steps.iter().filter(|s| s.success).count() }
    pub fn fail_count(&self) -> usize { self.steps.iter().filter(|s| !s.success).count() }
    pub fn total_ms(&self)   -> u64   { self.steps.iter().map(|s| s.duration_ms).sum() }
    pub fn success(&self)    -> bool  { self.steps.iter().all(|s| s.success) }
}

// ── campaign events (streaming) ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum CampaignEvent {
    IterationStarted { idx: usize, total: usize, row_summary: String },
    StepStarted { name: String },
    StepRetry { name: String, attempt: usize, max: usize, delay_secs: u64 },
    StepPoll  { name: String, attempt: usize, elapsed_secs: u64 },
    StepDone(StepResult),
    Finished(Vec<IterationResult>),
    Warning(String),
    Error(String),
}

// ── run state (used by TUI) ───────────────────────────────────────────────────

#[derive(Debug)]
pub enum CampaignRunState {
    Idle,
    Running {
        name: String,
        step_results: Vec<StepResult>,
        current_step: Option<String>,
    },
    Done {
        name: String,
        results: Vec<IterationResult>,
    },
}

// ── internal HTTP outcome ─────────────────────────────────────────────────────

struct HttpOutcome {
    status: u16,
    body_value: Option<Value>,
    resp_headers: HashMap<String, String>,
    extracted: HashMap<String, String>,
}

// ── loader ────────────────────────────────────────────────────────────────────

pub fn load(path: &str) -> Result<Campaign> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read campaign file '{}'", path))?;
    toml::from_str(&content)
        .with_context(|| format!("invalid TOML in '{}'", path))
}

// ── single-step preview (used by the builder's Run Step feature) ──────────────

// Run preceding steps to accumulate extracted variables, then run the target step.
pub async fn run_step_preview_with_context(
    steps: Vec<Step>,
    target_idx: usize,
    base_env: HashMap<String, String>,
) -> StepResult {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let (tx, _rx) = mpsc::unbounded_channel::<CampaignEvent>();
    let mut env = base_env;
    for step in steps.iter().take(target_idx) {
        if step.kind == "loop" {
            let (result, new_vars) = run_loop_step(&client, step, &env, true, &tx).await;
            env.extend(new_vars);
            let _ = result;
        } else {
            let result = run_single_step(&client, step, &env, true).await;
            env.extend(result.extracted);
        }
    }
    if let Some(step) = steps.get(target_idx) {
        if step.kind == "loop" {
            let (result, _) = run_loop_step(&client, step, &env, false, &tx).await;
            result
        } else {
            run_single_step(&client, step, &env, false).await
        }
    } else {
        StepResult {
            name: String::new(),
            method: String::new(),
            url: String::new(),
            status: None,
            duration_ms: 0,
            success: false,
            skipped: false,
            non_blocking: false,
            error: Some("step not found".into()),
            extracted: HashMap::new(),
            assertion_results: vec![],
            body_json: None,
            graphql: false,
            request_headers: vec![],
            request_body: None,
        }
    }
}

// ── streaming runner (core) ───────────────────────────────────────────────────

pub async fn run_streaming(campaign: Campaign, tx: mpsc::UnboundedSender<CampaignEvent>, overrides: HashMap<String, String>, only: Vec<String>, retry: u32) {
    let mut base_env: HashMap<String, String> = if let Some(ref name) = campaign.env_file {
        match crate::storage::load_env_by_name(name) {
            Ok(stored) => stored.vars,
            Err(e) => {
                let _ = tx.send(CampaignEvent::Warning(
                    format!("could not load env_file '{}': {}", name, e)
                ));
                HashMap::new()
            }
        }
    } else {
        HashMap::new()
    };
    base_env.extend(campaign.env.clone());
    for p in &campaign.params {
        if let Some(ref default) = p.default {
            base_env.entry(p.name.clone()).or_insert_with(|| default.clone());
        }
    }
    base_env.extend(overrides);

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(CampaignEvent::Error(e.to_string()));
            return;
        }
    };

    // Separate seed steps (kind = "seed") from iteration steps.
    let iteration_steps: Vec<Step> = campaign.steps.iter()
        .filter(|s| s.kind != "seed")
        .cloned()
        .collect();

    let rows: Vec<Row> = match campaign.connectors.as_slice() {
        [] => vec![HashMap::new()],
        [single] => {
            if let Some(ref seed_name) = single.from_step {
                // Find the seed step and run it once to obtain the JSON source.
                let seed = match campaign.steps.iter().find(|s| &s.name == seed_name) {
                    Some(s) => s,
                    None => {
                        let _ = tx.send(CampaignEvent::Error(
                            format!("seed step '{}' not found in steps", seed_name)
                        ));
                        return;
                    }
                };
                let url  = resolve(&seed.url, &base_env);
                let body = seed.body.as_deref().map(|b| resolve(b, &base_env));
                match execute_step(&client, seed, &url, body.as_deref(), &base_env).await {
                    Ok(http) => {
                        let json_str = http.body_value
                            .map(|v| v.to_string())
                            .unwrap_or_default();
                        match load_rows_from_json(&json_str, single.select.as_deref()) {
                            Ok(rows) => rows,
                            Err(e) => {
                                let _ = tx.send(CampaignEvent::Error(e.to_string()));
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(CampaignEvent::Error(
                            format!("seed step '{}' failed: {}", seed_name, e)
                        ));
                        return;
                    }
                }
            } else {
                match connector::load_rows(single) {
                    Ok(rows) => rows,
                    Err(e) => {
                        let _ = tx.send(CampaignEvent::Error(e.to_string()));
                        return;
                    }
                }
            }
        }
        _ => {
            let _ = tx.send(CampaignEvent::Error(
                "multiple connectors per campaign are not yet supported".into()
            ));
            return;
        }
    };

    let total = rows.len();
    let multi = total > 1;

    let mut all: Vec<IterationResult> = Vec::new();

    for (idx, row_vars) in rows.into_iter().enumerate() {
        let mut env = base_env.clone();
        env.extend(row_vars.clone());

        if multi {
            let row_summary = row_vars.iter()
                .map(|(k, v)| format!("{}={}", k, truncate(v, 20)))
                .collect::<Vec<_>>()
                .join("  ");
            let _ = tx.send(CampaignEvent::IterationStarted { idx, total, row_summary });
        }

        let step_results = run_steps_streaming(&client, &iteration_steps, campaign.continue_on_error, &env, &tx, &only, retry).await;

        all.push(IterationResult {
            row_index: if multi { Some(idx) } else { None },
            row_vars,
            steps: step_results,
        });
    }

    // Write output connectors: one JSON file per [[outputs]] entry.
    for output in &campaign.outputs {
        // Match both exact step name and foreach sub-step names ("name [i/n]").
        let foreach_prefix = format!("{} [", output.from_step);
        let step_matches = |name: &str| {
            name == output.from_step || name.starts_with(&foreach_prefix)
        };

        let bodies: Vec<Value> = all.iter()
            .flat_map(|iter| {
                // Accumulate vars (row_vars + each step's extracted) in order.
                let mut accumulated: HashMap<String, String> = iter.row_vars.clone();
                let mut sub_bodies: Vec<Value> = Vec::new();
                for s in &iter.steps {
                    accumulated.extend(s.extracted.clone());
                    if !step_matches(&s.name) || !s.success { continue; }
                    let body = match s.body_json.clone() {
                        Some(b) => b,
                        None => continue,
                    };
                    let selected = if let Some(ref sel) = output.select {
                        if !sel.is_empty() {
                            match extract_value_at(&body, sel) {
                                Some(v) => v,
                                None => continue,
                            }
                        } else { body }
                    } else { body };

                    if output.include_vars.is_empty() {
                        sub_bodies.push(selected);
                    } else {
                        let mut map = match selected {
                            Value::Object(m) => m,
                            other => {
                                let mut m = serde_json::Map::new();
                                m.insert("response".to_string(), other);
                                m
                            }
                        };
                        for var in &output.include_vars {
                            if let Some(val) = accumulated.get(var) {
                                map.insert(var.clone(), Value::String(val.clone()));
                            }
                        }
                        sub_bodies.push(Value::Object(map));
                    }
                }
                sub_bodies
            })
            .collect();

        if bodies.is_empty() {
            let _ = tx.send(CampaignEvent::Warning(
                format!("output '{}': no successful results from step '{}'", output.path, output.from_step)
            ));
            continue;
        }

        let json_out = Value::Array(bodies);
        if let Some(parent) = std::path::Path::new(&output.path).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        match serde_json::to_string_pretty(&json_out) {
            Ok(s) => {
                if let Err(e) = std::fs::write(&output.path, s) {
                    let _ = tx.send(CampaignEvent::Warning(
                        format!("output '{}': write failed: {}", output.path, e)
                    ));
                }
            }
            Err(e) => {
                let _ = tx.send(CampaignEvent::Warning(
                    format!("output '{}': serialization failed: {}", output.path, e)
                ));
            }
        }
    }

    let _ = tx.send(CampaignEvent::Finished(all));
}

// ── CLI runner (consumes streaming events) ────────────────────────────────────

pub async fn run(
    campaign: &Campaign,
    silent: bool,
    overrides: HashMap<String, String>,
    only: Vec<String>,
    format: OutputFormat,
    retry: u32,
) -> Result<()> {
    let text = format == OutputFormat::Text;
    macro_rules! out { ($($arg:tt)*) => { if !silent && text { println!($($arg)*); } } }

    if !only.is_empty() {
        out!("Filter   : {}", only.join(", "));
    }
    if retry > 0 {
        out!("Retry    : up to {} attempt(s), exponential backoff", retry);
    }
    out!("Campaign : {}", campaign.campaign.name);
    if !campaign.campaign.description.is_empty() {
        out!("           {}", campaign.campaign.description);
    }
    if !campaign.params.is_empty() {
        out!("Params   :");
        for p in &campaign.params {
            let value = overrides.get(&p.name)
                .or(p.default.as_ref())
                .map(|s| s.as_str())
                .unwrap_or("(not set)");
            if p.description.is_empty() {
                out!("  {} = {}", p.name, value);
            } else {
                out!("  {} = {}  ({})", p.name, value, p.description);
            }
        }
        out!();
    }
    if let Some(ref name) = campaign.env_file {
        out!("Env file  : {}", name);
    }
    if let Some(conn) = campaign.connectors.first() {
        if let Some(ref from) = conn.from_step {
            out!("Connector : json — seed step '{}'\n", from);
        } else {
            out!("Connector : {}\n", conn.path);
        }
    } else {
        out!();
    }

    let t0 = Instant::now();
    let (tx, mut rx) = mpsc::unbounded_channel::<CampaignEvent>();
    let owned = campaign.clone();
    tokio::spawn(async move { run_streaming(owned, tx, overrides, only, retry).await; });

    while let Some(event) = rx.recv().await {
        match event {
            CampaignEvent::IterationStarted { idx, total, row_summary } => {
                out!("── Row {}/{} — {}", idx + 1, total, row_summary);
            }
            CampaignEvent::StepStarted { .. } => {}
            CampaignEvent::StepRetry { name, attempt, max, delay_secs } => {
                if !silent && text {
                    eprintln!("  ⟳ retry {}/{} — {} — waiting {}s...", attempt, max, name, delay_secs);
                }
            }
            CampaignEvent::StepPoll { name, attempt, elapsed_secs } => {
                if !silent && text {
                    eprintln!("  ⟳ poll #{} — {} — {}s elapsed", attempt, name, elapsed_secs);
                }
            }
            CampaignEvent::Warning(msg) => {
                if !silent && text { eprintln!("  Warning: {}", msg); }
            }
            CampaignEvent::StepDone(sr) => {
                if !silent && text { print_step_result(&sr); }
            }
            CampaignEvent::Finished(results) => {
                let duration_ms = t0.elapsed().as_millis() as u64;
                match format {
                    OutputFormat::Text => {
                        if !silent {
                            print_report(campaign, &results);
                            for o in &campaign.outputs {
                                println!("  → output written: {}", o.path);
                            }
                        }
                    }
                    OutputFormat::Json => {
                        let json = build_json_report(campaign, &results, duration_ms);
                        println!("{}", serde_json::to_string_pretty(&json).unwrap_or_default());
                    }
                    OutputFormat::Csv => {
                        print_csv_report(&results);
                    }
                }
                let total_fail: usize = results.iter().map(|r| r.fail_count()).sum();
                if total_fail > 0 { std::process::exit(1); }
                break;
            }
            CampaignEvent::Error(e) => {
                if format == OutputFormat::Json {
                    let err = serde_json::json!({"error": e});
                    eprintln!("{}", serde_json::to_string_pretty(&err).unwrap_or_default());
                } else if !silent {
                    eprintln!("Campaign error: {}", e);
                }
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

fn build_json_report(campaign: &Campaign, results: &[IterationResult], duration_ms: u64) -> Value {
    let all_ok = results.iter().all(|r| r.success());
    let multi = results.len() > 1;

    let make_step = |s: &StepResult| {
        let mut obj = serde_json::Map::new();
        obj.insert("name".into(), Value::String(s.name.clone()));
        obj.insert("method".into(), Value::String(s.method.clone()));
        obj.insert("url".into(), Value::String(s.url.clone()));
        obj.insert("status".into(), s.status.map(|c| Value::Number(c.into())).unwrap_or(Value::Null));
        obj.insert("success".into(), Value::Bool(s.success));
        obj.insert("skipped".into(), Value::Bool(s.skipped));
        obj.insert("elapsed_ms".into(), Value::Number(s.duration_ms.into()));
        obj.insert("extracted".into(), Value::Object(
            s.extracted.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect()
        ));
        obj.insert("assertions".into(), Value::Array(
            s.assertion_results.iter().map(|(desc, passed)| {
                serde_json::json!({"description": desc, "passed": passed})
            }).collect()
        ));
        obj.insert("error".into(), s.error.as_ref().map(|e| Value::String(e.clone())).unwrap_or(Value::Null));
        Value::Object(obj)
    };

    if multi {
        let iterations: Vec<Value> = results.iter().map(|iter| {
            serde_json::json!({
                "index": iter.row_index,
                "steps": iter.steps.iter().map(make_step).collect::<Vec<_>>()
            })
        }).collect();
        serde_json::json!({
            "campaign": campaign.campaign.name,
            "success": all_ok,
            "duration_ms": duration_ms,
            "iterations": iterations
        })
    } else {
        let steps: Vec<Value> = results.iter()
            .flat_map(|r| r.steps.iter().map(make_step))
            .collect();
        serde_json::json!({
            "campaign": campaign.campaign.name,
            "success": all_ok,
            "duration_ms": duration_ms,
            "steps": steps
        })
    }
}

fn print_csv_report(results: &[IterationResult]) {
    println!("iteration,name,method,url,status,success,skipped,elapsed_ms,extracted,error");
    for iter in results {
        let idx = iter.row_index.map(|i| i.to_string()).unwrap_or_else(|| "0".into());
        for s in &iter.steps {
            let status = s.status.map(|c| c.to_string()).unwrap_or_default();
            let extracted = if s.extracted.is_empty() {
                String::new()
            } else {
                serde_json::to_string(&s.extracted).unwrap_or_default()
            };
            let error = s.error.as_deref().unwrap_or("");
            println!(
                "{},{},{},{},{},{},{},{},{},{}",
                idx,
                csv_escape(&s.name),
                s.method,
                csv_escape(&s.url),
                status,
                s.success,
                s.skipped,
                s.duration_ms,
                csv_escape(&extracted),
                csv_escape(error),
            );
        }
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn is_graphql_body(body: &str) -> bool {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("query").cloned())
        .is_some()
}

// ── single step executor (no env resolution, no streaming) ───────────────────

async fn run_single_step(
    client: &reqwest::Client,
    step: &Step,
    effective: &HashMap<String, String>,
    effective_coe: bool,
) -> StepResult {
    if step.kind == "comment" {
        return StepResult {
            name: step.name.clone(),
            method: "SKIP".into(),
            url: String::new(),
            status: None,
            duration_ms: 0,
            success: true,
            skipped: true,
            non_blocking: true,
            error: None,
            extracted: HashMap::new(),
            assertion_results: vec![],
            body_json: None,
            graphql: false,
            request_headers: vec![],
            request_body: None,
        };
    }

    if step.kind == "pause" {
        let t0 = Instant::now();
        tokio::time::sleep(std::time::Duration::from_millis(step.wait_ms)).await;
        return StepResult {
            name: step.name.clone(),
            method: "WAIT".into(),
            url: String::new(),
            status: None,
            duration_ms: t0.elapsed().as_millis() as u64,
            success: true,
            skipped: false,
            non_blocking: effective_coe,
            error: None,
            extracted: HashMap::new(),
            assertion_results: vec![],
            body_json: None,
            graphql: false,
            request_headers: vec![],
            request_body: None,
        };
    }

    if step.kind == "set" {
        let extracted: HashMap<String, String> = step.vars.iter()
            .map(|(k, v)| (k.clone(), resolve(v, effective)))
            .collect();
        return StepResult {
            name:              step.name.clone(),
            method:            "SET".into(),
            url:               String::new(),
            status:            None,
            duration_ms:       0,
            success:           true,
            skipped:           false,
            non_blocking:      effective_coe,
            error:             None,
            extracted,
            assertion_results: vec![],
            body_json:         None,
            graphql:           false,
            request_headers:   vec![],
            request_body:      None,
        };
    }

    if step.kind == "jq" {
        let t0 = Instant::now();
        return match run_jq_step(step, effective).await {
            Ok(extracted) => StepResult {
                name:              step.name.clone(),
                method:            "JQ  ".into(),
                url:               String::new(),
                status:            None,
                duration_ms:       t0.elapsed().as_millis() as u64,
                success:           true,
                skipped:           false,
                non_blocking:      effective_coe,
                error:             None,
                extracted,
                assertion_results: vec![],
                body_json:         None,
                graphql:           false,
                request_headers:   vec![],
                request_body:      None,
            },
            Err(e) => StepResult {
                name:              step.name.clone(),
                method:            "JQ  ".into(),
                url:               String::new(),
                status:            None,
                duration_ms:       t0.elapsed().as_millis() as u64,
                success:           false,
                skipped:           false,
                non_blocking:      effective_coe,
                error:             Some(e.to_string()),
                extracted:         HashMap::new(),
                assertion_results: vec![],
                body_json:         None,
                graphql:           false,
                request_headers:   vec![],
                request_body:      None,
            },
        };
    }

    if step.kind == "transform" {
        let t0 = Instant::now();
        return match run_transform_step(step, effective) {
            Ok(produced) => StepResult {
                name: step.name.clone(),
                method: "TRSF".into(),
                url: String::new(),
                status: None,
                duration_ms: t0.elapsed().as_millis() as u64,
                success: true,
                skipped: false,
                non_blocking: effective_coe,
                error: None,
                extracted: produced,
                assertion_results: vec![],
                body_json: None,
                graphql: false,
                request_headers: vec![],
                request_body: None,
            },
            Err(e) => StepResult {
                name: step.name.clone(),
                method: "TRSF".into(),
                url: String::new(),
                status: None,
                duration_ms: t0.elapsed().as_millis() as u64,
                success: false,
                skipped: false,
                non_blocking: effective_coe,
                error: Some(e.to_string()),
                extracted: HashMap::new(),
                assertion_results: vec![],
                body_json: None,
                graphql: false,
                request_headers: vec![],
                request_body: None,
            },
        };
    }

    if step.kind == "search" {
        let t0 = Instant::now();
        return match run_search_step(step, effective) {
            Ok(extracted) => StepResult {
                name: step.name.clone(),
                method: "SRCH".into(),
                url: String::new(),
                status: None,
                duration_ms: t0.elapsed().as_millis() as u64,
                success: true,
                skipped: false,
                non_blocking: effective_coe,
                error: None,
                extracted,
                assertion_results: vec![],
                body_json: None,
                graphql: false,
                request_headers: vec![],
                request_body: None,
            },
            Err(e) => StepResult {
                name: step.name.clone(),
                method: "SRCH".into(),
                url: String::new(),
                status: None,
                duration_ms: t0.elapsed().as_millis() as u64,
                success: false,
                skipped: false,
                non_blocking: effective_coe,
                error: Some(e.to_string()),
                extracted: HashMap::new(),
                assertion_results: vec![],
                body_json: None,
                graphql: false,
                request_headers: vec![],
                request_body: None,
            },
        };
    }

    if step.kind == "file" {
        let t0 = Instant::now();
        let path    = resolve(step.file_path.as_deref().unwrap_or(""), effective);
        let output  = resolve(step.file_output.as_deref().unwrap_or("FILE_DATA"), effective);
        let encoding = step.file_encoding.as_deref().unwrap_or("base64");
        return match run_file_step(&path, encoding) {
            Ok(content) => {
                let mut extracted = HashMap::new();
                extracted.insert(output, content);
                StepResult {
                    name: step.name.clone(),
                    method: "FILE".into(),
                    url: path,
                    status: None,
                    duration_ms: t0.elapsed().as_millis() as u64,
                    success: true,
                    skipped: false,
                    non_blocking: effective_coe,
                    error: None,
                    extracted,
                    assertion_results: vec![],
                    body_json: None,
                    graphql: false,
                    request_headers: vec![],
                    request_body: None,
                }
            }
            Err(e) => StepResult {
                name: step.name.clone(),
                method: "FILE".into(),
                url: path,
                status: None,
                duration_ms: t0.elapsed().as_millis() as u64,
                success: false,
                skipped: false,
                non_blocking: effective_coe,
                error: Some(e.to_string()),
                extracted: HashMap::new(),
                assertion_results: vec![],
                body_json: None,
                graphql: false,
                request_headers: vec![],
                request_body: None,
            },
        };
    }

    // GraphQL step — build body from query + variables, then send as HTTP POST.
    if step.kind == "graphql" {
        let url = resolve(&step.url, effective);
        let query = resolve(step.graphql_query.as_deref().unwrap_or(""), effective);
        let vars_obj: serde_json::Map<String, serde_json::Value> = step.graphql_variables.iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(resolve(v, effective))))
            .collect();
        let mut body_map = serde_json::Map::new();
        body_map.insert("query".into(), serde_json::Value::String(query));
        if !vars_obj.is_empty() {
            body_map.insert("variables".into(), serde_json::Value::Object(vars_obj));
        }
        let body_str = serde_json::to_string(&serde_json::Value::Object(body_map)).unwrap_or_default();
        let mut gql_step = step.clone();
        gql_step.method = if step.method.is_empty() { "POST".into() } else { step.method.clone() };
        gql_step.headers.entry("Content-Type".into()).or_insert_with(|| "application/json".into());
        let request_headers: Vec<(String, String)> = gql_step.headers.iter()
            .map(|(k, v)| (k.clone(), resolve(v, effective)))
            .collect();
        let t0 = Instant::now();
        match execute_step(client, &gql_step, &url, Some(&body_str), effective).await {
            Ok(http) => {
                let duration_ms = t0.elapsed().as_millis() as u64;
                let assertion_results = if step.assert.is_empty() {
                    vec![]
                } else {
                    evaluate_assertions(
                        &step.assert, http.status,
                        http.body_value.as_ref(), &http.resp_headers,
                        duration_ms, effective,
                    )
                };
                let http_ok   = http.status < 400;
                let assert_ok = assertion_results.iter().all(|(_, ok)| *ok);
                let success   = http_ok && assert_ok;
                let fail_count = assertion_results.iter().filter(|(_, ok)| !ok).count();
                let error = if !http_ok {
                    Some(format!("HTTP {}", http.status))
                } else if !assert_ok {
                    Some(format!("{} assertion(s) failed", fail_count))
                } else {
                    None
                };
                return StepResult {
                    name: step.name.clone(),
                    method: "POST".into(),
                    url,
                    status: Some(http.status),
                    duration_ms,
                    success,
                    skipped: false,
                    non_blocking: effective_coe,
                    error,
                    extracted: if success { http.extracted } else { HashMap::new() },
                    assertion_results,
                    body_json: http.body_value,
                    graphql: true,
                    request_headers,
                    request_body: Some(body_str),
                };
            }
            Err(e) => {
                return StepResult {
                    name: step.name.clone(),
                    method: "POST".into(),
                    url,
                    status: None,
                    duration_ms: t0.elapsed().as_millis() as u64,
                    success: false,
                    skipped: false,
                    non_blocking: effective_coe,
                    error: Some(e.to_string()),
                    extracted: HashMap::new(),
                    assertion_results: vec![],
                    body_json: None,
                    graphql: true,
                    request_headers,
                    request_body: Some(body_str),
                };
            }
        }
    }

    // Notify step — lightweight webhook POST; reuses HTTP machinery with message as body.
    if step.kind == "notify" {
        let mut http_step = step.clone();
        http_step.kind   = "http".into();
        if http_step.method.is_empty() { http_step.method = "POST".into(); }
        if let Some(ref msg) = step.message {
            http_step.body = Some(resolve(msg, effective));
        }
        http_step.headers.entry("Content-Type".to_string())
            .or_insert_with(|| "application/json".to_string());
        let url = resolve(&http_step.url, effective);
        let body = http_step.body.as_deref().map(|b| resolve(b, effective));
        let request_headers: Vec<(String, String)> = http_step.headers.iter()
            .map(|(k, v)| (k.clone(), resolve(v, effective)))
            .collect();
        let request_body = body.clone();
        let t0 = Instant::now();
        return match execute_step(client, &http_step, &url, body.as_deref(), effective).await {
            Ok(http) => {
                let success = http.status < 400;
                StepResult {
                    name:            step.name.clone(),
                    method:          "NTFY".into(),
                    url,
                    status:          Some(http.status),
                    duration_ms:     t0.elapsed().as_millis() as u64,
                    success,
                    skipped:         false,
                    non_blocking:    effective_coe,
                    error:           if !success { Some(format!("HTTP {}", http.status)) } else { None },
                    extracted:       HashMap::new(),
                    assertion_results: vec![],
                    body_json:       None,
                    graphql:         false,
                    request_headers,
                    request_body,
                }
            }
            Err(e) => StepResult {
                name:            step.name.clone(),
                method:          "NTFY".into(),
                url:             resolve(&step.url, effective),
                status:          None,
                duration_ms:     t0.elapsed().as_millis() as u64,
                success:         false,
                skipped:         false,
                non_blocking:    effective_coe,
                error:           Some(e.to_string()),
                extracted:       HashMap::new(),
                assertion_results: vec![],
                body_json:       None,
                graphql:         false,
                request_headers,
                request_body,
            },
        };
    }

    // HTTP step — capture the fully-resolved request snapshot for the TUI "Load in Request" feature.
    let url     = resolve(&step.url, effective);
    let body    = step.body.as_deref().map(|b| resolve(b, effective));
    let graphql = body.as_deref().map(is_graphql_body).unwrap_or(false);
    let request_headers: Vec<(String, String)> = step.headers.iter()
        .map(|(k, v)| (k.clone(), resolve(v, effective)))
        .collect();
    let request_body = body.clone();
    let t0      = Instant::now();

    match execute_step(client, step, &url, body.as_deref(), effective).await {
        Ok(http) => {
            let duration_ms = t0.elapsed().as_millis() as u64;
            let assertion_results = if step.assert.is_empty() {
                vec![]
            } else {
                evaluate_assertions(
                    &step.assert, http.status,
                    http.body_value.as_ref(), &http.resp_headers,
                    duration_ms, effective,
                )
            };
            let http_ok   = http.status < 400;
            let assert_ok = assertion_results.iter().all(|(_, ok)| *ok);
            let success   = http_ok && assert_ok;
            let fail_count = assertion_results.iter().filter(|(_, ok)| !ok).count();
            let error = if !http_ok {
                Some(format!("HTTP {}", http.status))
            } else if !assert_ok {
                Some(format!("{} assertion(s) failed", fail_count))
            } else {
                None
            };
            StepResult {
                name: step.name.clone(),
                method: step.method.clone(),
                url,
                status: Some(http.status),
                duration_ms,
                success,
                skipped: false,
                non_blocking: effective_coe,
                error,
                extracted: if success { http.extracted } else { HashMap::new() },
                assertion_results,
                body_json: http.body_value,
                graphql,
                request_headers,
                request_body,
            }
        }
        Err(e) => StepResult {
            name: step.name.clone(),
            method: step.method.clone(),
            url,
            status: None,
            duration_ms: t0.elapsed().as_millis() as u64,
            success: false,
            skipped: false,
            non_blocking: effective_coe,
            error: Some(e.to_string()),
            extracted: HashMap::new(),
            assertion_results: vec![],
            body_json: None,
            graphql,
            request_headers,
            request_body,
        },
    }
}

// ── streaming step runner ─────────────────────────────────────────────────────

async fn run_steps_streaming(
    client: &reqwest::Client,
    steps: &[Step],
    campaign_coe: bool,
    base_env: &HashMap<String, String>,
    tx: &mpsc::UnboundedSender<CampaignEvent>,
    only: &[String],
    retry: u32,
) -> Vec<StepResult> {
    let mut extracted: HashMap<String, String> = HashMap::new();
    let mut results = Vec::new();

    // Pre-scan: collect step names referenced by any parallel step so they can be skipped
    // in the main loop (they will be launched concurrently by their parent parallel step).
    let parallel_children: std::collections::HashSet<String> = steps.iter()
        .filter(|s| s.kind == "parallel")
        .flat_map(|s| s.parallel_steps.iter().cloned())
        .collect();

    for step in steps {
        let effective_coe = step.continue_on_error.unwrap_or(campaign_coe);
        let mut effective = base_env.clone();
        if let Some(ref env_name) = step.env {
            match crate::storage::load_env_by_name(env_name) {
                Ok(stored) => effective.extend(stored.vars),
                Err(e) => {
                    let _ = tx.send(CampaignEvent::Warning(
                        format!("env '{}' not found: {}", env_name, e)
                    ));
                }
            }
        }
        effective.extend(extracted.clone());

        // ── --only filter ─────────────────────────────────────────────────────
        if !only.is_empty() && !only.contains(&step.name) {
            let skipped = StepResult {
                name: step.name.clone(),
                method: "SKIP".into(),
                url: String::new(),
                status: None,
                duration_ms: 0,
                success: true,
                skipped: true,
                non_blocking: true,
                error: None,
                extracted: HashMap::new(),
                assertion_results: vec![],
                body_json: None,
                graphql: false,
                request_headers: vec![],
                request_body: None,
            };
            let _ = tx.send(CampaignEvent::StepDone(skipped.clone()));
            results.push(skipped);
            continue;
        }

        // ── parallel child — managed by its parent parallel step ─────────────
        if parallel_children.contains(&step.name) {
            continue;
        }

        // ── when condition ────────────────────────────────────────────────────
        if let Some(ref cond) = step.when {
            if !evaluate_when_condition(cond, &effective) {
                let skipped = StepResult {
                    name: step.name.clone(),
                    method: "SKIP".into(),
                    url: String::new(),
                    status: None,
                    duration_ms: 0,
                    success: true,
                    skipped: true,
                    non_blocking: effective_coe,
                    error: None,
                    extracted: HashMap::new(),
                    assertion_results: vec![],
                    body_json: None,
                    graphql: false,
                    request_headers: vec![],
                    request_body: None,
                };
                let _ = tx.send(CampaignEvent::StepDone(skipped.clone()));
                results.push(skipped);
                continue;
            }
        }

        // ── foreach ──────────────────────────────────────────────────────────
        if let Some(ref foreach_expr) = step.foreach {
            let array_str = resolve(foreach_expr, &effective);
            let items: Vec<Value> = serde_json::from_str::<Value>(&array_str)
                .ok()
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default();

            let total = items.len();
            if total == 0 {
                let _ = tx.send(CampaignEvent::Warning(
                    format!("step '{}': foreach resolved to an empty array", step.name)
                ));
                continue;
            }

            let mut foreach_failed = false;
            for (i, item) in items.iter().enumerate() {
                let item_str = match item {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                let mut iter_env = effective.clone();
                iter_env.insert("item".to_string(), item_str);
                iter_env.insert("item_index".to_string(), i.to_string());

                // Auto-inject indexed/named sub-fields: {{item_0}}, {{item_1}} for arrays;
                // {{item_fieldname}} for objects.
                match item {
                    Value::Array(arr) => {
                        for (idx, val) in arr.iter().enumerate() {
                            let v_str = match val {
                                Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                            iter_env.insert(format!("item_{}", idx), v_str);
                        }
                    }
                    Value::Object(obj) => {
                        for (k, v) in obj.iter() {
                            let v_str = match v {
                                Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                            iter_env.insert(format!("item_{}", k), v_str);
                        }
                    }
                    _ => {}
                }

                let mut iter_step = step.clone();
                iter_step.name   = format!("{} [{}/{}]", step.name, i + 1, total);
                iter_step.foreach = None; // prevent recursion

                let _ = tx.send(CampaignEvent::StepStarted { name: iter_step.name.clone() });
                let result = run_single_step(client, &iter_step, &iter_env, effective_coe).await;

                if !result.success && !effective_coe {
                    foreach_failed = true;
                    let _ = tx.send(CampaignEvent::StepDone(result.clone()));
                    results.push(result);
                    break;
                }
                let _ = tx.send(CampaignEvent::StepDone(result.clone()));
                results.push(result);
            }

            // Foreach steps do not propagate extracted vars to the outer scope.
            if foreach_failed { break; }
            continue;
        }

        // ── poll step ─────────────────────────────────────────────────────────
        if step.kind == "poll" {
            let _ = tx.send(CampaignEvent::StepStarted { name: step.name.clone() });
            let (poll_result, poll_extracted) =
                run_poll_step(client, step, &effective, effective_coe, tx).await;
            if poll_result.success {
                extracted.extend(poll_extracted);
            }
            let failed = !poll_result.success;
            let _ = tx.send(CampaignEvent::StepDone(poll_result.clone()));
            results.push(poll_result);
            if failed && !effective_coe { break; }
            continue;
        }

        // ── loop step ─────────────────────────────────────────────────────────
        if step.kind == "loop" {
            let _ = tx.send(CampaignEvent::StepStarted { name: step.name.clone() });
            let (loop_result, loop_extracted) =
                run_loop_step(client, step, &effective, effective_coe, tx).await;
            if loop_result.success {
                extracted.extend(loop_extracted);
            }
            let failed = !loop_result.success;
            let _ = tx.send(CampaignEvent::StepDone(loop_result.clone()));
            results.push(loop_result);
            if failed && !effective_coe { break; }
            continue;
        }

        // ── parallel step ─────────────────────────────────────────────────────
        if step.kind == "parallel" {
            let _ = tx.send(CampaignEvent::StepStarted { name: step.name.clone() });
            let (result, new_extracted) = run_parallel_step(client, step, steps, &effective, effective_coe, tx).await;
            if result.success { extracted.extend(new_extracted); }
            let failed = !result.success;
            let _ = tx.send(CampaignEvent::StepDone(result.clone()));
            results.push(result);
            if failed && !effective_coe { break; }
            continue;
        }

        // ── normal step ───────────────────────────────────────────────────────
        let retryable = retry > 0
            && matches!(step.kind.as_str(), "http" | "graphql" | "seed");

        let _ = tx.send(CampaignEvent::StepStarted { name: step.name.clone() });
        let mut result = run_single_step(client, step, &effective, effective_coe).await;

        if retryable && !result.success {
            for attempt in 1..=retry {
                let delay_secs = std::cmp::min(1u64 << (attempt as u64 - 1), 30);
                let _ = tx.send(CampaignEvent::StepRetry {
                    name: step.name.clone(),
                    attempt: attempt as usize,
                    max: retry as usize,
                    delay_secs,
                });
                tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                result = run_single_step(client, step, &effective, effective_coe).await;
                if result.success { break; }
            }
        }

        // Propagate extracted vars for non-foreach steps.
        if result.success {
            extracted.extend(result.extracted.clone());
        }

        let failed = !result.success;
        let _ = tx.send(CampaignEvent::StepDone(result.clone()));
        results.push(result);
        if failed && !effective_coe { break; }
    }

    results
}

// ── loop step ─────────────────────────────────────────────────────────────────

async fn run_loop_step(
    client: &reqwest::Client,
    step: &Step,
    base_env: &HashMap<String, String>,
    effective_coe: bool,
    tx: &mpsc::UnboundedSender<CampaignEvent>,
) -> (StepResult, HashMap<String, String>) {
    const MAX_ITERATIONS: usize = 1000;
    let mut iter_env = base_env.clone();
    let mut accumulated: Vec<Value> = Vec::new();
    let mut iter_count = 0usize;
    let mut last_url = String::new();
    let t0 = std::time::Instant::now();

    // Build an HTTP-only step (strip loop-specific fields for execution)
    let mut http_step = step.clone();
    http_step.kind   = "http".into();
    http_step.until  = None;
    http_step.accumulate = None;

    loop {
        if iter_count >= MAX_ITERATIONS {
            let _ = tx.send(CampaignEvent::Warning(
                format!("step '{}': reached max {} iterations, stopping", step.name, MAX_ITERATIONS)
            ));
            break;
        }

        let result = run_single_step(client, &http_step, &iter_env, true).await;
        last_url = result.url.clone();

        // Accumulate values from this iteration
        if let Some(ref acc) = step.accumulate {
            if let Some(ref body) = result.body_json {
                if let Some(val) = extract_value_at(body, &acc.from) {
                    match val {
                        Value::Array(arr) => accumulated.extend(arr),
                        other             => accumulated.push(other),
                    }
                }
            }
        }

        // Propagate extracted vars for next iteration
        iter_env.extend(result.extracted);
        iter_count += 1;

        if !result.success {
            let duration_ms = t0.elapsed().as_millis() as u64;
            let mut outer_extracted = HashMap::new();
            if let Some(ref acc) = step.accumulate {
                outer_extracted.insert(acc.var.clone(), Value::Array(accumulated).to_string());
            }
            return (StepResult {
                name: step.name.clone(),
                method: "LOOP".into(),
                url: last_url,
                status: result.status,
                duration_ms,
                success: false,
                skipped: false,
                non_blocking: effective_coe,
                error: result.error,
                extracted: outer_extracted.clone(),
                assertion_results: vec![],
                body_json: None,
                graphql: false,
                request_headers: vec![],
                request_body: None,
            }, outer_extracted);
        }

        // Check until condition
        if let Some(ref until) = step.until {
            if evaluate_until_condition(until, &iter_env) {
                break;
            }
        } else {
            // No until condition — run exactly once (safety)
            break;
        }
    }

    let duration_ms = t0.elapsed().as_millis() as u64;
    let mut outer_extracted: HashMap<String, String> = HashMap::new();

    // Store accumulated array
    if let Some(ref acc) = step.accumulate {
        outer_extracted.insert(acc.var.clone(), Value::Array(accumulated).to_string());
    }

    // Also expose last-iteration extracted vars
    outer_extracted.extend(iter_env.into_iter().filter(|(k, _)| !base_env.contains_key(k)));

    let body_json = outer_extracted.get(step.accumulate.as_ref().map(|a| a.var.as_str()).unwrap_or(""))
        .and_then(|s| serde_json::from_str(s).ok());

    (StepResult {
        name: step.name.clone(),
        method: "LOOP".into(),
        url: last_url,
        status: None,
        duration_ms,
        success: true,
        skipped: false,
        non_blocking: effective_coe,
        error: None,
        extracted: outer_extracted.clone(),
        assertion_results: vec![],
        body_json,
        graphql: false,
        request_headers: vec![],
        request_body: None,
    }, outer_extracted)
}

// ── poll step ─────────────────────────────────────────────────────────────────

async fn run_poll_step(
    client: &reqwest::Client,
    step: &Step,
    base_env: &HashMap<String, String>,
    effective_coe: bool,
    tx: &mpsc::UnboundedSender<CampaignEvent>,
) -> (StepResult, HashMap<String, String>) {
    const MAX_POLLS: usize = 500;
    let interval_ms = step.interval_ms.max(100);
    let timeout = std::time::Duration::from_secs(step.timeout_secs.max(1));
    let t0 = std::time::Instant::now();

    let mut http_step = step.clone();
    http_step.kind       = "http".into();
    http_step.until      = None;
    http_step.accumulate = None;

    let mut iter_env = base_env.clone();
    let mut last_result: Option<StepResult> = None;
    let mut poll_count = 0usize;

    loop {
        let elapsed = t0.elapsed();
        if elapsed >= timeout {
            let elapsed_secs = elapsed.as_secs();
            let err = format!("poll timeout after {}s ({} attempts)", elapsed_secs, poll_count);
            return (StepResult {
                name:              step.name.clone(),
                method:            "POLL".into(),
                url:               last_result.as_ref().map(|r| r.url.clone()).unwrap_or_default(),
                status:            last_result.as_ref().and_then(|r| r.status),
                duration_ms:       elapsed.as_millis() as u64,
                success:           false,
                skipped:           false,
                non_blocking:      effective_coe,
                error:             Some(err),
                extracted:         HashMap::new(),
                assertion_results: vec![],
                body_json:         None,
                graphql:           false,
                request_headers:   vec![],
                request_body:      None,
            }, HashMap::new());
        }

        if poll_count >= MAX_POLLS {
            let elapsed_secs = elapsed.as_secs();
            let err = format!("poll safety cap: {} iterations in {}s", MAX_POLLS, elapsed_secs);
            return (StepResult {
                name:              step.name.clone(),
                method:            "POLL".into(),
                url:               last_result.as_ref().map(|r| r.url.clone()).unwrap_or_default(),
                status:            last_result.as_ref().and_then(|r| r.status),
                duration_ms:       elapsed.as_millis() as u64,
                success:           false,
                skipped:           false,
                non_blocking:      effective_coe,
                error:             Some(err),
                extracted:         HashMap::new(),
                assertion_results: vec![],
                body_json:         None,
                graphql:           false,
                request_headers:   vec![],
                request_body:      None,
            }, HashMap::new());
        }

        poll_count += 1;
        let result = run_single_step(client, &http_step, &iter_env, true).await;
        iter_env.extend(result.extracted.clone());

        let _ = tx.send(CampaignEvent::StepPoll {
            name:         step.name.clone(),
            attempt:      poll_count,
            elapsed_secs: t0.elapsed().as_secs(),
        });

        last_result = Some(result.clone());

        // Check until condition on extracted vars
        if let Some(ref until) = step.until {
            if evaluate_until_condition(until, &iter_env) {
                let duration_ms = t0.elapsed().as_millis() as u64;
                let outer_extracted: HashMap<String, String> = iter_env.into_iter()
                    .filter(|(k, _)| !base_env.contains_key(k))
                    .collect();
                return (StepResult {
                    name:              step.name.clone(),
                    method:            "POLL".into(),
                    url:               result.url,
                    status:            result.status,
                    duration_ms,
                    success:           true,
                    skipped:           false,
                    non_blocking:      effective_coe,
                    error:             None,
                    extracted:         outer_extracted.clone(),
                    assertion_results: result.assertion_results,
                    body_json:         result.body_json,
                    graphql:           false,
                    request_headers:   result.request_headers,
                    request_body:      result.request_body,
                }, outer_extracted);
            }
        } else {
            // No until condition — single shot (degenerate case)
            break;
        }

        tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;
    }

    // Fallback (no until)
    let duration_ms = t0.elapsed().as_millis() as u64;
    let outer_extracted: HashMap<String, String> = iter_env.into_iter()
        .filter(|(k, _)| !base_env.contains_key(k))
        .collect();
    (StepResult {
        name:              step.name.clone(),
        method:            "POLL".into(),
        url:               last_result.as_ref().map(|r| r.url.clone()).unwrap_or_default(),
        status:            last_result.as_ref().and_then(|r| r.status),
        duration_ms,
        success:           true,
        skipped:           false,
        non_blocking:      effective_coe,
        error:             None,
        extracted:         outer_extracted.clone(),
        assertion_results: vec![],
        body_json:         None,
        graphql:           false,
        request_headers:   vec![],
        request_body:      None,
    }, outer_extracted)
}

// ── jq step ───────────────────────────────────────────────────────────────────

async fn run_jq_step(step: &Step, env: &HashMap<String, String>) -> anyhow::Result<HashMap<String, String>> {
    // Check that the system jq binary is available before attempting to run.
    let jq_available = tokio::process::Command::new("jq")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .is_ok();
    if !jq_available {
        return Err(anyhow::anyhow!(
            "jq n'est pas disponible — installez jq pour utiliser ce step (brew install jq / apt install jq)"
        ));
    }

    let input      = resolve(step.jq_input.as_deref().unwrap_or(""), env);
    let expr       = resolve(step.jq_expression.as_deref().unwrap_or("."), env);
    let output_var = step.jq_output.as_deref().unwrap_or("JQ_RESULT").to_string();

    let mut cmd = tokio::process::Command::new("jq");
    cmd.arg("-c"); // compact JSON output
    if step.jq_raw { cmd.arg("-r"); }
    // Pass extra variables via --argjson (values resolved from env then parsed as JSON)
    let mut jq_args_keys: Vec<&String> = step.jq_args.keys().collect();
    jq_args_keys.sort();
    for k in jq_args_keys {
        let resolved_val = resolve(step.jq_args.get(k).map(|s| s.as_str()).unwrap_or(""), env);
        cmd.arg("--argjson").arg(k).arg(&resolved_val);
    }
    cmd.arg(&expr);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn()
        .map_err(|_| anyhow::anyhow!(
            "jq n'est pas disponible — installez jq pour utiliser ce step (brew install jq / apt install jq)"
        ))?;

    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        stdin.write_all(input.as_bytes()).await?;
    }

    let output = child.wait_with_output().await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("jq: {}", stderr.trim()));
    }

    let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let mut extracted = HashMap::new();
    extracted.insert(output_var, result);
    Ok(extracted)
}

// ── parallel step ─────────────────────────────────────────────────────────────

async fn run_parallel_step(
    client: &reqwest::Client,
    step: &Step,
    all_steps: &[Step],
    effective: &HashMap<String, String>,
    effective_coe: bool,
    tx: &mpsc::UnboundedSender<CampaignEvent>,
) -> (StepResult, HashMap<String, String>) {
    let t0 = Instant::now();

    let children: Vec<Step> = all_steps.iter()
        .filter(|s| step.parallel_steps.contains(&s.name))
        .cloned()
        .collect();

    if children.is_empty() {
        let result = StepResult {
            name: step.name.clone(), method: "PAR ".into(), url: String::new(),
            status: None, duration_ms: 0, success: true, skipped: false,
            non_blocking: effective_coe, error: None, extracted: HashMap::new(),
            assertion_results: vec![], body_json: None, graphql: false,
            request_headers: vec![], request_body: None,
        };
        return (result, HashMap::new());
    }

    let mut set = tokio::task::JoinSet::new();
    for child in children {
        let client = client.clone();
        let env = effective.clone();
        let coe = child.continue_on_error.unwrap_or(effective_coe);
        set.spawn(async move {
            run_single_step(&client, &child, &env, coe).await
        });
    }

    let mut merged: HashMap<String, String> = HashMap::new();
    let mut any_failed = false;
    while let Some(res) = set.join_next().await {
        match res {
            Ok(result) => {
                if result.success {
                    merged.extend(result.extracted.clone());
                } else {
                    any_failed = true;
                }
                let _ = tx.send(CampaignEvent::StepDone(result));
            }
            Err(e) => {
                any_failed = true;
                let _ = tx.send(CampaignEvent::Warning(format!("parallel spawn error: {e}")));
            }
        }
    }

    let duration_ms = t0.elapsed().as_millis() as u64;
    let outer = StepResult {
        name:            step.name.clone(),
        method:          "PAR ".into(),
        url:             String::new(),
        status:          None,
        duration_ms,
        success:         !any_failed,
        skipped:         false,
        non_blocking:    effective_coe,
        error:           if any_failed { Some("one or more parallel steps failed".into()) } else { None },
        extracted:       merged.clone(),
        assertion_results: vec![],
        body_json:       None,
        graphql:         false,
        request_headers: vec![],
        request_body:    None,
    };
    (outer, merged)
}

// ── transform step ────────────────────────────────────────────────────────────

fn run_transform_step(
    step: &Step,
    env: &HashMap<String, String>,
) -> anyhow::Result<HashMap<String, String>> {
    let mut produced: HashMap<String, String> = HashMap::new();
    for t in &step.transforms {
        let mut local = env.clone();
        local.extend(produced.clone());
        let input = resolve(&t.input, &local);
        let value = match t.kind.as_str() {
            "template" => input,
            "upper"    => input.to_uppercase(),
            "lower"    => input.to_lowercase(),
            "trim"     => input.trim().to_string(),
            "regex" => {
                let pattern = t.pattern.as_deref()
                    .ok_or_else(|| anyhow::anyhow!("transform 'regex' requires 'pattern'"))?;
                let re = Regex::new(pattern)
                    .with_context(|| format!("invalid regex: {}", pattern))?;
                re.captures(&input)
                    .and_then(|c| c.get(t.group))
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default()
            }
            "replace" => {
                let from = resolve(
                    t.from.as_deref().ok_or_else(|| anyhow::anyhow!("transform 'replace' requires 'from'"))?,
                    &local,
                );
                let to = resolve(t.to.as_deref().unwrap_or(""), &local);
                input.replace(from.as_str(), to.as_str())
            }
            "split" => {
                let delim = t.delimiter.as_deref().unwrap_or(",");
                input.split(delim).nth(t.index).unwrap_or("").trim().to_string()
            }
            other => bail!(
                "unknown transform type '{}' (supported: template, regex, replace, split, trim, upper, lower)",
                other
            ),
        };
        produced.insert(t.output.clone(), value);
    }
    Ok(produced)
}

// ── HTTP step ─────────────────────────────────────────────────────────────────

async fn execute_step(
    client: &reqwest::Client,
    step: &Step,
    url: &str,
    body: Option<&str>,
    env: &HashMap<String, String>,
) -> anyhow::Result<HttpOutcome> {
    let mut req = match step.method.to_uppercase().as_str() {
        "GET"    => client.get(url),
        "POST"   => client.post(url),
        "PUT"    => client.put(url),
        "PATCH"  => client.patch(url),
        "DELETE" => client.delete(url),
        m => bail!("unknown HTTP method: {}", m),
    };
    let mut hmap = HeaderMap::new();
    for (k, v) in &step.headers {
        let name  = HeaderName::from_str(k).with_context(|| format!("invalid header name: {}", k))?;
        let value = HeaderValue::from_str(&resolve(v, env)).with_context(|| format!("invalid header value: {}", v))?;
        hmap.insert(name, value);
    }
    req = req.headers(hmap);

    if !step.multipart_parts.is_empty() {
        let mut form = reqwest::multipart::Form::new();
        for part_cfg in &step.multipart_parts {
            let part_name  = resolve(&part_cfg.name, env);
            let part_value = resolve(&part_cfg.value, env);
            let part = if let Some(path) = part_value.strip_prefix('@') {
                let bytes    = std::fs::read(path).with_context(|| format!("multipart: reading {path}"))?;
                let filename = std::path::Path::new(path)
                    .file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
                let mime = part_cfg.content_type.as_deref().unwrap_or("application/octet-stream");
                reqwest::multipart::Part::bytes(bytes).file_name(filename).mime_str(mime)?
            } else {
                let mut p = reqwest::multipart::Part::text(part_value);
                if let Some(ref ct) = part_cfg.content_type { p = p.mime_str(ct)?; }
                p
            };
            form = form.part(part_name, part);
        }
        req = req.multipart(form);
    } else if let Some(b) = body {
        req = req.body(b.to_owned());
    }

    let response = req.send().await
        .with_context(|| format!("request failed: {} {}", step.method, url))?;
    let status = response.status().as_u16();
    let resp_headers: HashMap<String, String> = response.headers()
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();
    let body_text  = response.text().await?;
    let body_value: Option<Value> = serde_json::from_str(&body_text).ok();

    let mut extracted = HashMap::new();
    if !step.extract.is_empty() {
        if let Some(ref json) = body_value {
            for (var, path) in &step.extract {
                if let Some(val) = extract_at(json, path) {
                    extracted.insert(var.clone(), val);
                }
            }
        }
    }
    Ok(HttpOutcome { status, body_value, resp_headers, extracted })
}

// ── assertion evaluator ───────────────────────────────────────────────────────

fn evaluate_assertions(
    assertions: &[Assertion],
    status: u16,
    body_value: Option<&Value>,
    resp_headers: &HashMap<String, String>,
    elapsed_ms: u64,
    env: &HashMap<String, String>,
) -> Vec<(String, bool)> {
    let mut results: Vec<(String, bool)> = Vec::new();
    macro_rules! push {
        ($ok:expr, $desc:expr) => { results.push(($desc, $ok)) };
    }

    for a in assertions {
        let target = resolve(&a.on, env);
        let actual: Option<Value> = if target == "status" {
            Some(Value::Number(status.into()))
        } else if target == "elapsed_ms" {
            Some(Value::Number(elapsed_ms.into()))
        } else if target == "body" {
            body_value.cloned()
        } else if let Some(path) = target.strip_prefix("body.") {
            body_value.and_then(|b| extract_value_at(b, path))
        } else if let Some(name) = target.strip_prefix("header.") {
            let lower = name.to_lowercase();
            resp_headers.iter()
                .find(|(k, _)| k.to_lowercase() == lower)
                .map(|(_, v)| Value::String(v.clone()))
        } else {
            None
        };

        if let Some(ref expected) = a.eq {
            let exp = resolve_value(expected, env);
            if values_eq(&actual, &exp) {
                push!(true,  format!("{} == {}", target, fmt_val(&exp)));
            } else {
                push!(false, format!("{} == {}  (got {})", target, fmt_val(&exp), fmt_opt(&actual)));
            }
        }
        if let Some(ref expected) = a.ne {
            let exp = resolve_value(expected, env);
            if values_eq(&actual, &exp) {
                push!(false, format!("{} != {}  (got {})", target, fmt_val(&exp), fmt_opt(&actual)));
            } else {
                push!(true,  format!("{} != {}", target, fmt_val(&exp)));
            }
        }
        if let Some(t) = a.lt {
            match actual.as_ref().and_then(val_as_f64) {
                Some(v) if v < t => push!(true,  format!("{} < {}", target, t)),
                Some(v)          => push!(false, format!("{} < {}  (got {})", target, t, v)),
                None             => push!(false, format!("{} < {}  (got {})", target, t, fmt_opt(&actual))),
            }
        }
        if let Some(t) = a.lte {
            match actual.as_ref().and_then(val_as_f64) {
                Some(v) if v <= t => push!(true,  format!("{} <= {}", target, t)),
                Some(v)           => push!(false, format!("{} <= {}  (got {})", target, t, v)),
                None              => push!(false, format!("{} <= {}  (got {})", target, t, fmt_opt(&actual))),
            }
        }
        if let Some(t) = a.gt {
            match actual.as_ref().and_then(val_as_f64) {
                Some(v) if v > t => push!(true,  format!("{} > {}", target, t)),
                Some(v)          => push!(false, format!("{} > {}  (got {})", target, t, v)),
                None             => push!(false, format!("{} > {}  (got {})", target, t, fmt_opt(&actual))),
            }
        }
        if let Some(t) = a.gte {
            match actual.as_ref().and_then(val_as_f64) {
                Some(v) if v >= t => push!(true,  format!("{} >= {}", target, t)),
                Some(v)           => push!(false, format!("{} >= {}  (got {})", target, t, v)),
                None              => push!(false, format!("{} >= {}  (got {})", target, t, fmt_opt(&actual))),
            }
        }
        if !a.in_.is_empty() {
            let resolved: Vec<Value> = a.in_.iter().map(|v| resolve_value(v, env)).collect();
            let opts = resolved.iter().map(fmt_val).collect::<Vec<_>>().join(", ");
            let found = actual.as_ref()
                .map(|v| resolved.iter().any(|e| values_eq(&Some(v.clone()), e)))
                .unwrap_or(false);
            if found {
                push!(true,  format!("{} in [{}]", target, opts));
            } else {
                push!(false, format!("{} in [{}]  (got {})", target, opts, fmt_opt(&actual)));
            }
        }
        if let Some(expected_exists) = a.exists {
            let actually_exists = matches!(&actual, Some(v) if !v.is_null());
            if actually_exists == expected_exists {
                push!(true,  format!("{} exists = {}", target, expected_exists));
            } else {
                push!(false, format!("{} exists == {}  (got {})", target, expected_exists, actually_exists));
            }
        }
        if let Some(ref substr) = a.contains {
            let s = resolve(substr, env);
            match actual.as_ref() {
                Some(Value::String(v)) if v.contains(s.as_str()) => {
                    push!(true,  format!("{} contains {:?}", target, s));
                }
                Some(Value::String(v)) => {
                    push!(false, format!("{} contains {:?}  (got {:?})", target, s, v));
                }
                _ => {
                    push!(false, format!("{} contains {:?}  (not a string: {})", target, s, fmt_opt(&actual)));
                }
            }
        }
        if let Some(ref pattern) = a.matches {
            let p = resolve(pattern, env);
            match Regex::new(&p) {
                Err(e) => push!(false, format!("{} matches {:?}  (invalid regex: {})", target, p, e)),
                Ok(re) => match actual.as_ref() {
                    Some(Value::String(v)) if re.is_match(v) => {
                        push!(true,  format!("{} matches {:?}", target, p));
                    }
                    Some(Value::String(v)) => {
                        push!(false, format!("{} matches {:?}  (got {:?})", target, p, v));
                    }
                    _ => {
                        push!(false, format!("{} matches {:?}  (not a string: {})", target, p, fmt_opt(&actual)));
                    }
                }
            }
        }
    }
    results
}

/// Compact label for an assertion — used in the TUI idle preview.
fn evaluate_when_condition(cond: &StepCondition, env: &HashMap<String, String>) -> bool {
    let value = env.get(&cond.var).map(|s| s.as_str()).unwrap_or("");
    if let Some(ref eq) = cond.eq  { return value == resolve(eq, env).as_str(); }
    if let Some(ref ne) = cond.ne  { return value != resolve(ne, env).as_str(); }
    if let Some(exists) = cond.exists { return env.contains_key(&cond.var) == exists; }
    if let Some(lt)  = cond.lt  { return value.parse::<f64>().map_or(false, |n| n < lt);  }
    if let Some(lte) = cond.lte { return value.parse::<f64>().map_or(false, |n| n <= lte); }
    !value.is_empty()
}

// Same logic for loop `until` condition — true means "stop".
fn evaluate_until_condition(cond: &StepCondition, env: &HashMap<String, String>) -> bool {
    evaluate_when_condition(cond, env)
}

pub fn when_label(cond: &StepCondition) -> String {
    if let Some(ref v) = cond.eq     { return format!("{} == {:?}", cond.var, v); }
    if let Some(ref v) = cond.ne     { return format!("{} != {:?}", cond.var, v); }
    if let Some(e)      = cond.exists { return format!("{} exists={}", cond.var, e); }
    format!("{} set", cond.var)
}

pub fn assertion_label(a: &Assertion) -> String {
    if let Some(ref v) = a.eq      { return format!("{} == {}", a.on, fmt_val(v)); }
    if let Some(ref v) = a.ne      { return format!("{} != {}", a.on, fmt_val(v)); }
    if let Some(v) = a.lt          { return format!("{} < {}", a.on, v); }
    if let Some(v) = a.lte         { return format!("{} <= {}", a.on, v); }
    if let Some(v) = a.gt          { return format!("{} > {}", a.on, v); }
    if let Some(v) = a.gte         { return format!("{} >= {}", a.on, v); }
    if !a.in_.is_empty() {
        let opts = a.in_.iter().map(fmt_val).collect::<Vec<_>>().join(", ");
        return format!("{} in [{}]", a.on, opts);
    }
    if let Some(v) = a.exists      { return format!("{} exists = {}", a.on, v); }
    if let Some(ref v) = a.contains { return format!("{} contains {:?}", a.on, v); }
    if let Some(ref v) = a.matches  { return format!("{} matches {:?}", a.on, v); }
    a.on.clone()
}

// ── CLI report ────────────────────────────────────────────────────────────────

fn print_step_result(sr: &StepResult) {
    if sr.skipped {
        println!("  ⊘ {:<22} (skipped)", sr.name);
        return;
    }
    let status_str = sr.status
        .map(|s| s.to_string())
        .unwrap_or_else(|| if sr.error.is_some() { "ERR".into() } else { "-".into() });
    let mark = if sr.success { "✓" } else { "✗" };
    let suffix = if !sr.success && sr.non_blocking { "  [continu]" } else { "" };
    println!("  {} {:<22} {:<7} {}  {:>6} ms  {}{}",
        mark, sr.name, sr.method, status_str, sr.duration_ms,
        sr.error.as_deref().unwrap_or(""), suffix);
    for (var, val) in &sr.extracted {
        println!("      ↳ {} = {}", var, truncate(val, 60));
    }
    for (desc, ok) in &sr.assertion_results {
        if !ok {
            println!("      ✗ assert: {}", desc);
        }
    }
}

fn print_report(campaign: &Campaign, results: &[IterationResult]) {
    let total_steps: usize = results.iter().map(|r| r.steps.len()).sum();
    let total_ok:    usize = results.iter().map(|r| r.ok_count()).sum();
    let total_fail:  usize = results.iter().map(|r| r.fail_count()).sum();
    let total_ms:    u64   = results.iter().map(|r| r.total_ms()).sum();
    let iters_ok:    usize = results.iter().filter(|r| r.success()).count();
    let iters_fail:  usize = results.iter().filter(|r| !r.success()).count();
    let multi = results.len() > 1;

    let width = 64usize;
    let bar   = "═".repeat(width);
    println!("\n╔{}╗", bar);
    println!("║  Campaign Report — {:<width$}║", campaign.campaign.name, width = width - 19);
    println!("╠{}╣", bar);
    if multi {
        println!("║  {:<width$}║",
            format!("Iterations : {} ok  /  {} failed", iters_ok, iters_fail), width = width - 2);
    }
    println!("║  {:<width$}║",
        format!("Steps      : {} ok  /  {} failed  ({} total)", total_ok, total_fail, total_steps),
        width = width - 2);
    println!("║  {:<width$}║", format!("Duration   : {} ms", total_ms), width = width - 2);

    if total_fail > 0 {
        println!("╠{}╣", bar);
        println!("║  Failures:{:<width$}║", "", width = width - 10);
        for iter in results.iter().filter(|r| !r.success()) {
            if let Some(idx) = iter.row_index {
                let row_label = iter.row_vars.iter()
                    .map(|(k, v)| format!("{}={}", k, truncate(v, 20)))
                    .collect::<Vec<_>>().join(" ");
                println!("║  Row {} — {:<width$}║", idx + 1, row_label,
                    width = width - 10 - format!("Row {} — ", idx + 1).len());
            }
            for step in iter.steps.iter().filter(|s| !s.success) {
                let msg = step.error.as_deref().unwrap_or("unknown error");
                println!("║    ✗ {} {} — {:<width$}║",
                    step.method, truncate(&step.url, 30), msg,
                    width = width.saturating_sub(10 + step.method.len() + 30.min(step.url.len())));
                for (desc, ok) in &step.assertion_results {
                    if !ok {
                        let line = format!("      · {}", desc);
                        println!("║  {:<width$}║", truncate(&line, width - 2), width = width - 2);
                    }
                }
            }
        }
    }
    println!("╠{}╣", bar);
    let verdict = if total_fail == 0 { "✓  ALL PASSED" } else { "✗  SOME STEPS FAILED" };
    println!("║  {:<width$}║", verdict, width = width - 2);
    println!("╚{}╝", bar);
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn resolve(s: &str, env: &HashMap<String, String>) -> String {
    let mut out = s.to_string();
    for (k, v) in env { out = out.replace(&format!("{{{{{}}}}}", k), v); }
    out
}

fn resolve_value(v: &Value, env: &HashMap<String, String>) -> Value {
    match v { Value::String(s) => Value::String(resolve(s, env)), other => other.clone() }
}

fn extract_at(value: &Value, path: &str) -> Option<String> {
    extract_value_at(value, path).map(|v| match v {
        Value::String(s) => s,
        other => other.to_string(),
    })
}

fn extract_value_at(value: &Value, path: &str) -> Option<Value> {
    let segments: Vec<&str> = path.split('.').collect();
    extract_segments(value, &segments)
}

fn extract_segments(value: &Value, segments: &[&str]) -> Option<Value> {
    if segments.is_empty() { return Some(value.clone()); }
    let (head, tail) = (segments[0], &segments[1..]);
    if head == "*" {
        let arr = value.as_array()?;
        let results: Vec<Value> = arr.iter()
            .filter_map(|el| extract_segments(el, tail))
            .collect();
        Some(Value::Array(results))
    } else if let Ok(idx) = head.parse::<usize>() {
        extract_segments(value.get(idx)?, tail)
    } else {
        extract_segments(value.get(head)?, tail)
    }
}

fn values_eq(actual: &Option<Value>, expected: &Value) -> bool {
    let Some(a) = actual else { return false };
    match (a, expected) {
        (Value::Number(a), Value::Number(b)) => a == b,
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Bool(a),   Value::Bool(b))   => a == b,
        (Value::String(s), Value::Number(n)) => s.parse::<f64>().ok() == n.as_f64(),
        (Value::Number(n), Value::String(s)) => n.as_f64() == s.parse::<f64>().ok(),
        _ => a == expected,
    }
}

fn val_as_f64(v: &Value) -> Option<f64> {
    match v { Value::Number(n) => n.as_f64(), Value::String(s) => s.parse().ok(), _ => None }
}

fn fmt_val(v: &Value) -> String {
    match v { Value::String(s) => format!("{:?}", s), other => other.to_string() }
}

fn fmt_opt(v: &Option<Value>) -> String {
    match v { Some(v) => fmt_val(v), None => "(none)".into() }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}…", &s[..max]) }
}

fn run_search_step(step: &Step, env: &HashMap<String, String>) -> Result<HashMap<String, String>> {
    let cfg = step.search.as_ref()
        .ok_or_else(|| anyhow::anyhow!("search step missing `search` config"))?;

    let input_str = resolve(&cfg.input, env);
    let array: Vec<serde_json::Value> = serde_json::from_str(&input_str)
        .map_err(|e| anyhow::anyhow!("search input is not a JSON array: {}", e))?;

    let re = regex::Regex::new(&cfg.pattern)
        .map_err(|e| anyhow::anyhow!("search: invalid regex \"{}\": {}", cfg.pattern, e))?;

    let output_var = resolve(&cfg.output, env);

    let matches: Vec<serde_json::Value> = array.into_iter().filter(|elem| {
        let haystack = if cfg.path.is_empty() {
            match elem {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            }
        } else {
            match extract_value_at(elem, &cfg.path) {
                Some(serde_json::Value::String(s)) => s,
                Some(v) => v.to_string(),
                None => return false,
            }
        };
        re.is_match(&haystack)
    }).collect();

    let result = if cfg.first_only {
        match matches.into_iter().next() {
            Some(v) => serde_json::to_string(&v).unwrap_or_default(),
            None => "null".to_string(),
        }
    } else {
        serde_json::to_string(&matches).unwrap_or_else(|_| "[]".to_string())
    };

    let mut extracted = HashMap::new();
    extracted.insert(output_var, result);
    Ok(extracted)
}

fn run_file_step(path: &str, encoding: &str) -> Result<String> {
    let bytes = std::fs::read(path).context(format!("reading file {path}"))?;
    let encoded = match encoding {
        "text" => String::from_utf8(bytes).context("file is not valid UTF-8")?,
        "hex"  => bytes.iter().map(|b| format!("{b:02x}")).collect(),
        _      => B64.encode(&bytes), // "base64" is the default
    };
    Ok(encoded)
}
