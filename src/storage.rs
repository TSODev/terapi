use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoredCollection {
    pub collection: CollectionMeta,
    #[serde(default)]
    pub folders: Vec<StoredFolder>,
    #[serde(default)]
    pub requests: Vec<StoredRequest>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CollectionMeta {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoredFolder {
    pub name: String,
    #[serde(default)]
    pub requests: Vec<StoredRequest>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct StoredAuth {
    #[serde(default)]
    pub auth_type: String,
    #[serde(default)]
    pub bearer_token: String,
    #[serde(default)]
    pub basic_username: String,
    #[serde(default)]
    pub basic_password: String,
    #[serde(default)]
    pub api_key_name: String,
    #[serde(default)]
    pub api_key_value: String,
    #[serde(default)]
    pub api_key_location: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoredRequest {
    pub name: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub auth: StoredAuth,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_follow_redirects")]
    pub follow_redirects: bool,
    #[serde(default)]
    pub skip_tls_verify: bool,
    #[serde(default)]
    pub cookie_jar: bool,
    // GraphQL
    #[serde(default)]
    pub graphql: bool,
    #[serde(default)]
    pub graphql_query: Option<String>,
    #[serde(default)]
    pub graphql_variables: HashMap<String, String>,
}

fn default_timeout() -> u64 { 30 }
fn default_follow_redirects() -> bool { true }

impl StoredRequest {
    pub fn new(name: impl Into<String>, method: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            method: method.into(),
            url: url.into(),
            headers: HashMap::new(),
            body: None,
            description: None,
            auth: StoredAuth::default(),
            timeout_secs: 30,
            follow_redirects: true,
            skip_tls_verify: false,
            cookie_jar: false,
            graphql: false,
            graphql_query: None,
            graphql_variables: HashMap::new(),
        }
    }
}

/// Resolve the terapi data directory using the following priority:
///   1. `TERAPI_DIR` environment variable
///   2. `./.terapi/` if the directory exists in the current working dir
///   3. `~/.config/terapi/` (XDG-compatible global fallback)
pub fn resolve_terapi_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("TERAPI_DIR") {
        return PathBuf::from(dir);
    }
    let local = PathBuf::from(".terapi");
    if local.is_dir() {
        return local;
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("terapi")
}

pub fn load_collections() -> Result<Vec<StoredCollection>> {
    let dir = resolve_terapi_dir().join("collections");

    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut entries: Vec<_> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "toml"))
        .collect();

    entries.sort_by_key(|e| e.file_name());

    let mut collections = Vec::new();
    for entry in entries {
        let content = std::fs::read_to_string(entry.path())?;
        let stored: StoredCollection = toml::from_str(&content)?;
        collections.push(stored);
    }

    Ok(collections)
}

pub fn save_collection(col: &StoredCollection) -> Result<()> {
    let dir = resolve_terapi_dir().join("collections");
    std::fs::create_dir_all(&dir)?;

    let filename = sanitize_filename(&col.collection.name);
    let path = dir.join(format!("{}.toml", filename));
    let content = toml::to_string_pretty(col)?;
    std::fs::write(path, content)?;
    Ok(())
}

pub fn delete_collection(name: &str) -> Result<()> {
    let dir = resolve_terapi_dir().join("collections");
    let path = dir.join(format!("{}.toml", sanitize_filename(name)));
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

// ── Environments ────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoredEnv {
    pub env: EnvMeta,
    #[serde(default)]
    pub vars: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EnvMeta {
    pub name: String,
}

pub fn load_envs() -> Result<Vec<StoredEnv>> {
    let dir = resolve_terapi_dir().join("envs");

    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut entries: Vec<_> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "toml"))
        .collect();

    entries.sort_by_key(|e| e.file_name());

    let mut envs = Vec::new();
    for entry in entries {
        let content = std::fs::read_to_string(entry.path())?;
        let stored: StoredEnv = toml::from_str(&content)?;
        envs.push(stored);
    }

    Ok(envs)
}

pub fn save_env(env: &StoredEnv) -> Result<()> {
    let dir = resolve_terapi_dir().join("envs");
    std::fs::create_dir_all(&dir)?;

    let filename = sanitize_filename(&env.env.name);
    let path = dir.join(format!("{}.toml", filename));
    let content = toml::to_string_pretty(env)?;
    std::fs::write(path, content)?;
    Ok(())
}

pub fn delete_env(name: &str) -> Result<()> {
    let dir = resolve_terapi_dir().join("envs");
    let path = dir.join(format!("{}.toml", sanitize_filename(name)));
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// Load a single environment by name from the terapi envs directory.
pub fn load_env_by_name(name: &str) -> Result<StoredEnv> {
    let path = resolve_terapi_dir()
        .join("envs")
        .join(format!("{}.toml", sanitize_filename(name)));
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read environment '{}' at {:?}", name, path))?;
    toml::from_str(&content)
        .with_context(|| format!("invalid TOML in environment file for '{}'", name))
}

/// Replace `{{VAR}}` placeholders in `text` using the given variable map.
pub fn resolve_vars(text: &str, vars: &std::collections::HashMap<String, String>) -> String {
    let mut out = text.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{{{}}}}}", k), v);
    }
    out
}

// ── History ──────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HistoryEntry {
    pub timestamp_secs: u64,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
    pub status: Option<u16>,
    pub elapsed_ms: Option<u64>,
    #[serde(default)]
    pub response_body: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct HistoryFile {
    #[serde(default)]
    entries: Vec<HistoryEntry>,
}

pub fn load_history() -> Result<Vec<HistoryEntry>> {
    let path = resolve_terapi_dir().join("history.toml");
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = std::fs::read_to_string(&path)?;
    let file: HistoryFile = toml::from_str(&content)?;
    Ok(file.entries)
}

pub fn save_history(entries: &[HistoryEntry]) -> Result<()> {
    let dir = resolve_terapi_dir();
    std::fs::create_dir_all(&dir)?;
    let file = HistoryFile { entries: entries.to_vec() };
    let content = toml::to_string_pretty(&file)?;
    std::fs::write(dir.join("history.toml"), content)?;
    Ok(())
}

pub fn format_timestamp(secs: u64) -> String {
    let s = (secs % 60) as u8;
    let m = ((secs / 60) % 60) as u8;
    let h = ((secs / 3600) % 24) as u8;
    let days = (secs / 86400) as u32;
    let (y, mo, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, h, m, s)
}

fn days_to_ymd(mut days: u32) -> (u32, u8, u8) {
    let mut year = 1970u32;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year { break; }
        days -= days_in_year;
        year += 1;
    }
    let months = if is_leap_year(year) {
        [31u8, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31u8, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u8;
    for &dim in &months {
        if days < dim as u32 { break; }
        days -= dim as u32;
        month += 1;
    }
    (year, month, days as u8 + 1)
}

fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

// ── Shared ───────────────────────────────────────────────────────────────────

pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
