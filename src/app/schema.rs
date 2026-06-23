use serde_json::Value;

use super::*;
use super::http::execute_http;

impl App {
    pub(super) fn fetch_schema(&mut self) {
        let url = self.request_url.trim().to_string();
        if url.is_empty() {
            self.schema_state = SchemaState::Error(
                "No URL — press e to set an endpoint first".into(),
            );
            return;
        }
        self.schema_state = SchemaState::Loading;

        let client = self.http_client.clone();
        let mut headers = self.request_headers.clone();
        if !headers.iter().any(|(k, _)| k.to_lowercase() == "content-type") {
            headers.push(("Content-Type".to_string(), "application/json".to_string()));
        }
        let tx = self.schema_tx.clone();

        tokio::spawn(async move {
            let body = serde_json::to_string(
                &serde_json::json!({"query": INTROSPECTION_QUERY}),
            )
            .unwrap_or_default();

            let outcome = execute_http(client, "POST", &url, &headers, Some(body)).await;

            let result = match outcome {
                Err(e) => Err(e),
                Ok(http) if http.status != 200 => Err(format!(
                    "HTTP {} — {}",
                    http.status,
                    http.body.chars().take(120).collect::<String>()
                )),
                Ok(http) => parse_introspection(&http.body),
            };
            let _ = tx.send(result);
        });
    }
}

const INTROSPECTION_QUERY: &str = r#"{
  __schema {
    types {
      name
      kind
      description
      fields(includeDeprecated: false) {
        name
        description
        type { name kind ofType { name kind ofType { name kind } } }
        args {
          name
          type { name kind ofType { name kind } }
        }
      }
      inputFields {
        name
        description
        type { name kind ofType { name kind } }
      }
      enumValues(includeDeprecated: false) {
        name
        description
      }
    }
  }
}"#;

fn format_type_ref(val: &Value) -> String {
    match val.get("kind").and_then(|k| k.as_str()) {
        Some("NON_NULL") => format!("{}!", format_type_ref(&val["ofType"])),
        Some("LIST") => format!("[{}]", format_type_ref(&val["ofType"])),
        _ => val
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("?")
            .to_string(),
    }
}

fn parse_field(f: &Value) -> GqlField {
    GqlField {
        name: f["name"].as_str().unwrap_or("").to_string(),
        description: f["description"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        type_str: format_type_ref(&f["type"]),
        args: f["args"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|a| GqlArg {
                        name: a["name"].as_str().unwrap_or("").to_string(),
                        type_str: format_type_ref(&a["type"]),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn parse_introspection(body: &str) -> Result<Vec<GqlType>, String> {
    let v: Value =
        serde_json::from_str(body).map_err(|e| format!("JSON parse error: {}", e))?;

    let types = v
        .pointer("/data/__schema/types")
        .and_then(|t| t.as_array())
        .ok_or_else(|| {
            if let Some(errs) = v.get("errors").and_then(|e| e.as_array()) {
                errs.first()
                    .and_then(|e| e["message"].as_str())
                    .map(|m| format!("GraphQL error: {}", m))
                    .unwrap_or_else(|| "Unknown GraphQL error".to_string())
            } else {
                "Unexpected response — missing /data/__schema/types".to_string()
            }
        })?;

    const SKIP: &[&str] = &[
        "String",
        "Boolean",
        "Int",
        "Float",
        "ID",
        "__Schema",
        "__Type",
        "__TypeKind",
        "__Field",
        "__InputValue",
        "__EnumValue",
        "__Directive",
        "__DirectiveLocation",
    ];

    let mut result: Vec<GqlType> = types
        .iter()
        .filter_map(|t| {
            let name = t["name"].as_str().unwrap_or("");
            if name.is_empty() || name.starts_with("__") || SKIP.contains(&name) {
                return None;
            }
            Some(GqlType {
                name: name.to_string(),
                kind: t["kind"].as_str().unwrap_or("").to_string(),
                description: t["description"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string()),
                fields: t["fields"]
                    .as_array()
                    .map(|arr| arr.iter().map(parse_field).collect())
                    .unwrap_or_default(),
                input_fields: t["inputFields"]
                    .as_array()
                    .map(|arr| arr.iter().map(parse_field).collect())
                    .unwrap_or_default(),
                enum_values: t["enumValues"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v["name"].as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect();

    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
}
