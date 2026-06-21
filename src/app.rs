use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

/// Current active tab.
#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    Request,
    Collections,
    History,
}

impl Tab {
    pub fn title(&self) -> &'static str {
        match self {
            Tab::Request => "Request",
            Tab::Collections => "Collections",
            Tab::History => "History",
        }
    }

    pub fn all() -> Vec<Tab> {
        vec![Tab::Request, Tab::Collections, Tab::History]
    }
}

/// Application state.
pub struct App {
    pub running: bool,
    pub active_tab: Tab,
    pub status_message: String,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            active_tab: Tab::Request,
            status_message: String::from("Ready — press q to quit, Tab to switch panels"),
        }
    }

    /// Handle a keyboard event and update state accordingly.
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.running = false;
            }
            KeyCode::Tab => {
                self.active_tab = match self.active_tab {
                    Tab::Request => Tab::Collections,
                    Tab::Collections => Tab::History,
                    Tab::History => Tab::Request,
                };
                self.status_message =
                    format!("Switched to {}", self.active_tab.title());
            }
            _ => {}
        }
        Ok(())
    }

    pub fn tick(&mut self) {
        // Future: background task updates (in-flight requests, etc.)
    }
}
