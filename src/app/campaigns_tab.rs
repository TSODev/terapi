use super::*;
use crate::campaign::{self, CampaignRunState};

impl App {
    pub fn run_selected_campaign(&mut self) {
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
            campaign::run_streaming(camp, tx).await;
        });
    }
}
