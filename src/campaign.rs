use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Campaign {
    pub campaign: Meta,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
pub struct Meta {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Step {
    pub name: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<String>,
}

pub fn load(path: &str) -> Result<Campaign> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read campaign file '{}'", path))?;
    toml::from_str(&content)
        .with_context(|| format!("invalid TOML in '{}'", path))
}

pub fn run(campaign: &Campaign) -> Result<()> {
    println!("Campaign : {}", campaign.campaign.name);
    if !campaign.campaign.description.is_empty() {
        println!("           {}", campaign.campaign.description);
    }
    if !campaign.env.is_empty() {
        println!("\nEnvironment:");
        for (k, v) in &campaign.env {
            println!("  {} = {}", k, v);
        }
    }
    println!("\nSteps ({}):", campaign.steps.len());
    for (i, step) in campaign.steps.iter().enumerate() {
        println!("  [{}/{}] {} {} — {}", i + 1, campaign.steps.len(), step.method, step.url, step.name);
    }
    println!("\n(execution not yet implemented)");
    Ok(())
}
