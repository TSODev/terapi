use anyhow::{bail, Context, Result};
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Instant;
use tokio::sync::mpsc;

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
}

#[derive(Debug, Deserialize, Clone)]
pub struct Meta {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Step {
    pub name: String,
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
}

fn default_http() -> String { "http".into() }

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
    pub non_blocking: bool,
    pub error: Option<String>,
    pub extracted: HashMap<String, String>,
    /// All assertion results: (description, passed).
    pub assertion_results: Vec<(String, bool)>,
    /// Raw JSON body — used by output connectors; None for transform/seed steps.
    pub body_json: Option<Value>,
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
    StepStarted { name: String, method: String },
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

// ── streaming runner (core) ───────────────────────────────────────────────────

pub async fn run_streaming(campaign: Campaign, tx: mpsc::UnboundedSender<CampaignEvent>, overrides: HashMap<String, String>) {
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

        let step_results = run_steps_streaming(&client, &iteration_steps, campaign.continue_on_error, &env, &tx).await;

        all.push(IterationResult {
            row_index: if multi { Some(idx) } else { None },
            row_vars,
            steps: step_results,
        });
    }

    // Write output connectors: one JSON file per [[outputs]] entry.
    for output in &campaign.outputs {
        let bodies: Vec<Value> = all.iter()
            .flat_map(|iter| &iter.steps)
            .filter(|s| s.name == output.from_step && s.success)
            .filter_map(|s| {
                let body = s.body_json.clone()?;
                if let Some(ref sel) = output.select {
                    if !sel.is_empty() {
                        return extract_value_at(&body, sel).cloned();
                    }
                }
                Some(body)
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

pub async fn run(campaign: &Campaign, silent: bool, overrides: HashMap<String, String>) -> Result<()> {
    macro_rules! out { ($($arg:tt)*) => { if !silent { println!($($arg)*); } } }

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

    let (tx, mut rx) = mpsc::unbounded_channel::<CampaignEvent>();
    let owned = campaign.clone();
    tokio::spawn(async move { run_streaming(owned, tx, overrides).await; });

    while let Some(event) = rx.recv().await {
        match event {
            CampaignEvent::IterationStarted { idx, total, row_summary } => {
                out!("── Row {}/{} — {}", idx + 1, total, row_summary);
            }
            CampaignEvent::StepStarted { .. } => {}
            CampaignEvent::Warning(msg) => out!("  Warning: {}", msg),
            CampaignEvent::StepDone(sr) => {
                if !silent { print_step_result(&sr); }
            }
            CampaignEvent::Finished(results) => {
                if !silent {
                    print_report(campaign, &results);
                    for o in &campaign.outputs {
                        out!("  → output written: {}", o.path);
                    }
                }
                let total_fail: usize = results.iter().map(|r| r.fail_count()).sum();
                if total_fail > 0 { std::process::exit(1); }
                break;
            }
            CampaignEvent::Error(e) => {
                if !silent { eprintln!("Campaign error: {}", e); }
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

// ── streaming step runner ─────────────────────────────────────────────────────

async fn run_steps_streaming(
    client: &reqwest::Client,
    steps: &[Step],
    campaign_coe: bool,
    base_env: &HashMap<String, String>,
    tx: &mpsc::UnboundedSender<CampaignEvent>,
) -> Vec<StepResult> {
    let mut extracted: HashMap<String, String> = HashMap::new();
    let mut results = Vec::new();

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

        let method_display = match step.kind.as_str() {
            "transform" => "TRSF".into(),
            "pause"     => "WAIT".into(),
            _           => step.method.clone(),
        };
        let _ = tx.send(CampaignEvent::StepStarted {
            name: step.name.clone(),
            method: method_display,
        });

        let result = if step.kind == "pause" {
            let t0 = Instant::now();
            tokio::time::sleep(std::time::Duration::from_millis(step.wait_ms)).await;
            StepResult {
                name: step.name.clone(),
                method: "WAIT".into(),
                url: String::new(),
                status: None,
                duration_ms: t0.elapsed().as_millis() as u64,
                success: true,
                non_blocking: effective_coe,
                error: None,
                extracted: HashMap::new(),
                assertion_results: vec![],
                body_json: None,
            }
        } else if step.kind == "transform" {
            let t0 = Instant::now();
            match run_transform_step(step, &effective) {
                Ok(produced) => {
                    extracted.extend(produced.clone());
                    StepResult {
                        name: step.name.clone(),
                        method: "TRSF".into(),
                        url: String::new(),
                        status: None,
                        duration_ms: t0.elapsed().as_millis() as u64,
                        success: true,
                        non_blocking: effective_coe,
                        error: None,
                        extracted: produced,
                        assertion_results: vec![],
                        body_json: None,
                    }
                }
                Err(e) => StepResult {
                    name: step.name.clone(),
                    method: "TRSF".into(),
                    url: String::new(),
                    status: None,
                    duration_ms: t0.elapsed().as_millis() as u64,
                    success: false,
                    non_blocking: effective_coe,
                    error: Some(e.to_string()),
                    extracted: HashMap::new(),
                    assertion_results: vec![],
                    body_json: None,
                },
            }
        } else {
            let url  = resolve(&step.url, &effective);
            let body = step.body.as_deref().map(|b| resolve(b, &effective));
            let t0   = Instant::now();

            match execute_step(client, step, &url, body.as_deref(), &effective).await {
                Ok(http) => {
                    let duration_ms = t0.elapsed().as_millis() as u64;
                    let assertion_results = if step.assert.is_empty() {
                        vec![]
                    } else {
                        evaluate_assertions(
                            &step.assert, http.status,
                            http.body_value.as_ref(), &http.resp_headers,
                            duration_ms, &effective,
                        )
                    };
                    let http_ok   = http.status < 400;
                    let assert_ok = assertion_results.iter().all(|(_, ok)| *ok);
                    let success   = http_ok && assert_ok;
                    if success { extracted.extend(http.extracted.clone()); }
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
                        non_blocking: effective_coe,
                        error,
                        extracted: if success { http.extracted } else { HashMap::new() },
                        assertion_results,
                        body_json: http.body_value,
                    }
                }
                Err(e) => StepResult {
                    name: step.name.clone(),
                    method: step.method.clone(),
                    url,
                    status: None,
                    duration_ms: t0.elapsed().as_millis() as u64,
                    success: false,
                    non_blocking: effective_coe,
                    error: Some(e.to_string()),
                    extracted: HashMap::new(),
                    assertion_results: vec![],
                    body_json: None,
                },
            }
        };

        let failed = !result.success;
        let _ = tx.send(CampaignEvent::StepDone(result.clone()));
        results.push(result);
        if failed && !effective_coe { break; }
    }

    results
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
    if let Some(b) = body { req = req.body(b.to_owned()); }

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
            body_value.and_then(|b| extract_value_at(b, path)).cloned()
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
        Value::String(s) => s.clone(),
        other => other.to_string(),
    })
}

fn extract_value_at<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for segment in path.split('.') {
        current = if let Ok(idx) = segment.parse::<usize>() {
            current.get(idx)?
        } else {
            current.get(segment)?
        };
    }
    Some(current)
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
