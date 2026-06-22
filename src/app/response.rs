use std::collections::HashSet;

use super::*;
use super::http::http_status_label;

impl App {
    pub fn tick(&mut self) {
        if let Ok(outcome) = self.response_rx.try_recv() {
            self.request_loading = false;
            match outcome {
                Ok(http) => {
                    self.response_status = Some(http.status);
                    self.response_elapsed_ms = Some(http.elapsed_ms);
                    self.response_headers = http.headers;
                    self.response_body = Some(http.body);
                    self.response_cursor = 0;
                    self.response_scroll = 0;
                    self.response_folds = HashSet::new();
                    self.status_message = format!(
                        "{}  {}ms  —  Tab: panels  e: edit URL  s: send  m: method  ←/→: section  r: raw  q: quit",
                        http_status_label(self.response_status.unwrap_or(0)),
                        self.response_elapsed_ms.unwrap_or(0),
                    );
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
