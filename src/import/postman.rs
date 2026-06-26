#![allow(dead_code)]

use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;

use crate::storage::{
    CollectionMeta, EnvMeta, StoredAuth, StoredCollection, StoredEnv, StoredFolder, StoredRequest,
};

// ── Postman v2.1 JSON structures ─────────────────────────────────────────────

#[derive(Deserialize, Debug)]
pub struct PostmanCollection {
    pub info: PostmanInfo,
    #[serde(default)]
    pub item: Vec<PostmanItem>,
    #[serde(default)]
    pub variable: Vec<PostmanVariable>,
    pub auth: Option<PostmanAuth>,
}

#[derive(Deserialize, Debug)]
pub struct PostmanInfo {
    pub name: String,
    pub description: Option<PostmanDescription>,
    #[serde(default)]
    pub schema: String,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum PostmanDescription {
    Simple(String),
    Object {
        content: Option<String>,
        #[serde(rename = "type")]
        _mime: Option<String>,
    },
}

#[derive(Deserialize, Debug)]
pub struct PostmanVariable {
    pub key: String,
    #[serde(default)]
    pub value: String,
    pub enabled: Option<bool>,
}

#[derive(Deserialize, Debug)]
pub struct PostmanItem {
    pub name: String,
    // folders have nested `item`; requests have `request`
    pub item: Option<Vec<PostmanItem>>,
    pub request: Option<PostmanRequest>,
    #[serde(default)]
    pub event: Vec<PostmanEvent>,
}

#[derive(Deserialize, Debug)]
pub struct PostmanRequest {
    pub method: Option<String>,
    pub url: Option<PostmanUrl>,
    #[serde(default)]
    pub header: Vec<PostmanHeader>,
    pub body: Option<PostmanBody>,
    pub auth: Option<PostmanAuth>,
    pub description: Option<PostmanDescription>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum PostmanUrl {
    Simple(String),
    Object {
        raw: Option<String>,
        #[serde(default)]
        query: Vec<PostmanQueryParam>,
    },
}

#[derive(Deserialize, Debug)]
pub struct PostmanHeader {
    pub key: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Deserialize, Debug)]
pub struct PostmanQueryParam {
    pub key: Option<String>,
    pub value: Option<String>,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Deserialize, Debug)]
pub struct PostmanBody {
    pub mode: Option<String>,
    pub raw: Option<String>,
    pub graphql: Option<PostmanGraphqlBody>,
    pub urlencoded: Option<Vec<PostmanKeyValue>>,
    pub formdata: Option<Vec<PostmanFormData>>,
}

#[derive(Deserialize, Debug)]
pub struct PostmanGraphqlBody {
    pub query: Option<String>,
    pub variables: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct PostmanKeyValue {
    pub key: String,
    pub value: Option<String>,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Deserialize, Debug)]
pub struct PostmanFormData {
    pub key: String,
    #[serde(rename = "type")]
    pub field_type: Option<String>,
    pub value: Option<String>,
    pub src: Option<String>,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Deserialize, Debug)]
pub struct PostmanAuth {
    #[serde(rename = "type")]
    pub auth_type: String,
    pub bearer: Option<Vec<PostmanAuthParam>>,
    pub basic: Option<Vec<PostmanAuthParam>>,
    pub apikey: Option<Vec<PostmanAuthParam>>,
    pub oauth2: Option<Vec<PostmanAuthParam>>,
}

#[derive(Deserialize, Debug)]
pub struct PostmanAuthParam {
    pub key: String,
    pub value: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
pub struct PostmanEvent {
    pub listen: Option<String>,
}

// ── Postman environment ───────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
pub struct PostmanEnvironment {
    pub name: String,
    #[serde(default)]
    pub values: Vec<PostmanEnvValue>,
}

#[derive(Deserialize, Debug)]
pub struct PostmanEnvValue {
    pub key: String,
    #[serde(default)]
    pub value: String,
    #[serde(default = "bool_true")]
    pub enabled: bool,
}

fn bool_true() -> bool {
    true
}

// ── Import report ─────────────────────────────────────────────────────────────

pub struct ImportReport {
    pub source_name: String,
    pub is_env_only: bool,
    pub requests_imported: usize,
    pub folders_imported: usize,
    pub scripts_ignored: usize,
    pub formdata_degraded: usize,
    pub urlencoded_degraded: usize,
    pub env_created: Option<(String, usize)>,
    pub dest: String,
    pub existed: bool,
}

impl ImportReport {
    pub fn print(&self) {
        println!();
        if self.is_env_only {
            println!("Import: {} (Postman environment)", self.source_name);
            println!();
            if let Some((ref name, count)) = self.env_created {
                println!("  ✓ {:>3} variables → env \"{}\"", count, name);
            }
        } else {
            println!("Import: {} (Postman v2.1)", self.source_name);
            println!();
            println!("  ✓ {:>3} requests imported", self.requests_imported);
            if self.folders_imported > 0 {
                println!("  ✓ {:>3} folders", self.folders_imported);
            }
            if let Some((ref name, count)) = self.env_created {
                println!("  ✓ {:>3} variables → env \"{}\"", count, name);
            }
            if self.scripts_ignored > 0 {
                println!("  ⚠ {:>3} pre-request/test scripts ignored", self.scripts_ignored);
            }
            if self.formdata_degraded > 0 {
                println!(
                    "  ⚠ {:>3} form-data bodies converted to raw text",
                    self.formdata_degraded
                );
            }
            if self.urlencoded_degraded > 0 {
                println!(
                    "  ⚠ {:>3} urlencoded bodies converted to raw text",
                    self.urlencoded_degraded
                );
            }
        }
        println!();
        let verb = if self.existed { "Updated " } else { "Saved  " };
        println!("  {} → {}", verb, self.dest);
        println!();
    }
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Detect and import a Postman JSON file (collection or environment).
pub fn import_postman(path: &str, content: &str) -> Result<ImportReport> {
    let json: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| anyhow::anyhow!("not valid JSON: {}", e))?;

    if json.get("_postman_variable_scope").is_some() {
        import_environment(content)
    } else if json
        .get("info")
        .and_then(|i| i.get("schema"))
        .and_then(|s| s.as_str())
        .map_or(false, |s| s.contains("postman"))
    {
        import_collection(path, content)
    } else {
        anyhow::bail!(
            "not a recognised Postman file (expected collection v2.1 or environment JSON)"
        )
    }
}

// ── Collection import ─────────────────────────────────────────────────────────

fn import_collection(path: &str, content: &str) -> Result<ImportReport> {
    let _ = path;
    let col: PostmanCollection = serde_json::from_str(content)
        .map_err(|e| anyhow::anyhow!("failed to parse Postman collection: {}", e))?;

    let mut report = ImportReport {
        source_name: col.info.name.clone(),
        is_env_only: false,
        requests_imported: 0,
        folders_imported: 0,
        scripts_ignored: 0,
        formdata_degraded: 0,
        urlencoded_degraded: 0,
        env_created: None,
        dest: String::new(),
        existed: false,
    };

    let mut root_requests: Vec<StoredRequest> = Vec::new();
    let mut folders: Vec<StoredFolder> = Vec::new();

    for item in &col.item {
        if item.item.is_some() {
            folders.push(parse_folder(item, &col.auth, &mut report)?);
        } else if let Some(ref req) = item.request {
            root_requests.push(parse_request(
                &item.name,
                req,
                &col.auth,
                &item.event,
                &mut report,
            )?);
        }
    }

    let stored_col = StoredCollection {
        collection: CollectionMeta {
            name: col.info.name.clone(),
            description: col.info.description.as_ref().map(desc_text).unwrap_or_default(),
        },
        folders,
        requests: root_requests,
        path: String::new(),
    };

    // Save
    let dir = crate::storage::resolve_terapi_dir().join("collections");
    std::fs::create_dir_all(&dir)?;
    let filename = crate::storage::sanitize_filename(&stored_col.collection.name);
    let dest = dir.join(format!("{}.toml", filename));
    report.existed = dest.exists();
    std::fs::write(&dest, toml::to_string_pretty(&stored_col)?)?;
    report.dest = dest.to_string_lossy().to_string();

    // Collection variables → env
    let vars: HashMap<String, String> = col
        .variable
        .iter()
        .filter(|v| v.enabled.unwrap_or(true) && !v.key.is_empty())
        .map(|v| (v.key.clone(), v.value.clone()))
        .collect();

    if !vars.is_empty() {
        let env_name = format!("{} vars", col.info.name);
        let count = vars.len();
        crate::storage::save_env(&StoredEnv {
            env: EnvMeta {
                name: env_name.clone(),
            },
            vars,
        })?;
        report.env_created = Some((env_name, count));
    }

    Ok(report)
}

// ── Environment import ────────────────────────────────────────────────────────

fn import_environment(content: &str) -> Result<ImportReport> {
    let env: PostmanEnvironment = serde_json::from_str(content)
        .map_err(|e| anyhow::anyhow!("failed to parse Postman environment: {}", e))?;

    let vars: HashMap<String, String> = env
        .values
        .iter()
        .filter(|v| v.enabled && !v.key.is_empty())
        .map(|v| (v.key.clone(), v.value.clone()))
        .collect();

    let count = vars.len();
    let stored_env = StoredEnv {
        env: EnvMeta {
            name: env.name.clone(),
        },
        vars,
    };

    let dir = crate::storage::resolve_terapi_dir().join("envs");
    std::fs::create_dir_all(&dir)?;
    let filename = crate::storage::sanitize_filename(&env.name);
    let dest = dir.join(format!("{}.toml", filename));
    let existed = dest.exists();
    crate::storage::save_env(&stored_env)?;

    Ok(ImportReport {
        source_name: env.name.clone(),
        is_env_only: true,
        requests_imported: 0,
        folders_imported: 0,
        scripts_ignored: 0,
        formdata_degraded: 0,
        urlencoded_degraded: 0,
        env_created: Some((env.name, count)),
        dest: dest.to_string_lossy().to_string(),
        existed,
    })
}

// ── Folder ────────────────────────────────────────────────────────────────────

fn parse_folder(
    item: &PostmanItem,
    parent_auth: &Option<PostmanAuth>,
    report: &mut ImportReport,
) -> Result<StoredFolder> {
    report.folders_imported += 1;
    let mut requests: Vec<StoredRequest> = Vec::new();

    if let Some(ref children) = item.item {
        for child in children {
            if let Some(ref sub_items) = child.item {
                // Nested sub-folder: flatten with "SubFolder / Request" naming
                report.folders_imported += 1;
                for sub in sub_items {
                    if let Some(ref req) = sub.request {
                        let name = format!("{} / {}", child.name, sub.name);
                        requests.push(parse_request(
                            &name,
                            req,
                            parent_auth,
                            &sub.event,
                            report,
                        )?);
                    }
                }
            } else if let Some(ref req) = child.request {
                requests.push(parse_request(
                    &child.name,
                    req,
                    parent_auth,
                    &child.event,
                    report,
                )?);
            }
        }
    }

    Ok(StoredFolder {
        name: item.name.clone(),
        requests,
    })
}

// ── Request ───────────────────────────────────────────────────────────────────

fn parse_request(
    name: &str,
    req: &PostmanRequest,
    parent_auth: &Option<PostmanAuth>,
    events: &[PostmanEvent],
    report: &mut ImportReport,
) -> Result<StoredRequest> {
    report.requests_imported += 1;

    // Scripts
    report.scripts_ignored += events
        .iter()
        .filter(|e| {
            e.listen
                .as_deref()
                .map_or(false, |l| l == "prerequest" || l == "test")
        })
        .count();

    let method = req.method.clone().unwrap_or_else(|| "GET".to_string());

    // URL — always use the `raw` field which already includes query string
    let url = match &req.url {
        None => String::new(),
        Some(PostmanUrl::Simple(s)) => s.clone(),
        Some(PostmanUrl::Object { raw, .. }) => raw.clone().unwrap_or_default(),
    };

    // Headers (skip disabled)
    let mut headers: HashMap<String, String> = HashMap::new();
    for h in &req.header {
        if !h.disabled && !h.key.is_empty() {
            headers.insert(h.key.clone(), h.value.clone());
        }
    }

    // Body
    let mut body: Option<String> = None;
    let mut graphql = false;
    let mut graphql_query: Option<String> = None;
    let mut graphql_variables: HashMap<String, String> = HashMap::new();

    if let Some(ref b) = req.body {
        match b.mode.as_deref() {
            Some("raw") => {
                body = b.raw.clone().filter(|s| !s.is_empty());
            }
            Some("graphql") => {
                graphql = true;
                if let Some(ref gql) = b.graphql {
                    graphql_query = gql.query.clone();
                    if let Some(ref vars_str) = gql.variables {
                        if let Ok(serde_json::Value::Object(map)) =
                            serde_json::from_str(vars_str)
                        {
                            for (k, v) in map {
                                graphql_variables.insert(k, json_value_to_string(&v));
                            }
                        }
                    }
                }
            }
            Some("urlencoded") => {
                if let Some(ref pairs) = b.urlencoded {
                    let parts: Vec<String> = pairs
                        .iter()
                        .filter(|p| !p.disabled)
                        .map(|p| {
                            format!("{}={}", p.key, p.value.as_deref().unwrap_or(""))
                        })
                        .collect();
                    if !parts.is_empty() {
                        body = Some(parts.join("&"));
                        report.urlencoded_degraded += 1;
                    }
                }
            }
            Some("formdata") => {
                if let Some(ref parts) = b.formdata {
                    let lines: Vec<String> = parts
                        .iter()
                        .filter(|p| !p.disabled)
                        .map(|p| {
                            if p.field_type.as_deref() == Some("file") {
                                format!(
                                    "{}: @{}",
                                    p.key,
                                    p.src.as_deref().unwrap_or("")
                                )
                            } else {
                                format!(
                                    "{}: {}",
                                    p.key,
                                    p.value.as_deref().unwrap_or("")
                                )
                            }
                        })
                        .collect();
                    if !lines.is_empty() {
                        body = Some(lines.join("\n"));
                        report.formdata_degraded += 1;
                    }
                }
            }
            _ => {}
        }
    }

    // Auth: request-level > parent (collection/folder)
    let effective_auth = req.auth.as_ref().or(parent_auth.as_ref());
    let auth = parse_auth(effective_auth);

    let description = req
        .description
        .as_ref()
        .map(desc_text)
        .filter(|s| !s.is_empty());

    Ok(StoredRequest {
        name: name.to_string(),
        method,
        url,
        headers,
        body,
        description,
        auth,
        timeout_secs: 30,
        follow_redirects: true,
        skip_tls_verify: false,
        cookie_jar: false,
        graphql,
        graphql_query,
        graphql_variables,
    })
}

// ── Auth ──────────────────────────────────────────────────────────────────────

fn empty_auth() -> StoredAuth {
    StoredAuth {
        oauth2_redirect_port: 9876,
        ..Default::default()
    }
}

fn parse_auth(auth: Option<&PostmanAuth>) -> StoredAuth {
    let Some(auth) = auth else {
        return empty_auth();
    };

    match auth.auth_type.as_str() {
        "noauth" | "none" => empty_auth(),
        "bearer" => StoredAuth {
            auth_type: "bearer".to_string(),
            bearer_token: auth_param(auth.bearer.as_deref(), "token"),
            ..empty_auth()
        },
        "basic" => StoredAuth {
            auth_type: "basic".to_string(),
            basic_username: auth_param(auth.basic.as_deref(), "username"),
            basic_password: auth_param(auth.basic.as_deref(), "password"),
            ..empty_auth()
        },
        "apikey" => {
            let location = auth_param(auth.apikey.as_deref(), "in");
            StoredAuth {
                auth_type: "apikey".to_string(),
                api_key_name: auth_param(auth.apikey.as_deref(), "key"),
                api_key_value: auth_param(auth.apikey.as_deref(), "value"),
                api_key_location: if location == "query" {
                    "query".to_string()
                } else {
                    "header".to_string()
                },
                ..empty_auth()
            }
        }
        "oauth2" => StoredAuth {
            auth_type: "oauth2_client_credentials".to_string(),
            oauth2_token_url: auth_param(auth.oauth2.as_deref(), "accessTokenUrl"),
            oauth2_client_id: auth_param(auth.oauth2.as_deref(), "clientId"),
            oauth2_client_secret: auth_param(auth.oauth2.as_deref(), "clientSecret"),
            oauth2_scope: auth_param(auth.oauth2.as_deref(), "scope"),
            oauth2_redirect_port: 9876,
            ..empty_auth()
        },
        _ => empty_auth(),
    }
}

fn auth_param(params: Option<&[PostmanAuthParam]>, key: &str) -> String {
    params
        .unwrap_or(&[])
        .iter()
        .find(|p| p.key == key)
        .and_then(|p| p.value.as_ref())
        .map(json_value_to_string)
        .unwrap_or_default()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn desc_text(d: &PostmanDescription) -> String {
    match d {
        PostmanDescription::Simple(s) => s.clone(),
        PostmanDescription::Object { content, .. } => content.clone().unwrap_or_default(),
    }
}

fn json_value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
