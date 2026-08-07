#![allow(dead_code)]

//! OpenAPI 3.x → terapi collection import (static, one-shot — like the Postman/
//! Insomnia importers, no live spec browsing and no re-import/merge logic).
//!
//! Parses a deliberately minimal, lenient subset of the spec (hand-rolled structs,
//! not a strict `openapiv3`-style crate) so real-world specs with minor deviations
//! still import something useful rather than failing outright — same philosophy as
//! `postman.rs`/`insomnia.rs`. `$ref` is only resolved for `components.schemas`
//! (request body shapes); `$ref`-only parameters and Swagger 2.0 documents are
//! reported/rejected rather than guessed at.

use anyhow::Result;
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::Value as Json;
use std::collections::HashMap;

use crate::storage::{
    CollectionMeta, EnvMeta, StoredCollection, StoredEnv, StoredFolder, StoredRequest,
};

use super::ImportReport;

// ── OpenAPI 3.x structures (minimal, lenient) ──────────────────────────────────

#[derive(Deserialize, Debug)]
struct OpenApiDoc {
    openapi: Option<String>,
    /// Presence alone means "this is Swagger 2.0, not OpenAPI 3.x" — reported as an error.
    swagger: Option<String>,
    info: OpenApiInfo,
    #[serde(default)]
    servers: Vec<OpenApiServer>,
    #[serde(default)]
    paths: IndexMap<String, PathItem>,
    #[serde(default)]
    components: Components,
    /// Document-level default security requirement, used when an operation has none of its own.
    security: Option<Vec<IndexMap<String, Vec<String>>>>,
}

#[derive(Deserialize, Debug)]
struct OpenApiInfo {
    title: String,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Deserialize, Debug)]
struct OpenApiServer {
    url: String,
}

#[derive(Deserialize, Debug, Default)]
struct PathItem {
    #[serde(default)]
    parameters: Vec<Parameter>,
    get: Option<Operation>,
    post: Option<Operation>,
    put: Option<Operation>,
    patch: Option<Operation>,
    delete: Option<Operation>,
    // head/options/trace deliberately unmapped — terapi's interactive Request tab
    // only cycles GET/POST/PUT/PATCH/DELETE, so importing them would silently
    // degrade to GET the moment the request is loaded in the TUI.
}

impl PathItem {
    fn operations(&self) -> Vec<(&'static str, &Operation)> {
        [
            ("GET", &self.get),
            ("POST", &self.post),
            ("PUT", &self.put),
            ("PATCH", &self.patch),
            ("DELETE", &self.delete),
        ]
        .into_iter()
        .filter_map(|(m, op)| op.as_ref().map(|o| (m, o)))
        .collect()
    }
}

#[derive(Deserialize, Debug, Default)]
struct Operation {
    #[serde(rename = "operationId")]
    operation_id: Option<String>,
    summary: Option<String>,
    description: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    parameters: Vec<Parameter>,
    #[serde(rename = "requestBody")]
    request_body: Option<RequestBody>,
    security: Option<Vec<IndexMap<String, Vec<String>>>>,
}

#[derive(Deserialize, Debug, Default)]
struct Parameter {
    #[serde(rename = "$ref")]
    r#ref: Option<String>,
    #[serde(default)]
    name: String,
    #[serde(rename = "in", default)]
    location: String,
    #[serde(default)]
    required: bool,
    schema: Option<Schema>,
    example: Option<Json>,
}

#[derive(Deserialize, Debug, Default)]
struct RequestBody {
    #[serde(default)]
    content: IndexMap<String, MediaType>,
}

#[derive(Deserialize, Debug, Default)]
struct MediaType {
    schema: Option<Schema>,
    example: Option<Json>,
    #[serde(default)]
    examples: IndexMap<String, ExampleObj>,
}

#[derive(Deserialize, Debug, Default)]
struct ExampleObj {
    value: Option<Json>,
}

#[derive(Deserialize, Debug, Default, Clone)]
struct Schema {
    #[serde(rename = "$ref")]
    r#ref: Option<String>,
    #[serde(rename = "type")]
    schema_type: Option<String>,
    #[serde(default)]
    properties: IndexMap<String, Schema>,
    items: Option<Box<Schema>>,
    example: Option<Json>,
    default: Option<Json>,
    #[serde(rename = "enum")]
    enum_values: Option<Vec<Json>>,
}

#[derive(Deserialize, Debug, Default)]
struct Components {
    #[serde(default)]
    schemas: IndexMap<String, Schema>,
    #[serde(rename = "securitySchemes", default)]
    security_schemes: IndexMap<String, SecurityScheme>,
}

#[derive(Deserialize, Debug)]
struct SecurityScheme {
    #[serde(rename = "type")]
    scheme_type: String,
    scheme: Option<String>,
    #[serde(rename = "in")]
    location: Option<String>,
    name: Option<String>,
    flows: Option<OAuthFlows>,
}

#[derive(Deserialize, Debug, Default)]
struct OAuthFlows {
    #[serde(rename = "clientCredentials")]
    client_credentials: Option<OAuthFlow>,
    #[serde(rename = "authorizationCode")]
    authorization_code: Option<OAuthFlow>,
}

#[derive(Deserialize, Debug, Default)]
struct OAuthFlow {
    #[serde(rename = "tokenUrl")]
    token_url: Option<String>,
    #[serde(rename = "authorizationUrl")]
    authorization_url: Option<String>,
    #[serde(default)]
    scopes: IndexMap<String, String>,
}

// ── Public entry point ─────────────────────────────────────────────────────────

/// Import an OpenAPI 3.x document, JSON or YAML.
pub fn import_openapi(content: &str, is_yaml: bool) -> Result<ImportReport> {
    let doc: OpenApiDoc = if is_yaml {
        serde_yaml::from_str(content).map_err(|e| anyhow::anyhow!("failed to parse OpenAPI YAML: {}", e))?
    } else {
        serde_json::from_str(content).map_err(|e| anyhow::anyhow!("failed to parse OpenAPI JSON: {}", e))?
    };

    if doc.openapi.is_none() {
        if doc.swagger.is_some() {
            anyhow::bail!("Swagger 2.0 is not supported — only OpenAPI 3.x documents (a top-level `openapi: 3.x.x` field)");
        }
        anyhow::bail!("not a recognised OpenAPI document (missing top-level `openapi` field)");
    }

    let mut notes: Vec<String> = Vec::new();
    let mut env_vars: HashMap<String, String> = HashMap::new();

    let base_url = doc.servers.first().map(|s| s.url.clone()).unwrap_or_default();
    if !base_url.is_empty() {
        env_vars.insert("base_url".to_string(), base_url);
    } else {
        notes.push("no `servers` entry found — base_url left blank in the generated env".to_string());
    }

    let mut folders: IndexMap<String, Vec<StoredRequest>> = IndexMap::new();
    let mut root_requests: Vec<StoredRequest> = Vec::new();
    let mut ref_params_skipped = 0usize;

    for (path, item) in &doc.paths {
        for (method, op) in item.operations() {
            let req = build_request(
                path,
                method,
                op,
                &item.parameters,
                &doc,
                &mut env_vars,
                &mut ref_params_skipped,
            );
            match op.tags.first() {
                Some(tag) => folders.entry(tag.clone()).or_default().push(req),
                None => root_requests.push(req),
            }
        }
    }

    if ref_params_skipped > 0 {
        notes.push(format!(
            "{} parameter(s) using `$ref` were skipped (only inline parameters are resolved)",
            ref_params_skipped
        ));
    }

    let stored_folders: Vec<StoredFolder> = folders
        .into_iter()
        .map(|(name, requests)| StoredFolder { name, requests })
        .collect();

    let requests_imported = stored_folders.iter().map(|f| f.requests.len()).sum::<usize>() + root_requests.len();
    let folders_imported = stored_folders.len();

    let stored_col = StoredCollection {
        collection: CollectionMeta {
            name: doc.info.title.clone(),
            description: doc.info.description.clone().unwrap_or_default(),
        },
        folders: stored_folders,
        requests: root_requests,
        path: String::new(),
    };

    let dir = crate::storage::resolve_terapi_dir().join("collections");
    std::fs::create_dir_all(&dir)?;
    let filename = crate::storage::sanitize_filename(&stored_col.collection.name);
    let dest = dir.join(format!("{}.toml", filename));
    let existed = dest.exists();
    std::fs::write(&dest, toml::to_string_pretty(&stored_col)?)?;

    let mut env_created = None;
    if !env_vars.is_empty() {
        let env_name = format!("{} vars", doc.info.title);
        let count = env_vars.len();
        crate::storage::save_env(&StoredEnv {
            env: EnvMeta { name: env_name.clone(), sensitive: false },
            vars: env_vars,
        })?;
        env_created = Some((env_name, count));
    }

    Ok(ImportReport {
        source_name: doc.info.title,
        format: format!("OpenAPI {}", doc.openapi.unwrap_or_default()),
        is_env_only: false,
        requests_imported,
        folders_imported,
        scripts_ignored: 0,
        formdata_degraded: 0,
        urlencoded_degraded: 0,
        env_created,
        dest: dest.to_string_lossy().to_string(),
        existed,
        notes,
    })
}

// ── Request construction ───────────────────────────────────────────────────────

fn build_request(
    path: &str,
    method: &str,
    op: &Operation,
    path_level_params: &[Parameter],
    doc: &OpenApiDoc,
    env_vars: &mut HashMap<String, String>,
    ref_params_skipped: &mut usize,
) -> StoredRequest {
    // OpenAPI's `{param}` path templating is already terapi's `{{param}}` syntax
    // once braces are doubled.
    let url_path = path.replace('{', "{{").replace('}', "}}");
    let name = op
        .summary
        .clone()
        .or_else(|| op.operation_id.clone())
        .unwrap_or_else(|| format!("{} {}", method, path));

    let mut req = StoredRequest::new(name, method.to_string(), format!("{{{{base_url}}}}{}", url_path));
    req.description = op.description.clone();
    // StoredAuth's #[derive(Default)] gives 0, not the app's actual default of 9876
    // (only `#[serde(default = "...")]` — deserialization — uses the custom function).
    // Postman's own importer works around the same quirk in its `empty_auth()`.
    req.auth.oauth2_redirect_port = 9876;

    let mut query_pairs: Vec<String> = Vec::new();

    for p in path_level_params.iter().chain(op.parameters.iter()) {
        if p.r#ref.is_some() {
            *ref_params_skipped += 1;
            continue;
        }
        if p.name.is_empty() {
            continue;
        }
        let placeholder = format!("{{{{{}}}}}", p.name);
        match p.location.as_str() {
            "query" => query_pairs.push(format!("{}={}", p.name, placeholder)),
            "header" => {
                req.headers.insert(p.name.clone(), placeholder);
            }
            // "path" needs no extra wiring — it's already embedded via {{name}} above.
            _ => {}
        }
        let example = p
            .example
            .clone()
            .or_else(|| p.schema.as_ref().and_then(|s| s.example.clone().or_else(|| s.default.clone())))
            .map(|v| json_to_env_string(&v))
            .unwrap_or_default();
        env_vars.entry(p.name.clone()).or_insert(example);
    }

    if !query_pairs.is_empty() {
        req.url = format!("{}?{}", req.url, query_pairs.join("&"));
    }

    if let Some(rb) = &op.request_body {
        if let Some(mt) = rb.content.get("application/json") {
            let body_value = mt
                .example
                .clone()
                .or_else(|| mt.examples.values().find_map(|e| e.value.clone()))
                .or_else(|| mt.schema.as_ref().map(|s| synthesize_example(s, &doc.components.schemas, 0)));
            if let Some(v) = body_value {
                if let Ok(pretty) = serde_json::to_string_pretty(&v) {
                    req.body = Some(pretty);
                    req.headers
                        .entry("Content-Type".to_string())
                        .or_insert_with(|| "application/json".to_string());
                }
            }
        }
    }

    let security = op.security.as_ref().or(doc.security.as_ref());
    if let Some(security) = security {
        if let Some(scheme_name) = security.iter().flat_map(|req| req.keys()).next() {
            if let Some(scheme) = doc.components.security_schemes.get(scheme_name) {
                apply_security_scheme(&mut req, scheme_name, scheme, env_vars);
            }
        }
    }

    req
}

/// A parameter's example/default value, flattened to the plain string terapi's
/// `{{VAR}}` substitution expects (not a JSON-encoded string).
fn json_to_env_string(v: &Json) -> String {
    match v {
        Json::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Best-effort example body from a JSON Schema: prefers `example`/`default`/`enum`
/// at each level, otherwise synthesizes a placeholder value from `type`. Follows
/// `$ref` into `components.schemas` up to a small depth limit (guards against
/// circular refs in a malformed spec) — the one piece of `$ref` resolution this
/// importer does, since request-body schemas overwhelmingly use it in practice.
fn synthesize_example(schema: &Schema, schemas: &IndexMap<String, Schema>, depth: u8) -> Json {
    if depth > 8 {
        return Json::Null;
    }
    if let Some(r) = &schema.r#ref {
        return r
            .rsplit('/')
            .next()
            .and_then(|name| schemas.get(name))
            .map(|resolved| synthesize_example(resolved, schemas, depth + 1))
            .unwrap_or(Json::Null);
    }
    if let Some(example) = &schema.example {
        return example.clone();
    }
    if let Some(default) = &schema.default {
        return default.clone();
    }
    if let Some(first) = schema.enum_values.as_ref().and_then(|v| v.first()) {
        return first.clone();
    }
    match schema.schema_type.as_deref() {
        Some("array") => {
            let item = schema
                .items
                .as_ref()
                .map(|i| synthesize_example(i, schemas, depth + 1))
                .unwrap_or(Json::Null);
            Json::Array(vec![item])
        }
        Some("string") => Json::String(String::new()),
        Some("integer") => Json::Number(0.into()),
        Some("number") => serde_json::json!(0.0),
        Some("boolean") => Json::Bool(false),
        Some("object") | None if !schema.properties.is_empty() => {
            let mut map = serde_json::Map::new();
            for (k, v) in &schema.properties {
                map.insert(k.clone(), synthesize_example(v, schemas, depth + 1));
            }
            Json::Object(map)
        }
        _ => Json::Null,
    }
}

/// Maps a `components.securitySchemes` entry onto terapi's `StoredAuth`, seeding
/// placeholder env vars for whatever secret the scheme needs. OAuth2 comes out
/// more complete than Postman's own importer, since OpenAPI specs carry the real
/// `tokenUrl`/`authorizationUrl` rather than a value the user has to supply.
fn apply_security_scheme(
    req: &mut StoredRequest,
    scheme_name: &str,
    scheme: &SecurityScheme,
    env_vars: &mut HashMap<String, String>,
) {
    match (scheme.scheme_type.as_str(), scheme.scheme.as_deref()) {
        ("http", Some("bearer")) => {
            let var = format!("{}_token", scheme_name);
            req.auth.auth_type = "bearer".to_string();
            req.auth.bearer_token = format!("{{{{{}}}}}", var);
            env_vars.entry(var).or_default();
        }
        ("http", Some("basic")) => {
            req.auth.auth_type = "basic".to_string();
            req.auth.basic_username = "{{username}}".to_string();
            req.auth.basic_password = "{{password}}".to_string();
            env_vars.entry("username".to_string()).or_default();
            env_vars.entry("password".to_string()).or_default();
        }
        ("apiKey", _) => {
            let var = format!("{}_key", scheme_name);
            req.auth.auth_type = "apikey".to_string();
            req.auth.api_key_name = scheme.name.clone().unwrap_or_default();
            req.auth.api_key_value = format!("{{{{{}}}}}", var);
            req.auth.api_key_location = if scheme.location.as_deref() == Some("query") {
                "query".to_string()
            } else {
                "header".to_string()
            };
            env_vars.entry(var).or_default();
        }
        ("oauth2", _) => {
            if let Some(flows) = &scheme.flows {
                if let Some(flow) = &flows.client_credentials {
                    req.auth.auth_type = "oauth2_client_credentials".to_string();
                    req.auth.oauth2_token_url = flow.token_url.clone().unwrap_or_default();
                    req.auth.oauth2_scope = flow.scopes.keys().cloned().collect::<Vec<_>>().join(" ");
                } else if let Some(flow) = &flows.authorization_code {
                    req.auth.auth_type = "oauth2_authorization_code".to_string();
                    req.auth.oauth2_token_url = flow.token_url.clone().unwrap_or_default();
                    req.auth.oauth2_auth_url = flow.authorization_url.clone().unwrap_or_default();
                    req.auth.oauth2_scope = flow.scopes.keys().cloned().collect::<Vec<_>>().join(" ");
                }
                if !req.auth.auth_type.is_empty() {
                    req.auth.oauth2_client_id = "{{client_id}}".to_string();
                    req.auth.oauth2_client_secret = "{{client_secret}}".to_string();
                    env_vars.entry("client_id".to_string()).or_default();
                    env_vars.entry("client_secret".to_string()).or_default();
                }
            }
        }
        _ => {}
    }
}
