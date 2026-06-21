mod app;
mod event;
mod json_highlight;
mod ui;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

use app::App;
use event::{Event, EventHandler};

/// Terapi — keyboard-driven TUI for REST and GraphQL APIs
#[derive(Parser)]
#[command(name = "terapi", version, about, long_about = None)]
struct Cli {
    /// Load a JSON file into the response viewer (demo/dev mode)
    #[arg(long, value_name = "FILE")]
    demo: Option<String>,

    // Future subcommands (run, validate, …) will be added here
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let json = load_json(cli.demo.as_deref());

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

fn load_json(path: Option<&str>) -> Option<String> {
    let p = path?;
    match std::fs::read_to_string(p) {
        Ok(content) => Some(content),
        Err(e) => {
            eprintln!("terapi: cannot read '{}': {}", p, e);
            None
        }
    }
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
