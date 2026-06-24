use super::{HttpOutcome, HttpResult};

pub(super) async fn execute_http(
    client: reqwest::Client,
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: Option<String>,
    follow_redirects: bool,
) -> HttpOutcome {
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
    use std::str::FromStr;

    let t0 = std::time::Instant::now();

    // Build the shared HeaderMap once (reused across redirect hops).
    let mut hmap = HeaderMap::new();
    for (k, v) in headers {
        if let (Ok(name), Ok(val)) = (HeaderName::from_str(k), HeaderValue::from_str(v)) {
            hmap.insert(name, val);
        }
    }

    let mut current_method = method.to_string();
    let mut current_url    = url.to_string();
    let mut current_body   = body;
    let mut redirect_chain: Vec<(u16, String)> = Vec::new();

    let resp = loop {
        let mut req = match current_method.to_uppercase().as_str() {
            "POST"   => client.post(&current_url),
            "PUT"    => client.put(&current_url),
            "PATCH"  => client.patch(&current_url),
            "DELETE" => client.delete(&current_url),
            _        => client.get(&current_url),
        };
        req = req.headers(hmap.clone());
        if let Some(ref b) = current_body {
            req = req.body(b.clone());
        }

        let resp = req.send().await.map_err(|e| {
            use std::error::Error;
            let mut msg = e.to_string();
            let mut src = e.source();
            while let Some(cause) = src {
                msg.push_str(&format!("\n  caused by: {}", cause));
                src = cause.source();
            }
            msg
        })?;

        let status = resp.status().as_u16();

        // Follow redirect?
        if follow_redirects && (300..400).contains(&status) && redirect_chain.len() < 20 {
            let location = resp
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();

            let next_url = resolve_redirect_url(&current_url, &location);
            redirect_chain.push((status, next_url.clone()));

            // 301/302/303 collapse to GET; 307/308 keep original method+body.
            if matches!(status, 301 | 302 | 303) {
                current_method = "GET".to_string();
                current_body   = None;
            }
            current_url = next_url;
            continue;
        }

        break resp;
    };

    let elapsed_ms = t0.elapsed().as_millis() as u64;
    let status = resp.status().as_u16();

    let headers: Vec<(String, String)> = resp
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let body = resp.text().await.map_err(|e| e.to_string())?;

    Ok(HttpResult { status, body, headers, elapsed_ms, redirect_chain })
}

/// Resolve a redirect Location against the current URL.
fn resolve_redirect_url(base: &str, location: &str) -> String {
    if location.starts_with("http://") || location.starts_with("https://") {
        return location.to_string();
    }
    if let Ok(base_url) = reqwest::Url::parse(base) {
        if let Ok(resolved) = base_url.join(location) {
            return resolved.to_string();
        }
    }
    location.to_string()
}

pub(super) fn split_url_params(url: &str) -> (String, Vec<(String, String)>) {
    match url.find('?') {
        None => (url.to_string(), Vec::new()),
        Some(pos) => {
            let base = url[..pos].to_string();
            let params = url[pos + 1..]
                .split('&')
                .filter(|s| !s.is_empty())
                .map(|pair| {
                    let mut it = pair.splitn(2, '=');
                    let k = it.next().unwrap_or("").to_string();
                    let v = it.next().unwrap_or("").to_string();
                    (k, v)
                })
                .collect();
            (base, params)
        }
    }
}

pub(super) fn serialize_body_json(pairs: &[(String, String)]) -> String {
    use serde_json::{Map, Number, Value};
    let mut map = Map::new();
    for (k, v) in pairs {
        let val = if v == "null" {
            Value::Null
        } else if v == "true" {
            Value::Bool(true)
        } else if v == "false" {
            Value::Bool(false)
        } else if let Ok(n) = v.parse::<i64>() {
            Value::Number(Number::from(n))
        } else if let Ok(f) = v.parse::<f64>() {
            Value::Number(Number::from_f64(f).unwrap_or(Number::from(0)))
        } else {
            Value::String(v.clone())
        };
        map.insert(k.clone(), val);
    }
    serde_json::to_string_pretty(&Value::Object(map)).unwrap_or_else(|_| "{}".to_string())
}

pub(super) fn http_status_label(status: u16) -> String {
    let text = match status {
        200 => "200 OK",
        201 => "201 Created",
        204 => "204 No Content",
        301 => "301 Moved Permanently",
        302 => "302 Found",
        304 => "304 Not Modified",
        400 => "400 Bad Request",
        401 => "401 Unauthorized",
        403 => "403 Forbidden",
        404 => "404 Not Found",
        405 => "405 Method Not Allowed",
        409 => "409 Conflict",
        422 => "422 Unprocessable Entity",
        429 => "429 Too Many Requests",
        500 => "500 Internal Server Error",
        502 => "502 Bad Gateway",
        503 => "503 Service Unavailable",
        _   => return format!("{}", status),
    };
    text.to_string()
}
