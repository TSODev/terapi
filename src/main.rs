mod app;
mod event;
mod json_highlight;
mod ui;

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

use app::App;
use event::{Event, EventHandler};

#[tokio::main]
async fn main() -> Result<()> {
    let json = load_initial_json();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, json).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

/// Load JSON from the first CLI argument, or fall back to `demo.json` in the
/// current directory. Returns None if neither is found.
fn load_initial_json() -> Option<String> {
    if let Some(path) = std::env::args().nth(1) {
        match std::fs::read_to_string(&path) {
            Ok(content) => return Some(content),
            Err(e) => eprintln!("terapi: cannot read '{}': {}", path, e),
        }
    }

    std::fs::read_to_string("demo.json").ok()
}

async fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, json: Option<String>) -> Result<()> {
    let mut app = App::new(json);
    let events = EventHandler::new(250);

    while app.running {
        terminal.draw(|frame| ui::render(frame, &app))?;

        match events.next()? {
            Event::Key(key) => app.handle_key(key)?,
            Event::Tick => app.tick(),
        }
    }

    Ok(())
}
