use anyhow::{Context, Result};
use chrono::{Duration, Local};
use rand::Rng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoredCollection {
    pub collection: CollectionMeta,
    #[serde(default)]
    pub folders: Vec<StoredFolder>,
    #[serde(default)]
    pub requests: Vec<StoredRequest>,
    #[serde(skip)]
    pub path: String,
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
    // OAuth2
    #[serde(default)]
    pub oauth2_token_url: String,
    #[serde(default)]
    pub oauth2_client_id: String,
    #[serde(default)]
    pub oauth2_client_secret: String,
    #[serde(default)]
    pub oauth2_scope: String,
    #[serde(default)]
    pub oauth2_auth_url: String,
    #[serde(default = "default_oauth2_redirect_port")]
    pub oauth2_redirect_port: u16,
}

fn default_oauth2_redirect_port() -> u16 { 9876 }

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
        let path = entry.path();
        let content = std::fs::read_to_string(&path)?;
        let mut stored: StoredCollection = toml::from_str(&content)?;
        stored.path = path.to_string_lossy().to_string();
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

pub fn delete_collection_by_path(path: &str) -> Result<()> {
    let p = std::path::Path::new(path);
    if p.exists() {
        std::fs::remove_file(p)?;
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
pub const BUILTIN_VAR_NAMES: &[&str] = &[
    "DATE", "DATE+1", "DATE-1",
    "TIME", "TIME+1", "TIME-1",
    "DATETIME", "DATETIME+1", "DATETIME-1",
    "TIMESTAMP", "TIMESTAMP_MS",
    "UUID",
    "RANDOM_INT", "RANDOM_STRING",
    "APPNAME", "VERSION",
];

static BUILTIN_RE: OnceLock<Regex> = OnceLock::new();

fn builtin_re() -> &'static Regex {
    BUILTIN_RE.get_or_init(|| {
        Regex::new(
            r"\{\{(DATETIME|DATE|TIME|TIMESTAMP_MS|TIMESTAMP|UUID|RANDOM_INT|RANDOM_STRING|APPNAME|VERSION)([+-]\d+[dhm]?)?\}\}"
        ).unwrap()
    })
}

pub fn resolve_builtin_vars(text: &str) -> String {
    builtin_re()
        .replace_all(text, |caps: &regex::Captures| {
            builtin_value(&caps[1], caps.get(2).map_or("", |m| m.as_str()))
        })
        .to_string()
}

fn parse_offset(s: &str, default_unit: char) -> Duration {
    if s.is_empty() { return Duration::zero(); }
    let negative = s.starts_with('-');
    let digits = s.trim_start_matches(|c| c == '+' || c == '-');
    let (n, unit) = if digits.ends_with('d') {
        (digits[..digits.len() - 1].parse::<i64>().unwrap_or(0), 'd')
    } else if digits.ends_with('h') {
        (digits[..digits.len() - 1].parse::<i64>().unwrap_or(0), 'h')
    } else if digits.ends_with('m') {
        (digits[..digits.len() - 1].parse::<i64>().unwrap_or(0), 'm')
    } else {
        (digits.parse::<i64>().unwrap_or(0), default_unit)
    };
    let n = if negative { -n } else { n };
    match unit {
        'd' => Duration::days(n),
        'h' => Duration::hours(n),
        'm' => Duration::minutes(n),
        _ => Duration::zero(),
    }
}

fn builtin_value(name: &str, offset: &str) -> String {
    match name {
        "DATE" => (Local::now() + parse_offset(offset, 'd')).format("%Y-%m-%d").to_string(),
        "TIME" => (Local::now() + parse_offset(offset, 'h')).format("%H:%M:%S").to_string(),
        "DATETIME" => (Local::now() + parse_offset(offset, 'd')).format("%Y-%m-%dT%H:%M:%S").to_string(),
        "TIMESTAMP" => (Local::now() + parse_offset(offset, 's')).timestamp().to_string(),
        "TIMESTAMP_MS" => (Local::now() + parse_offset(offset, 's')).timestamp_millis().to_string(),
        "UUID" => {
            let mut rng = rand::thread_rng();
            let b: [u8; 16] = rng.gen();
            let b = [
                b[0], b[1], b[2], b[3], b[4], b[5],
                (b[6] & 0x0f) | 0x40, b[7],
                (b[8] & 0x3f) | 0x80, b[9],
                b[10], b[11], b[12], b[13], b[14], b[15],
            ];
            format!(
                "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                b[0],b[1],b[2],b[3],b[4],b[5],b[6],b[7],b[8],b[9],b[10],b[11],b[12],b[13],b[14],b[15]
            )
        }
        "RANDOM_INT" => rand::thread_rng().gen_range(0u32..100_000).to_string(),
        "RANDOM_STRING" => {
            let mut rng = rand::thread_rng();
            (0..8).map(|_| {
                let n = rng.gen_range(0u8..36);
                if n < 10 { (b'0' + n) as char } else { (b'a' + n - 10) as char }
            }).collect()
        }
        "APPNAME" => "terapi".to_string(),
        "VERSION" => env!("CARGO_PKG_VERSION").to_string(),
        _ => format!("{{{{{}}}}}", name),
    }
}

pub fn resolve_vars(text: &str, vars: &std::collections::HashMap<String, String>) -> String {
    let mut out = text.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{{{}}}}}", k), v);
    }
    resolve_builtin_vars(&out)
}

// ── Session state ────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default)]
struct AppStateFile {
    #[serde(default)]
    active_env: Option<String>,
}

pub fn load_active_env() -> Option<String> {
    let path = resolve_terapi_dir().join("state.toml");
    let content = std::fs::read_to_string(path).ok()?;
    let state: AppStateFile = toml::from_str(&content).ok()?;
    state.active_env
}

pub fn save_active_env(name: Option<&str>) -> Result<()> {
    let dir = resolve_terapi_dir();
    std::fs::create_dir_all(&dir)?;
    let state = AppStateFile { active_env: name.map(|s| s.to_string()) };
    let content = toml::to_string_pretty(&state)?;
    std::fs::write(dir.join("state.toml"), content)?;
    Ok(())
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
    pub graphql: bool,
    #[serde(default)]
    pub graphql_query: Option<String>,
    #[serde(default)]
    pub graphql_variables: HashMap<String, String>,
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

// ── Campaigns ─────────────────────────────────────────────────────────────────

/// Returns (display_name, full_path, Campaign) for every .toml in <terapi_dir>/campaigns/
pub fn load_campaigns() -> Vec<(String, String, crate::campaign::Campaign)> {
    let dir = resolve_terapi_dir().join("campaigns");
    let Ok(entries) = std::fs::read_dir(&dir) else { return vec![]; };

    let mut result = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") { continue; }
        let name = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
            .to_string();
        let path_str = path.to_string_lossy().to_string();
        if let Ok(campaign) = crate::campaign::load(&path_str) {
            result.push((name, path_str, campaign));
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
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
