use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

pub type Row = HashMap<String, String>;

// ── TOML schema ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct ConnectorConfig {
    #[serde(rename = "type")]
    pub kind: String,
    /// File path (csv or json). Not required when from_step is set.
    #[serde(default)]
    pub path: String,
    /// JSON connector: dot-path to the array to iterate (optional — root if omitted).
    #[serde(default)]
    pub select: Option<String>,
    /// JSON connector: name of a seed step whose response body is used as source.
    /// When set, `path` is ignored.
    #[serde(default)]
    pub from_step: Option<String>,
}

// ── public API ────────────────────────────────────────────────────────────────

/// Load all rows from a connector. Each row is a map of variable_name → value
/// that will be merged into the campaign env for one iteration.
pub fn load_rows(config: &ConnectorConfig) -> Result<Vec<Row>> {
    match config.kind.as_str() {
        "csv"  => load_csv(&config.path),
        "json" => load_json(&config.path, config.select.as_deref()),
        other  => bail!("unknown connector type '{}' (supported: csv, json)", other),
    }
}

// ── connectors ────────────────────────────────────────────────────────────────

fn load_csv(path: &str) -> Result<Vec<Row>> {
    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(path)
        .with_context(|| format!("cannot open CSV file '{}'", path))?;

    let mut rows = Vec::new();
    for result in rdr.deserialize::<Row>() {
        let row = result.with_context(|| format!("invalid row in CSV '{}'", path))?;
        rows.push(row);
    }

    if rows.is_empty() {
        bail!("CSV file '{}' contains no data rows", path);
    }

    Ok(rows)
}

/// Parse rows from a JSON string — used by the `from_step` seed response path.
pub fn load_rows_from_json(json_str: &str, select: Option<&str>) -> Result<Vec<Row>> {
    let root: Value = serde_json::from_str(json_str)
        .context("seed step response is not valid JSON")?;
    json_to_rows(&root, select)
}

fn load_json(path: &str, select: Option<&str>) -> Result<Vec<Row>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("cannot open JSON file '{}'", path))?;
    let root: Value = serde_json::from_str(&content)
        .with_context(|| format!("invalid JSON in '{}'", path))?;
    json_to_rows(&root, select)
}

fn json_to_rows(root: &Value, select: Option<&str>) -> Result<Vec<Row>> {
    // Navigate to the target array via dot-path if provided.
    let target = if let Some(expr) = select.filter(|s| !s.is_empty()) {
        let mut cur = root;
        for segment in expr.split('.') {
            cur = if let Ok(idx) = segment.parse::<usize>() {
                cur.get(idx).with_context(|| format!("JSON path '{}': index {} out of bounds", expr, idx))?
            } else {
                cur.get(segment).with_context(|| format!("JSON path '{}': key '{}' not found", expr, segment))?
            };
        }
        cur
    } else {
        &root
    };

    let array = target.as_array()
        .with_context(|| format!("JSON connector: '{}' is not an array", select.unwrap_or("(root)")))?;

    if array.is_empty() {
        bail!("JSON connector: array at '{}' is empty", select.unwrap_or("(root)"));
    }

    let rows = array.iter().map(|elem| flatten_value(elem, "")).collect();
    Ok(rows)
}

/// Recursively flatten a JSON value into dot-notation variable names.
/// Scalars → stringified; objects → recurse with `prefix.key`; arrays → JSON string.
fn flatten_value(value: &Value, prefix: &str) -> Row {
    let mut row = Row::new();
    flatten_into(value, prefix, &mut row);
    row
}

fn flatten_into(value: &Value, prefix: &str, out: &mut Row) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() { k.clone() } else { format!("{}.{}", prefix, k) };
                flatten_into(v, &key, out);
            }
        }
        Value::Array(_) => {
            // Serialize nested arrays as JSON strings — use a transform step to split if needed.
            let key = if prefix.is_empty() { "value".to_string() } else { prefix.to_string() };
            out.insert(key, value.to_string());
        }
        Value::String(s) => {
            let key = if prefix.is_empty() { "value".to_string() } else { prefix.to_string() };
            out.insert(key, s.clone());
        }
        Value::Number(n) => {
            let key = if prefix.is_empty() { "value".to_string() } else { prefix.to_string() };
            out.insert(key, n.to_string());
        }
        Value::Bool(b) => {
            let key = if prefix.is_empty() { "value".to_string() } else { prefix.to_string() };
            out.insert(key, b.to_string());
        }
        Value::Null => {
            let key = if prefix.is_empty() { "value".to_string() } else { prefix.to_string() };
            out.insert(key, String::new());
        }
    }
}
