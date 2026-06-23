use super::*;
use crate::campaign::{self, CampaignRunState};
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
            campaign::run_streaming(camp, tx, overrides).await;
        });
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
