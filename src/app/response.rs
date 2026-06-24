use std::collections::{HashMap, HashSet};

use crossterm::event::{KeyCode, KeyEvent};

use super::*;
use super::http::http_status_label;
use crate::campaign::{CampaignEvent, CampaignRunState};
use crate::storage::HistoryEntry;

impl App {
    pub fn tick(&mut self) {
        // ── poll campaign events ───────────────────────────────────────────
        while let Ok(event) = self.campaign_rx.try_recv() {
            match event {
                CampaignEvent::StepStarted { name, .. } => {
                    if let CampaignRunState::Running { ref mut current_step, .. } = self.campaign_run_state {
                        *current_step = Some(name);
                    }
                }
                CampaignEvent::StepDone(result) => {
                    if let CampaignRunState::Running { ref mut step_results, ref mut current_step, .. } = self.campaign_run_state {
                        *current_step = None;
                        step_results.push(result);
                    }
                }
                CampaignEvent::Finished(results) => {
                    let name = match &self.campaign_run_state {
                        CampaignRunState::Running { name, .. } => name.clone(),
                        _ => String::new(),
                    };
                    let ok: usize  = results.iter().map(|r| r.ok_count()).sum();
                    let err: usize = results.iter().map(|r| r.fail_count()).sum();
                    self.status_message = format!("Campaign done — {} ok  {} failed  Tab: switch panel  q: quit", ok, err);
                    self.campaign_run_state = CampaignRunState::Done { name, results };
                }
                CampaignEvent::IterationStarted { idx, total, .. } => {
                    self.status_message = format!("Running campaign — iteration {}/{}…", idx + 1, total);
                }
                CampaignEvent::Warning(msg) => {
                    self.status_message = format!("Campaign warning: {}", msg);
                }
                CampaignEvent::Error(msg) => {
                    let name = match &self.campaign_run_state {
                        CampaignRunState::Running { name, .. } => name.clone(),
                        _ => String::new(),
                    };
                    self.campaign_run_state = CampaignRunState::Done {
                        name,
                        results: vec![],
                    };
                    self.status_message = format!("Campaign error: {}", msg);
                }
            }
        }

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

        // ── poll OAuth2 token results ──────────────────────────────────────
        if let Ok(outcome) = self.oauth2_rx.try_recv() {
            match outcome {
                Ok(token) => {
                    let key = self.auth_config.oauth2_cache_key();
                    self.oauth2_token_cache.insert(key, token);
                    self.oauth2_wait_state = OAuth2WaitState::Idle;
                    self.status_message = "OAuth2 token obtained — press s to send".into();
                    // If request was waiting for token, fire it now
                    if self.request_loading {
                        self.request_loading = false;
                        self.send_request();
                    }
                }
                Err(msg) => {
                    self.oauth2_wait_state = OAuth2WaitState::Error(msg.clone());
                    self.request_loading = false;
                    let short = msg.chars().take(80).collect::<String>();
                    self.status_message = format!("OAuth2 error: {}", short);
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
            let gql_query = if self.graphql_mode {
                let q = self.graphql_query_textarea.lines().join("\n");
                if q.trim().is_empty() { None } else { Some(q) }
            } else { None };
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
                graphql: self.graphql_mode,
                graphql_query: gql_query,
                graphql_variables: if self.graphql_mode {
                    self.graphql_vars.iter().cloned().collect()
                } else {
                    HashMap::new()
                },
            };
            // Remove any existing entry with the same request signature before inserting.
            self.history.retain(|e| {
                if e.graphql != entry.graphql || e.method != entry.method || e.url != entry.url {
                    return true;
                }
                if e.graphql {
                    e.graphql_query != entry.graphql_query
                } else {
                    e.body != entry.body
                }
            });
            self.history.insert(0, entry);
            if self.history.len() > 100 {
                self.history.truncate(100);
            }
            let _ = crate::storage::save_history(&self.history);
        }
    }

    pub(super) fn handle_json_search_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.json_search = None;
                self.update_request_status_hint();
            }
            KeyCode::Backspace => {
                if let Some(ref mut s) = self.json_search {
                    s.pop();
                }
            }
            KeyCode::Char('>') => {
                self.json_search_next();
            }
            KeyCode::Char('<') => {
                self.json_search_prev();
            }
            KeyCode::Char(c) => {
                if let Some(ref mut s) = self.json_search {
                    s.push(c);
                    self.json_search_jump_first();
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn json_search_match_indices(&self) -> Vec<usize> {
        let term = match &self.json_search {
            Some(s) if !s.is_empty() => s.to_lowercase(),
            _ => return vec![],
        };
        let json = match &self.response_body {
            Some(j) => j,
            None => return vec![],
        };
        crate::json_highlight::rows(json, &self.response_folds)
            .into_iter()
            .enumerate()
            .filter(|(_, r)| {
                r.key.to_lowercase().contains(&term)
                    || r.value_preview.to_lowercase().contains(&term)
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn json_search_jump_first(&mut self) {
        let indices = self.json_search_match_indices();
        if let Some(&first) = indices.first() {
            self.response_cursor = first;
            self.sync_scroll();
        }
    }

    pub(super) fn json_search_next(&mut self) {
        let indices = self.json_search_match_indices();
        if indices.is_empty() { return; }
        let next = indices.iter()
            .find(|&&i| i > self.response_cursor)
            .or_else(|| indices.first())
            .copied();
        if let Some(idx) = next {
            self.response_cursor = idx;
            self.sync_scroll();
        }
    }

    pub(super) fn json_search_prev(&mut self) {
        let indices = self.json_search_match_indices();
        if indices.is_empty() { return; }
        let prev = indices.iter().rev()
            .find(|&&i| i < self.response_cursor)
            .or_else(|| indices.last())
            .copied();
        if let Some(idx) = prev {
            self.response_cursor = idx;
            self.sync_scroll();
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
