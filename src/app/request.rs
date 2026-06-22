use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::collections::HashSet;
use tui_textarea::TextArea;

use super::*;
use super::http::{execute_http, serialize_body_json};
use crate::storage::{StoredAuth, StoredRequest};

fn base64_encode(input: &str) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i] as u32;
        let b1 = if i + 1 < bytes.len() { bytes[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < bytes.len() { bytes[i + 2] as u32 } else { 0 };
        out.push(CHARS[((b0 >> 2) & 0x3f) as usize] as char);
        out.push(CHARS[(((b0 << 4) | (b1 >> 4)) & 0x3f) as usize] as char);
        out.push(if i + 1 < bytes.len() { CHARS[(((b1 << 2) | (b2 >> 6)) & 0x3f) as usize] as char } else { '=' });
        out.push(if i + 2 < bytes.len() { CHARS[(b2 & 0x3f) as usize] as char } else { '=' });
        i += 3;
    }
    out
}

impl App {
    pub(super) fn send_request(&mut self) {
        if self.request_loading {
            return;
        }
        let url = self.request_url.trim().to_string();
        if url.is_empty() {
            self.status_message = "No URL — press e to enter one".into();
            return;
        }

        let env_vars = self.active_env_idx
            .and_then(|i| self.environments.get(i))
            .map(|e| e.vars.clone())
            .unwrap_or_default();

        let url_with_params = if self.request_url_params.is_empty() {
            url.clone()
        } else {
            let sep = if url.contains('?') { '&' } else { '?' };
            let query: String = self.request_url_params.iter()
                .filter(|(k, _)| !k.is_empty())
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            format!("{}{}{}", url, sep, query)
        };
        let resolved_url = crate::storage::resolve_vars(&url_with_params, &env_vars);

        let mut resolved_headers: Vec<(String, String)> = self.request_headers.iter()
            .map(|(k, v)| (
                crate::storage::resolve_vars(k, &env_vars),
                crate::storage::resolve_vars(v, &env_vars),
            ))
            .collect();

        // Apply auth config
        let resolved_url = match &self.auth_config.auth_type {
            AuthType::ApiKey if self.auth_config.api_key_location == ApiKeyLocation::QueryParam => {
                let name = crate::storage::resolve_vars(&self.auth_config.api_key_name, &env_vars);
                let val  = crate::storage::resolve_vars(&self.auth_config.api_key_value, &env_vars);
                if !name.is_empty() {
                    let sep = if resolved_url.contains('?') { '&' } else { '?' };
                    format!("{}{}{}={}", resolved_url, sep, name, val)
                } else {
                    resolved_url
                }
            }
            _ => resolved_url,
        };
        match &self.auth_config.auth_type {
            AuthType::None => {}
            AuthType::Bearer => {
                let token = crate::storage::resolve_vars(&self.auth_config.bearer_token, &env_vars);
                if !token.is_empty() {
                    resolved_headers.push(("Authorization".to_string(), format!("Bearer {}", token)));
                }
            }
            AuthType::Basic => {
                let user = crate::storage::resolve_vars(&self.auth_config.basic_username, &env_vars);
                let pass = crate::storage::resolve_vars(&self.auth_config.basic_password, &env_vars);
                if !user.is_empty() {
                    let encoded = base64_encode(&format!("{}:{}", user, pass));
                    resolved_headers.push(("Authorization".to_string(), format!("Basic {}", encoded)));
                }
            }
            AuthType::ApiKey => {
                if self.auth_config.api_key_location == ApiKeyLocation::Header {
                    let name = crate::storage::resolve_vars(&self.auth_config.api_key_name, &env_vars);
                    let val  = crate::storage::resolve_vars(&self.auth_config.api_key_value, &env_vars);
                    if !name.is_empty() {
                        resolved_headers.push((name, val));
                    }
                }
            }
        }

        let method = METHODS[self.request_method_idx].to_string();
        let tx = self.response_tx.clone();
        let skip_tls = self.skip_tls_verify;

        let body = self.body_string()
            .map(|b| crate::storage::resolve_vars(&b, &env_vars));

        self.last_request_raw = Some(RawRequest {
            method: method.clone(),
            url: resolved_url.clone(),
            headers: resolved_headers.clone(),
            body: body.clone(),
        });

        self.request_loading = true;
        self.request_focus = RequestFocus::Response;
        self.status_message = format!("Sending {} {}…", method, resolved_url);

        tokio::spawn(async move {
            let result = execute_http(&method, &resolved_url, &resolved_headers, body, skip_tls).await;
            let _ = tx.send(result);
        });
    }

    pub fn new_request(&mut self) {
        self.request_url = String::new();
        self.request_method_idx = 0;
        self.request_url_params = Vec::new();
        self.url_params_cursor = 0;
        self.request_headers = Vec::new();
        self.header_cursor = 0;
        self.body_mode = BodyMode::Text;
        self.body_textarea = TextArea::default();
        self.body_json_pairs = Vec::new();
        self.body_json_cursor = 0;
        self.description_textarea = TextArea::default();
        self.request_focus = RequestFocus::Response;
        self.auth_config = AuthConfig::default();
        self.auth_field_cursor = 0;
        self.editing_request_origin = None;
        self.editing_request_name = String::new();
        self.last_request_raw = None;
        self.response_body = None;
        self.response_status = None;
        self.response_elapsed_ms = None;
        self.response_headers = Vec::new();
        self.response_cursor = 0;
        self.response_scroll = 0;
        self.response_folds = HashSet::new();
        self.var_picker = None;
        self.status_message = "New request — e: edit URL  ←/→: section  s: send  S: save  q: quit".into();
    }

    pub(super) fn body_string(&self) -> Option<String> {
        match self.body_mode {
            BodyMode::Text => {
                let text = self.body_textarea.lines().join("\n");
                if text.trim().is_empty() { None } else { Some(text) }
            }
            BodyMode::Json => {
                if self.body_json_pairs.is_empty() { None }
                else { Some(serialize_body_json(&self.body_json_pairs)) }
            }
        }
    }

    pub(super) fn save_request_to_collection(
        &mut self,
        name: String,
        collection_idx: usize,
        folder_idx: Option<usize>,
    ) -> Result<()> {
        use std::collections::HashMap as HMap;
        let url = if self.request_url_params.is_empty() {
            self.request_url.clone()
        } else {
            let sep = if self.request_url.contains('?') { '&' } else { '?' };
            let query = self.request_url_params.iter()
                .filter(|(k, _)| !k.is_empty())
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            format!("{}{}{}", self.request_url, sep, query)
        };
        let desc_text = self.description_textarea.lines().join("\n");
        let req = StoredRequest {
            name,
            method: METHODS[self.request_method_idx].to_string(),
            url,
            headers: self.request_headers.iter().cloned().collect::<HMap<_, _>>(),
            body: self.body_string(),
            description: if desc_text.trim().is_empty() { None } else { Some(desc_text) },
            auth: StoredAuth {
                auth_type: self.auth_config.auth_type.as_str().to_string(),
                bearer_token: self.auth_config.bearer_token.clone(),
                basic_username: self.auth_config.basic_username.clone(),
                basic_password: self.auth_config.basic_password.clone(),
                api_key_name: self.auth_config.api_key_name.clone(),
                api_key_value: self.auth_config.api_key_value.clone(),
                api_key_location: self.auth_config.api_key_location.as_str().to_string(),
            },
        };
        let col_name = self.stored_collections[collection_idx].collection.name.clone();
        if let Some(fi) = folder_idx {
            self.stored_collections[collection_idx].folders[fi].requests.push(req);
        } else {
            self.stored_collections[collection_idx].requests.push(req);
        }
        crate::storage::save_collection(&self.stored_collections[collection_idx])?;
        self.status_message = format!("Saved to \"{}\"  —  S: save again  s: send  q: quit", col_name);
        Ok(())
    }

    pub(super) fn toggle_body_mode(&mut self) {
        match self.body_mode {
            BodyMode::Text => {
                let text = self.body_textarea.lines().join("\n");
                if let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(&text) {
                    self.body_json_pairs = map.into_iter()
                        .map(|(k, v)| {
                            let s = match &v {
                                serde_json::Value::String(s) => s.clone(),
                                serde_json::Value::Null => "null".to_string(),
                                other => other.to_string(),
                            };
                            (k, s)
                        })
                        .collect();
                    self.body_json_cursor = 0;
                }
                self.body_mode = BodyMode::Json;
            }
            BodyMode::Json => {
                if !self.body_json_pairs.is_empty() {
                    let json = serialize_body_json(&self.body_json_pairs);
                    let lines: Vec<String> = json.lines().map(|l| l.to_string()).collect();
                    self.body_textarea = TextArea::from(lines);
                }
                self.body_mode = BodyMode::Text;
            }
        }
        self.update_request_status_hint();
    }

    pub(super) fn handle_body_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.code == KeyCode::Esc {
            self.request_focus = RequestFocus::Response;
            self.update_request_status_hint();
            return Ok(());
        }
        match self.body_mode {
            BodyMode::Text => {
                self.body_textarea.input(tui_textarea::Input::from(key));
                if key.code == KeyCode::Char('{') {
                    let last = self.body_textarea.lines().last().cloned().unwrap_or_default();
                    if last.ends_with("{{") {
                        self.open_var_picker(VarPickerTarget::BodyText);
                    }
                }
            }
            BodyMode::Json => {
                self.handle_body_json_key(key)?;
            }
        }
        Ok(())
    }

    fn handle_body_json_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Up => {
                if self.body_json_cursor > 0 {
                    self.body_json_cursor -= 1;
                }
            }
            KeyCode::Down => {
                if self.body_json_cursor + 1 < self.body_json_pairs.len() {
                    self.body_json_cursor += 1;
                }
            }
            KeyCode::Char('a') => {
                self.modal = Some(ModalState::BodyPair {
                    key: String::new(),
                    value: String::new(),
                    active_field: VarField::Key,
                    edit_idx: None,
                });
            }
            KeyCode::Char('d') if !self.body_json_pairs.is_empty() => {
                self.body_json_pairs.remove(self.body_json_cursor);
                if self.body_json_cursor > 0 && self.body_json_cursor >= self.body_json_pairs.len() {
                    self.body_json_cursor -= 1;
                }
            }
            KeyCode::Enter | KeyCode::Char('e') if !self.body_json_pairs.is_empty() => {
                let (k, v) = self.body_json_pairs[self.body_json_cursor].clone();
                self.modal = Some(ModalState::BodyPair {
                    key: k,
                    value: v,
                    active_field: VarField::Key,
                    edit_idx: Some(self.body_json_cursor),
                });
            }
            _ => {}
        }
        Ok(())
    }

    pub fn update_request_status_hint(&mut self) {
        self.status_message = match self.active_request_tab {
            RequestTab::UrlParams => "Tab: panels  a: add  d: delete  Enter: edit  ↑/↓: navigate  ←/→: section  s: send  S: save  q: quit".into(),
            RequestTab::Headers => "Tab: panels  a: add  d: delete  ↑/↓: navigate  ←/→: section  e: edit URL  s: send  S: save  q: quit".into(),
            RequestTab::Body => match self.body_mode {
                BodyMode::Text => "Tab: panels  i: edit body  t: JSON mode  ←/→: section  s: send  S: save  q: quit".into(),
                BodyMode::Json => "Tab: panels  i: edit fields  t: text mode  ←/→: section  s: send  S: save  q: quit".into(),
            },
            RequestTab::Options => "Tab: panels  Space/Enter: toggle option  ←/→: section  s: send  S: save  q: quit".into(),
            RequestTab::Auth => "Tab: panels  ↑/↓: field  Space/Enter: cycle type or edit  ←/→: section  s: send  S: save  q: quit".into(),
            RequestTab::Description => "Tab: panels  i: edit description  ←/→: section  s: send  S: save  q: quit".into(),
        };
    }

    pub fn active_method(&self) -> &'static str {
        METHODS[self.request_method_idx]
    }

    pub(super) fn load_from_history(&mut self, idx: usize) {
        if let Some(entry) = self.history.get(idx).cloned() {
            self.request_method_idx = METHODS.iter().position(|&m| m == entry.method).unwrap_or(0);
            self.request_url = entry.url.clone();
            self.request_url_params = Vec::new();
            self.url_params_cursor = 0;
            self.request_headers = entry.headers.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            self.request_headers.sort_by(|a, b| a.0.cmp(&b.0));
            self.header_cursor = 0;
            self.body_textarea = if let Some(body) = &entry.body {
                let lines: Vec<String> = body.lines().map(|l| l.to_string()).collect();
                TextArea::from(lines)
            } else {
                TextArea::default()
            };
            self.body_mode = BodyMode::Text;
            self.body_json_pairs = Vec::new();
            self.body_json_cursor = 0;
            self.request_focus = RequestFocus::Response;
            self.response_body = None;
            self.response_status = None;
            self.response_elapsed_ms = None;
            self.response_headers = Vec::new();
            self.response_cursor = 0;
            self.response_scroll = 0;
            self.response_folds = HashSet::new();
            self.active_tab = Tab::Request;
            self.active_request_tab = RequestTab::Description;
            self.status_message = format!(
                "Loaded from history: {}  —  s: send  e: edit URL  q: quit",
                entry.url
            );
        }
    }

    pub(super) fn delete_history_entry(&mut self, idx: usize) {
        if idx < self.history.len() {
            self.history.remove(idx);
            if self.history_cursor >= self.history.len() && !self.history.is_empty() {
                self.history_cursor = self.history.len() - 1;
            }
            let _ = crate::storage::save_history(&self.history);
        }
    }
}
