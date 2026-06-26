pub mod types;
mod browser;
mod checker;
mod editor;
pub(super) mod step_editor;
mod ui;

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::PathBuf;

use crate::campaign::{Campaign, CampaignEvent, CampaignRunState, Meta};
use types::{CampaignSettingsMode, IoEditorMode, ParamEditorMode, StepEditorMode, VariablesMode};
use crate::event::{Event, EventHandler};
use crate::storage::StoredCollection;
use tokio::sync::mpsc;
use types::*;

pub struct BuilderApp {
    pub running: bool,
    pub campaign: Campaign,
    pub path: Option<PathBuf>,
    pub cursor: usize,
    pub focus: BuilderFocus,
    pub modified: bool,
    pub stored_collections: Vec<StoredCollection>,
    pub stored_env_names: Vec<String>,
    pub status_message: String,
    pub description_textarea: tui_textarea::TextArea<'static>,
    /// Comment block at the very top of the TOML file (before [campaign]).
    pub header_comment: String,
    /// Per-step comment blocks, stored as `# lines` above [[steps]] in TOML.
    /// Parallel to campaign.steps; not persisted via serde.
    pub step_comments: Vec<String>,
    // ── Run state ──────────────────────────────────────────────────────────────
    pub run_state: CampaignRunState,
    pub campaign_rx: Option<mpsc::UnboundedReceiver<CampaignEvent>>,
    /// true while the "save before quit?" confirmation overlay is shown
    pub quit_confirm: bool,
    // ── Single-step preview ────────────────────────────────────────────────────
    pub step_preview_running: bool,
    pub step_preview_result: Option<crate::campaign::StepResult>,
    pub step_preview_rx: Option<mpsc::UnboundedReceiver<crate::campaign::StepResult>>,
}

impl BuilderApp {
    pub fn new(path: Option<PathBuf>) -> Self {
        let (campaign, step_comments, header_comment) = if let Some(ref p) = path {
            let camp = crate::campaign::load(p.to_str().unwrap_or(""))
                .unwrap_or_else(|_| empty_campaign("new_campaign"));
            let (step_c, header_c) = std::fs::read_to_string(p)
                .map(|content| {
                    let sc = parse_step_comments(&content, camp.steps.len());
                    let hc = parse_header_comment(&content);
                    (sc, hc)
                })
                .unwrap_or_else(|_| (vec![String::new(); camp.steps.len()], String::new()));
            (camp, step_c, header_c)
        } else {
            (empty_campaign("new_campaign"), Vec::new(), String::new())
        };
        let stored_collections = crate::storage::load_collections().unwrap_or_default();
        let stored_env_names = crate::storage::load_envs()
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.env.name)
            .collect();
        Self {
            running: true,
            campaign,
            path,
            cursor: 0,
            focus: BuilderFocus::Pipeline,
            modified: false,
            stored_collections,
            stored_env_names,
            status_message: String::new(),
            description_textarea: tui_textarea::TextArea::default(),
            header_comment,
            step_comments,
            run_state: CampaignRunState::Idle,
            campaign_rx: None,
            quit_confirm: false,
            step_preview_running: false,
            step_preview_result: None,
            step_preview_rx: None,
        }
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        if self.quit_confirm {
            return self.handle_quit_confirm_key(key);
        }
        match self.focus.clone() {
            BuilderFocus::Pipeline => self.handle_pipeline_key(key),
            BuilderFocus::Catalog { cursor, insert_after } => {
                self.handle_catalog_key(key, cursor, insert_after)
            }
            BuilderFocus::StepEditor { step_idx, section_cursor, sub_cursor, mode, desc_active } => {
                step_editor::handle_key(self, key, step_idx, section_cursor, sub_cursor, mode, desc_active)
            }
            BuilderFocus::CollectionBrowser { for_step, col_cursor, expanded } => {
                browser::handle_key(self, key, for_step, col_cursor, expanded)
            }
            BuilderFocus::CampaignSettings { cursor, mode } => {
                self.handle_campaign_settings_key(key, cursor, mode)
            }
            BuilderFocus::Checker { .. }                  => self.handle_overlay_key(key),
            BuilderFocus::TomlPreview { scroll }          => self.handle_preview_key(key, scroll),
            BuilderFocus::Variables { cursor, mode }      => self.handle_variables_key(key, cursor, mode),
            BuilderFocus::Run { scroll }                  => self.handle_run_key(key, scroll),
            BuilderFocus::ParamsEditor { cursor, mode }     => self.handle_params_editor_key(key, cursor, mode),
            BuilderFocus::ConnectorsEditor { cursor, mode } => self.handle_connectors_key(key, cursor, mode),
            BuilderFocus::OutputsEditor { cursor, mode }    => self.handle_outputs_key(key, cursor, mode),
            BuilderFocus::PipelineConnectors { cursor }     => self.handle_pipeline_connectors_key(key, cursor),
            BuilderFocus::PipelineOutputs { cursor }        => self.handle_pipeline_outputs_key(key, cursor),
            BuilderFocus::OutputStepPicker { output_idx, step_cursor, f1, f2, f3, output_cursor } => {
                self.handle_output_step_picker_key(key, output_idx, step_cursor, f1, f2, f3, output_cursor)
            }
        }
    }

    // ── Pipeline ──────────────────────────────────────────────────────────────

    fn handle_pipeline_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        use crossterm::event::KeyCode;
        let step_count = self.campaign.steps.len();
        match key.code {
            KeyCode::Char('q') => {
                if self.modified {
                    self.quit_confirm = true;
                } else {
                    self.running = false;
                }
            }
            KeyCode::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                } else if !self.campaign.connectors.is_empty() {
                    self.focus = BuilderFocus::PipelineConnectors {
                        cursor: self.campaign.connectors.len() - 1,
                    };
                }
            }
            KeyCode::Down => {
                if step_count > 0 && self.cursor < step_count - 1 {
                    self.cursor += 1;
                } else if !self.campaign.outputs.is_empty() {
                    self.focus = BuilderFocus::PipelineOutputs { cursor: 0 };
                }
            }
            KeyCode::Enter | KeyCode::Char('e') if step_count > 0 => {
                self.focus = BuilderFocus::StepEditor {
                    step_idx: self.cursor,
                    section_cursor: 0,
                    sub_cursor: 0,
                    mode: StepEditorMode::Browse,
                    desc_active: false,
                };
            }
            KeyCode::Char('n') => {
                self.focus = BuilderFocus::Catalog { insert_after: None, cursor: 0 };
            }
            KeyCode::Char('i') if step_count > 0 => {
                self.focus = BuilderFocus::Catalog { insert_after: Some(self.cursor), cursor: 0 };
            }
            KeyCode::Char('K') => editor::move_step_up(self),
            KeyCode::Char('J') => editor::move_step_down(self),
            KeyCode::Char('d') if step_count > 0 => editor::delete_step(self),
            KeyCode::Char('c') => {
                let results = checker::run(self);
                self.focus = BuilderFocus::Checker { results };
            }
            KeyCode::Char('p') => {
                self.focus = BuilderFocus::TomlPreview { scroll: 0 };
            }
            KeyCode::Char('v') => {
                self.focus = BuilderFocus::Variables { cursor: 0, mode: VariablesMode::Browse };
            }
            KeyCode::Char('s') if key.modifiers.is_empty() => {
                self.focus = BuilderFocus::CampaignSettings {
                    cursor: 0,
                    mode: CampaignSettingsMode::Browse,
                };
            }
            KeyCode::Char('r') if !self.campaign.steps.is_empty() => {
                self.start_run();
            }
            KeyCode::Char('w') => self.save()?,
            _ => {}
        }
        Ok(())
    }

    // ── Catalog ───────────────────────────────────────────────────────────────

    fn handle_catalog_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        cursor: usize,
        insert_after: Option<usize>,
    ) -> Result<()> {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => { self.focus = BuilderFocus::Pipeline; }
            KeyCode::Up => {
                let new = cursor.saturating_sub(1);
                self.focus = BuilderFocus::Catalog { insert_after, cursor: new };
            }
            KeyCode::Down => {
                let new = (cursor + 1).min(BRICK_KINDS.len() - 1);
                self.focus = BuilderFocus::Catalog { insert_after, cursor: new };
            }
            KeyCode::Enter => {
                let kind = &BRICK_KINDS[cursor];
                match kind {
                    BrickKind::Connector => {
                        self.focus = BuilderFocus::ConnectorsEditor {
                            cursor: self.campaign.connectors.len(),
                            mode: IoEditorMode::Edit {
                                idx: None,
                                f0: "csv".into(),
                                f1: String::new(),
                                f2: String::new(),
                                f3: String::new(),
                                field: 0,
                            },
                        };
                    }
                    BrickKind::Output => {
                        self.focus = BuilderFocus::OutputStepPicker {
                            output_idx: None,
                            step_cursor: 0,
                            f1: String::new(), f2: String::new(), f3: String::new(),
                            output_cursor: self.campaign.outputs.len(),
                        };
                    }
                    _ => {
                        let step = new_step_for(kind);
                        let pos = match insert_after {
                            Some(after) => after + 1,
                            None => self.campaign.steps.len(),
                        };
                        self.campaign.steps.insert(pos, step);
                        self.step_comments.insert(pos.min(self.step_comments.len()), String::new());
                        self.cursor = pos;
                        self.modified = true;
                        self.focus = BuilderFocus::StepEditor {
                            step_idx: pos,
                            section_cursor: 0,
                            sub_cursor: 0,
                            mode: StepEditorMode::Browse,
                            desc_active: false,
                        };
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    // ── Overlays (checker, preview, variables) ────────────────────────────────

    fn handle_overlay_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        use crossterm::event::KeyCode;
        if key.code == KeyCode::Esc {
            self.focus = BuilderFocus::Pipeline;
        }
        Ok(())
    }

    fn handle_preview_key(&mut self, key: crossterm::event::KeyEvent, scroll: usize) -> Result<()> {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => { self.focus = BuilderFocus::Pipeline; }
            KeyCode::Up   => { self.focus = BuilderFocus::TomlPreview { scroll: scroll.saturating_sub(1) }; }
            KeyCode::Down => { self.focus = BuilderFocus::TomlPreview { scroll: scroll + 1 }; }
            _ => {}
        }
        Ok(())
    }

    fn handle_variables_key(&mut self, key: crossterm::event::KeyEvent, cursor: usize, mode: VariablesMode) -> Result<()> {
        use crossterm::event::KeyCode;

        fn sorted_keys(env: &std::collections::HashMap<String, String>) -> Vec<String> {
            let mut keys: Vec<String> = env.keys().cloned().collect();
            keys.sort();
            keys
        }

        match mode {
            VariablesMode::Browse => {
                let var_count = self.campaign.env.len();
                match key.code {
                    KeyCode::Esc => { self.focus = BuilderFocus::Pipeline; }
                    KeyCode::Up  => {
                        self.focus = BuilderFocus::Variables { cursor: cursor.saturating_sub(1), mode: VariablesMode::Browse };
                    }
                    KeyCode::Down => {
                        let new = if var_count > 0 { (cursor + 1).min(var_count - 1) } else { 0 };
                        self.focus = BuilderFocus::Variables { cursor: new, mode: VariablesMode::Browse };
                    }
                    KeyCode::Char('a') => {
                        self.focus = BuilderFocus::Variables {
                            cursor,
                            mode: VariablesMode::Edit { original_key: None, key: String::new(), value: String::new(), field: 0 },
                        };
                    }
                    KeyCode::Char('d') if var_count > 0 => {
                        let keys = sorted_keys(&self.campaign.env);
                        if let Some(k) = keys.get(cursor) {
                            self.campaign.env.remove(k);
                            self.modified = true;
                            let new_count = self.campaign.env.len();
                            let new_cursor = if new_count > 0 { cursor.min(new_count - 1) } else { 0 };
                            self.focus = BuilderFocus::Variables { cursor: new_cursor, mode: VariablesMode::Browse };
                        }
                    }
                    KeyCode::Enter if var_count > 0 => {
                        let keys = sorted_keys(&self.campaign.env);
                        if let Some(k) = keys.get(cursor) {
                            let v = self.campaign.env.get(k).cloned().unwrap_or_default();
                            self.focus = BuilderFocus::Variables {
                                cursor,
                                mode: VariablesMode::Edit {
                                    original_key: Some(k.clone()),
                                    key: k.clone(),
                                    value: v,
                                    field: 1, // start on value
                                },
                            };
                        }
                    }
                    _ => {}
                }
            }
            VariablesMode::Edit { original_key, key: mut var_key, value: mut var_value, field } => {
                match key.code {
                    KeyCode::Esc => {
                        self.focus = BuilderFocus::Variables { cursor, mode: VariablesMode::Browse };
                    }
                    KeyCode::Tab | KeyCode::Enter => {
                        if field == 0 {
                            self.focus = BuilderFocus::Variables {
                                cursor,
                                mode: VariablesMode::Edit { original_key, key: var_key, value: var_value, field: 1 },
                            };
                        } else {
                            let trimmed_key = var_key.trim().to_string();
                            if !trimmed_key.is_empty() {
                                if let Some(ref old) = original_key {
                                    if old != &trimmed_key {
                                        self.campaign.env.remove(old);
                                    }
                                }
                                self.campaign.env.insert(trimmed_key.clone(), var_value.trim().to_string());
                                self.modified = true;
                                let keys = sorted_keys(&self.campaign.env);
                                let new_cursor = keys.iter().position(|k| k == &trimmed_key).unwrap_or(cursor);
                                self.focus = BuilderFocus::Variables { cursor: new_cursor, mode: VariablesMode::Browse };
                            } else {
                                self.focus = BuilderFocus::Variables { cursor, mode: VariablesMode::Browse };
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        if field == 0 { var_key.pop(); } else { var_value.pop(); }
                        self.focus = BuilderFocus::Variables { cursor, mode: VariablesMode::Edit { original_key, key: var_key, value: var_value, field } };
                    }
                    KeyCode::Char(c) => {
                        if field == 0 { var_key.push(c); } else { var_value.push(c); }
                        self.focus = BuilderFocus::Variables { cursor, mode: VariablesMode::Edit { original_key, key: var_key, value: var_value, field } };
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    // ── Campaign settings ─────────────────────────────────────────────────────

    fn handle_campaign_settings_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        cursor: usize,
        mode: CampaignSettingsMode,
    ) -> Result<()> {
        use crossterm::event::KeyCode;
        const FIELDS: usize = 5; // Name, Description, Continue on error, Env, Params

        match mode {
            CampaignSettingsMode::Browse => match key.code {
                KeyCode::Esc => {
                    self.focus = BuilderFocus::Pipeline;
                }
                KeyCode::Up => {
                    let new = cursor.saturating_sub(1);
                    self.focus = BuilderFocus::CampaignSettings { cursor: new, mode: CampaignSettingsMode::Browse };
                }
                KeyCode::Down => {
                    let new = (cursor + 1).min(FIELDS - 1);
                    self.focus = BuilderFocus::CampaignSettings { cursor: new, mode: CampaignSettingsMode::Browse };
                }
                KeyCode::Enter => match cursor {
                    0 => {
                        let buf = self.campaign.campaign.name.clone();
                        self.focus = BuilderFocus::CampaignSettings { cursor, mode: CampaignSettingsMode::EditText { buffer: buf } };
                    }
                    1 => {
                        let buf = self.campaign.campaign.description.clone();
                        self.focus = BuilderFocus::CampaignSettings { cursor, mode: CampaignSettingsMode::EditText { buffer: buf } };
                    }
                    2 => {
                        self.campaign.continue_on_error = !self.campaign.continue_on_error;
                        self.modified = true;
                        self.focus = BuilderFocus::CampaignSettings { cursor, mode: CampaignSettingsMode::Browse };
                    }
                    3 => {
                        self.cycle_env(1);
                        self.focus = BuilderFocus::CampaignSettings { cursor, mode: CampaignSettingsMode::Browse };
                    }
                    4 => {
                        self.focus = BuilderFocus::ParamsEditor { cursor: 0, mode: ParamEditorMode::Browse };
                    }
                    _ => {}
                },
                KeyCode::Char(' ') if cursor == 2 => {
                    self.campaign.continue_on_error = !self.campaign.continue_on_error;
                    self.modified = true;
                    self.focus = BuilderFocus::CampaignSettings { cursor, mode: CampaignSettingsMode::Browse };
                }
                KeyCode::Right if cursor == 3 => {
                    self.cycle_env(1);
                    self.focus = BuilderFocus::CampaignSettings { cursor, mode: CampaignSettingsMode::Browse };
                }
                KeyCode::Left if cursor == 3 => {
                    self.cycle_env(-1);
                    self.focus = BuilderFocus::CampaignSettings { cursor, mode: CampaignSettingsMode::Browse };
                }
                _ => {}
            },
            CampaignSettingsMode::EditText { mut buffer } => match key.code {
                KeyCode::Esc => {
                    self.focus = BuilderFocus::CampaignSettings { cursor, mode: CampaignSettingsMode::Browse };
                }
                KeyCode::Enter => {
                    match cursor {
                        0 => { self.campaign.campaign.name = buffer.trim().to_string(); }
                        1 => { self.campaign.campaign.description = buffer.trim().to_string(); }
                        _ => {}
                    }
                    self.modified = true;
                    self.focus = BuilderFocus::CampaignSettings { cursor, mode: CampaignSettingsMode::Browse };
                }
                KeyCode::Backspace => {
                    buffer.pop();
                    self.focus = BuilderFocus::CampaignSettings { cursor, mode: CampaignSettingsMode::EditText { buffer } };
                }
                KeyCode::Char(c) => {
                    buffer.push(c);
                    self.focus = BuilderFocus::CampaignSettings { cursor, mode: CampaignSettingsMode::EditText { buffer } };
                }
                _ => {}
            },
        }
        Ok(())
    }

    fn cycle_env(&mut self, delta: i32) {
        let envs = &self.stored_env_names;
        if envs.is_empty() {
            return;
        }
        let current_idx = if let Some(ref name) = self.campaign.env_file {
            envs.iter().position(|e| e == name).map(|i| i as i32).unwrap_or(-1)
        } else {
            -1_i32
        };
        let n = envs.len() as i32;
        let next = (current_idx + delta).rem_euclid(n + 1);
        if next as usize == envs.len() {
            self.campaign.env_file = None;
        } else {
            self.campaign.env_file = Some(envs[next as usize].clone());
        }
        self.modified = true;
    }

    // ── Params editor ─────────────────────────────────────────────────────────

    fn handle_params_editor_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        cursor: usize,
        mode: ParamEditorMode,
    ) -> Result<()> {
        use crossterm::event::KeyCode;

        match mode {
            ParamEditorMode::Browse => {
                let n = self.campaign.params.len();
                match key.code {
                    KeyCode::Esc => {
                        self.focus = BuilderFocus::CampaignSettings {
                            cursor: 4,
                            mode: CampaignSettingsMode::Browse,
                        };
                    }
                    KeyCode::Up => {
                        let new = cursor.saturating_sub(1);
                        self.focus = BuilderFocus::ParamsEditor { cursor: new, mode: ParamEditorMode::Browse };
                    }
                    KeyCode::Down => {
                        let new = if n > 0 { (cursor + 1).min(n - 1) } else { 0 };
                        self.focus = BuilderFocus::ParamsEditor { cursor: new, mode: ParamEditorMode::Browse };
                    }
                    KeyCode::Char('a') => {
                        self.focus = BuilderFocus::ParamsEditor {
                            cursor,
                            mode: ParamEditorMode::AddParam {
                                name: String::new(),
                                desc: String::new(),
                                default_val: String::new(),
                                field: 0,
                            },
                        };
                    }
                    KeyCode::Char('d') if n > 0 && cursor < n => {
                        self.campaign.params.remove(cursor);
                        self.modified = true;
                        let new = cursor.min(self.campaign.params.len().saturating_sub(1));
                        self.focus = BuilderFocus::ParamsEditor { cursor: new, mode: ParamEditorMode::Browse };
                    }
                    KeyCode::Enter if n > 0 && cursor < n => {
                        let p = &self.campaign.params[cursor];
                        self.focus = BuilderFocus::ParamsEditor {
                            cursor,
                            mode: ParamEditorMode::EditParam {
                                idx: cursor,
                                name: p.name.clone(),
                                desc: p.description.clone(),
                                default_val: p.default.clone().unwrap_or_default(),
                                field: 0,
                            },
                        };
                    }
                    _ => {}
                }
            }

            ParamEditorMode::AddParam { mut name, mut desc, mut default_val, field } => {
                match key.code {
                    KeyCode::Esc => {
                        self.focus = BuilderFocus::ParamsEditor { cursor, mode: ParamEditorMode::Browse };
                    }
                    KeyCode::Tab | KeyCode::Enter => {
                        let next_field = field + 1;
                        if next_field > 2 {
                            // Save
                            if !name.trim().is_empty() {
                                self.campaign.params.push(crate::campaign::CampaignParam {
                                    name: name.trim().to_string(),
                                    description: desc.trim().to_string(),
                                    default: if default_val.trim().is_empty() { None } else { Some(default_val.trim().to_string()) },
                                });
                                self.modified = true;
                            }
                            let new_cursor = self.campaign.params.len().saturating_sub(1);
                            self.focus = BuilderFocus::ParamsEditor { cursor: new_cursor, mode: ParamEditorMode::Browse };
                        } else {
                            self.focus = BuilderFocus::ParamsEditor { cursor, mode: ParamEditorMode::AddParam { name, desc, default_val, field: next_field } };
                        }
                    }
                    KeyCode::Backspace => {
                        match field { 0 => { name.pop(); } 1 => { desc.pop(); } _ => { default_val.pop(); } }
                        self.focus = BuilderFocus::ParamsEditor { cursor, mode: ParamEditorMode::AddParam { name, desc, default_val, field } };
                    }
                    KeyCode::Char(c) => {
                        match field { 0 => name.push(c), 1 => desc.push(c), _ => default_val.push(c) }
                        self.focus = BuilderFocus::ParamsEditor { cursor, mode: ParamEditorMode::AddParam { name, desc, default_val, field } };
                    }
                    _ => {}
                }
            }

            ParamEditorMode::EditParam { idx, mut name, mut desc, mut default_val, field } => {
                match key.code {
                    KeyCode::Esc => {
                        self.focus = BuilderFocus::ParamsEditor { cursor, mode: ParamEditorMode::Browse };
                    }
                    KeyCode::Tab | KeyCode::Enter => {
                        let next_field = field + 1;
                        if next_field > 2 {
                            // Save
                            if idx < self.campaign.params.len() && !name.trim().is_empty() {
                                let p = &mut self.campaign.params[idx];
                                p.name        = name.trim().to_string();
                                p.description = desc.trim().to_string();
                                p.default     = if default_val.trim().is_empty() { None } else { Some(default_val.trim().to_string()) };
                                self.modified = true;
                            }
                            self.focus = BuilderFocus::ParamsEditor { cursor, mode: ParamEditorMode::Browse };
                        } else {
                            self.focus = BuilderFocus::ParamsEditor { cursor, mode: ParamEditorMode::EditParam { idx, name, desc, default_val, field: next_field } };
                        }
                    }
                    KeyCode::Backspace => {
                        match field { 0 => { name.pop(); } 1 => { desc.pop(); } _ => { default_val.pop(); } }
                        self.focus = BuilderFocus::ParamsEditor { cursor, mode: ParamEditorMode::EditParam { idx, name, desc, default_val, field } };
                    }
                    KeyCode::Char(c) => {
                        match field { 0 => name.push(c), 1 => desc.push(c), _ => default_val.push(c) }
                        self.focus = BuilderFocus::ParamsEditor { cursor, mode: ParamEditorMode::EditParam { idx, name, desc, default_val, field } };
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    // ── Connectors editor ─────────────────────────────────────────────────────

    fn handle_connectors_key(&mut self, key: crossterm::event::KeyEvent, cursor: usize, mode: IoEditorMode) -> Result<()> {
        use crossterm::event::KeyCode;
        const KINDS: &[&str] = &["csv", "json"];

        match mode {
            IoEditorMode::Browse => {
                let n = self.campaign.connectors.len();
                match key.code {
                    KeyCode::Esc => { self.focus = BuilderFocus::Pipeline; }
                    KeyCode::Up   => { self.focus = BuilderFocus::ConnectorsEditor { cursor: cursor.saturating_sub(1), mode: IoEditorMode::Browse }; }
                    KeyCode::Down => { self.focus = BuilderFocus::ConnectorsEditor { cursor: if n > 0 { (cursor+1).min(n-1) } else { 0 }, mode: IoEditorMode::Browse }; }
                    KeyCode::Char('a') => {
                        self.focus = BuilderFocus::ConnectorsEditor { cursor, mode: IoEditorMode::Edit { idx: None, f0: "csv".into(), f1: String::new(), f2: String::new(), f3: String::new(), field: 0 } };
                    }
                    KeyCode::Char('d') if n > 0 && cursor < n => {
                        self.campaign.connectors.remove(cursor);
                        self.modified = true;
                        let new = cursor.min(self.campaign.connectors.len().saturating_sub(1));
                        self.focus = BuilderFocus::ConnectorsEditor { cursor: new, mode: IoEditorMode::Browse };
                    }
                    KeyCode::Enter if n > 0 && cursor < n => {
                        let c = &self.campaign.connectors[cursor];
                        self.focus = BuilderFocus::ConnectorsEditor { cursor, mode: IoEditorMode::Edit {
                            idx: Some(cursor),
                            f0: c.kind.clone(),
                            f1: c.path.clone(),
                            f2: c.select.clone().unwrap_or_default(),
                            f3: c.from_step.clone().unwrap_or_default(),
                            field: 0,
                        }};
                    }
                    _ => {}
                }
            }
            IoEditorMode::Edit { idx, mut f0, mut f1, mut f2, mut f3, field } => {
                use crossterm::event::KeyCode;
                let max_field = 3u8;
                match key.code {
                    KeyCode::Esc => { self.focus = BuilderFocus::ConnectorsEditor { cursor, mode: IoEditorMode::Browse }; }
                    KeyCode::Left if field == 0 => {
                        let i = KINDS.iter().position(|&k| k == f0).unwrap_or(0);
                        f0 = KINDS[(i + KINDS.len() - 1) % KINDS.len()].to_string();
                        self.focus = BuilderFocus::ConnectorsEditor { cursor, mode: IoEditorMode::Edit { idx, f0, f1, f2, f3, field } };
                    }
                    KeyCode::Right if field == 0 => {
                        let i = KINDS.iter().position(|&k| k == f0).unwrap_or(0);
                        f0 = KINDS[(i + 1) % KINDS.len()].to_string();
                        self.focus = BuilderFocus::ConnectorsEditor { cursor, mode: IoEditorMode::Edit { idx, f0, f1, f2, f3, field } };
                    }
                    KeyCode::Tab | KeyCode::Enter if field == 0 => {
                        self.focus = BuilderFocus::ConnectorsEditor { cursor, mode: IoEditorMode::Edit { idx, f0, f1, f2, f3, field: 1 } };
                    }
                    KeyCode::Tab | KeyCode::Enter => {
                        if field >= max_field {
                            self.save_connector(idx, &f0, &f1, &f2, &f3, cursor);
                        } else {
                            self.focus = BuilderFocus::ConnectorsEditor { cursor, mode: IoEditorMode::Edit { idx, f0, f1, f2, f3, field: field + 1 } };
                        }
                    }
                    KeyCode::Backspace if field > 0 => {
                        match field { 1 => { f1.pop(); } 2 => { f2.pop(); } _ => { f3.pop(); } }
                        self.focus = BuilderFocus::ConnectorsEditor { cursor, mode: IoEditorMode::Edit { idx, f0, f1, f2, f3, field } };
                    }
                    KeyCode::Char(c) if field > 0 => {
                        match field { 1 => f1.push(c), 2 => f2.push(c), _ => f3.push(c) }
                        self.focus = BuilderFocus::ConnectorsEditor { cursor, mode: IoEditorMode::Edit { idx, f0, f1, f2, f3, field } };
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn save_connector(&mut self, idx: Option<usize>, kind: &str, path: &str, select: &str, from_step: &str, _cursor: usize) {
        let conn = crate::connector::ConnectorConfig {
            kind: kind.to_string(),
            path: path.trim().to_string(),
            select: if select.trim().is_empty() { None } else { Some(select.trim().to_string()) },
            from_step: if from_step.trim().is_empty() { None } else { Some(from_step.trim().to_string()) },
        };
        match idx {
            Some(i) if i < self.campaign.connectors.len() => { self.campaign.connectors[i] = conn; }
            _ => { self.campaign.connectors.push(conn); }
        }
        self.modified = true;
        let new_cursor = idx.unwrap_or(self.campaign.connectors.len() - 1);
        self.focus = BuilderFocus::ConnectorsEditor { cursor: new_cursor, mode: IoEditorMode::Browse };
    }

    // ── Outputs editor ────────────────────────────────────────────────────────

    fn handle_outputs_key(&mut self, key: crossterm::event::KeyEvent, cursor: usize, mode: IoEditorMode) -> Result<()> {
        use crossterm::event::KeyCode;

        match mode {
            IoEditorMode::Browse => {
                let n = self.campaign.outputs.len();
                match key.code {
                    KeyCode::Esc => { self.focus = BuilderFocus::Pipeline; }
                    KeyCode::Up   => { self.focus = BuilderFocus::OutputsEditor { cursor: cursor.saturating_sub(1), mode: IoEditorMode::Browse }; }
                    KeyCode::Down => { self.focus = BuilderFocus::OutputsEditor { cursor: if n > 0 { (cursor+1).min(n-1) } else { 0 }, mode: IoEditorMode::Browse }; }
                    KeyCode::Char('a') => {
                        self.focus = BuilderFocus::OutputStepPicker {
                            output_idx: None,
                            step_cursor: 0,
                            f1: String::new(), f2: String::new(), f3: String::new(),
                            output_cursor: cursor,
                        };
                    }
                    KeyCode::Char('d') if n > 0 && cursor < n => {
                        self.campaign.outputs.remove(cursor);
                        self.modified = true;
                        let new = cursor.min(self.campaign.outputs.len().saturating_sub(1));
                        self.focus = BuilderFocus::OutputsEditor { cursor: new, mode: IoEditorMode::Browse };
                    }
                    KeyCode::Enter if n > 0 && cursor < n => {
                        let o = &self.campaign.outputs[cursor];
                        let step_cursor = self.campaign.steps.iter()
                            .position(|s| s.name == o.from_step)
                            .unwrap_or(0);
                        self.focus = BuilderFocus::OutputStepPicker {
                            output_idx: Some(cursor),
                            step_cursor,
                            f1: o.path.clone(),
                            f2: o.select.clone().unwrap_or_default(),
                            f3: o.include_vars.join(", "),
                            output_cursor: cursor,
                        };
                    }
                    _ => {}
                }
            }
            IoEditorMode::Edit { idx, f0, mut f1, mut f2, mut f3, field } => {
                // field 0 (from_step) is set by OutputStepPicker — here we start at field >= 1
                const MAX_FIELD: u8 = 3;
                match key.code {
                    KeyCode::Esc => { self.focus = BuilderFocus::OutputsEditor { cursor, mode: IoEditorMode::Browse }; }
                    KeyCode::Tab | KeyCode::Enter => {
                        if field >= MAX_FIELD {
                            self.save_output(idx, &f0, &f1, &f2, &f3, cursor);
                        } else {
                            self.focus = BuilderFocus::OutputsEditor { cursor, mode: IoEditorMode::Edit { idx, f0, f1, f2, f3, field: field + 1 } };
                        }
                    }
                    KeyCode::Backspace => {
                        match field { 1 => { f1.pop(); } 2 => { f2.pop(); } _ => { f3.pop(); } }
                        self.focus = BuilderFocus::OutputsEditor { cursor, mode: IoEditorMode::Edit { idx, f0, f1, f2, f3, field } };
                    }
                    KeyCode::Char(c) => {
                        match field { 1 => f1.push(c), 2 => f2.push(c), _ => f3.push(c) }
                        self.focus = BuilderFocus::OutputsEditor { cursor, mode: IoEditorMode::Edit { idx, f0, f1, f2, f3, field } };
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn save_output(&mut self, idx: Option<usize>, from_step: &str, path: &str, select: &str, include_vars: &str, _cursor: usize) {
        let out = crate::campaign::OutputConfig {
            from_step: from_step.trim().to_string(),
            path: path.trim().to_string(),
            select: if select.trim().is_empty() { None } else { Some(select.trim().to_string()) },
            include_vars: include_vars.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        };
        match idx {
            Some(i) if i < self.campaign.outputs.len() => { self.campaign.outputs[i] = out; }
            _ => { self.campaign.outputs.push(out); }
        }
        self.modified = true;
        let new_cursor = idx.unwrap_or(self.campaign.outputs.len() - 1);
        self.focus = BuilderFocus::OutputsEditor { cursor: new_cursor, mode: IoEditorMode::Browse };
    }

    // ── Output step picker ────────────────────────────────────────────────────

    fn handle_output_step_picker_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        output_idx: Option<usize>,
        step_cursor: usize,
        f1: String, f2: String, f3: String,
        output_cursor: usize,
    ) -> Result<()> {
        use crossterm::event::KeyCode;
        let steps: Vec<&str> = self.campaign.steps.iter()
            .filter(|s| s.kind != "comment" && s.kind != "transform" && s.kind != "pause" && s.kind != "file")
            .map(|s| s.name.as_str())
            .collect();
        let n = steps.len();
        match key.code {
            KeyCode::Esc => {
                self.focus = BuilderFocus::OutputsEditor { cursor: output_cursor, mode: IoEditorMode::Browse };
            }
            KeyCode::Up => {
                let new = step_cursor.saturating_sub(1);
                self.focus = BuilderFocus::OutputStepPicker { output_idx, step_cursor: new, f1, f2, f3, output_cursor };
            }
            KeyCode::Down => {
                let new = if n > 0 { (step_cursor + 1).min(n - 1) } else { 0 };
                self.focus = BuilderFocus::OutputStepPicker { output_idx, step_cursor: new, f1, f2, f3, output_cursor };
            }
            KeyCode::Enter if n > 0 => {
                let selected = steps[step_cursor.min(n - 1)].to_string();
                self.focus = BuilderFocus::OutputsEditor {
                    cursor: output_cursor,
                    mode: IoEditorMode::Edit {
                        idx: output_idx,
                        f0: selected,
                        f1, f2, f3,
                        field: 1,
                    },
                };
            }
            _ => {}
        }
        Ok(())
    }

    // ── Pipeline connector/output inline navigation ───────────────────────────

    fn handle_pipeline_connectors_key(&mut self, key: crossterm::event::KeyEvent, cursor: usize) -> Result<()> {
        use crossterm::event::KeyCode;
        let n = self.campaign.connectors.len();
        match key.code {
            KeyCode::Esc => { self.focus = BuilderFocus::Pipeline; }
            KeyCode::Up => {
                if cursor > 0 {
                    self.focus = BuilderFocus::PipelineConnectors { cursor: cursor - 1 };
                }
                // at top: stay here (no wrapping back to steps above connectors)
            }
            KeyCode::Down => {
                if cursor + 1 < n {
                    self.focus = BuilderFocus::PipelineConnectors { cursor: cursor + 1 };
                } else {
                    // bottom of connectors → back to steps (step 0)
                    self.cursor = 0;
                    self.focus = BuilderFocus::Pipeline;
                }
            }
            KeyCode::Enter if n > 0 => {
                let c = &self.campaign.connectors[cursor];
                self.focus = BuilderFocus::ConnectorsEditor {
                    cursor,
                    mode: IoEditorMode::Edit {
                        idx: Some(cursor),
                        f0: c.kind.clone(),
                        f1: c.path.clone(),
                        f2: c.select.clone().unwrap_or_default(),
                        f3: c.from_step.clone().unwrap_or_default(),
                        field: 1,
                    },
                };
            }
            KeyCode::Char('d') if n > 0 => {
                self.campaign.connectors.remove(cursor);
                self.modified = true;
                if self.campaign.connectors.is_empty() {
                    self.cursor = 0;
                    self.focus = BuilderFocus::Pipeline;
                } else {
                    self.focus = BuilderFocus::PipelineConnectors {
                        cursor: cursor.min(self.campaign.connectors.len() - 1),
                    };
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_pipeline_outputs_key(&mut self, key: crossterm::event::KeyEvent, cursor: usize) -> Result<()> {
        use crossterm::event::KeyCode;
        let n = self.campaign.outputs.len();
        match key.code {
            KeyCode::Esc => { self.focus = BuilderFocus::Pipeline; }
            KeyCode::Up => {
                if cursor > 0 {
                    self.focus = BuilderFocus::PipelineOutputs { cursor: cursor - 1 };
                } else {
                    // top of outputs → back to last step
                    let last = self.campaign.steps.len().saturating_sub(1);
                    self.cursor = last;
                    self.focus = BuilderFocus::Pipeline;
                }
            }
            KeyCode::Down => {
                if cursor + 1 < n {
                    self.focus = BuilderFocus::PipelineOutputs { cursor: cursor + 1 };
                }
                // at bottom: stay here
            }
            KeyCode::Enter if n > 0 => {
                let o = &self.campaign.outputs[cursor];
                let step_cursor = self.campaign.steps.iter()
                    .position(|s| s.name == o.from_step)
                    .unwrap_or(0);
                self.focus = BuilderFocus::OutputStepPicker {
                    output_idx: Some(cursor),
                    step_cursor,
                    f1: o.path.clone(),
                    f2: o.select.clone().unwrap_or_default(),
                    f3: o.include_vars.join(", "),
                    output_cursor: cursor,
                };
            }
            KeyCode::Char('d') if n > 0 => {
                self.campaign.outputs.remove(cursor);
                self.modified = true;
                if self.campaign.outputs.is_empty() {
                    let last = self.campaign.steps.len().saturating_sub(1);
                    self.cursor = last;
                    self.focus = BuilderFocus::Pipeline;
                } else {
                    self.focus = BuilderFocus::PipelineOutputs {
                        cursor: cursor.min(self.campaign.outputs.len() - 1),
                    };
                }
            }
            _ => {}
        }
        Ok(())
    }

    // ── Run ───────────────────────────────────────────────────────────────────

    fn start_run(&mut self) {
        let campaign = self.campaign.clone();
        let name = campaign.campaign.name.clone();
        let (tx, rx) = mpsc::unbounded_channel::<CampaignEvent>();
        self.run_state = CampaignRunState::Running {
            name,
            step_results: vec![],
            current_step: None,
        };
        self.campaign_rx = Some(rx);
        self.focus = BuilderFocus::Run { scroll: 0 };
        tokio::spawn(async move {
            crate::campaign::run_streaming(campaign, tx, std::collections::HashMap::new()).await;
        });
    }

    pub fn start_step_preview(&mut self, step_idx: usize) {
        let steps = self.campaign.steps.clone();
        let mut env: std::collections::HashMap<String, String> =
            if let Some(ref name) = self.campaign.env_file {
                crate::storage::load_env_by_name(name)
                    .map(|s| s.vars)
                    .unwrap_or_default()
            } else {
                std::collections::HashMap::new()
            };
        env.extend(self.campaign.env.clone());
        let (tx, rx) = mpsc::unbounded_channel::<crate::campaign::StepResult>();
        self.step_preview_rx = Some(rx);
        self.step_preview_result = None;
        self.step_preview_running = true;
        self.status_message = "Running step…".into();
        tokio::spawn(async move {
            let result = crate::campaign::run_step_preview_with_context(steps, step_idx, env).await;
            let _ = tx.send(result);
        });
    }

    pub fn handle_tick(&mut self) {
        // Poll single-step preview channel
        if let Some(ref mut rx) = self.step_preview_rx {
            if let Ok(result) = rx.try_recv() {
                let ok = result.success;
                self.step_preview_result = Some(result);
                self.step_preview_running = false;
                self.step_preview_rx = None;
                self.status_message = if ok { "Step OK".into() } else { "Step failed".into() };
            }
        }

        // Drain the channel into a local vec to avoid borrow conflicts
        let events: Vec<CampaignEvent> = {
            let Some(ref mut rx) = self.campaign_rx else { return };
            let mut buf = Vec::new();
            while let Ok(ev) = rx.try_recv() { buf.push(ev); }
            buf
        };

        let mut finished = false;
        for event in events {
            match event {
                CampaignEvent::StepStarted { name } => {
                    if let CampaignRunState::Running { ref mut current_step, .. } = self.run_state {
                        *current_step = Some(name);
                    }
                }
                CampaignEvent::StepDone(result) => {
                    if let CampaignRunState::Running { ref mut step_results, ref mut current_step, .. } = self.run_state {
                        step_results.push(result);
                        *current_step = None;
                    }
                }
                CampaignEvent::Finished(results) => {
                    let name = if let CampaignRunState::Running { ref name, .. } = self.run_state {
                        name.clone()
                    } else { String::new() };
                    self.run_state = CampaignRunState::Done { name, results };
                    finished = true;
                }
                CampaignEvent::Error(e) => {
                    self.status_message = format!("Run error: {}", e);
                }
                CampaignEvent::Warning(w) => {
                    self.status_message = format!("Warning: {}", w);
                }
                _ => {}
            }
        }
        if finished {
            self.campaign_rx = None;
        }
    }

    fn handle_run_key(&mut self, key: crossterm::event::KeyEvent, scroll: usize) -> Result<()> {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => {
                self.focus = BuilderFocus::Pipeline;
            }
            KeyCode::Up => {
                self.focus = BuilderFocus::Run { scroll: scroll.saturating_sub(1) };
            }
            KeyCode::Down => {
                self.focus = BuilderFocus::Run { scroll: scroll + 1 };
            }
            KeyCode::Char('r') => {
                // Re-run
                if matches!(self.run_state, CampaignRunState::Done { .. } | CampaignRunState::Idle) {
                    self.start_run();
                }
            }
            _ => {}
        }
        Ok(())
    }

    // ── Persistence ───────────────────────────────────────────────────────────

    fn handle_quit_confirm_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Err(e) = self.save() {
                    self.status_message = format!("Save error: {e}");
                    self.quit_confirm = false;
                } else {
                    self.running = false;
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.running = false;
            }
            KeyCode::Esc => {
                self.quit_confirm = false;
            }
            _ => {}
        }
        Ok(())
    }

    fn save(&mut self) -> Result<()> {
        let path = match &self.path {
            Some(p) => p.clone(),
            None => {
                let dir = crate::storage::resolve_terapi_dir().join("campaigns");
                std::fs::create_dir_all(&dir)?;
                let name = crate::storage::sanitize_filename(&self.campaign.campaign.name);
                dir.join(format!("{}.toml", name))
            }
        };
        let toml_str = generate_toml(&self.campaign, &self.step_comments, &self.header_comment);
        std::fs::write(&path, &toml_str)?;
        self.path = Some(path.clone());
        self.modified = false;
        self.status_message = format!("Saved: {}", path.display());
        Ok(())
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn run(path: Option<String>) -> Result<()> {
    let path = path.map(PathBuf::from);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run_builder(&mut terminal, path);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_builder(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    path: Option<PathBuf>,
) -> Result<()> {
    let mut app = BuilderApp::new(path);
    let events = EventHandler::new(250);

    while app.running {
        terminal.draw(|frame| ui::render(frame, &app))?;
        match events.next()? {
            Event::Key(key) => app.handle_key(key)?,
            Event::Tick => app.handle_tick(),
        }
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_header_comment(content: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for raw in content.lines() {
        let trimmed = raw.trim();
        if trimmed.starts_with('[') {
            break;
        }
        if trimmed.starts_with('#') {
            let stripped = if trimmed.starts_with("# ") { &trimmed[2..] }
                           else { &trimmed[1..] };
            lines.push(stripped.to_string());
        }
        // blank lines between comment lines: skip silently
    }
    lines.join("\n")
}

fn parse_step_comments(content: &str, step_count: usize) -> Vec<String> {
    let lines: Vec<&str> = content.lines().collect();
    let step_positions: Vec<usize> = lines.iter().enumerate()
        .filter(|(_, l)| l.trim() == "[[steps]]")
        .map(|(i, _)| i)
        .collect();

    let mut comments = vec![String::new(); step_count];
    for (idx, &sl) in step_positions.iter().enumerate() {
        if idx >= step_count { break; }
        let mut block: Vec<String> = Vec::new();
        let mut j = sl;
        while j > 0 {
            j -= 1;
            let trimmed = lines[j].trim();
            if trimmed.starts_with('#') {
                let stripped = if trimmed.starts_with("# ") { &trimmed[2..] }
                               else { &trimmed[1..] };
                block.push(stripped.to_string());
            } else {
                break;
            }
        }
        block.reverse();
        comments[idx] = block.join("\n");
    }
    comments
}

fn toml_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
     .replace('"', "\\\"")
     .replace('\n', "\\n")
     .replace('\r', "")
     .replace('\t', "\\t")
}

fn empty_campaign(name: &str) -> Campaign {
    Campaign {
        campaign: Meta { name: name.to_string(), description: String::new() },
        params: vec![],
        env: std::collections::HashMap::new(),
        env_file: None,
        connectors: vec![],
        steps: vec![],
        outputs: vec![],
        continue_on_error: false,
    }
}

fn new_step_for(kind: &BrickKind) -> crate::campaign::Step {
    use crate::campaign::Step;
    match kind {
        BrickKind::Http => Step {
            name: "New HTTP step".into(),
            kind: "http".into(),
            method: "GET".into(),
            url: String::new(),
            headers: std::collections::HashMap::new(),
            body: None,
            wait_ms: 0,
            env: None,
            extract: std::collections::HashMap::new(),
            assert: vec![],
            transforms: vec![],
            continue_on_error: None,
            foreach: None,
            when: None,
            description: String::new(),
            file_path: None, file_output: None, file_encoding: None, multipart_parts: vec![],
            graphql_query: None, graphql_variables: std::collections::HashMap::new(), until: None, accumulate: None,
        },
        BrickKind::Loop => Step {
            name: "Paginate".into(),
            kind: "loop".into(),
            method: "GET".into(),
            url: String::new(),
            headers: std::collections::HashMap::new(),
            body: None,
            wait_ms: 0,
            env: None,
            extract: std::collections::HashMap::new(),
            assert: vec![],
            transforms: vec![],
            continue_on_error: None,
            foreach: None,
            when: None,
            description: String::new(),
            file_path: None, file_output: None, file_encoding: None, multipart_parts: vec![],
            graphql_query: None, graphql_variables: std::collections::HashMap::new(),
            until: Some(crate::campaign::StepCondition {
                var: "CURSOR".into(),
                eq: None, ne: None,
                exists: Some(false),
                lt: None, lte: None,
            }),
            accumulate: Some(crate::campaign::AccumulateConfig {
                var: "ALL_ITEMS".into(),
                from: "items.*".into(),
            }),
        },
        BrickKind::GraphQL => Step {
            name: "GraphQL query".into(),
            kind: "graphql".into(),
            method: "POST".into(),
            url: String::new(),
            headers: std::collections::HashMap::new(),
            body: None,
            wait_ms: 0,
            env: None,
            extract: std::collections::HashMap::new(),
            assert: vec![],
            transforms: vec![],
            continue_on_error: None,
            foreach: None,
            when: None,
            description: String::new(),
            file_path: None, file_output: None, file_encoding: None, multipart_parts: vec![],
            graphql_query: Some("{\n  \n}".into()),
            graphql_variables: std::collections::HashMap::new(), until: None, accumulate: None,
        },
        BrickKind::Transform => Step {
            name: "New transform".into(),
            kind: "transform".into(),
            method: String::new(),
            url: String::new(),
            headers: std::collections::HashMap::new(),
            body: None,
            wait_ms: 0,
            env: None,
            extract: std::collections::HashMap::new(),
            assert: vec![],
            transforms: vec![],
            continue_on_error: None,
            foreach: None,
            when: None,
            description: String::new(),
            file_path: None, file_output: None, file_encoding: None, multipart_parts: vec![],
            graphql_query: None, graphql_variables: std::collections::HashMap::new(), until: None, accumulate: None,
        },
        BrickKind::Pause => Step {
            name: "Pause".into(),
            kind: "pause".into(),
            method: String::new(),
            url: String::new(),
            headers: std::collections::HashMap::new(),
            body: None,
            wait_ms: 1000,
            env: None,
            extract: std::collections::HashMap::new(),
            assert: vec![],
            transforms: vec![],
            continue_on_error: None,
            foreach: None,
            when: None,
            description: String::new(),
            file_path: None, file_output: None, file_encoding: None, multipart_parts: vec![],
            graphql_query: None, graphql_variables: std::collections::HashMap::new(), until: None, accumulate: None,
        },
        BrickKind::Seed => Step {
            name: "Seed".into(),
            kind: "seed".into(),
            method: "GET".into(),
            url: String::new(),
            headers: std::collections::HashMap::new(),
            body: None,
            wait_ms: 0,
            env: None,
            extract: std::collections::HashMap::new(),
            assert: vec![],
            transforms: vec![],
            continue_on_error: None,
            foreach: None,
            when: None,
            description: String::new(),
            file_path: None, file_output: None, file_encoding: None, multipart_parts: vec![],
            graphql_query: None, graphql_variables: std::collections::HashMap::new(), until: None, accumulate: None,
        },
        BrickKind::Comment => Step {
            name: "Comment text here".into(),
            kind: "comment".into(),
            method: String::new(),
            url: String::new(),
            headers: std::collections::HashMap::new(),
            body: None,
            wait_ms: 0,
            env: None,
            extract: std::collections::HashMap::new(),
            assert: vec![],
            transforms: vec![],
            continue_on_error: None,
            foreach: None,
            when: None,
            description: String::new(),
            file_path: None, file_output: None, file_encoding: None, multipart_parts: vec![],
            graphql_query: None, graphql_variables: std::collections::HashMap::new(), until: None, accumulate: None,
        },
        BrickKind::FileLoader => Step {
            name: "Load file".into(),
            kind: "file".into(),
            method: String::new(),
            url: String::new(),
            headers: std::collections::HashMap::new(),
            body: None,
            wait_ms: 0,
            env: None,
            extract: std::collections::HashMap::new(),
            assert: vec![],
            transforms: vec![],
            continue_on_error: None,
            foreach: None,
            when: None,
            description: String::new(),
            file_path: Some(String::new()),
            file_output: Some("FILE_DATA".into()),
            file_encoding: Some("base64".into()),
            multipart_parts: vec![],
            graphql_query: None, graphql_variables: std::collections::HashMap::new(), until: None, accumulate: None,
        },
        // Connector and Output are handled directly in handle_catalog_key — not steps
        BrickKind::Connector | BrickKind::Output => unreachable!(),
    }
}

pub(super) fn generate_toml(campaign: &Campaign, step_comments: &[String], header_comment: &str) -> String {
    let mut out = String::new();
    if !header_comment.is_empty() {
        for line in header_comment.lines() {
            out.push_str(&format!("# {}\n", line));
        }
        out.push('\n');
    }
    let m = &campaign.campaign;
    out.push_str(&format!("[campaign]\nname        = \"{}\"\ndescription = \"{}\"\n", m.name, m.description));

    if campaign.continue_on_error {
        out.push_str("continue_on_error = true\n");
    }
    if let Some(ref env) = campaign.env_file {
        out.push_str(&format!("env_file = \"{}\"\n", env));
    }

    for c in &campaign.connectors {
        out.push_str("\n[[connectors]]\n");
        out.push_str(&format!("type = \"{}\"\n", c.kind));
        if !c.path.is_empty() {
            out.push_str(&format!("path = \"{}\"\n", toml_escape(&c.path)));
        }
        if let Some(ref s) = c.select {
            out.push_str(&format!("select = \"{}\"\n", toml_escape(s)));
        }
        if let Some(ref fs) = c.from_step {
            out.push_str(&format!("from_step = \"{}\"\n", toml_escape(fs)));
        }
    }

    for o in &campaign.outputs {
        out.push_str("\n[[outputs]]\n");
        out.push_str(&format!("from_step = \"{}\"\n", toml_escape(&o.from_step)));
        out.push_str(&format!("path      = \"{}\"\n", toml_escape(&o.path)));
        if let Some(ref s) = o.select {
            out.push_str(&format!("select = \"{}\"\n", toml_escape(s)));
        }
        if !o.include_vars.is_empty() {
            let vars: Vec<String> = o.include_vars.iter().map(|v| format!("\"{}\"", toml_escape(v))).collect();
            out.push_str(&format!("include_vars = [{}]\n", vars.join(", ")));
        }
    }

    for p in &campaign.params {
        out.push_str("\n[[params]]\n");
        out.push_str(&format!("name        = \"{}\"\n", toml_escape(&p.name)));
        if !p.description.is_empty() {
            out.push_str(&format!("description = \"{}\"\n", toml_escape(&p.description)));
        }
        if let Some(ref d) = p.default {
            out.push_str(&format!("default     = \"{}\"\n", toml_escape(d)));
        }
    }

    if !campaign.env.is_empty() {
        out.push_str("\n[env]\n");
        let mut vars: Vec<_> = campaign.env.iter().collect();
        vars.sort_by_key(|(k, _)| k.as_str());
        for (k, v) in vars {
            out.push_str(&format!("{} = \"{}\"\n", k, v));
        }
    }

    for (i, step) in campaign.steps.iter().enumerate() {
        if step.kind == "comment" {
            out.push_str(&format!("\n# {}\n", step.name));
            continue;
        }
        let comment = step_comments.get(i).map(|s| s.as_str()).unwrap_or("");
        if !comment.is_empty() {
            out.push('\n');
            for line in comment.lines() {
                out.push_str(&format!("# {}\n", line));
            }
        }
        out.push_str("\n[[steps]]\n");
        out.push_str(&format!("name   = \"{}\"\n", step.name));
        if !step.description.is_empty() {
            out.push_str(&format!("description = \"{}\"\n", toml_escape(&step.description)));
        }
        if step.kind != "http" {
            out.push_str(&format!("kind   = \"{}\"\n", step.kind));
        }
        if !step.method.is_empty() {
            out.push_str(&format!("method = \"{}\"\n", step.method));
        }
        if !step.url.is_empty() {
            out.push_str(&format!("url    = \"{}\"\n", step.url));
        }
        if let Some(ref body) = step.body {
            if !body.is_empty() {
                if body.contains('\n') {
                    // multi-line: TOML literal block string
                    out.push_str(&format!("body   = '''\n{}\n'''\n", body));
                } else {
                    // single-line: literal string (avoids escaping double quotes in JSON)
                    out.push_str(&format!("body   = '{}'\n", body.replace('\'', "\\'")));
                }
            }
        }
        if let Some(ref q) = step.graphql_query {
            if !q.is_empty() {
                if q.contains('\n') {
                    out.push_str(&format!("graphql_query = '''\n{}\n'''\n", q));
                } else {
                    out.push_str(&format!("graphql_query = '{}'\n", q.replace('\'', "\\'")));
                }
            }
        }
        if step.wait_ms > 0 {
            out.push_str(&format!("wait_ms = {}\n", step.wait_ms));
        }
        if let Some(ref p) = step.file_path {
            if !p.is_empty() { out.push_str(&format!("file_path     = \"{}\"\n", p)); }
        }
        if let Some(ref o) = step.file_output {
            if o != "FILE_DATA" { out.push_str(&format!("file_output   = \"{}\"\n", o)); }
        }
        if let Some(ref e) = step.file_encoding {
            if e != "base64" { out.push_str(&format!("file_encoding  = \"{}\"\n", e)); }
        }
        if let Some(foreach) = &step.foreach {
            out.push_str(&format!("foreach = \"{}\"\n", foreach));
        }
        if let Some(env) = &step.env {
            out.push_str(&format!("env    = \"{}\"\n", env));
        }
        if let Some(coe) = step.continue_on_error {
            out.push_str(&format!("continue_on_error = {}\n", coe));
        }
        // Inline scalars (when/assert/transforms) must come before any [subtable] headers
        if let Some(when) = &step.when {
            let mut w = format!("when   = {{var = \"{}\"", when.var);
            if let Some(eq) = &when.eq { w.push_str(&format!(", eq = \"{}\"", eq)); }
            if let Some(ne) = &when.ne { w.push_str(&format!(", ne = \"{}\"", ne)); }
            if let Some(b)  = when.exists { w.push_str(&format!(", exists = {}", b)); }
            w.push_str("}\n");
            out.push_str(&w);
        }
        if !step.assert.is_empty() {
            let parts: Vec<String> = step.assert.iter().map(assertion_inline).collect();
            out.push_str(&format!("assert = [{}]\n", parts.join(", ")));
        }
        if !step.transforms.is_empty() {
            let parts: Vec<String> = step.transforms.iter().map(|t| {
                let mut fields = vec![
                    format!("type = \"{}\"", t.kind),
                    format!("input = \"{}\"", toml_escape(&t.input)),
                    format!("output = \"{}\"", toml_escape(&t.output)),
                ];
                if let Some(ref p) = t.pattern   { fields.push(format!("pattern = \"{}\"",   toml_escape(p))); }
                if t.group != 0                  { fields.push(format!("group = {}", t.group)); }
                if let Some(ref f) = t.from      { fields.push(format!("from = \"{}\"",      toml_escape(f))); }
                if let Some(ref to) = t.to       { fields.push(format!("to = \"{}\"",        toml_escape(to))); }
                if let Some(ref d) = t.delimiter { fields.push(format!("delimiter = \"{}\"", toml_escape(d))); }
                if t.index != 0                  { fields.push(format!("index = {}", t.index)); }
                format!("{{ {} }}", fields.join(", "))
            }).collect();
            out.push_str(&format!("transforms = [\n{}\n]\n",
                parts.iter().map(|p| format!("  {}", p)).collect::<Vec<_>>().join(",\n")));
        }
        if let Some(until) = &step.until {
            let mut u = format!("until = {{var = \"{}\"", until.var);
            if let Some(eq)  = &until.eq    { u.push_str(&format!(", eq = \"{}\"", eq)); }
            if let Some(ne)  = &until.ne    { u.push_str(&format!(", ne = \"{}\"", ne)); }
            if let Some(b)   = until.exists { u.push_str(&format!(", exists = {}", b)); }
            if let Some(lt)  = until.lt     { u.push_str(&format!(", lt = {}", lt)); }
            if let Some(lte) = until.lte    { u.push_str(&format!(", lte = {}", lte)); }
            u.push_str("}\n");
            out.push_str(&u);
        }
        if let Some(acc) = &step.accumulate {
            out.push_str(&format!("accumulate = {{var = \"{}\", from = \"{}\"}}\n", acc.var, acc.from));
        }
        // Subtable headers after all scalar/inline fields
        if !step.headers.is_empty() {
            out.push_str("[steps.headers]\n");
            let mut headers: Vec<_> = step.headers.iter().collect();
            headers.sort_by_key(|(k, _)| k.as_str());
            for (k, v) in headers {
                out.push_str(&format!("{} = \"{}\"\n", k, v));
            }
        }
        if !step.graphql_variables.is_empty() {
            out.push_str("[steps.graphql_variables]\n");
            let mut vars: Vec<_> = step.graphql_variables.iter().collect();
            vars.sort_by_key(|(k, _)| k.as_str());
            for (k, v) in vars {
                out.push_str(&format!("{} = \"{}\"\n", k, toml_escape(v)));
            }
        }
        if !step.extract.is_empty() {
            out.push_str("[steps.extract]\n");
            let mut ex: Vec<_> = step.extract.iter().collect();
            ex.sort_by_key(|(k, _)| k.as_str());
            for (k, v) in ex {
                out.push_str(&format!("{} = \"{}\"\n", k, v));
            }
        }
        for mp in &step.multipart_parts {
            out.push_str("\n[[steps.multipart_parts]]\n");
            out.push_str(&format!("name  = \"{}\"\n", toml_escape(&mp.name)));
            out.push_str(&format!("value = \"{}\"\n", toml_escape(&mp.value)));
            if let Some(ref ct) = mp.content_type {
                out.push_str(&format!("content_type = \"{}\"\n", toml_escape(ct)));
            }
        }
    }

    out
}

fn assertion_inline(a: &crate::campaign::Assertion) -> String {
    let mut parts = vec![format!("on = \"{}\"", a.on.replace('"', "\\\""))];
    if let Some(v) = &a.eq      { parts.push(format!("eq = {}", json_val_str(v))); }
    if let Some(v) = &a.ne      { parts.push(format!("ne = {}", json_val_str(v))); }
    if let Some(v) = &a.lt      { parts.push(format!("lt = {}", v)); }
    if let Some(v) = &a.lte     { parts.push(format!("lte = {}", v)); }
    if let Some(v) = &a.gt      { parts.push(format!("gt = {}", v)); }
    if let Some(v) = &a.gte     { parts.push(format!("gte = {}", v)); }
    if let Some(v) = &a.contains { parts.push(format!("contains = \"{}\"", v.replace('"', "\\\""))); }
    if let Some(v) = &a.matches  { parts.push(format!("matches = \"{}\"", v.replace('"', "\\\""))); }
    if let Some(b) = &a.exists  { parts.push(format!("exists = {}", b)); }
    format!("{{{}}}", parts.join(", "))
}

fn json_val_str(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b)   => b.to_string(),
        serde_json::Value::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
        other                        => format!("\"{}\"", other),
    }
}
