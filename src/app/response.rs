use std::collections::{HashMap, HashSet};

use super::*;
use super::http::http_status_label;
use crate::storage::HistoryEntry;

impl App {
    pub fn tick(&mut self) {
        if let Ok(outcome) = self.schema_rx.try_recv() {
            match outcome {
                Ok(SchemaMsg::TypeList(types)) => {
                    self.schema_type_cursor = 0;
                    self.schema_field_scroll = 0;
                    self.schema_state = SchemaState::Ready {
                        types,
                        detail: SchemaDetail::None,
                    };
                }
                Ok(SchemaMsg::TypeDetail(detail)) => {
                    if let SchemaState::Ready { detail: ref mut d, .. } = self.schema_state {
                        *d = SchemaDetail::Loaded(detail);
                    }
                }
                Err(msg) => {
                    match self.schema_state {
                        SchemaState::LoadingList => {
                            self.schema_state = SchemaState::Error(msg);
                        }
                        SchemaState::Ready { detail: ref mut d, .. } => {
                            *d = SchemaDetail::Error(msg);
                        }
                        _ => {
                            self.schema_state = SchemaState::Error(msg);
                        }
                    }
                }
            }
        }

        if let Ok(outcome) = self.response_rx.try_recv() {
            self.request_loading = false;
            match outcome {
                Ok(http) => {
                    self.response_status = Some(http.status);
                    self.response_elapsed_ms = Some(http.elapsed_ms);
                    self.response_headers = http.headers.clone();
                    self.response_body = Some(http.body.clone());
                    self.response_cursor = 0;
                    self.response_scroll = 0;
                    self.response_folds = HashSet::new();
                    self.status_message = format!(
                        "{}  {}ms  —  Tab: panels  e: edit URL  s: send  m: method  ←/→: section  r: raw  q: quit",
                        http_status_label(self.response_status.unwrap_or(0)),
                        self.response_elapsed_ms.unwrap_or(0),
                    );
                    self.record_history(Some(http.status), Some(http.elapsed_ms), Some(http.body));
                }
                Err(msg) => {
                    self.response_status = None;
                    self.response_body = Some(format!("Error: {}", msg));
                    self.response_view = ResponseView::Raw;
                    self.response_cursor = 0;
                    self.response_scroll = 0;
                    self.response_folds = HashSet::new();
                    let short = msg.lines().next().unwrap_or(&msg).chars().take(80).collect::<String>();
                    self.status_message = format!("Error: {}  —  r: JSON view  e: edit URL  s: retry  q: quit", short);
                    self.record_history(None, None, Some(format!("Error: {}", msg)));
                }
            }
        }
    }

    pub(super) fn response_line_count(&self) -> usize {
        crate::json_highlight::rows(
            self.response_body.as_deref().unwrap_or(""),
            &self.response_folds,
        )
        .len()
    }

    pub(super) fn sync_scroll(&mut self) {
        self.response_scroll = (self.response_cursor as u16).saturating_sub(3);
    }

    fn record_history(&mut self, status: Option<u16>, elapsed_ms: Option<u64>, response_body: Option<String>) {
        if let Some(raw) = &self.last_request_raw {
            let entry = HistoryEntry {
                timestamp_secs: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                method: raw.method.clone(),
                url: raw.url.clone(),
                headers: raw.headers.iter().cloned().collect::<HashMap<_, _>>(),
                body: raw.body.clone(),
                status,
                elapsed_ms,
                response_body,
            };
            self.history.insert(0, entry);
            if self.history.len() > 100 {
                self.history.truncate(100);
            }
            let _ = crate::storage::save_history(&self.history);
        }
    }

    pub(super) fn toggle_response_fold(&mut self) {
        let json = self.response_body.as_deref().unwrap_or("");
        let json_rows = crate::json_highlight::rows(json, &self.response_folds);

        if let Some(path) = json_rows
            .get(self.response_cursor)
            .and_then(|r| r.fold_path.clone())
        {
            if !self.response_folds.remove(&path) {
                self.response_folds.insert(path);
            }
            let new_len = self.response_line_count();
            if self.response_cursor >= new_len && new_len > 0 {
                self.response_cursor = new_len - 1;
            }
            self.sync_scroll();
        }
    }
}
