use std::collections::HashMap;

use serde_json::{Map, Value};

/// Best-effort detection of an XML response body: trusts a `Content-Type`
/// containing "xml" (covers `text/xml`, `application/xml`, `application/atom+xml`,
/// SRU's `application/sru+xml`…), falls back to sniffing the body's first
/// non-whitespace character when the header is missing or wrong (common on
/// public APIs like Gallica's SRU endpoint).
pub fn is_xml(body: &str, content_type: Option<&str>) -> bool {
    if let Some(ct) = content_type {
        if ct.to_lowercase().contains("xml") {
            return true;
        }
    }
    body.trim_start().starts_with('<')
}

/// Detects an HTML page (error/block page, login wall…) — real-world HTML is
/// rarely well-formed XML (unescaped entities, IE conditional comments, void
/// elements), so `roxmltree` correctly rejects it and callers fall through
/// here to show something more useful than a raw parser error.
pub fn is_html(body: &str) -> bool {
    let t = body.trim_start().to_lowercase();
    t.starts_with("<!doctype html") || t.starts_with("<html")
}

/// Builds a small JSON payload explaining that the body is HTML rather than
/// JSON/XML, with a preview of the raw content — shown in the JSON tree view
/// in place of a bare `serde_json` "expected value at line 1 column 1" error.
pub fn html_notice_json(body: &str) -> String {
    let preview: String = body.chars().take(300).collect();
    let mut map = Map::new();
    map.insert(
        "notice".to_string(),
        Value::String("This is an HTML page, not JSON or well-formed XML — likely an error or block page from the server (WAF, login wall, 403/500…).".to_string()),
    );
    map.insert("body_preview".to_string(), Value::String(preview));
    serde_json::to_string_pretty(&Value::Object(map)).unwrap_or_default()
}

/// Best-effort JSON text for an arbitrary response body: converts XML to
/// JSON (tagged `FromXML: true`), shows an HTML notice for error/block
/// pages, or passes the body through unchanged if it looks like neither.
/// The single entry point every JSON-tree consumer (render, fold, search,
/// external editor/diff…) should go through, so they never drift apart on
/// what "the JSON for this response" means.
pub fn to_json_text(body: &str, content_type: Option<&str>) -> String {
    if is_xml(body, content_type) {
        if let Ok(json) = xml_to_json(body) {
            return json;
        }
        if is_html(body) {
            return html_notice_json(body);
        }
    }
    body.to_string()
}

/// Converts an XML document to a JSON string using an arbitrary (there is no
/// canonical XML→JSON mapping) but readable convention:
/// - attributes become `@name` keys
/// - text content of a leaf element becomes its value directly (or `#text`
///   alongside attributes/children when both are present)
/// - repeated sibling tags become a JSON array
/// - the root element name becomes the single top-level key
pub fn xml_to_json(body: &str) -> Result<String, String> {
    let doc = roxmltree::Document::parse(body).map_err(|e| e.to_string())?;
    let root = doc.root_element();
    let mut top = Map::new();
    // Marks this tree as a converted view, not the server's real JSON — the
    // conversion below uses an arbitrary convention, not a canonical one.
    top.insert("FromXML".to_string(), Value::Bool(true));
    top.insert(root.tag_name().name().to_string(), node_to_value(root));
    serde_json::to_string_pretty(&Value::Object(top)).map_err(|e| e.to_string())
}

fn node_to_value(node: roxmltree::Node) -> Value {
    let mut map = Map::new();
    for attr in node.attributes() {
        map.insert(format!("@{}", attr.name()), Value::String(attr.value().to_string()));
    }

    let mut order: Vec<String> = Vec::new();
    let mut grouped: HashMap<String, Vec<Value>> = HashMap::new();
    for child in node.children().filter(|c| c.is_element()) {
        let tag = child.tag_name().name().to_string();
        if !grouped.contains_key(&tag) {
            order.push(tag.clone());
        }
        grouped.entry(tag).or_default().push(node_to_value(child));
    }

    // Collapse whitespace (including embedded newlines/tabs from a pretty-printed
    // source document) to single spaces — a raw '\n' in a JSON string value
    // corrupts the response viewer's table rendering (row overlaps its neighbours).
    let text: String = node.children()
        .filter(|c| c.is_text())
        .filter_map(|c| c.text())
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if order.is_empty() {
        // Leaf element: plain string value, unless it also carries attributes.
        if map.is_empty() {
            return Value::String(text);
        }
        if !text.is_empty() {
            map.insert("#text".to_string(), Value::String(text));
        }
        return Value::Object(map);
    }

    for tag in order {
        let mut values = grouped.remove(&tag).unwrap_or_default();
        let entry = if values.len() == 1 { values.pop().unwrap() } else { Value::Array(values) };
        map.insert(tag, entry);
    }
    if !text.is_empty() {
        map.insert("#text".to_string(), Value::String(text));
    }
    Value::Object(map)
}
