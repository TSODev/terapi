use super::*;
use crate::campaign::{self, CampaignRunState, StepResult};
use std::collections::HashMap;

impl App {
    pub fn run_selected_campaign(&mut self, overrides: HashMap<String, String>) {
        let Some(entry) = self.campaigns.get(self.campaign_cursor) else {
            self.status_message = "No campaign selected".into();
            return;
        };
        let name = entry.name.clone();
        let camp = entry.campaign.clone();
        self.campaign_run_state = CampaignRunState::Running {
            name: name.clone(),
            step_results: Vec::new(),
            current_step: None,
        };
        self.status_message = format!("Running campaign: {}…", name);
        let tx = self.campaign_tx.clone();
        tokio::spawn(async move {
            campaign::run_streaming(camp, tx, overrides, vec![], 0).await;
        });
    }

    pub fn load_campaign_step_to_request(&mut self) {
        // Collect all HTTP steps (skip WAIT/TRSF) from the Done state.
        let steps: Vec<StepResult> = match &self.campaign_run_state {
            CampaignRunState::Done { results, .. } => {
                results.iter()
                    .flat_map(|r| r.steps.iter().cloned())
                    .filter(|s| s.method != "WAIT" && s.method != "TRSF")
                    .collect()
            }
            _ => return,
        };
        let Some(sr) = steps.get(self.campaign_done_cursor) else { return; };

        // URL + params
        self.set_url(&sr.url.clone());
        self.request_url_params = Vec::new();
        self.url_params_cursor = 0;
        self.parse_url_into_params();

        // Headers
        self.request_headers = sr.request_headers.clone();
        self.header_cursor = 0;

        // Reset response area
        self.response_body = None;
        self.response_status = None;
        self.response_elapsed_ms = None;
        self.response_headers = Vec::new();
        self.response_cursor = 0;
        self.response_scroll = 0;
        self.response_folds = std::collections::HashSet::new();
        self.last_request_raw = None;
        self.response_redirects = Vec::new();
        self.response_cookies = Vec::new();
        self.request_focus = RequestFocus::Response;

        let name = sr.name.clone();

        if sr.graphql {
            // Parse the GQL query out of the request body JSON.
            self.graphql_mode = true;
            if let Some(ref body_str) = sr.request_body {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(body_str) {
                    self.graphql_query_textarea = if let Some(q) = v.get("query").and_then(|q| q.as_str()) {
                        let lines: Vec<String> = q.lines().map(|l| l.to_string()).collect();
                        tui_textarea::TextArea::from(lines)
                    } else {
                        tui_textarea::TextArea::default()
                    };
                    self.graphql_vars = if let Some(vars) = v.get("variables").and_then(|v| v.as_object()) {
                        vars.iter()
                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                            .collect()
                    } else {
                        Vec::new()
                    };
                }
            }
            self.graphql_vars_cursor = 0;
            self.active_graphql_tab = GraphqlTab::Query;
            self.status_message = format!(
                "GQL loaded from campaign '{}' — i: edit query  s: send  S: save",
                name
            );
        } else {
            self.graphql_mode = false;
            self.request_method_idx = METHODS.iter()
                .position(|&m| m == sr.method)
                .unwrap_or(0);
            self.body_textarea = if let Some(ref body) = sr.request_body {
                let lines: Vec<String> = body.lines().map(|l| l.to_string()).collect();
                tui_textarea::TextArea::from(lines)
            } else {
                tui_textarea::TextArea::default()
            };
            self.body_mode = BodyMode::Text;
            self.body_json_pairs = Vec::new();
            self.body_json_cursor = 0;
            self.active_request_tab = RequestTab::Description;
            self.status_message = format!(
                "Loaded from campaign '{}' — s: send  e: edit URL  S: save  r: HTTP view",
                name
            );
        }

        self.active_tab = Tab::Request;
    }

    pub fn open_campaign_params_or_run(&mut self) {
        let Some(entry) = self.campaigns.get(self.campaign_cursor) else {
            return;
        };
        if entry.campaign.params.is_empty() {
            self.run_selected_campaign(HashMap::new());
        } else {
            let params: Vec<(String, String, String)> = entry.campaign.params.iter()
                .map(|p| (
                    p.name.clone(),
                    p.description.clone(),
                    p.default.clone().unwrap_or_default(),
                ))
                .collect();
            self.modal = Some(ModalState::CampaignParams {
                campaign_idx: self.campaign_cursor,
                params,
                cursor: 0,
                editing: false,
                input: String::new(),
            });
        }
    }
}
