use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use super::BuilderApp;
use super::types::{BuilderFocus, PairTarget, StepEditorMode, StepSection};

pub const METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE"];

pub const TRANSFORM_KINDS: &[&str] = &[
    "template", "regex", "replace", "split", "trim", "upper", "lower",
];

pub fn sections_for(kind: &str) -> Vec<StepSection> {
    match kind {
        "comment" => vec![
            StepSection::Name,
        ],
        "pause" => vec![
            StepSection::Name,
            StepSection::WaitMs,
        ],
        "transform" => vec![
            StepSection::Name,
            StepSection::TransformKind,
            StepSection::TransformInput,
            StepSection::TransformOutput,
        ],
        _ => vec![
            StepSection::Name,
            StepSection::Method,
            StepSection::Url,
            StepSection::Headers,
            StepSection::Body,
            StepSection::Extract,
            StepSection::Assertions,
            StepSection::Foreach,
            StepSection::When,
            StepSection::ContinueOnError,
            StepSection::LoadFromCollection,
        ],
    }
}

pub fn handle_key(
    app: &mut BuilderApp,
    key: KeyEvent,
    step_idx: usize,
    section_cursor: usize,
    sub_cursor: usize,
    mode: StepEditorMode,
    desc_active: bool,
) -> Result<()> {
    // Description textarea captures all keys when active
    if desc_active {
        if key.code == KeyCode::Esc {
            save_description(app, step_idx);
            app.focus = BuilderFocus::StepEditor {
                step_idx, section_cursor, sub_cursor,
                mode: StepEditorMode::Browse,
                desc_active: false,
            };
        } else {
            app.description_textarea.input(tui_textarea::Input::from(key));
            app.focus = BuilderFocus::StepEditor {
                step_idx, section_cursor, sub_cursor, mode,
                desc_active: true,
            };
        }
        return Ok(());
    }

    match mode {
        StepEditorMode::Browse =>
            handle_browse(app, key, step_idx, section_cursor, sub_cursor),
        StepEditorMode::EditText { buffer } =>
            handle_edit_text(app, key, step_idx, section_cursor, sub_cursor, buffer),
        StepEditorMode::AddPairStage1 { target, buffer } =>
            handle_add_stage1(app, key, step_idx, section_cursor, sub_cursor, target, buffer),
        StepEditorMode::AddPairStage2 { target, key: pair_key, buffer } =>
            handle_add_stage2(app, key, step_idx, section_cursor, sub_cursor, target, pair_key, buffer),
    }
}

fn save_description(app: &mut BuilderApp, step_idx: usize) {
    let text = app.description_textarea.lines().join("\n");
    if app.campaign.steps[step_idx].description != text {
        app.campaign.steps[step_idx].description = text;
        app.modified = true;
    }
}

// ── Browse mode ───────────────────────────────────────────────────────────────

fn handle_browse(
    app: &mut BuilderApp,
    key: KeyEvent,
    step_idx: usize,
    section_cursor: usize,
    sub_cursor: usize,
) -> Result<()> {
    let kind = app.campaign.steps[step_idx].kind.clone();
    let sections = sections_for(&kind);
    let n = sections.len();
    let section = sections[section_cursor].clone();

    match key.code {
        KeyCode::Esc => {
            app.focus = BuilderFocus::Pipeline;
            return Ok(());
        }
        KeyCode::Up => {
            if section_cursor == 0 {
                // Enter description textarea
                let desc = app.campaign.steps[step_idx].description.clone();
                let lines: Vec<String> = desc.lines().map(String::from).collect();
                app.description_textarea = if lines.is_empty() {
                    tui_textarea::TextArea::default()
                } else {
                    tui_textarea::TextArea::from(lines)
                };
                app.focus = BuilderFocus::StepEditor {
                    step_idx, section_cursor, sub_cursor,
                    mode: StepEditorMode::Browse,
                    desc_active: true,
                };
            } else {
                let c = section_cursor.saturating_sub(1);
                set_focus(app, step_idx, c, 0, StepEditorMode::Browse);
            }
            return Ok(());
        }
        KeyCode::Down => {
            let c = (section_cursor + 1).min(n.saturating_sub(1));
            set_focus(app, step_idx, c, 0, StepEditorMode::Browse);
            return Ok(());
        }
        _ => {}
    }

    match &section {
        // ── Text fields ───────────────────────────────────────────────────────
        StepSection::Name => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].name.clone();
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { buffer: buf });
            }
        }
        StepSection::Url => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].url.clone();
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { buffer: buf });
            }
        }
        StepSection::Body => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].body.clone().unwrap_or_default();
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { buffer: buf });
            }
        }
        StepSection::Foreach => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].foreach.clone().unwrap_or_default();
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { buffer: buf });
            }
        }
        StepSection::WaitMs => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].wait_ms.to_string();
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { buffer: buf });
            }
        }
        StepSection::TransformInput => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].transforms.first()
                    .map(|t| t.input.clone()).unwrap_or_default();
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { buffer: buf });
            }
        }
        StepSection::TransformOutput => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].transforms.first()
                    .map(|t| t.output.clone()).unwrap_or_default();
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { buffer: buf });
            }
        }

        // ── Cycle fields ──────────────────────────────────────────────────────
        StepSection::Method => {
            let idx = {
                let m = &app.campaign.steps[step_idx].method;
                METHODS.iter().position(|&x| x == m.as_str()).unwrap_or(0)
            };
            let new_idx = match key.code {
                KeyCode::Enter | KeyCode::Right => (idx + 1) % METHODS.len(),
                KeyCode::Left => (idx + METHODS.len() - 1) % METHODS.len(),
                _ => return Ok(()),
            };
            app.campaign.steps[step_idx].method = METHODS[new_idx].to_string();
            app.modified = true;
        }
        StepSection::TransformKind => {
            let idx = {
                let k = app.campaign.steps[step_idx].transforms.first()
                    .map(|t| t.kind.as_str()).unwrap_or("template");
                TRANSFORM_KINDS.iter().position(|&x| x == k).unwrap_or(0)
            };
            let new_idx = match key.code {
                KeyCode::Enter | KeyCode::Right => (idx + 1) % TRANSFORM_KINDS.len(),
                KeyCode::Left => (idx + TRANSFORM_KINDS.len() - 1) % TRANSFORM_KINDS.len(),
                _ => return Ok(()),
            };
            ensure_transform(app, step_idx);
            app.campaign.steps[step_idx].transforms[0].kind = TRANSFORM_KINDS[new_idx].to_string();
            app.modified = true;
        }

        // ── Toggle fields ─────────────────────────────────────────────────────
        StepSection::ContinueOnError => {
            if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                let cur = app.campaign.steps[step_idx].continue_on_error.unwrap_or(false);
                app.campaign.steps[step_idx].continue_on_error = Some(!cur);
                app.modified = true;
            }
        }

        // ── List sections ─────────────────────────────────────────────────────
        StepSection::Headers => match key.code {
            KeyCode::Char('a') => {
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::AddPairStage1 { target: PairTarget::Headers, buffer: String::new() });
            }
            KeyCode::Char('d') => {
                let keys = sorted_keys(&app.campaign.steps[step_idx].headers);
                if let Some(k) = keys.last() {
                    let k = k.clone();
                    app.campaign.steps[step_idx].headers.remove(&k);
                    app.modified = true;
                }
            }
            _ => {}
        },
        StepSection::Extract => match key.code {
            KeyCode::Char('a') => {
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::AddPairStage1 { target: PairTarget::Extract, buffer: String::new() });
            }
            KeyCode::Char('d') => {
                let keys = sorted_keys(&app.campaign.steps[step_idx].extract);
                if let Some(k) = keys.last() {
                    let k = k.clone();
                    app.campaign.steps[step_idx].extract.remove(&k);
                    app.modified = true;
                }
            }
            _ => {}
        },
        StepSection::Assertions => match key.code {
            KeyCode::Char('d') => {
                let step = &mut app.campaign.steps[step_idx];
                if !step.assert.is_empty() {
                    step.assert.pop();
                    app.modified = true;
                }
            }
            _ => {}
        },

        // ── Action ───────────────────────────────────────────────────────────
        StepSection::LoadFromCollection => {
            if matches!(key.code, KeyCode::Enter | KeyCode::Char('L')) {
                let mut expanded = std::collections::HashSet::new();
                for ci in 0..app.stored_collections.len() {
                    expanded.insert(format!("c{ci}"));
                }
                app.focus = BuilderFocus::CollectionBrowser {
                    for_step: step_idx,
                    col_cursor: 0,
                    expanded,
                };
            }
        }

        StepSection::When => {}
    }

    Ok(())
}

// ── EditText mode ─────────────────────────────────────────────────────────────

fn handle_edit_text(
    app: &mut BuilderApp,
    key: KeyEvent,
    step_idx: usize,
    section_cursor: usize,
    sub_cursor: usize,
    mut buffer: String,
) -> Result<()> {
    let kind = app.campaign.steps[step_idx].kind.clone();
    let sections = sections_for(&kind);
    let section = sections[section_cursor].clone();

    match key.code {
        KeyCode::Esc => {
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
        }
        KeyCode::Enter => {
            apply_text_edit(app, step_idx, &section, &buffer);
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
        }
        KeyCode::Backspace => {
            buffer.pop();
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { buffer });
        }
        KeyCode::Char(c) => {
            buffer.push(c);
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { buffer });
        }
        _ => {}
    }
    Ok(())
}

fn apply_text_edit(app: &mut BuilderApp, step_idx: usize, section: &StepSection, value: &str) {
    {
        let step = &mut app.campaign.steps[step_idx];
        match section {
            StepSection::Name    => step.name    = value.to_string(),
            StepSection::Url     => step.url     = value.to_string(),
            StepSection::Body    => step.body    = if value.is_empty() { None } else { Some(value.to_string()) },
            StepSection::Foreach => step.foreach = if value.is_empty() { None } else { Some(value.to_string()) },
            StepSection::WaitMs  => step.wait_ms = value.parse().unwrap_or(0),
            StepSection::TransformInput => {
                ensure_transform_step(step);
                step.transforms[0].input = value.to_string();
            }
            StepSection::TransformOutput => {
                ensure_transform_step(step);
                step.transforms[0].output = value.to_string();
            }
            _ => return,
        }
    }
    app.modified = true;
}

// ── AddPair modes ─────────────────────────────────────────────────────────────

fn handle_add_stage1(
    app: &mut BuilderApp,
    key: KeyEvent,
    step_idx: usize,
    section_cursor: usize,
    sub_cursor: usize,
    target: PairTarget,
    mut buffer: String,
) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
        }
        KeyCode::Enter if !buffer.is_empty() => {
            let key_str = buffer.trim().to_string();
            set_focus(app, step_idx, section_cursor, sub_cursor,
                StepEditorMode::AddPairStage2 { target, key: key_str, buffer: String::new() });
        }
        KeyCode::Backspace => {
            buffer.pop();
            set_focus(app, step_idx, section_cursor, sub_cursor,
                StepEditorMode::AddPairStage1 { target, buffer });
        }
        KeyCode::Char(c) => {
            buffer.push(c);
            set_focus(app, step_idx, section_cursor, sub_cursor,
                StepEditorMode::AddPairStage1 { target, buffer });
        }
        _ => {}
    }
    Ok(())
}

fn handle_add_stage2(
    app: &mut BuilderApp,
    key: KeyEvent,
    step_idx: usize,
    section_cursor: usize,
    sub_cursor: usize,
    target: PairTarget,
    pair_key: String,
    mut buffer: String,
) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
        }
        KeyCode::Enter => {
            let step = &mut app.campaign.steps[step_idx];
            match target {
                PairTarget::Headers => { step.headers.insert(pair_key, buffer.trim().to_string()); }
                PairTarget::Extract => { step.extract.insert(pair_key, buffer.trim().to_string()); }
            }
            app.modified = true;
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
        }
        KeyCode::Backspace => {
            buffer.pop();
            set_focus(app, step_idx, section_cursor, sub_cursor,
                StepEditorMode::AddPairStage2 { target, key: pair_key, buffer });
        }
        KeyCode::Char(c) => {
            buffer.push(c);
            set_focus(app, step_idx, section_cursor, sub_cursor,
                StepEditorMode::AddPairStage2 { target, key: pair_key, buffer });
        }
        _ => {}
    }
    Ok(())
}

// ── Value display helper ──────────────────────────────────────────────────────

pub fn current_value(app: &BuilderApp, step_idx: usize, section: &StepSection) -> String {
    let step = &app.campaign.steps[step_idx];
    match section {
        StepSection::Name      => step.name.clone(),
        StepSection::Method    => step.method.clone(),
        StepSection::Url       => step.url.clone(),
        StepSection::Body      => step.body.clone().unwrap_or_default(),
        StepSection::Foreach   => step.foreach.clone().unwrap_or_default(),
        StepSection::WaitMs    => step.wait_ms.to_string(),
        StepSection::ContinueOnError =>
            if step.continue_on_error.unwrap_or(false) { "[x]".into() } else { "[ ]".into() },
        StepSection::Headers   => format!("({} items)", step.headers.len()),
        StepSection::Extract   => format!("({} items)", step.extract.len()),
        StepSection::Assertions=> format!("({} items)", step.assert.len()),
        StepSection::When      => step.when.as_ref().map(|w| {
            if let Some(eq) = &w.eq { format!("{} == \"{}\"", w.var, eq) }
            else if let Some(ne) = &w.ne { format!("{} != \"{}\"", w.var, ne) }
            else { format!("{} exists", w.var) }
        }).unwrap_or_default(),
        StepSection::TransformKind =>
            step.transforms.first().map(|t| t.kind.clone()).unwrap_or_else(|| "template".into()),
        StepSection::TransformInput =>
            step.transforms.first().map(|t| t.input.clone()).unwrap_or_default(),
        StepSection::TransformOutput =>
            step.transforms.first().map(|t| t.output.clone()).unwrap_or_default(),
        StepSection::LoadFromCollection => String::new(),
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn set_focus(app: &mut BuilderApp, step_idx: usize, section_cursor: usize, sub_cursor: usize, mode: StepEditorMode) {
    app.focus = BuilderFocus::StepEditor { step_idx, section_cursor, sub_cursor, mode, desc_active: false };
}

pub fn sorted_keys(map: &std::collections::HashMap<String, String>) -> Vec<String> {
    let mut keys: Vec<String> = map.keys().cloned().collect();
    keys.sort();
    keys
}

fn ensure_transform(app: &mut BuilderApp, step_idx: usize) {
    if app.campaign.steps[step_idx].transforms.is_empty() {
        app.campaign.steps[step_idx].transforms.push(blank_transform());
    }
}

fn ensure_transform_step(step: &mut crate::campaign::Step) {
    if step.transforms.is_empty() {
        step.transforms.push(blank_transform());
    }
}

fn blank_transform() -> crate::campaign::Transform {
    crate::campaign::Transform {
        kind: "template".into(),
        input: String::new(),
        output: String::new(),
        pattern: None,
        group: 1,
        from: None,
        to: None,
        delimiter: None,
        index: 0,
    }
}
