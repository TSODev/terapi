mod app;
mod builder;
mod campaign;
mod connector;
mod event;
mod import;
mod json_highlight;
mod storage;
mod ui;

use anyhow::Result;
use clap::{Parser, Subcommand};
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
    #[command(subcommand)]
    command: Option<Commands>,

    /// Load a JSON file into the response viewer (demo/dev mode)
    #[arg(long, value_name = "FILE", global = true)]
    demo: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a campaign from a TOML file (headless)
    Run {
        /// Path to the campaign TOML file
        #[arg(value_name = "CAMPAIGN")]
        file: String,

        /// Suppress all output — exit 0 on success, 1 on failure
        #[arg(long, short = 's')]
        silent: bool,

        /// Override a campaign parameter: KEY=VALUE (repeatable)
        #[arg(long, short = 'p', value_name = "KEY=VALUE")]
        param: Vec<String>,

        /// Run only the named step(s) — skip all others (repeatable)
        #[arg(long, value_name = "STEP_NAME")]
        only: Vec<String>,

        /// Output format: text (default), json, csv
        #[arg(long, value_name = "FORMAT", default_value = "text")]
        format: String,

        /// Retry failed HTTP/GraphQL steps up to N times with exponential backoff
        #[arg(long, default_value_t = 0, value_name = "N")]
        retry: u32,
    },

    /// Import a collection or campaign TOML file into the terapi directory
    Import {
        /// Path to the collection or campaign TOML file to import
        #[arg(value_name = "FILE")]
        file: String,
    },

    /// Build or edit a campaign interactively (TUI campaign editor)
    Build {
        /// Path to an existing campaign TOML file (optional — starts blank if omitted)
        #[arg(value_name = "CAMPAIGN")]
        file: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Run { file, silent, param, only, format, retry }) => {
            let camp = campaign::load(&file)?;
            let mut overrides: std::collections::HashMap<String, String> = param.iter()
                .filter_map(|p| p.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
                .collect();
            // Prompt for params that have no default and were not supplied via --param
            if !silent {
                for p in &camp.params {
                    if !overrides.contains_key(&p.name) && p.default.is_none() {
                        let prompt = if p.description.is_empty() {
                            format!("{}: ", p.name)
                        } else {
                            format!("{} ({}): ", p.name, p.description)
                        };
                        eprint!("{}", prompt);
                        let mut line = String::new();
                        std::io::stdin().read_line(&mut line)?;
                        overrides.insert(p.name.clone(), line.trim().to_string());
                    }
                }
            }
            let fmt = match format.as_str() {
                "json" => campaign::OutputFormat::Json,
                "csv"  => campaign::OutputFormat::Csv,
                _      => campaign::OutputFormat::Text,
            };
            campaign::run(&camp, silent, overrides, only, fmt, retry).await?;
        }
        Some(Commands::Import { file }) => {
            import_collection(&file)?;
        }
        Some(Commands::Build { file }) => {
            builder::run(file)?;
        }
        None => launch_tui(load_json(cli.demo.as_deref())).await?,
    }

    Ok(())
}

fn import_collection(path: &str) -> Result<()> {
    use anyhow::Context;

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read '{}'", path))?;

    // Detect JSON (Postman collection or environment)
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "json" {
        let report = import::import_json(path, &content)
            .with_context(|| format!("failed to import '{}'", path))?;
        report.print();
        return Ok(());
    }

    // TOML: terapi collection or campaign
    let parsed: toml::Value = toml::from_str(&content)
        .with_context(|| format!("'{}' is not valid TOML", path))?;

    if parsed.get("campaign").is_some() {
        // ── Campaign ──────────────────────────────────────────────────────────
        let camp: campaign::Campaign = toml::from_str(&content)
            .with_context(|| format!("'{}' is not a valid campaign TOML", path))?;

        let dir = storage::resolve_terapi_dir().join("campaigns");
        std::fs::create_dir_all(&dir)?;

        let filename = storage::sanitize_filename(&camp.campaign.name);
        let dest = dir.join(format!("{}.toml", filename));
        let existed = dest.exists();
        std::fs::write(&dest, &content)?;

        if existed {
            println!("Updated  campaign \"{}\" → {}", camp.campaign.name, dest.display());
        } else {
            println!("Imported campaign \"{}\" → {}", camp.campaign.name, dest.display());
        }
    } else if parsed.get("collection").is_some() {
        // ── Collection ────────────────────────────────────────────────────────
        let col: storage::StoredCollection = toml::from_str(&content)
            .with_context(|| format!("'{}' is not a valid collection TOML", path))?;

        let dir = storage::resolve_terapi_dir().join("collections");
        std::fs::create_dir_all(&dir)?;

        let filename = storage::sanitize_filename(&col.collection.name);
        let dest = dir.join(format!("{}.toml", filename));
        let existed = dest.exists();
        std::fs::write(&dest, &content)?;

        if existed {
            println!("Updated  collection \"{}\" → {}", col.collection.name, dest.display());
        } else {
            println!("Imported collection \"{}\" → {}", col.collection.name, dest.display());
        }
    } else {
        anyhow::bail!(
            "'{}' is not a recognised terapi file (must have a [collection] or [campaign] section)",
            path
        );
    }

    Ok(())
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

async fn launch_tui(json: Option<String>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run_tui(&mut terminal, json).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_tui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, json: Option<String>) -> Result<()> {
    let mut app = App::new(json);
    let events = EventHandler::new(250);

    while app.running {
        terminal.draw(|frame| ui::render(frame, &app))?;

        match events.next()? {
            Event::Key(key) => app.handle_key(key)?,
            Event::Tick => app.tick(),
        }

        if app.pending_diff_open {
            app.pending_diff_open = false;
            if let (Some(prev), Some(curr)) = (&app.previous_response_body.clone(), &app.response_body.clone()) {
                let prev_path = "/tmp/terapi_prev.json";
                let curr_path = "/tmp/terapi_curr.json";
                let _ = std::fs::write(prev_path, prev);
                let _ = std::fs::write(curr_path, curr);
                let cmd = std::env::var("TERAPI_DIFF")
                    .map(|d| format!("{} \"{}\" \"{}\"", d, prev_path, curr_path))
                    .unwrap_or_else(|_| format!(
                        "diff -u \"{}\" \"{}\" | ${{PAGER:-less -R}}",
                        prev_path, curr_path
                    ));
                disable_raw_mode()?;
                execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                let _ = std::process::Command::new("sh").arg("-c").arg(&cmd).status();
                enable_raw_mode()?;
                execute!(terminal.backend_mut(), EnterAlternateScreen)?;
                terminal.clear()?;
                app.status_message = "r: cycle view  d: diff with previous  /: search".into();
            }
        }

        if app.pending_json_editor_open {
            app.pending_json_editor_open = false;
            let body = app.body_string().unwrap_or_default();
            let tmp = "/tmp/terapi_body.json";
            let _ = std::fs::write(tmp, &body);
            let editor = std::env::var("TERAPI_JSON_EDITOR")
                .unwrap_or_else(|_| "jsoned".to_string());
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
            // Launch directly (not via sh -c) to preserve TTY inheritance for TUI editors.
            // Fall back to sh -c only if the editor string contains shell metacharacters.
            if editor.contains(|c: char| matches!(c, ' ' | '|' | '>' | '<' | '&' | ';')) {
                let cmd = format!("{} \"{}\"", editor, tmp);
                let _ = std::process::Command::new("sh").arg("-c").arg(&cmd).status();
            } else {
                let _ = std::process::Command::new(&editor).arg(tmp).status();
            }
            enable_raw_mode()?;
            execute!(terminal.backend_mut(), EnterAlternateScreen)?;
            terminal.clear()?;
            if let Ok(new_body) = std::fs::read_to_string(tmp) {
                app.set_body_text(new_body);
            }
            app.update_request_status_hint();
        }

        if let Some(path) = app.pending_editor_open.take() {
            let editor = std::env::var("EDITOR")
                .or_else(|_| std::env::var("VISUAL"))
                .unwrap_or_else(|_| "vi".to_string());
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
            let _ = std::process::Command::new(&editor).arg(&path).status();
            enable_raw_mode()?;
            execute!(terminal.backend_mut(), EnterAlternateScreen)?;
            terminal.clear()?;
            // Reload collections and campaigns from disk
            app.stored_collections = crate::storage::load_collections().unwrap_or_default();
            let campaigns_data = crate::storage::load_campaigns();
            app.campaigns = campaigns_data.into_iter()
                .map(|(name, path, campaign)| crate::app::CampaignEntry { name, path, campaign })
                .collect();
            app.status_message = format!("Reloaded after editing in {}", editor);
        }
    }

    Ok(())
}
