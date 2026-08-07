pub mod insomnia;
pub mod openapi;
pub mod postman;

use anyhow::Result;
pub use postman::ImportReport;

/// Detect the JSON format (Postman collection, Postman environment, Insomnia v4,
/// OpenAPI 3.x) and dispatch to the right parser.
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

    // OpenAPI 3.x (JSON form) / Swagger 2.0 (rejected with a clear message downstream)
    if json.get("openapi").is_some() || json.get("swagger").is_some() {
        return openapi::import_openapi(content, false);
    }

    anyhow::bail!(
        "unrecognised JSON format — expected Postman v2.1 collection/environment, Insomnia v4 export, or OpenAPI 3.x document"
    )
}

/// Import an OpenAPI 3.x document in YAML form (the file extension already tells
/// us it can't be a terapi TOML file or a Postman/Insomnia JSON export).
pub fn import_yaml(content: &str) -> Result<ImportReport> {
    openapi::import_openapi(content, true)
}
