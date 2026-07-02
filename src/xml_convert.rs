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

    let text: String = node.children()
        .filter(|c| c.is_text())
        .filter_map(|c| c.text())
        .collect::<Vec<_>>()
        .join("")
        .trim()
        .to_string();

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

