use anyhow::{bail, Context, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;

// ── TOML schema ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct Campaign {
    pub campaign: Meta,
    #[serde(default)]
    pub env: HashMap<String, String>,
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

// ── loader ───────────────────────────────────────────────────────────────────

pub fn load(path: &str) -> Result<Campaign> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read campaign file '{}'", path))?;
    toml::from_str(&content)
        .with_context(|| format!("invalid TOML in '{}'", path))
}

// ── runner ───────────────────────────────────────────────────────────────────

pub async fn run(campaign: &Campaign) -> Result<()> {
    println!("Campaign : {}", campaign.campaign.name);
    if !campaign.campaign.description.is_empty() {
        println!("           {}", campaign.campaign.description);
    }
    println!();

    // Runtime env: starts from [env], grows with extracted values
    let mut env: HashMap<String, String> = campaign.env.clone();

    let client = reqwest::Client::new();

    for (i, step) in campaign.steps.iter().enumerate() {
        let prefix = format!("[{}/{}]", i + 1, campaign.steps.len());

        let url = resolve(&step.url, &env);
        let body = step.body.as_deref().map(|b| resolve(b, &env));

        println!("{} {} {}  — {}", prefix, step.method, url, step.name);

        // Build request
        let mut req = match step.method.to_uppercase().as_str() {
            "GET"    => client.get(&url),
            "POST"   => client.post(&url),
            "PUT"    => client.put(&url),
            "PATCH"  => client.patch(&url),
            "DELETE" => client.delete(&url),
            m => bail!("unknown HTTP method: {}", m),
        };

        let mut hmap = HeaderMap::new();
        for (k, v) in &step.headers {
            let name  = HeaderName::from_str(k)
                .with_context(|| format!("invalid header name: {}", k))?;
            let value = HeaderValue::from_str(&resolve(v, &env))
                .with_context(|| format!("invalid header value: {}", v))?;
            hmap.insert(name, value);
        }
        req = req.headers(hmap);

        if let Some(b) = body {
            req = req.body(b);
        }

        // Send
        let response = req.send().await
            .with_context(|| format!("request failed: {} {}", step.method, url))?;

        let status = response.status();
        let body_text = response.text().await?;

        println!("  → {} {}", status.as_u16(),
            status.canonical_reason().unwrap_or(""));

        // Extract variables from JSON response
        if !step.extract.is_empty() {
            match serde_json::from_str::<Value>(&body_text) {
                Ok(json) => {
                    for (var, path) in &step.extract {
                        match extract_at(&json, path) {
                            Some(value) => {
                                let preview = truncate(&value, 60);
                                println!("  extract {} = {}", var, preview);
                                env.insert(var.clone(), value);
                            }
                            None => {
                                println!("  extract {} — path '{}' not found", var, path);
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("  (response is not JSON, extraction skipped: {})", e);
                }
            }
        }

        if !status.is_success() {
            bail!("step '{}' failed with status {}", step.name, status);
        }

        println!();
    }

    println!("Campaign completed ({} steps).", campaign.steps.len());
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Replace every `{{VAR}}` occurrence in `s` with its value from `env`.
fn resolve(s: &str, env: &HashMap<String, String>) -> String {
    let mut out = s.to_string();
    for (k, v) in env {
        out = out.replace(&format!("{{{{{}}}}}", k), v);
    }
    out
}

/// Walk a `serde_json::Value` using a dot-separated path.
/// Segments that parse as usize index into arrays.
///
/// Examples:
///   "token"            → value["token"]
///   "user.id"          → value["user"]["id"]
///   "data.items.0.name"→ value["data"]["items"][0]["name"]
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
