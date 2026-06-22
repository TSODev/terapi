use anyhow::Result;
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
}

impl StoredRequest {
    pub fn new(name: impl Into<String>, method: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            method: method.into(),
            url: url.into(),
            headers: HashMap::new(),
            body: None,
            description: None,
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
