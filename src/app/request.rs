use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::collections::HashSet;
use tui_textarea::TextArea;

use super::*;
use super::http::{execute_http, serialize_body_json};
use crate::storage::StoredRequest;

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

        let resolved_headers: Vec<(String, String)> = self.request_headers.iter()
            .map(|(k, v)| (
                crate::storage::resolve_vars(k, &env_vars),
                crate::storage::resolve_vars(v, &env_vars),
            ))
            .collect();

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
        self.request_focus = RequestFocus::Response;
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
        let req = StoredRequest {
            name,
            method: METHODS[self.request_method_idx].to_string(),
            url,
            headers: self.request_headers.iter().cloned().collect::<HMap<_, _>>(),
            body: self.body_string(),
            description: None,
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
            RequestTab::UrlParams => "Tab: panels  a: add  d: delete  Enter: edit  ↑/↓: navigate  ←/→: section  s: send  q: quit".into(),
            RequestTab::Headers => "Tab: panels  a: add  d: delete  ↑/↓: navigate  ←/→: section  e: edit URL  s: send  q: quit".into(),
            RequestTab::Body => match self.body_mode {
                BodyMode::Text => "Tab: panels  i: edit body  t: JSON mode  ←/→: section  s: send  q: quit".into(),
                BodyMode::Json => "Tab: panels  i: edit fields  t: text mode  ←/→: section  s: send  q: quit".into(),
            },
            RequestTab::Options => "Tab: panels  Space/Enter: toggle option  ←/→: section  s: send  q: quit".into(),
            _ => "Tab: panels  e: edit URL  s: send  S: save  n: new  m: method  ←/→: section  ↑/↓: cursor  r: raw  q: quit".into(),
        };
    }

    pub fn active_method(&self) -> &'static str {
        METHODS[self.request_method_idx]
    }
}
