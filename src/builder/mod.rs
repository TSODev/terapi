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

use crate::campaign::{Campaign, Meta};
use types::StepEditorMode;
use crate::event::{Event, EventHandler};
use crate::storage::StoredCollection;
use types::*;

pub struct BuilderApp {
    pub running: bool,
    pub campaign: Campaign,
    pub path: Option<PathBuf>,
    pub cursor: usize,
    pub focus: BuilderFocus,
    pub modified: bool,
    pub stored_collections: Vec<StoredCollection>,
    pub status_message: String,
}

impl BuilderApp {
    pub fn new(path: Option<PathBuf>) -> Self {
        let campaign = if let Some(ref p) = path {
            crate::campaign::load(p.to_str().unwrap_or(""))
                .unwrap_or_else(|_| empty_campaign("new_campaign"))
        } else {
            empty_campaign("new_campaign")
        };
        let stored_collections = crate::storage::load_collections().unwrap_or_default();
        Self {
            running: true,
            campaign,
            path,
            cursor: 0,
            focus: BuilderFocus::Pipeline,
            modified: false,
            stored_collections,
            status_message: String::new(),
        }
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match self.focus.clone() {
            BuilderFocus::Pipeline => self.handle_pipeline_key(key),
            BuilderFocus::Catalog { cursor, insert_after } => {
                self.handle_catalog_key(key, cursor, insert_after)
            }
            BuilderFocus::StepEditor { step_idx, section_cursor, sub_cursor, mode } => {
                step_editor::handle_key(self, key, step_idx, section_cursor, sub_cursor, mode)
            }
            BuilderFocus::CollectionBrowser { for_step, col_cursor, expanded } => {
                browser::handle_key(self, key, for_step, col_cursor, expanded)
            }
            BuilderFocus::Checker { .. }         => self.handle_overlay_key(key),
            BuilderFocus::TomlPreview { scroll } => self.handle_preview_key(key, scroll),
            BuilderFocus::Variables { cursor }   => self.handle_variables_key(key, cursor),
        }
    }

    // ── Pipeline ──────────────────────────────────────────────────────────────

    fn handle_pipeline_key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        use crossterm::event::KeyCode;
        let step_count = self.campaign.steps.len();
        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Up => {
                if self.cursor > 0 { self.cursor -= 1; }
            }
            KeyCode::Down => {
                if step_count > 0 && self.cursor < step_count - 1 {
                    self.cursor += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char('e') if step_count > 0 => {
                self.focus = BuilderFocus::StepEditor {
                    step_idx: self.cursor,
                    section_cursor: 0,
                    sub_cursor: 0,
                    mode: StepEditorMode::Browse,
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
                self.focus = BuilderFocus::Variables { cursor: 0 };
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
                let step = new_step_for(kind);
                let pos = match insert_after {
                    Some(after) => after + 1,
                    None => self.campaign.steps.len(),
                };
                self.campaign.steps.insert(pos, step);
                self.cursor = pos;
                self.modified = true;
                // TODO: open StepEditor for the newly created step
                self.focus = BuilderFocus::Pipeline;
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

    fn handle_variables_key(&mut self, key: crossterm::event::KeyEvent, cursor: usize) -> Result<()> {
        use crossterm::event::KeyCode;
        let var_count = self.campaign.env.len();
        match key.code {
            KeyCode::Esc  => { self.focus = BuilderFocus::Pipeline; }
            KeyCode::Up   => {
                let new = cursor.saturating_sub(1);
                self.focus = BuilderFocus::Variables { cursor: new };
            }
            KeyCode::Down => {
                let new = if var_count > 0 { (cursor + 1).min(var_count - 1) } else { 0 };
                self.focus = BuilderFocus::Variables { cursor: new };
            }
            _ => {}
        }
        Ok(())
    }

    // ── Persistence ───────────────────────────────────────────────────────────

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
        let toml_str = generate_toml(&self.campaign);
        std::fs::write(&path, &toml_str)?;
        self.path = Some(path.clone());
        self.modified = false;
        self.status_message = format!("Sauvegardé : {}", path.display());
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
            Event::Tick => {}
        }
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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
        },
    }
}

fn generate_toml(campaign: &Campaign) -> String {
    let mut out = String::new();
    let m = &campaign.campaign;
    out.push_str(&format!("[campaign]\nname        = \"{}\"\ndescription = \"{}\"\n", m.name, m.description));

    if !campaign.env.is_empty() {
        out.push_str("\n[env]\n");
        let mut vars: Vec<_> = campaign.env.iter().collect();
        vars.sort_by_key(|(k, _)| k.as_str());
        for (k, v) in vars {
            out.push_str(&format!("{} = \"{}\"\n", k, v));
        }
    }

    for step in &campaign.steps {
        out.push_str("\n[[steps]]\n");
        out.push_str(&format!("name   = \"{}\"\n", step.name));
        if step.kind != "http" {
            out.push_str(&format!("kind   = \"{}\"\n", step.kind));
        }
        if !step.method.is_empty() {
            out.push_str(&format!("method = \"{}\"\n", step.method));
        }
        if !step.url.is_empty() {
            out.push_str(&format!("url    = \"{}\"\n", step.url));
        }
        if step.wait_ms > 0 {
            out.push_str(&format!("wait_ms = {}\n", step.wait_ms));
        }
        if let Some(foreach) = &step.foreach {
            out.push_str(&format!("foreach = \"{}\"\n", foreach));
        }
        if !step.headers.is_empty() {
            out.push_str("[steps.headers]\n");
            let mut headers: Vec<_> = step.headers.iter().collect();
            headers.sort_by_key(|(k, _)| k.as_str());
            for (k, v) in headers {
                out.push_str(&format!("{} = \"{}\"\n", k, v));
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
    }

    out
}
