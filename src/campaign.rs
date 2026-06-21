use anyhow::{bail, Context, Result};
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
    /// One connector = one data source; the campaign runs once per row.
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
    /// variable_name = "dot.path.in.response"
    #[serde(default)]
    pub extract: HashMap<String, String>,
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
}

#[derive(Debug)]
pub struct IterationResult {
    /// Row index (0-based). None for single-run (no connector).
    pub row_index: Option<usize>,
    /// A snapshot of connector row vars for display.
    pub row_vars: Row,
    pub steps: Vec<StepResult>,
}

impl IterationResult {
    pub fn ok_count(&self)   -> usize { self.steps.iter().filter(|s| s.success).count() }
    pub fn fail_count(&self) -> usize { self.steps.iter().filter(|s| !s.success).count() }
    pub fn total_ms(&self)   -> u64   { self.steps.iter().map(|s| s.duration_ms).sum() }
    pub fn success(&self)    -> bool  { self.steps.iter().all(|s| s.success) }
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

    // Build iteration list from connectors
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
        let mut env = campaign.env.clone();
        env.extend(row_vars.clone());

        if use_iter_prefix {
            let row_summary = row_vars.iter()
                .map(|(k, v)| format!("{}={}", k, truncate(v, 20)))
                .collect::<Vec<_>>()
                .join("  ");
            out!("── Row {}/{} — {} ", idx + 1, total_iters, row_summary);
        }

        let step_results = run_steps(&client, &campaign.steps, &mut env).await;

        for sr in &step_results {
            let status_str = sr.status
                .map(|s| format!("{}", s))
                .unwrap_or_else(|| "ERR".into());
            let result_str = if sr.success { "✓" } else { "✗" };
            out!("  {} {:<22} {:<7} {}  {:>6} ms  {}",
                result_str, sr.name, sr.method, status_str, sr.duration_ms,
                sr.error.as_deref().unwrap_or(""));
            for (var, val) in &sr.extracted {
                out!("      ↳ {} = {}", var, truncate(val, 60));
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

    // In silent mode, propagate failure as a non-zero exit code
    let total_fail: usize = all.iter().map(|r| r.fail_count()).sum();
    if total_fail > 0 {
        std::process::exit(1);
    }

    Ok(())
}

// ── step execution ────────────────────────────────────────────────────────────

/// Run all steps for one iteration. Errors are captured into StepResult,
/// not propagated — execution stops at the first failure.
async fn run_steps(
    client: &reqwest::Client,
    steps: &[Step],
    env: &mut HashMap<String, String>,
) -> Vec<StepResult> {
    let mut results = Vec::new();

    for step in steps {
        let url  = resolve(&step.url, env);
        let body = step.body.as_deref().map(|b| resolve(b, env));
        let t0   = Instant::now();

        let outcome = execute_step(client, step, &url, body.as_deref(), env).await;
        let duration_ms = t0.elapsed().as_millis() as u64;

        match outcome {
            Ok((status, extracted)) => {
                let success = status < 400;
                for (k, v) in &extracted {
                    env.insert(k.clone(), v.clone());
                }
                let error = if success { None } else { Some(format!("HTTP {}", status)) };
                let failed = !success;
                results.push(StepResult {
                    name: step.name.clone(),
                    method: step.method.clone(),
                    url,
                    status: Some(status),
                    duration_ms,
                    success,
                    error,
                    extracted,
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
) -> Result<(u16, HashMap<String, String>)> {
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
    let body_text = response.text().await?;

    // Extract variables
    let mut extracted: HashMap<String, String> = HashMap::new();
    if !step.extract.is_empty() {
        if let Ok(json) = serde_json::from_str::<Value>(&body_text) {
            for (var, path) in &step.extract {
                if let Some(value) = extract_at(&json, path) {
                    extracted.insert(var.clone(), value);
                }
            }
        }
    }

    Ok((status, extracted))
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
                println!("║  Row {} — {:<width$}║", idx + 1, row_label, width = width - 10 - format!("Row {} — ", idx + 1).len());
            }
            for step in iter.steps.iter().filter(|s| !s.success) {
                let msg = step.error.as_deref().unwrap_or("unknown error");
                println!("║    ✗ {} {} — {:<width$}║",
                    step.method, truncate(&step.url, 30), msg,
                    width = width.saturating_sub(10 + step.method.len() + 30.min(step.url.len())));
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

/// Walk a JSON value with dot-separated path. Numeric segments index arrays.
fn extract_at(value: &Value, path: &str) -> Option<String> {
    let mut current = value;
    for segment in path.split('.') {
        current = if let Ok(idx) = segment.parse::<usize>() {
            current.get(idx)?
        } else {
            current.get(segment)?
        };
    }
    Some(match current {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    })
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
