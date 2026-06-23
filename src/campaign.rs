use anyhow::{bail, Context, Result};
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Instant;

use crate::connector::{self, ConnectorConfig, Row};

// ── TOML schema ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct Campaign {
    pub campaign: Meta,
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Name of a terapi environment to load as base (e.g. "production").
    #[serde(default)]
    pub env_file: Option<String>,
    #[serde(default)]
    pub connectors: Vec<ConnectorConfig>,
    #[serde(default)]
    pub steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
pub struct Meta {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct Step {
    pub name: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
    /// Named terapi environment to use for this step only.
    #[serde(default)]
    pub env: Option<String>,
    /// Dot-path extraction from JSON response: VAR_NAME = "path.to.value"
    #[serde(default)]
    pub extract: HashMap<String, String>,
    /// Assertions evaluated after the response is received.
    #[serde(default)]
    pub assert: Vec<Assertion>,
}

/// A single assertion on a response field.
///
/// `on` targets:
///   "status"          — HTTP status code (number)
///   "elapsed_ms"      — response time in ms (number)
///   "body"            — full parsed JSON body
///   "body.x.y"        — dot-path inside the JSON body
///   "header.x-name"   — response header value (case-insensitive)
///
/// Operators (at least one required):
///   eq / ne / lt / lte / gt / gte / in / exists / contains / matches
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

// ── result types ──────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct StepResult {
    pub name: String,
    pub method: String,
    pub url: String,
    pub status: Option<u16>,
    pub duration_ms: u64,
    pub success: bool,
    pub error: Option<String>,
    pub extracted: HashMap<String, String>,
    pub assertion_failures: Vec<String>,
}

#[derive(Debug)]
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

// ── runner ────────────────────────────────────────────────────────────────────

pub async fn run(campaign: &Campaign, silent: bool) -> Result<()> {
    macro_rules! out {
        ($($arg:tt)*) => { if !silent { println!($($arg)*); } }
    }

    out!("Campaign : {}", campaign.campaign.name);
    if !campaign.campaign.description.is_empty() {
        out!("           {}", campaign.campaign.description);
    }

    let mut base_env: HashMap<String, String> = if let Some(ref name) = campaign.env_file {
        match crate::storage::load_env_by_name(name) {
            Ok(stored) => {
                out!("Env file  : {} ({} vars)", name, stored.vars.len());
                stored.vars
            }
            Err(e) => {
                out!("Warning   : could not load env_file '{}': {}", name, e);
                HashMap::new()
            }
        }
    } else {
        HashMap::new()
    };
    base_env.extend(campaign.env.clone());

    let rows: Vec<Row> = match campaign.connectors.as_slice() {
        [] => {
            out!();
            vec![HashMap::new()]
        }
        [single] => {
            let rows = connector::load_rows(single)?;
            out!("Connector : {} ({} rows)\n", single.path, rows.len());
            rows
        }
        _ => bail!("multiple connectors per campaign are not yet supported"),
    };

    let total_iters = rows.len();
    let use_iter_prefix = total_iters > 1;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let mut all: Vec<IterationResult> = Vec::new();

    for (idx, row_vars) in rows.into_iter().enumerate() {
        let mut env = base_env.clone();
        env.extend(row_vars.clone());

        if use_iter_prefix {
            let row_summary = row_vars.iter()
                .map(|(k, v)| format!("{}={}", k, truncate(v, 20)))
                .collect::<Vec<_>>()
                .join("  ");
            out!("── Row {}/{} — {} ", idx + 1, total_iters, row_summary);
        }

        let step_results = run_steps(&client, &campaign.steps, &env, silent).await;

        for sr in &step_results {
            let status_str = sr.status
                .map(|s| s.to_string())
                .unwrap_or_else(|| "ERR".into());
            let mark = if sr.success { "✓" } else { "✗" };
            out!("  {} {:<22} {:<7} {}  {:>6} ms  {}",
                mark, sr.name, sr.method, status_str, sr.duration_ms,
                sr.error.as_deref().unwrap_or(""));
            for (var, val) in &sr.extracted {
                out!("      ↳ {} = {}", var, truncate(val, 60));
            }
            for msg in &sr.assertion_failures {
                out!("      ✗ assert: {}", msg);
            }
        }

        all.push(IterationResult {
            row_index: if use_iter_prefix { Some(idx) } else { None },
            row_vars,
            steps: step_results,
        });

        if use_iter_prefix { out!(); }
    }

    if !silent {
        print_report(campaign, &all);
    }

    let total_fail: usize = all.iter().map(|r| r.fail_count()).sum();
    if total_fail > 0 {
        std::process::exit(1);
    }

    Ok(())
}

// ── step execution ────────────────────────────────────────────────────────────

async fn run_steps(
    client: &reqwest::Client,
    steps: &[Step],
    base_env: &HashMap<String, String>,
    silent: bool,
) -> Vec<StepResult> {
    macro_rules! warn {
        ($($arg:tt)*) => { if !silent { println!($($arg)*); } }
    }

    let mut extracted: HashMap<String, String> = HashMap::new();
    let mut results = Vec::new();

    for step in steps {
        let mut effective = base_env.clone();

        if let Some(ref env_name) = step.env {
            match crate::storage::load_env_by_name(env_name) {
                Ok(stored) => effective.extend(stored.vars),
                Err(e) => warn!("  Warning: env '{}' not found: {}", env_name, e),
            }
        }
        effective.extend(extracted.clone());

        let url  = resolve(&step.url, &effective);
        let body = step.body.as_deref().map(|b| resolve(b, &effective));
        let t0   = Instant::now();

        let outcome = execute_step(client, step, &url, body.as_deref(), &effective).await;
        let duration_ms = t0.elapsed().as_millis() as u64;

        match outcome {
            Ok(http) => {
                let assertion_failures = if step.assert.is_empty() {
                    vec![]
                } else {
                    evaluate_assertions(
                        &step.assert,
                        http.status,
                        http.body_value.as_ref(),
                        &http.resp_headers,
                        duration_ms,
                        &effective,
                    )
                };

                let http_ok   = http.status < 400;
                let assert_ok = assertion_failures.is_empty();
                let success   = http_ok && assert_ok;

                if success {
                    extracted.extend(http.extracted.clone());
                }

                let error = if !http_ok {
                    Some(format!("HTTP {}", http.status))
                } else if !assert_ok {
                    Some(format!("{} assertion(s) failed", assertion_failures.len()))
                } else {
                    None
                };

                let failed = !success;
                results.push(StepResult {
                    name: step.name.clone(),
                    method: step.method.clone(),
                    url,
                    status: Some(http.status),
                    duration_ms,
                    success,
                    error,
                    extracted: if success { http.extracted } else { HashMap::new() },
                    assertion_failures,
                });
                if failed { break; }
            }
            Err(e) => {
                results.push(StepResult {
                    name: step.name.clone(),
                    method: step.method.clone(),
                    url,
                    status: None,
                    duration_ms,
                    success: false,
                    error: Some(e.to_string()),
                    extracted: HashMap::new(),
                    assertion_failures: vec![],
                });
                break;
            }
        }
    }

    results
}

async fn execute_step(
    client: &reqwest::Client,
    step: &Step,
    url: &str,
    body: Option<&str>,
    env: &HashMap<String, String>,
) -> Result<HttpOutcome> {
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
        let name  = HeaderName::from_str(k)
            .with_context(|| format!("invalid header name: {}", k))?;
        let value = HeaderValue::from_str(&resolve(v, env))
            .with_context(|| format!("invalid header value: {}", v))?;
        hmap.insert(name, value);
    }
    req = req.headers(hmap);

    if let Some(b) = body {
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

    let mut extracted: HashMap<String, String> = HashMap::new();
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
) -> Vec<String> {
    let mut failures = Vec::new();

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
            if !values_eq(&actual, &exp) {
                failures.push(format!(
                    "{} == {}  (got {})",
                    target, fmt_val(&exp), fmt_opt(&actual)
                ));
            }
        }

        if let Some(ref expected) = a.ne {
            let exp = resolve_value(expected, env);
            if values_eq(&actual, &exp) {
                failures.push(format!(
                    "{} != {}  (got {})",
                    target, fmt_val(&exp), fmt_opt(&actual)
                ));
            }
        }

        if let Some(threshold) = a.lt {
            match actual.as_ref().and_then(val_as_f64) {
                Some(v) if v < threshold => {}
                Some(v) => failures.push(format!("{} < {}  (got {})", target, threshold, v)),
                None    => failures.push(format!("{} < {}  (got {})", target, threshold, fmt_opt(&actual))),
            }
        }
        if let Some(threshold) = a.lte {
            match actual.as_ref().and_then(val_as_f64) {
                Some(v) if v <= threshold => {}
                Some(v) => failures.push(format!("{} <= {}  (got {})", target, threshold, v)),
                None    => failures.push(format!("{} <= {}  (got {})", target, threshold, fmt_opt(&actual))),
            }
        }
        if let Some(threshold) = a.gt {
            match actual.as_ref().and_then(val_as_f64) {
                Some(v) if v > threshold => {}
                Some(v) => failures.push(format!("{} > {}  (got {})", target, threshold, v)),
                None    => failures.push(format!("{} > {}  (got {})", target, threshold, fmt_opt(&actual))),
            }
        }
        if let Some(threshold) = a.gte {
            match actual.as_ref().and_then(val_as_f64) {
                Some(v) if v >= threshold => {}
                Some(v) => failures.push(format!("{} >= {}  (got {})", target, threshold, v)),
                None    => failures.push(format!("{} >= {}  (got {})", target, threshold, fmt_opt(&actual))),
            }
        }

        if !a.in_.is_empty() {
            let resolved: Vec<Value> = a.in_.iter().map(|v| resolve_value(v, env)).collect();
            let found = actual.as_ref()
                .map(|v| resolved.iter().any(|e| values_eq(&Some(v.clone()), e)))
                .unwrap_or(false);
            if !found {
                let opts = resolved.iter().map(fmt_val).collect::<Vec<_>>().join(", ");
                failures.push(format!("{} in [{}]  (got {})", target, opts, fmt_opt(&actual)));
            }
        }

        if let Some(expected_exists) = a.exists {
            let actually_exists = matches!(&actual, Some(v) if !v.is_null());
            if actually_exists != expected_exists {
                failures.push(format!(
                    "{} exists == {}  (got {})",
                    target, expected_exists, actually_exists
                ));
            }
        }

        if let Some(ref substr) = a.contains {
            let s = resolve(substr, env);
            match actual.as_ref() {
                Some(Value::String(v)) if v.contains(s.as_str()) => {}
                Some(Value::String(v)) => failures.push(format!(
                    "{} contains {:?}  (got {:?})", target, s, v
                )),
                _ => failures.push(format!(
                    "{} contains {:?}  (not a string: {})", target, s, fmt_opt(&actual)
                )),
            }
        }

        if let Some(ref pattern) = a.matches {
            let p = resolve(pattern, env);
            match Regex::new(&p) {
                Err(e) => failures.push(format!(
                    "{} matches {:?}  (invalid regex: {})", target, p, e
                )),
                Ok(re) => match actual.as_ref() {
                    Some(Value::String(v)) if re.is_match(v) => {}
                    Some(Value::String(v)) => failures.push(format!(
                        "{} matches {:?}  (got {:?})", target, p, v
                    )),
                    _ => failures.push(format!(
                        "{} matches {:?}  (not a string: {})", target, p, fmt_opt(&actual)
                    )),
                }
            }
        }
    }

    failures
}

// ── report ────────────────────────────────────────────────────────────────────

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
            format!("Iterations : {} ok  /  {} failed", iters_ok, iters_fail),
            width = width - 2);
    }
    println!("║  {:<width$}║",
        format!("Steps      : {} ok  /  {} failed  ({} total)", total_ok, total_fail, total_steps),
        width = width - 2);
    println!("║  {:<width$}║",
        format!("Duration   : {} ms", total_ms),
        width = width - 2);

    if total_fail > 0 {
        println!("╠{}╣", bar);
        println!("║  Failures:{:<width$}║", "", width = width - 10);
        for iter in results.iter().filter(|r| !r.success()) {
            if let Some(idx) = iter.row_index {
                let row_label = iter.row_vars.iter()
                    .map(|(k, v)| format!("{}={}", k, truncate(v, 20)))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!("║  Row {} — {:<width$}║",
                    idx + 1, row_label,
                    width = width - 10 - format!("Row {} — ", idx + 1).len());
            }
            for step in iter.steps.iter().filter(|s| !s.success) {
                let msg = step.error.as_deref().unwrap_or("unknown error");
                println!("║    ✗ {} {} — {:<width$}║",
                    step.method, truncate(&step.url, 30), msg,
                    width = width.saturating_sub(10 + step.method.len() + 30.min(step.url.len())));
                for af in &step.assertion_failures {
                    let line = format!("      · {}", af);
                    println!("║  {:<width$}║", truncate(&line, width - 2), width = width - 2);
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
    for (k, v) in env {
        out = out.replace(&format!("{{{{{}}}}}", k), v);
    }
    out
}

fn resolve_value(v: &Value, env: &HashMap<String, String>) -> Value {
    match v {
        Value::String(s) => Value::String(resolve(s, env)),
        other => other.clone(),
    }
}

/// Walk a JSON value with dot-separated path. Numeric segments index arrays.
fn extract_at(value: &Value, path: &str) -> Option<String> {
    extract_value_at(value, path).map(|v| match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    })
}

/// Like extract_at but returns the raw Value reference (no stringify).
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

/// Equality comparison with cross-type coercion (string "42" == number 42).
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
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn fmt_val(v: &Value) -> String {
    match v {
        Value::String(s) => format!("{:?}", s),
        other => other.to_string(),
    }
}

fn fmt_opt(v: &Option<Value>) -> String {
    match v {
        Some(v) => fmt_val(v),
        None    => "(none)".into(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}…", &s[..max]) }
}
