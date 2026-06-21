use std::collections::HashSet;

use serde_json::Value;

#[derive(Debug, Clone)]
pub enum ValueType {
    Object(usize),
    Array(usize),
    Str,
    Number,
    Boolean,
    Null,
}

impl ValueType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Object(_) => "Object",
            Self::Array(_) => "Array",
            Self::Str => "String",
            Self::Number => "Number",
            Self::Boolean => "Boolean",
            Self::Null => "Null",
        }
    }
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
        }),

        Value::Bool(b) => result.push(JsonRow {
            depth,
            key,
            value_type: ValueType::Boolean,
            value_preview: b.to_string(),
            fold_path: None,
            is_folded: false,
        }),

        Value::Number(n) => result.push(JsonRow {
            depth,
            key,
            value_type: ValueType::Number,
            value_preview: n.to_string(),
            fold_path: None,
            is_folded: false,
        }),

        Value::String(s) => result.push(JsonRow {
            depth,
            key,
            value_type: ValueType::Str,
            value_preview: format!("\"{}\"", s),
            fold_path: None,
            is_folded: false,
        }),

        Value::Array(arr) => {
            let count = arr.len();
            let is_folded = !arr.is_empty() && folds.contains(path);
            let fold_path = if count > 0 { Some(path.to_string()) } else { None };

            result.push(JsonRow {
                depth,
                key,
                value_type: ValueType::Array(count),
                value_preview: if is_folded || count == 0 {
                    format!("[ {} ]", count)
                } else {
                    String::new()
                },
                fold_path,
                is_folded,
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

            result.push(JsonRow {
                depth,
                key,
                value_type: ValueType::Object(count),
                value_preview: if is_folded || count == 0 {
                    format!("{{ {} }}", count)
                } else {
                    String::new()
                },
                fold_path,
                is_folded,
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
