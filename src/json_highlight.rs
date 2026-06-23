use std::collections::HashSet;

use serde_json::Value;

#[derive(Debug, Clone)]
pub enum ValueType {
    Object,
    Array,
    Str,
    Number,
    Boolean,
    Null,
}

#[derive(Debug, Clone)]
pub struct JsonRow {
    pub depth: usize,
    pub key: String,
    pub value_type: ValueType,
    /// Shown in the Value column. Empty for expanded objects/arrays.
    pub value_preview: String,
    /// Some(path) if this row can be folded/unfolded.
    pub fold_path: Option<String>,
    pub is_folded: bool,
    /// Full dot-notation path usable in campaign `extract` fields (e.g. `results.0.city`).
    pub dot_path: String,
}

/// Convert an internal slash-path to the dot-notation used by campaign extraction.
pub fn to_dot_path(internal: &str) -> String {
    if internal.is_empty() {
        return String::new();
    }
    internal.trim_start_matches('/').replace('/', ".")
}

/// Build a flat list of rows from a JSON string, respecting the current fold state.
pub fn rows(json: &str, folds: &HashSet<String>) -> Vec<JsonRow> {
    match serde_json::from_str::<Value>(json) {
        Ok(value) => {
            let mut result = Vec::new();
            collect(&value, 0, "(root)".to_string(), "", folds, &mut result);
            result
        }
        Err(e) => vec![JsonRow {
            depth: 0,
            key: "error".into(),
            value_type: ValueType::Null,
            value_preview: format!("Parse error: {e}"),
            fold_path: None,
            is_folded: false,
            dot_path: String::new(),
        }],
    }
}

fn collect(
    value: &Value,
    depth: usize,
    key: String,
    path: &str,
    folds: &HashSet<String>,
    result: &mut Vec<JsonRow>,
) {
    match value {
        Value::Null => result.push(JsonRow {
            depth,
            key,
            value_type: ValueType::Null,
            value_preview: "null".into(),
            fold_path: None,
            is_folded: false,
            dot_path: to_dot_path(path),
        }),

        Value::Bool(b) => result.push(JsonRow {
            depth,
            key,
            value_type: ValueType::Boolean,
            value_preview: b.to_string(),
            fold_path: None,
            is_folded: false,
            dot_path: to_dot_path(path),
        }),

        Value::Number(n) => result.push(JsonRow {
            depth,
            key,
            value_type: ValueType::Number,
            value_preview: n.to_string(),
            fold_path: None,
            is_folded: false,
            dot_path: to_dot_path(path),
        }),

        Value::String(s) => result.push(JsonRow {
            depth,
            key,
            value_type: ValueType::Str,
            value_preview: format!("\"{}\"", s),
            fold_path: None,
            is_folded: false,
            dot_path: to_dot_path(path),
        }),

        Value::Array(arr) => {
            let count = arr.len();
            let is_folded = !arr.is_empty() && folds.contains(path);
            let fold_path = if count > 0 { Some(path.to_string()) } else { None };

            let value_preview = if count == 0 {
                "[ ]".into()
            } else if is_folded {
                preview_array(arr)
            } else {
                String::new()
            };

            result.push(JsonRow {
                depth,
                key,
                value_type: ValueType::Array,
                value_preview,
                fold_path,
                is_folded,
                dot_path: to_dot_path(path),
            });

            if !is_folded {
                for (i, item) in arr.iter().enumerate() {
                    collect(
                        item,
                        depth + 1,
                        format!("[{}]", i),
                        &format!("{}/{}", path, i),
                        folds,
                        result,
                    );
                }
            }
        }

        Value::Object(map) => {
            let count = map.len();
            let is_folded = !map.is_empty() && folds.contains(path);
            let fold_path = if count > 0 { Some(path.to_string()) } else { None };

            let value_preview = if count == 0 {
                "{ }".into()
            } else if is_folded {
                preview_object(map)
            } else {
                String::new()
            };

            result.push(JsonRow {
                depth,
                key,
                value_type: ValueType::Object,
                value_preview,
                fold_path,
                is_folded,
                dot_path: to_dot_path(path),
            });

            if !is_folded {
                for (k, v) in map.iter() {
                    collect(
                        v,
                        depth + 1,
                        k.clone(),
                        &format!("{}/{}", path, k),
                        folds,
                        result,
                    );
                }
            }
        }
    }
}

// ── inline preview helpers ────────────────────────────────────────────────────

/// One-line preview of a primitive value, or a short summary for containers.
fn preview_atom(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("\"{}\"", s),
        Value::Array(a) => format!("[{}]", a.len()),
        Value::Object(m) => format!("{{{}}}", m.len()),
    }
}

/// Build `[ v1, v2, … ]` for a folded array, capped at ~80 chars.
fn preview_array(arr: &[Value]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut len = 4usize; // "[ " + " ]"
    for item in arr {
        let s = preview_atom(item);
        if len + s.len() > 80 {
            parts.push("…".into());
            break;
        }
        len += s.len() + 2; // ", "
        parts.push(s);
    }
    format!("[ {} ]", parts.join(", "))
}

/// Build `{ k: v, k: v, … }` for a folded object, capped at ~80 chars.
fn preview_object(map: &serde_json::Map<String, Value>) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut len = 4usize; // "{ " + " }"
    for (k, v) in map {
        let s = format!("{}: {}", k, preview_atom(v));
        if len + s.len() > 80 {
            parts.push("…".into());
            break;
        }
        len += s.len() + 2;
        parts.push(s);
    }
    format!("{{ {} }}", parts.join(", "))
}
