use serde_json::Value;

use super::*;
use super::http::execute_http;

impl App {
    // Phase 1 — fetch all type names/kinds (depth 2, passes any CDN limit)
    pub(super) fn fetch_schema(&mut self) {
        let url = self.request_url.trim().to_string();
        if url.is_empty() {
            self.schema_state = SchemaState::Error(
                "No URL — press e to set an endpoint first".into(),
            );
            return;
        }
        self.schema_state = SchemaState::LoadingList;

        let client = self.http_client.clone();
        let headers = headers_with_ct(&self.request_headers);
        let tx = self.schema_tx.clone();

        tokio::spawn(async move {
            const Q: &str = r#"{ __schema { types { name kind } } }"#;
            let body = serde_json::to_string(&serde_json::json!({"query": Q}))
                .unwrap_or_default();

            let result = match execute_http(client, "POST", &url, &headers, Some(body), false).await {
                Err(e) => Err(e),
                Ok(h) if h.status != 200 => Err(format!("HTTP {} — {}", h.status,
                    h.body.chars().take(200).collect::<String>())),
                Ok(h) => parse_type_list(&h.body),
            };
            let _ = tx.send(result.map(SchemaMsg::TypeList));
        });
    }

    // Phase 2 — fetch field details for one type (depth 3: __type → fields → type)
    pub(super) fn fetch_type_detail(&mut self, type_name: String) {
        let url = self.request_url.trim().to_string();
        if url.is_empty() { return; }

        if let SchemaState::Ready { ref mut detail, .. } = self.schema_state {
            *detail = SchemaDetail::Loading;
        }

        let client = self.http_client.clone();
        let headers = headers_with_ct(&self.request_headers);
        let tx = self.schema_tx.clone();

        tokio::spawn(async move {
            let q = format!(
                r#"{{ __type(name: "{}") {{ description fields(includeDeprecated: false) {{ name type {{ name kind ofType {{ name kind }} }} args {{ name type {{ name kind }} }} }} inputFields {{ name type {{ name kind ofType {{ name kind }} }} }} enumValues(includeDeprecated: false) {{ name }} }} }}"#,
                type_name
            );
            let body = serde_json::to_string(&serde_json::json!({"query": q}))
                .unwrap_or_default();

            let result = match execute_http(client, "POST", &url, &headers, Some(body), false).await {
                Err(e) => Err(e),
                Ok(h) if h.status != 200 => Err(format!("HTTP {} — {}", h.status,
                    h.body.chars().take(200).collect::<String>())),
                Ok(h) => parse_type_detail(&type_name, &h.body),
            };
            let _ = tx.send(result.map(SchemaMsg::TypeDetail));
        });
    }
}

fn headers_with_ct(headers: &[(String, String)]) -> Vec<(String, String)> {
    let mut h = headers.to_vec();
    if !h.iter().any(|(k, _)| k.to_lowercase() == "content-type") {
        h.push(("Content-Type".to_string(), "application/json".to_string()));
    }
    h
}

fn format_type_ref(val: &Value) -> String {
    match val.get("kind").and_then(|k| k.as_str()) {
        Some("NON_NULL") => format!("{}!", format_type_ref(&val["ofType"])),
        Some("LIST")     => format!("[{}]", format_type_ref(&val["ofType"])),
        _                => val.get("name").and_then(|n| n.as_str()).unwrap_or("?").to_string(),
    }
}

fn parse_type_list(body: &str) -> Result<Vec<GqlTypeSummary>, String> {
    let v: Value = serde_json::from_str(body)
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let types = v.pointer("/data/__schema/types")
        .and_then(|t| t.as_array())
        .ok_or_else(|| graphql_error(&v))?;

    const SKIP: &[&str] = &[
        "String", "Boolean", "Int", "Float", "ID",
        "__Schema", "__Type", "__TypeKind", "__Field", "__InputValue",
        "__EnumValue", "__Directive", "__DirectiveLocation",
    ];

    let mut result: Vec<GqlTypeSummary> = types.iter()
        .filter_map(|t| {
            let name = t["name"].as_str().unwrap_or("");
            if name.is_empty() || name.starts_with("__") || SKIP.contains(&name) {
                return None;
            }
            Some(GqlTypeSummary {
                name: name.to_string(),
                kind: t["kind"].as_str().unwrap_or("").to_string(),
            })
        })
        .collect();

    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
}

fn parse_type_detail(name: &str, body: &str) -> Result<GqlTypeDetail, String> {
    let v: Value = serde_json::from_str(body)
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let t = v.pointer("/data/__type")
        .ok_or_else(|| graphql_error(&v))?;

    if t.is_null() {
        return Err(format!("Type '{}' not found in schema", name));
    }

    let parse_field = |f: &Value| -> GqlField {
        GqlField {
            name: f["name"].as_str().unwrap_or("").to_string(),
            type_str: format_type_ref(&f["type"]),
            args: f["args"].as_array()
                .map(|arr| arr.iter().map(|a| GqlArg {
                    name: a["name"].as_str().unwrap_or("").to_string(),
                    type_str: format_type_ref(&a["type"]),
                }).collect())
                .unwrap_or_default(),
        }
    };

    Ok(GqlTypeDetail {
        name: name.to_string(),
        kind: t.pointer("/kind").and_then(|k| k.as_str()).unwrap_or("").to_string(),
        description: t["description"].as_str().filter(|s| !s.is_empty()).map(|s| s.to_string()),
        fields: t["fields"].as_array()
            .map(|arr| arr.iter().map(parse_field).collect())
            .unwrap_or_default(),
        input_fields: t["inputFields"].as_array()
            .map(|arr| arr.iter().map(parse_field).collect())
            .unwrap_or_default(),
        enum_values: t["enumValues"].as_array()
            .map(|arr| arr.iter()
                .filter_map(|v| v["name"].as_str().map(|s| s.to_string()))
                .collect())
            .unwrap_or_default(),
    })
}

fn graphql_error(v: &Value) -> String {
    v.get("errors")
        .and_then(|e| e.as_array())
        .and_then(|arr| arr.first())
        .and_then(|e| e["message"].as_str())
        .map(|m| format!("GraphQL error: {}", m))
        .unwrap_or_else(|| "Unexpected response structure".to_string())
}
