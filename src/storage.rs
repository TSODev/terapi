use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::app::CollectionNode;

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

pub fn load_collections() -> Result<Vec<CollectionNode>> {
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
        collections.push(stored_to_node(stored));
    }

    Ok(collections)
}

#[allow(dead_code)]
pub fn save_collection(col: &StoredCollection) -> Result<()> {
    let dir = resolve_terapi_dir().join("collections");
    std::fs::create_dir_all(&dir)?;

    let filename = sanitize_filename(&col.collection.name);
    let path = dir.join(format!("{}.toml", filename));
    let content = toml::to_string_pretty(col)?;
    std::fs::write(path, content)?;
    Ok(())
}

fn stored_to_node(col: StoredCollection) -> CollectionNode {
    let mut children: Vec<CollectionNode> = col
        .folders
        .into_iter()
        .map(|folder| CollectionNode::Folder {
            name: folder.name,
            expanded: false,
            children: folder.requests.into_iter().map(request_to_node).collect(),
        })
        .collect();

    children.extend(col.requests.into_iter().map(request_to_node));

    CollectionNode::Folder {
        name: col.collection.name,
        expanded: true,
        children,
    }
}

fn request_to_node(req: StoredRequest) -> CollectionNode {
    CollectionNode::Request {
        name: req.name,
        method: req.method,
        url: req.url,
    }
}

#[allow(dead_code)]
fn sanitize_filename(name: &str) -> String {
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
