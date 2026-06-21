use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

pub type Row = HashMap<String, String>;

// ── TOML schema ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ConnectorConfig {
    #[serde(rename = "type")]
    pub kind: String,
    pub path: String,
}

// ── public API ────────────────────────────────────────────────────────────────

/// Load all rows from a connector. Each row is a map of variable_name → value
/// that will be merged into the campaign env for one iteration.
pub fn load_rows(config: &ConnectorConfig) -> Result<Vec<Row>> {
    match config.kind.as_str() {
        "csv" => load_csv(&config.path),
        other => bail!("unknown connector type '{}' (supported: csv)", other),
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
