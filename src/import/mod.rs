pub mod insomnia;
pub mod postman;

use anyhow::Result;
pub use postman::ImportReport;

/// Detect the JSON format (Postman collection, Postman environment, Insomnia v4)
/// and dispatch to the right parser.
pub fn import_json(path: &str, content: &str) -> Result<ImportReport> {
    let json: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| anyhow::anyhow!("not valid JSON: {}", e))?;

    // Insomnia v4 export
    if json.get("_type").and_then(|v| v.as_str()) == Some("export")
        && json.get("resources").is_some()
    {
        return insomnia::import_insomnia(content);
    }

    // Postman environment
    if json.get("_postman_variable_scope").is_some() {
        return postman::import_postman(path, content);
    }

    // Postman collection v2.x
    if json
        .get("info")
        .and_then(|i| i.get("schema"))
        .and_then(|s| s.as_str())
        .map_or(false, |s| s.contains("postman"))
    {
        return postman::import_postman(path, content);
    }

    anyhow::bail!(
        "unrecognised JSON format — expected Postman v2.1 collection/environment or Insomnia v4 export"
    )
}
