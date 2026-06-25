use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use serde_json::Value;

use super::BuilderApp;
use super::types::{ASSERT_OPS, WHEN_OPS, BuilderFocus, PairTarget, StepEditorMode, StepSection};

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
            StepSection::Description,
            StepSection::WaitMs,
        ],
        "transform" => vec![
            StepSection::Name,
            StepSection::Description,
            StepSection::TransformKind,
            StepSection::TransformInput,
            StepSection::TransformOutput,
        ],
        "file" => vec![
            StepSection::Name,
            StepSection::Description,
            StepSection::FilePath,
            StepSection::FileOutput,
            StepSection::FileEncoding,
        ],
        "graphql" => vec![
            StepSection::Name,
            StepSection::Description,
            StepSection::Url,
            StepSection::Headers,
            StepSection::GraphqlQuery,
            StepSection::GraphqlVariables,
            StepSection::Extract,
            StepSection::Assertions,
            StepSection::Foreach,
            StepSection::When,
            StepSection::ContinueOnError,
            StepSection::LoadFromCollection,
        ],
        _ => vec![
            StepSection::Name,
            StepSection::Description,
            StepSection::Method,
            StepSection::Url,
            StepSection::Headers,
            StepSection::Body,
            StepSection::MultipartParts,
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
    // Body textarea captures all keys when in EditBody mode
    if matches!(mode, StepEditorMode::EditBody) {
        if key.code == KeyCode::Esc {
            let text = app.description_textarea.lines().join("\n");
            let step = &mut app.campaign.steps[step_idx];
            step.body = if text.trim().is_empty() { None } else { Some(text) };
            app.modified = true;
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
        } else {
            app.description_textarea.input(tui_textarea::Input::from(key));
            app.focus = BuilderFocus::StepEditor {
                step_idx, section_cursor, sub_cursor,
                mode: StepEditorMode::EditBody,
                desc_active: false,
            };
        }
        return Ok(());
    }

    // GraphQL query textarea captures all keys when in EditGraphqlQuery mode
    if matches!(mode, StepEditorMode::EditGraphqlQuery) {
        if key.code == KeyCode::Esc {
            let text = app.description_textarea.lines().join("\n");
            let step = &mut app.campaign.steps[step_idx];
            step.graphql_query = if text.trim().is_empty() { None } else { Some(text) };
            app.modified = true;
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
        } else {
            app.description_textarea.input(tui_textarea::Input::from(key));
            app.focus = BuilderFocus::StepEditor {
                step_idx, section_cursor, sub_cursor,
                mode: StepEditorMode::EditGraphqlQuery,
                desc_active: false,
            };
        }
        return Ok(());
    }

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
        StepEditorMode::AddAssertPath { buffer } =>
            handle_assert_path(app, key, step_idx, section_cursor, sub_cursor, buffer),
        StepEditorMode::AddAssertOp { path, op } =>
            handle_assert_op(app, key, step_idx, section_cursor, sub_cursor, path, op),
        StepEditorMode::AddAssertValue { path, op, buffer } =>
            handle_assert_value(app, key, step_idx, section_cursor, sub_cursor, path, op, buffer),
        StepEditorMode::EditWhenVar { buffer } =>
            handle_when_var(app, key, step_idx, section_cursor, sub_cursor, buffer),
        StepEditorMode::EditWhenOp { var, op } =>
            handle_when_op(app, key, step_idx, section_cursor, sub_cursor, var, op),
        StepEditorMode::EditWhenValue { var, op, buffer } =>
            handle_when_value(app, key, step_idx, section_cursor, sub_cursor, var, op, buffer),
        StepEditorMode::AddMultipart { idx, name, value, content_type, stage } =>
            handle_add_multipart(app, key, step_idx, section_cursor, sub_cursor, idx, name, value, content_type, stage),
        StepEditorMode::ExtractPicker { key: pair_key, paths, filter, cursor } =>
            handle_extract_picker(app, key, step_idx, section_cursor, sub_cursor, pair_key, paths, filter, cursor),
        StepEditorMode::EditBody => Ok(()), // handled at the top of handle_key
        StepEditorMode::EditGraphqlQuery => Ok(()), // handled at the top of handle_key
    }
}

fn save_description(app: &mut BuilderApp, step_idx: usize) {
    let text = app.description_textarea.lines().join("\n");
    let current = app.step_comments.get(step_idx).map(|s| s.as_str()).unwrap_or("");
    if current != text {
        while app.step_comments.len() <= step_idx {
            app.step_comments.push(String::new());
        }
        app.step_comments[step_idx] = text;
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
                // Enter comments textarea
                let comment = app.step_comments.get(step_idx).cloned().unwrap_or_default();
                let lines: Vec<String> = comment.lines().map(String::from).collect();
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
        KeyCode::Char('r') if kind != "comment" => {
            app.start_step_preview(step_idx);
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
        StepSection::Description => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].description.clone();
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
                let body = app.campaign.steps[step_idx].body.clone().unwrap_or_default();
                let lines: Vec<String> = body.lines().map(String::from).collect();
                app.description_textarea = if lines.is_empty() {
                    tui_textarea::TextArea::default()
                } else {
                    tui_textarea::TextArea::from(lines)
                };
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditBody);
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
        StepSection::FilePath => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].file_path.clone().unwrap_or_default();
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { buffer: buf });
            }
        }
        StepSection::FileOutput => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].file_output.clone().unwrap_or_else(|| "FILE_DATA".into());
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { buffer: buf });
            }
        }
        StepSection::FileEncoding => {
            const FILE_ENCODINGS: &[&str] = &["base64", "text", "hex"];
            let cur = app.campaign.steps[step_idx].file_encoding.clone().unwrap_or_else(|| "base64".into());
            let idx = FILE_ENCODINGS.iter().position(|&x| x == cur.as_str()).unwrap_or(0);
            let new_idx = match key.code {
                KeyCode::Enter | KeyCode::Right => (idx + 1) % FILE_ENCODINGS.len(),
                KeyCode::Left => (idx + FILE_ENCODINGS.len() - 1) % FILE_ENCODINGS.len(),
                _ => return Ok(()),
            };
            app.campaign.steps[step_idx].file_encoding = Some(FILE_ENCODINGS[new_idx].to_string());
            app.modified = true;
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
            KeyCode::Char('a') => {
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::AddAssertPath { buffer: String::new() });
            }
            KeyCode::Char('d') => {
                let step = &mut app.campaign.steps[step_idx];
                if !step.assert.is_empty() {
                    step.assert.pop();
                    app.modified = true;
                }
            }
            _ => {}
        },
        StepSection::MultipartParts => match key.code {
            KeyCode::Char('a') => {
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::AddMultipart {
                        idx: None,
                        name: String::new(), value: String::new(),
                        content_type: String::new(), stage: 0,
                    });
            }
            KeyCode::Enter => {
                // Edit last part (or add if empty)
                let step = &app.campaign.steps[step_idx];
                if step.multipart_parts.is_empty() {
                    set_focus(app, step_idx, section_cursor, sub_cursor,
                        StepEditorMode::AddMultipart {
                            idx: None,
                            name: String::new(), value: String::new(),
                            content_type: String::new(), stage: 0,
                        });
                } else {
                    let last = step.multipart_parts.len() - 1;
                    let p = &step.multipart_parts[last];
                    set_focus(app, step_idx, section_cursor, sub_cursor,
                        StepEditorMode::AddMultipart {
                            idx: Some(last),
                            name: p.name.clone(),
                            value: p.value.clone(),
                            content_type: p.content_type.clone().unwrap_or_default(),
                            stage: 0,
                        });
                }
            }
            KeyCode::Char('d') => {
                let step = &mut app.campaign.steps[step_idx];
                if !step.multipart_parts.is_empty() {
                    step.multipart_parts.pop();
                    app.modified = true;
                }
            }
            _ => {}
        },

        StepSection::GraphqlQuery => {
            if matches!(key.code, KeyCode::Enter | KeyCode::Char('i')) {
                let q = app.campaign.steps[step_idx].graphql_query.clone().unwrap_or_default();
                app.description_textarea = tui_textarea::TextArea::new(
                    q.lines().map(|l| l.to_string()).collect()
                );
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditGraphqlQuery);
            }
        }
        StepSection::GraphqlVariables => match key.code {
            KeyCode::Char('a') => {
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::AddPairStage1 { target: PairTarget::GraphqlVariables, buffer: String::new() });
            }
            KeyCode::Char('d') => {
                let keys = sorted_keys(&app.campaign.steps[step_idx].graphql_variables);
                if let Some(k) = keys.last() {
                    let k = k.clone();
                    app.campaign.steps[step_idx].graphql_variables.remove(&k);
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

        StepSection::When => match key.code {
            KeyCode::Enter => {
                // Pre-fill from existing condition if any
                let buf = app.campaign.steps[step_idx].when.as_ref()
                    .map(|w| w.var.clone())
                    .unwrap_or_default();
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::EditWhenVar { buffer: buf });
            }
            KeyCode::Char('d') => {
                app.campaign.steps[step_idx].when = None;
                app.modified = true;
            }
            _ => {}
        },
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
            StepSection::Name        => step.name        = value.to_string(),
            StepSection::Description => step.description = value.to_string(),
            StepSection::Url         => step.url         = value.to_string(),
            StepSection::Body        => step.body        = if value.is_empty() { None } else { Some(value.to_string()) },
            StepSection::Foreach     => step.foreach     = if value.is_empty() { None } else { Some(value.to_string()) },
            StepSection::WaitMs      => step.wait_ms     = value.parse().unwrap_or(0),
            StepSection::TransformInput => {
                ensure_transform_step(step);
                step.transforms[0].input = value.to_string();
            }
            StepSection::TransformOutput => {
                ensure_transform_step(step);
                step.transforms[0].output = value.to_string();
            }
            StepSection::FilePath   => step.file_path   = if value.is_empty() { None } else { Some(value.to_string()) },
            StepSection::FileOutput => step.file_output = if value.is_empty() { None } else { Some(value.to_string()) },
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
    // Tab on Extract value opens the JSON path picker (only when preview result exists)
    if key.code == KeyCode::Tab {
        if target == PairTarget::Extract {
            if let Some(ref result) = app.step_preview_result {
                if let Some(ref body) = result.body_json {
                    let paths = collect_json_paths(body);
                    let filter = buffer.clone();
                    let cursor = 0;
                    set_focus(app, step_idx, section_cursor, sub_cursor,
                        StepEditorMode::ExtractPicker { key: pair_key, paths, filter, cursor });
                    return Ok(());
                }
            }
        }
        return Ok(());
    }

    match key.code {
        KeyCode::Esc => {
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
        }
        KeyCode::Enter => {
            let step = &mut app.campaign.steps[step_idx];
            match target {
                PairTarget::Headers          => { step.headers.insert(pair_key, buffer.trim().to_string()); }
                PairTarget::Extract          => { step.extract.insert(pair_key, buffer.trim().to_string()); }
                PairTarget::GraphqlVariables => { step.graphql_variables.insert(pair_key, buffer.trim().to_string()); }
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

fn handle_extract_picker(
    app: &mut BuilderApp,
    key: KeyEvent,
    step_idx: usize,
    section_cursor: usize,
    sub_cursor: usize,
    pair_key: String,
    paths: Vec<String>,
    mut filter: String,
    mut cursor: usize,
) -> Result<()> {
    let filtered: Vec<&String> = paths.iter()
        .filter(|p| p.to_lowercase().contains(&filter.to_lowercase()))
        .collect();

    match key.code {
        KeyCode::Esc | KeyCode::Tab => {
            // Return to stage2 with the current filter as buffer
            set_focus(app, step_idx, section_cursor, sub_cursor,
                StepEditorMode::AddPairStage2 {
                    target: PairTarget::Extract,
                    key: pair_key,
                    buffer: filter,
                });
        }
        KeyCode::Enter => {
            // Insert selected path into step.extract and return to Browse
            if let Some(&path) = filtered.get(cursor) {
                let step = &mut app.campaign.steps[step_idx];
                step.extract.insert(pair_key, path.clone());
                app.modified = true;
            }
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
        }
        KeyCode::Up => {
            cursor = cursor.saturating_sub(1);
            set_focus(app, step_idx, section_cursor, sub_cursor,
                StepEditorMode::ExtractPicker { key: pair_key, paths, filter, cursor });
        }
        KeyCode::Down => {
            if !filtered.is_empty() {
                cursor = (cursor + 1).min(filtered.len().saturating_sub(1));
            }
            set_focus(app, step_idx, section_cursor, sub_cursor,
                StepEditorMode::ExtractPicker { key: pair_key, paths, filter, cursor });
        }
        KeyCode::Backspace => {
            filter.pop();
            cursor = 0;
            set_focus(app, step_idx, section_cursor, sub_cursor,
                StepEditorMode::ExtractPicker { key: pair_key, paths, filter, cursor: 0 });
        }
        KeyCode::Char(c) => {
            filter.push(c);
            cursor = 0;
            set_focus(app, step_idx, section_cursor, sub_cursor,
                StepEditorMode::ExtractPicker { key: pair_key, paths, filter, cursor: 0 });
        }
        _ => {}
    }
    Ok(())
}

// ── Assertion add flow ────────────────────────────────────────────────────────

fn handle_assert_path(
    app: &mut BuilderApp,
    key: KeyEvent,
    step_idx: usize,
    section_cursor: usize,
    sub_cursor: usize,
    mut buffer: String,
) -> Result<()> {
    match key.code {
        KeyCode::Esc => set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse),
        KeyCode::Enter if !buffer.is_empty() => {
            set_focus(app, step_idx, section_cursor, sub_cursor,
                StepEditorMode::AddAssertOp { path: buffer.trim().to_string(), op: 0 });
        }
        KeyCode::Backspace => {
            buffer.pop();
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::AddAssertPath { buffer });
        }
        KeyCode::Char(c) => {
            buffer.push(c);
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::AddAssertPath { buffer });
        }
        _ => {}
    }
    Ok(())
}

fn handle_assert_op(
    app: &mut BuilderApp,
    key: KeyEvent,
    step_idx: usize,
    section_cursor: usize,
    sub_cursor: usize,
    path: String,
    op: usize,
) -> Result<()> {
    match key.code {
        KeyCode::Esc => set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse),
        KeyCode::Left => {
            let new_op = if op == 0 { ASSERT_OPS.len() - 1 } else { op - 1 };
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::AddAssertOp { path, op: new_op });
        }
        KeyCode::Right => {
            let new_op = (op + 1) % ASSERT_OPS.len();
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::AddAssertOp { path, op: new_op });
        }
        KeyCode::Enter => {
            if ASSERT_OPS[op].1 {
                // needs a value
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::AddAssertValue { path, op, buffer: String::new() });
            } else {
                // no value needed (exists / not exists)
                let a = build_assertion(&path, op, "");
                app.campaign.steps[step_idx].assert.push(a);
                app.modified = true;
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_assert_value(
    app: &mut BuilderApp,
    key: KeyEvent,
    step_idx: usize,
    section_cursor: usize,
    sub_cursor: usize,
    path: String,
    op: usize,
    mut buffer: String,
) -> Result<()> {
    match key.code {
        KeyCode::Esc => set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse),
        KeyCode::Enter => {
            let a = build_assertion(&path, op, buffer.trim());
            app.campaign.steps[step_idx].assert.push(a);
            app.modified = true;
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
        }
        KeyCode::Backspace => {
            buffer.pop();
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::AddAssertValue { path, op, buffer });
        }
        KeyCode::Char(c) => {
            buffer.push(c);
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::AddAssertValue { path, op, buffer });
        }
        _ => {}
    }
    Ok(())
}

fn build_assertion(path: &str, op_idx: usize, value: &str) -> crate::campaign::Assertion {
    let mut a = crate::campaign::Assertion {
        on: path.to_string(),
        eq: None, ne: None,
        lt: None, lte: None, gt: None, gte: None,
        in_: vec![],
        exists: None, contains: None, matches: None,
    };
    let json_val = || {
        value.parse::<i64>()
            .map(|n| serde_json::Value::Number(n.into()))
            .unwrap_or_else(|_| serde_json::Value::String(value.to_string()))
    };
    match op_idx {
        0 => a.eq       = Some(json_val()),
        1 => a.ne       = Some(json_val()),
        2 => a.lt       = value.parse().ok(),
        3 => a.lte      = value.parse().ok(),
        4 => a.gt       = value.parse().ok(),
        5 => a.gte      = value.parse().ok(),
        6 => a.contains = Some(value.to_string()),
        7 => a.matches  = Some(value.to_string()),
        8 => a.exists   = Some(true),
        9 => a.exists   = Some(false),
        _ => {}
    }
    a
}

// ── When condition edit flow ──────────────────────────────────────────────────

fn handle_when_var(
    app: &mut BuilderApp,
    key: KeyEvent,
    step_idx: usize,
    section_cursor: usize,
    sub_cursor: usize,
    mut buffer: String,
) -> Result<()> {
    match key.code {
        KeyCode::Esc => set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse),
        KeyCode::Enter if !buffer.is_empty() => {
            // Find initial op from existing condition
            let init_op = app.campaign.steps[step_idx].when.as_ref()
                .map(|w| {
                    if w.eq.is_some() { 0 }
                    else if w.ne.is_some() { 1 }
                    else if w.exists == Some(true) { 2 }
                    else { 3 }
                })
                .unwrap_or(0);
            set_focus(app, step_idx, section_cursor, sub_cursor,
                StepEditorMode::EditWhenOp { var: buffer.trim().to_string(), op: init_op });
        }
        KeyCode::Backspace => {
            buffer.pop();
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditWhenVar { buffer });
        }
        KeyCode::Char(c) => {
            buffer.push(c);
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditWhenVar { buffer });
        }
        _ => {}
    }
    Ok(())
}

fn handle_when_op(
    app: &mut BuilderApp,
    key: KeyEvent,
    step_idx: usize,
    section_cursor: usize,
    sub_cursor: usize,
    var: String,
    op: usize,
) -> Result<()> {
    match key.code {
        KeyCode::Esc => set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse),
        KeyCode::Left => {
            let new_op = if op == 0 { WHEN_OPS.len() - 1 } else { op - 1 };
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditWhenOp { var, op: new_op });
        }
        KeyCode::Right => {
            let new_op = (op + 1) % WHEN_OPS.len();
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditWhenOp { var, op: new_op });
        }
        KeyCode::Enter => {
            if WHEN_OPS[op].1 {
                let init_val = app.campaign.steps[step_idx].when.as_ref()
                    .and_then(|w| w.eq.as_ref().or(w.ne.as_ref()).cloned())
                    .unwrap_or_default();
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::EditWhenValue { var, op, buffer: init_val });
            } else {
                app.campaign.steps[step_idx].when = Some(build_when(&var, op, ""));
                app.modified = true;
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_when_value(
    app: &mut BuilderApp,
    key: KeyEvent,
    step_idx: usize,
    section_cursor: usize,
    sub_cursor: usize,
    var: String,
    op: usize,
    mut buffer: String,
) -> Result<()> {
    match key.code {
        KeyCode::Esc => set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse),
        KeyCode::Enter => {
            app.campaign.steps[step_idx].when = Some(build_when(&var, op, buffer.trim()));
            app.modified = true;
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
        }
        KeyCode::Backspace => {
            buffer.pop();
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditWhenValue { var, op, buffer });
        }
        KeyCode::Char(c) => {
            buffer.push(c);
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditWhenValue { var, op, buffer });
        }
        _ => {}
    }
    Ok(())
}

fn build_when(var: &str, op_idx: usize, value: &str) -> crate::campaign::StepCondition {
    let mut w = crate::campaign::StepCondition {
        var: var.to_string(),
        eq: None, ne: None, exists: None,
    };
    match op_idx {
        0 => w.eq     = Some(value.to_string()),
        1 => w.ne     = Some(value.to_string()),
        2 => w.exists = Some(true),
        3 => w.exists = Some(false),
        _ => {}
    }
    w
}

// ── Value display helper ──────────────────────────────────────────────────────

pub fn current_value(app: &BuilderApp, step_idx: usize, section: &StepSection) -> String {
    let step = &app.campaign.steps[step_idx];
    match section {
        StepSection::Name        => step.name.clone(),
        StepSection::Description => step.description.clone(),
        StepSection::Method      => step.method.clone(),
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
        StepSection::FilePath    => step.file_path.clone().unwrap_or_default(),
        StepSection::FileOutput  => step.file_output.clone().unwrap_or_else(|| "FILE_DATA".into()),
        StepSection::FileEncoding=> step.file_encoding.clone().unwrap_or_else(|| "base64".into()),
        StepSection::MultipartParts => format!("({} parts)", step.multipart_parts.len()),
        StepSection::GraphqlQuery     => step.graphql_query.clone().unwrap_or_default(),
        StepSection::GraphqlVariables => format!("({} vars)", step.graphql_variables.len()),
        StepSection::LoadFromCollection => String::new(),
    }
}

// ── AddMultipart flow ─────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn handle_add_multipart(
    app: &mut BuilderApp,
    key: KeyEvent,
    step_idx: usize,
    section_cursor: usize,
    sub_cursor: usize,
    idx: Option<usize>,
    mut name: String,
    mut value: String,
    mut content_type: String,
    stage: u8,
) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
        }
        KeyCode::Enter | KeyCode::Tab => {
            match stage {
                0 => set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::AddMultipart { idx, name, value, content_type, stage: 1 }),
                1 => set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::AddMultipart { idx, name, value, content_type, stage: 2 }),
                _ => {
                    // stage 2 → save
                    let ct = if content_type.trim().is_empty() { None } else { Some(content_type.trim().to_string()) };
                    let part = crate::campaign::MultipartPart { name, value, content_type: ct };
                    let step = &mut app.campaign.steps[step_idx];
                    match idx {
                        Some(i) if i < step.multipart_parts.len() => step.multipart_parts[i] = part,
                        _ => step.multipart_parts.push(part),
                    }
                    app.modified = true;
                    set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
                }
            }
        }
        KeyCode::Backspace => {
            match stage {
                0 => { name.pop(); }
                1 => { value.pop(); }
                _ => { content_type.pop(); }
            }
            set_focus(app, step_idx, section_cursor, sub_cursor,
                StepEditorMode::AddMultipart { idx, name, value, content_type, stage });
        }
        KeyCode::Char(c) => {
            match stage {
                0 => name.push(c),
                1 => value.push(c),
                _ => content_type.push(c),
            }
            set_focus(app, step_idx, section_cursor, sub_cursor,
                StepEditorMode::AddMultipart { idx, name, value, content_type, stage });
        }
        _ => {}
    }
    Ok(())
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

// ── JSON dot-path extraction ──────────────────────────────────────────────────

pub fn collect_json_paths(value: &Value) -> Vec<String> {
    let mut paths = Vec::new();
    collect_paths_recursive(value, String::new(), &mut paths);
    paths
}

fn collect_paths_recursive(value: &Value, prefix: String, paths: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let path = if prefix.is_empty() { k.clone() } else { format!("{}.{}", prefix, k) };
                paths.push(path.clone());
                collect_paths_recursive(v, path, paths);
            }
        }
        Value::Array(arr) => {
            // Wildcard path for the whole array (useful for extraction)
            if !prefix.is_empty() {
                let wc = format!("{}.*", prefix);
                // Add wildcard sub-field paths from first element
                if let Some(first) = arr.first() {
                    if let Value::Object(map) = first {
                        for k in map.keys() {
                            paths.push(format!("{}.{}", wc, k));
                        }
                    }
                }
                paths.push(wc);
            }
            // Individual index paths (first 10 elements)
            for (i, v) in arr.iter().enumerate().take(10) {
                let path = format!("{}.{}", prefix, i);
                paths.push(path.clone());
                collect_paths_recursive(v, path, paths);
            }
        }
        _ => {}
    }
}
