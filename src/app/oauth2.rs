use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::types::CachedToken;

// ── Client Credentials ────────────────────────────────────────────────────────

pub async fn fetch_client_credentials_token(
    client: reqwest::Client,
    token_url: String,
    client_id: String,
    client_secret: String,
    scope: String,
) -> Result<CachedToken, String> {
    let mut form = format!(
        "grant_type=client_credentials&client_id={}&client_secret={}",
        percent_encode(&client_id),
        percent_encode(&client_secret),
    );
    if !scope.is_empty() {
        form.push_str(&format!("&scope={}", percent_encode(&scope)));
    }

    let resp = client
        .post(&token_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form)
        .send()
        .await
        .map_err(|e| format!("request failed: {}", e))?;

    let status = resp.status().as_u16();
    let body = resp.text().await.map_err(|e| format!("read body: {}", e))?;

    if status < 200 || status >= 300 {
        return Err(format!("token endpoint returned HTTP {}: {}", status, truncate(&body, 120)));
    }

    parse_token_response(&body)
}

// ── Authorization Code — step 1: open browser ────────────────────────────────

pub fn open_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let cmd = ("open", vec![url]);
    #[cfg(target_os = "linux")]
    let cmd = ("xdg-open", vec![url]);
    #[cfg(target_os = "windows")]
    let cmd = ("cmd", vec!["/c", "start", url]);

    std::process::Command::new(cmd.0)
        .args(&cmd.1)
        .spawn()
        .map_err(|e| format!("cannot open browser: {}", e))?;
    Ok(())
}

// ── Authorization Code — step 2: local TCP listener ──────────────────────────

pub async fn wait_for_auth_code(port: u16) -> Result<String, String> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|e| format!("cannot bind port {}: {}", port, e))?;

    let accept = async {
        let (mut stream, _) = listener.accept().await
            .map_err(|e| format!("accept error: {}", e))?;

        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).await
            .map_err(|e| format!("read error: {}", e))?;
        let request = String::from_utf8_lossy(&buf[..n]);

        // Extract code from GET /?code=XXX HTTP/1.1
        let code = request
            .lines()
            .next()
            .and_then(|line| {
                let path = line.split_whitespace().nth(1)?;
                path.split('?')
                    .nth(1)?
                    .split('&')
                    .find(|p| p.starts_with("code="))
                    .map(|p| p[5..].to_string())
            })
            .ok_or_else(|| "no code parameter in callback".to_string())?;

        let html = b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
            <html><body><h2>Authorization successful.</h2>\
            <p>You may close this tab and return to Terapi.</p></body></html>";
        let _ = stream.write_all(html).await;

        Ok::<String, String>(code)
    };

    tokio::time::timeout(Duration::from_secs(300), accept)
        .await
        .map_err(|_| "browser authorization timed out (5 min)".to_string())?
}

// ── Authorization Code — step 3: exchange code for token ─────────────────────

pub async fn exchange_code_for_token(
    client: reqwest::Client,
    token_url: String,
    client_id: String,
    client_secret: String,
    code: String,
    redirect_port: u16,
) -> Result<CachedToken, String> {
    let redirect_uri = format!("http://127.0.0.1:{}", redirect_port);
    let form = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&client_secret={}",
        percent_encode(&code),
        percent_encode(&redirect_uri),
        percent_encode(&client_id),
        percent_encode(&client_secret),
    );

    let resp = client
        .post(&token_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form)
        .send()
        .await
        .map_err(|e| format!("request failed: {}", e))?;

    let status = resp.status().as_u16();
    let body = resp.text().await.map_err(|e| format!("read body: {}", e))?;

    if status < 200 || status >= 300 {
        return Err(format!("token endpoint returned HTTP {}: {}", status, truncate(&body, 120)));
    }

    parse_token_response(&body)
}

// ── Shared helpers ────────────────────────────────────────────────────────────

fn parse_token_response(body: &str) -> Result<CachedToken, String> {
    let v: serde_json::Value = serde_json::from_str(body)
        .map_err(|_| format!("invalid JSON from token endpoint: {}", truncate(body, 120)))?;

    let access_token = v["access_token"]
        .as_str()
        .ok_or_else(|| "token response missing access_token".to_string())?
        .to_string();

    let expires_at = v["expires_in"].as_u64().map(|secs| {
        // Shave 10 s off to avoid using an almost-expired token
        Instant::now() + Duration::from_secs(secs.saturating_sub(10))
    });

    Ok(CachedToken { access_token, expires_at })
}

pub fn percent_encode_pub(s: &str) -> String { percent_encode(s) }

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
            | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() }
    else { format!("{}…", s.chars().take(n).collect::<String>()) }
}
