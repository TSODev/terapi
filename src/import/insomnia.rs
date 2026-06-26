#![allow(dead_code)]

use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;

use crate::storage::{
    CollectionMeta, EnvMeta, StoredAuth, StoredCollection, StoredEnv, StoredFolder, StoredRequest,
};
use super::ImportReport;

// ── Insomnia v4 JSON structures ───────────────────────────────────────────────

#[derive(Deserialize, Debug)]
pub struct InsomniaExport {
    pub resources: Vec<InsomniaResource>,
}

#[derive(Deserialize, Debug)]
pub struct InsomniaResource {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(rename = "_type")]
    pub resource_type: String,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "parentId", default)]
    pub parent_id: String,
    #[serde(default)]
    pub description: String,

    // Request fields
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub headers: Vec<InsomniaHeader>,
    pub body: Option<InsomniaBody>,
    pub authentication: Option<InsomniaAuth>,
    #[serde(default)]
    pub parameters: Vec<InsomniaParam>,

    // Environment fields
    #[serde(default)]
    pub data: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize, Debug)]
pub struct InsomniaHeader {
    pub name: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Deserialize, Debug)]
pub struct InsomniaBody {
    #[serde(rename = "mimeType", default)]
    pub mime_type: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub params: Vec<InsomniaParam>,
}

#[derive(Deserialize, Debug)]
pub struct InsomniaParam {
    pub name: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Deserialize, Debug)]
pub struct InsomniaAuth {
    #[serde(rename = "type", default)]
    pub auth_type: String,
    // Bearer
    pub token: Option<String>,
    // Basic
    pub username: Option<String>,
    pub password: Option<String>,
    // API Key
    pub key: Option<String>,
    pub value: Option<String>,
    #[serde(rename = "addTo")]
    pub add_to: Option<String>,
    // OAuth2
    #[serde(rename = "grantType")]
    pub grant_type: Option<String>,
    #[serde(rename = "accessTokenUrl")]
    pub access_token_url: Option<String>,
    #[serde(rename = "clientId")]
    pub client_id: Option<String>,
    #[serde(rename = "clientSecret")]
    pub client_secret: Option<String>,
    pub scope: Option<String>,
    #[serde(rename = "authorizationUrl")]
    pub authorization_url: Option<String>,
}

// ── Public entry point ────────────────────────────────────────────────────────

pub fn import_insomnia(content: &str) -> Result<ImportReport> {
    let export: InsomniaExport = serde_json::from_str(content)
        .map_err(|e| anyhow::anyhow!("failed to parse Insomnia export: {}", e))?;

    let resources = &export.resources;

    // Find workspace (collection root)
    let workspace = resources
        .iter()
        .find(|r| r.resource_type == "workspace")
        .ok_or_else(|| anyhow::anyhow!("no workspace found in Insomnia export"))?;

    let ws_id = &workspace.id;

    let mut report = ImportReport {
        source_name: workspace.name.clone(),
        format: "Insomnia v4".to_string(),
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

    // Index resources by id for quick lookup
    let by_id: HashMap<&str, &InsomniaResource> = resources
        .iter()
        .map(|r| (r.id.as_str(), r))
        .collect();

    // Find top-level folders (parentId = workspace)
    let root_folders: Vec<&InsomniaResource> = resources
        .iter()
        .filter(|r| r.resource_type == "request_group" && &r.parent_id == ws_id)
        .collect();

    // Find root requests (parentId = workspace, not in any folder)
    let root_requests_res: Vec<&InsomniaResource> = resources
        .iter()
        .filter(|r| r.resource_type == "request" && &r.parent_id == ws_id)
        .collect();

    // Build folders
    let mut folders: Vec<StoredFolder> = Vec::new();
    for folder_res in &root_folders {
        folders.push(build_folder(folder_res, resources, &by_id, &mut report)?);
    }

    // Build root requests
    let mut root_requests: Vec<StoredRequest> = Vec::new();
    for req_res in &root_requests_res {
        root_requests.push(build_request(req_res, &mut report)?);
    }

    let stored_col = StoredCollection {
        collection: CollectionMeta {
            name: workspace.name.clone(),
            description: workspace.description.clone(),
        },
        folders,
        requests: root_requests,
        path: String::new(),
    };

    // Save collection
    let dir = crate::storage::resolve_terapi_dir().join("collections");
    std::fs::create_dir_all(&dir)?;
    let filename = crate::storage::sanitize_filename(&stored_col.collection.name);
    let dest = dir.join(format!("{}.toml", filename));
    report.existed = dest.exists();
    std::fs::write(&dest, toml::to_string_pretty(&stored_col)?)?;
    report.dest = dest.to_string_lossy().to_string();

    // Import environments
    // Base environment: parentId = workspace
    // Sub-environments: parentId = base env id
    let base_envs: Vec<&InsomniaResource> = resources
        .iter()
        .filter(|r| r.resource_type == "environment" && &r.parent_id == ws_id)
        .collect();

    let mut total_env_vars = 0usize;
    let mut envs_created = 0usize;

    for base_env in &base_envs {
        // Sub-environments of this base env
        let sub_envs: Vec<&InsomniaResource> = resources
            .iter()
            .filter(|r| r.resource_type == "environment" && r.parent_id == base_env.id)
            .collect();

        if sub_envs.is_empty() {
            // Base env with no sub-envs → save as-is
            let (name, vars) = env_vars(base_env, &workspace.name);
            let count = vars.len();
            if count > 0 {
                save_env_file(&name, vars)?;
                total_env_vars += count;
                envs_created += 1;
            }
        } else {
            // Save each sub-env, merging base env vars
            let base_vars: HashMap<String, String> = env_data_to_map(&base_env.data);
            for sub_env in &sub_envs {
                let mut merged = base_vars.clone();
                merged.extend(env_data_to_map(&sub_env.data));
                let name = if sub_env.name.is_empty() {
                    workspace.name.clone()
                } else {
                    sub_env.name.clone()
                };
                let count = merged.len();
                if count > 0 {
                    save_env_file(&name, merged)?;
                    total_env_vars += count;
                    envs_created += 1;
                }
            }
        }
    }

    if envs_created > 0 {
        let label = if envs_created == 1 {
            "1 env created".to_string()
        } else {
            format!("{} envs created", envs_created)
        };
        report.env_created = Some((label, total_env_vars));
    }

    Ok(report)
}

// ── Folder builder ────────────────────────────────────────────────────────────

fn build_folder(
    folder_res: &InsomniaResource,
    all: &[InsomniaResource],
    by_id: &HashMap<&str, &InsomniaResource>,
    report: &mut ImportReport,
) -> Result<StoredFolder> {
    report.folders_imported += 1;
    let mut requests: Vec<StoredRequest> = Vec::new();

    for r in all {
        if r.parent_id != folder_res.id {
            continue;
        }
        if r.resource_type == "request" {
            requests.push(build_request(r, report)?);
        } else if r.resource_type == "request_group" {
            // Sub-folder: flatten with "SubFolder / Request" naming
            report.folders_imported += 1;
            for sub in all {
                if sub.parent_id == r.id && sub.resource_type == "request" {
                    let mut req = build_request(sub, report)?;
                    req.name = format!("{} / {}", r.name, sub.name);
                    requests.push(req);
                }
            }
        } else if r.resource_type == "grpc_request" || r.resource_type == "websocket_request" {
            report.scripts_ignored += 1;
        }
    }

    let _ = by_id;
    Ok(StoredFolder {
        name: folder_res.name.clone(),
        requests,
    })
}

// ── Request builder ───────────────────────────────────────────────────────────

fn build_request(r: &InsomniaResource, report: &mut ImportReport) -> Result<StoredRequest> {
    report.requests_imported += 1;

    let method = if r.method.is_empty() {
        "GET".to_string()
    } else {
        r.method.clone()
    };

    // URL: append query parameters if not already in URL
    let url = build_url(&r.url, &r.parameters);

    // Headers
    let mut headers: HashMap<String, String> = HashMap::new();
    for h in &r.headers {
        if !h.disabled && !h.name.is_empty() {
            headers.insert(h.name.clone(), h.value.clone());
        }
    }

    // Body
    let mut body: Option<String> = None;
    let mut graphql = false;
    let mut graphql_query: Option<String> = None;
    let mut graphql_variables: HashMap<String, String> = HashMap::new();

    if let Some(ref b) = r.body {
        match b.mime_type.as_str() {
            "application/graphql" => {
                graphql = true;
                // Insomnia stores GQL body as JSON: {"query":"...","variables":{}}
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&b.text) {
                    graphql_query = v["query"].as_str().map(|s| s.to_string());
                    if let Some(obj) = v["variables"].as_object() {
                        for (k, val) in obj {
                            graphql_variables.insert(k.clone(), json_str(val));
                        }
                    }
                } else {
                    // Plain query string (no JSON wrapper)
                    graphql_query = Some(b.text.clone());
                }
            }
            "application/x-www-form-urlencoded" => {
                let parts: Vec<String> = b
                    .params
                    .iter()
                    .filter(|p| !p.disabled)
                    .map(|p| format!("{}={}", p.name, p.value))
                    .collect();
                if !parts.is_empty() {
                    body = Some(parts.join("&"));
                    report.urlencoded_degraded += 1;
                }
            }
            "multipart/form-data" => {
                let lines: Vec<String> = b
                    .params
                    .iter()
                    .filter(|p| !p.disabled)
                    .map(|p| format!("{}: {}", p.name, p.value))
                    .collect();
                if !lines.is_empty() {
                    body = Some(lines.join("\n"));
                    report.formdata_degraded += 1;
                }
            }
            _ => {
                if !b.text.is_empty() {
                    body = Some(b.text.clone());
                }
            }
        }
    }

    // Auth
    let auth = build_auth(r.authentication.as_ref());

    let description = if r.description.is_empty() {
        None
    } else {
        Some(r.description.clone())
    };

    Ok(StoredRequest {
        name: r.name.clone(),
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

// ── Auth builder ──────────────────────────────────────────────────────────────

fn build_auth(auth: Option<&InsomniaAuth>) -> StoredAuth {
    let Some(a) = auth else {
        return empty_auth();
    };

    match a.auth_type.as_str() {
        "bearer" => StoredAuth {
            auth_type: "bearer".to_string(),
            bearer_token: a.token.clone().unwrap_or_default(),
            ..empty_auth()
        },
        "basic" => StoredAuth {
            auth_type: "basic".to_string(),
            basic_username: a.username.clone().unwrap_or_default(),
            basic_password: a.password.clone().unwrap_or_default(),
            ..empty_auth()
        },
        "apikey" => {
            let location = a.add_to.as_deref().unwrap_or("header");
            StoredAuth {
                auth_type: "apikey".to_string(),
                api_key_name: a.key.clone().unwrap_or_default(),
                api_key_value: a.value.clone().unwrap_or_default(),
                api_key_location: if location.contains("query") {
                    "query".to_string()
                } else {
                    "header".to_string()
                },
                ..empty_auth()
            }
        }
        "oauth2" => {
            let auth_url = a.authorization_url.clone().unwrap_or_default();
            let grant = a.grant_type.as_deref().unwrap_or("client_credentials");
            if grant.contains("authorization_code") || grant.contains("code") {
                StoredAuth {
                    auth_type: "oauth2_authorization_code".to_string(),
                    oauth2_token_url: a.access_token_url.clone().unwrap_or_default(),
                    oauth2_client_id: a.client_id.clone().unwrap_or_default(),
                    oauth2_client_secret: a.client_secret.clone().unwrap_or_default(),
                    oauth2_scope: a.scope.clone().unwrap_or_default(),
                    oauth2_auth_url: auth_url,
                    ..empty_auth()
                }
            } else {
                StoredAuth {
                    auth_type: "oauth2_client_credentials".to_string(),
                    oauth2_token_url: a.access_token_url.clone().unwrap_or_default(),
                    oauth2_client_id: a.client_id.clone().unwrap_or_default(),
                    oauth2_client_secret: a.client_secret.clone().unwrap_or_default(),
                    oauth2_scope: a.scope.clone().unwrap_or_default(),
                    ..empty_auth()
                }
            }
        }
        _ => empty_auth(),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn build_url(base: &str, params: &[InsomniaParam]) -> String {
    let active: Vec<(&str, &str)> = params
        .iter()
        .filter(|p| !p.disabled)
        .map(|p| (p.name.as_str(), p.value.as_str()))
        .collect();

    if active.is_empty() || base.contains('?') {
        return base.to_string();
    }

    let qs: Vec<String> = active
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();
    format!("{}?{}", base, qs.join("&"))
}

fn env_data_to_map(data: &HashMap<String, serde_json::Value>) -> HashMap<String, String> {
    data.iter()
        .map(|(k, v)| (k.clone(), json_str(v)))
        .collect()
}

fn env_vars(
    res: &InsomniaResource,
    workspace_name: &str,
) -> (String, HashMap<String, String>) {
    let name = if res.name.is_empty() || res.name == "Base Environment" {
        format!("{} vars", workspace_name)
    } else {
        res.name.clone()
    };
    (name, env_data_to_map(&res.data))
}

fn save_env_file(name: &str, vars: HashMap<String, String>) -> Result<()> {
    crate::storage::save_env(&StoredEnv {
        env: EnvMeta { name: name.to_string() },
        vars,
    })
}

fn empty_auth() -> StoredAuth {
    StoredAuth {
        oauth2_redirect_port: 9876,
        ..Default::default()
    }
}

fn json_str(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}
