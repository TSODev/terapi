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
        "loop" => vec![
            StepSection::Name,
            StepSection::Description,
            StepSection::LoopMethod,
            StepSection::LoopUrl,
            StepSection::LoopHeaders,
            StepSection::LoopUntilVar,
            StepSection::LoopUntilCond,
            StepSection::LoopAccumulateVar,
            StepSection::LoopAccumulateFrom,
            StepSection::LoopExtract,
            StepSection::LoopContinueOnError,
        ],
        "jq" => vec![
            StepSection::Name,
            StepSection::Description,
            StepSection::JqInput,
            StepSection::JqExpression,
            StepSection::JqOutput,
            StepSection::JqRaw,
            StepSection::When,
        ],
        "set" => vec![
            StepSection::Name,
            StepSection::Description,
            StepSection::SetVars,
            StepSection::When,
        ],
        "poll" => vec![
            StepSection::Name,
            StepSection::Description,
            StepSection::PollMethod,
            StepSection::PollUrl,
            StepSection::PollHeaders,
            StepSection::PollUntilVar,
            StepSection::PollUntilCond,
            StepSection::PollIntervalMs,
            StepSection::PollTimeoutSecs,
            StepSection::PollExtract,
            StepSection::PollContinueOnError,
        ],
        "search" => vec![
            StepSection::Name,
            StepSection::Description,
            StepSection::SearchInput,
            StepSection::SearchPath,
            StepSection::SearchMatch,
            StepSection::SearchOutput,
            StepSection::SearchFirstOnly,
        ],
        "parallel" => vec![
            StepSection::Name,
            StepSection::Description,
            StepSection::ParallelSteps,
            StepSection::ContinueOnError,
        ],
        "notify" => vec![
            StepSection::Name,
            StepSection::Description,
            StepSection::NotifyUrl,
            StepSection::NotifyMethod,
            StepSection::NotifyMessage,
            StepSection::Headers,
            StepSection::When,
            StepSection::ContinueOnError,
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
        StepEditorMode::EditText { buffer, cursor } =>
            handle_edit_text(app, key, step_idx, section_cursor, sub_cursor, buffer, cursor),
        StepEditorMode::AddPairStage1 { target, buffer } =>
            handle_add_stage1(app, key, step_idx, section_cursor, sub_cursor, target, buffer),
        StepEditorMode::AddPairStage2 { target, key: pair_key, buffer, cursor } =>
            handle_add_stage2(app, key, step_idx, section_cursor, sub_cursor, target, pair_key, buffer, cursor),
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
        StepEditorMode::AddParallelStep { cursor } =>
            handle_add_parallel_step(app, key, step_idx, section_cursor, sub_cursor, cursor),
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
            // Within Extract: navigate items with sub_cursor before leaving the section
            if section == StepSection::Extract && sub_cursor > 0 {
                set_focus(app, step_idx, section_cursor, sub_cursor - 1, StepEditorMode::Browse);
                return Ok(());
            }
            if section_cursor == 0 {
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
            // Within Extract: navigate items with sub_cursor before leaving the section
            if section == StepSection::Extract {
                let len = app.campaign.steps[step_idx].extract.len();
                if sub_cursor + 1 < len {
                    set_focus(app, step_idx, section_cursor, sub_cursor + 1, StepEditorMode::Browse);
                    return Ok(());
                }
            }
            let c = (section_cursor + 1).min(n.saturating_sub(1));
            set_focus(app, step_idx, c, 0, StepEditorMode::Browse);
            return Ok(());
        }
        KeyCode::Char('r') if kind != "comment" => {
            app.start_step_preview(step_idx);
            return Ok(());
        }
        KeyCode::Char('L') if matches!(kind.as_str(), "http" | "graphql" | "seed") => {
            let mut expanded = std::collections::HashSet::new();
            for ci in 0..app.stored_collections.len() {
                expanded.insert(format!("c{ci}"));
            }
            app.focus = BuilderFocus::CollectionBrowser {
                for_step: step_idx,
                col_cursor: 0,
                expanded,
            };
            return Ok(());
        }
        _ => {}
    }

    match &section {
        // ── Text fields ───────────────────────────────────────────────────────
        StepSection::Name => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].name.clone();
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
            }
        }
        StepSection::Description => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].description.clone();
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
            }
        }
        StepSection::Url => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].url.clone();
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
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
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
            }
        }
        StepSection::WaitMs => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].wait_ms.to_string();
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
            }
        }
        StepSection::TransformInput => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].transforms.first()
                    .map(|t| t.input.clone()).unwrap_or_default();
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
            }
        }
        StepSection::TransformOutput => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].transforms.first()
                    .map(|t| t.output.clone()).unwrap_or_default();
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
            }
        }
        StepSection::FilePath => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].file_path.clone().unwrap_or_default();
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
            }
        }
        StepSection::FileOutput => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].file_output.clone().unwrap_or_else(|| "FILE_DATA".into());
                set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
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
                if let Some(k) = keys.get(sub_cursor) {
                    let k = k.clone();
                    app.campaign.steps[step_idx].extract.remove(&k);
                    app.modified = true;
                    let new_len = app.campaign.steps[step_idx].extract.len();
                    let new_sub = if new_len == 0 { 0 } else { sub_cursor.min(new_len - 1) };
                    set_focus(app, step_idx, section_cursor, new_sub, StepEditorMode::Browse);
                }
            }
            KeyCode::Enter => {
                let keys = sorted_keys(&app.campaign.steps[step_idx].extract);
                if let Some(k) = keys.get(sub_cursor) {
                    let k = k.clone();
                    let v = app.campaign.steps[step_idx].extract.get(&k).cloned().unwrap_or_default();
                    let cursor = v.chars().count();
                    set_focus(app, step_idx, section_cursor, sub_cursor,
                        StepEditorMode::AddPairStage2 {
                            target: PairTarget::Extract,
                            key: k,
                            buffer: v,
                            cursor,
                        });
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

        // ── Loop sections ─────────────────────────────────────────────────────
        StepSection::LoopUrl => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].url.clone();
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
            }
        }
        StepSection::LoopMethod => {
            // cycle GET/POST/PUT/PATCH/DELETE
            if matches!(key.code, KeyCode::Enter | KeyCode::Left | KeyCode::Right) {
                let step = &mut app.campaign.steps[step_idx];
                let methods = ["GET","POST","PUT","PATCH","DELETE"];
                let idx = methods.iter().position(|&m| m == step.method).unwrap_or(0);
                step.method = methods[if key.code == KeyCode::Left {
                    (idx + methods.len() - 1) % methods.len()
                } else {
                    (idx + 1) % methods.len()
                }].to_string();
                app.modified = true;
            }
        }
        StepSection::LoopHeaders => match key.code {
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
        StepSection::LoopUntilVar => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].until.as_ref()
                    .map(|u| u.var.clone()).unwrap_or_default();
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
            }
        }
        StepSection::LoopUntilCond => {
            // Cycle: exists=false | exists=true | eq="" | ne=""
            if matches!(key.code, KeyCode::Enter | KeyCode::Left | KeyCode::Right) {
                let step = &mut app.campaign.steps[step_idx];
                let until = step.until.get_or_insert(crate::campaign::StepCondition {
                    var: "CURSOR".into(), eq: None, ne: None, exists: Some(false), lt: None, lte: None,
                });
                // Determine current state index
                let idx = if until.exists == Some(false) { 0 }
                    else if until.exists == Some(true) { 1 }
                    else if until.eq.is_some() { 2 }
                    else if until.ne.is_some() { 3 }
                    else if until.lt.is_some() { 4 }
                    else { 0 };
                let next = if key.code == KeyCode::Left { (idx + 4) % 5 } else { (idx + 1) % 5 };
                until.eq = None; until.ne = None; until.exists = None; until.lt = None; until.lte = None;
                match next {
                    0 => until.exists = Some(false),
                    1 => until.exists = Some(true),
                    2 => until.eq     = Some(String::new()),
                    3 => until.ne     = Some(String::new()),
                    _ => until.lt     = Some(0.0),
                }
                app.modified = true;
            }
        }
        StepSection::LoopAccumulateVar => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].accumulate.as_ref()
                    .map(|a| a.var.clone()).unwrap_or_default();
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
            }
        }
        StepSection::LoopAccumulateFrom => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].accumulate.as_ref()
                    .map(|a| a.from.clone()).unwrap_or_default();
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
            }
        }
        StepSection::LoopExtract => match key.code {
            KeyCode::Char('a') => {
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::AddPairStage1 { target: PairTarget::Extract, buffer: String::new() });
            }
            KeyCode::Char('d') => {
                let keys = sorted_keys(&app.campaign.steps[step_idx].extract);
                if let Some(k) = keys.get(sub_cursor) {
                    let k = k.clone();
                    app.campaign.steps[step_idx].extract.remove(&k);
                    app.modified = true;
                }
            }
            KeyCode::Enter => {
                let keys = sorted_keys(&app.campaign.steps[step_idx].extract);
                if let Some(k) = keys.get(sub_cursor) {
                    let k = k.clone();
                    let v = app.campaign.steps[step_idx].extract.get(&k).cloned().unwrap_or_default();
                    let cursor = v.chars().count();
                    set_focus(app, step_idx, section_cursor, sub_cursor,
                        StepEditorMode::AddPairStage2 { target: PairTarget::Extract, key: k, buffer: v, cursor });
                }
            }
            _ => {}
        },
        StepSection::LoopContinueOnError => {
            if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                let cur = app.campaign.steps[step_idx].continue_on_error.unwrap_or(false);
                app.campaign.steps[step_idx].continue_on_error = Some(!cur);
                app.modified = true;
            }
        }

        // ── Search step sections ──────────────────────────────────────────────
        StepSection::SearchInput | StepSection::SearchPath | StepSection::SearchMatch | StepSection::SearchOutput => {
            if key.code == KeyCode::Enter {
                let step = &app.campaign.steps[step_idx];
                let cfg = step.search.as_ref();
                let buf = match section {
                    StepSection::SearchInput  => cfg.map(|c| c.input.clone()).unwrap_or_default(),
                    StepSection::SearchPath   => cfg.map(|c| c.path.clone()).unwrap_or_default(),
                    StepSection::SearchMatch  => cfg.map(|c| c.pattern.clone()).unwrap_or_default(),
                    StepSection::SearchOutput => cfg.map(|c| c.output.clone()).unwrap_or_default(),
                    _ => unreachable!(),
                };
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
            }
        }
        StepSection::SearchFirstOnly => {
            if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                let step = &mut app.campaign.steps[step_idx];
                let cfg = step.search.get_or_insert(crate::campaign::SearchConfig {
                    input: String::new(), path: String::new(),
                    pattern: String::new(), output: "RESULTS".into(), first_only: false,
                });
                cfg.first_only = !cfg.first_only;
                app.modified = true;
            }
        }

        // ── JQ step sections ──────────────────────────────────────────────────
        StepSection::JqInput | StepSection::JqExpression | StepSection::JqOutput => {
            if key.code == KeyCode::Enter {
                let step = &app.campaign.steps[step_idx];
                let buf = match section {
                    StepSection::JqInput      => step.jq_input.clone().unwrap_or_else(|| "{{RESPONSE}}".into()),
                    StepSection::JqExpression => step.jq_expression.clone().unwrap_or_else(|| ".".into()),
                    StepSection::JqOutput     => step.jq_output.clone().unwrap_or_else(|| "JQ_RESULT".into()),
                    _ => unreachable!(),
                };
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
            }
        }
        StepSection::JqRaw => {
            if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                app.campaign.steps[step_idx].jq_raw = !app.campaign.steps[step_idx].jq_raw;
                app.modified = true;
            }
        }

        // ── Set step sections ─────────────────────────────────────────────────
        StepSection::SetVars => match key.code {
            KeyCode::Char('a') => {
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::AddPairStage1 { target: PairTarget::Vars, buffer: String::new() });
            }
            KeyCode::Char('d') => {
                let keys = sorted_keys(&app.campaign.steps[step_idx].vars);
                if let Some(k) = keys.get(sub_cursor) {
                    let k = k.clone();
                    app.campaign.steps[step_idx].vars.remove(&k);
                    app.modified = true;
                }
            }
            KeyCode::Enter => {
                let keys = sorted_keys(&app.campaign.steps[step_idx].vars);
                if let Some(k) = keys.get(sub_cursor) {
                    let k = k.clone();
                    let v = app.campaign.steps[step_idx].vars.get(&k).cloned().unwrap_or_default();
                    let cursor = v.chars().count();
                    set_focus(app, step_idx, section_cursor, sub_cursor,
                        StepEditorMode::AddPairStage2 { target: PairTarget::Vars, key: k, buffer: v, cursor });
                }
            }
            _ => {}
        },

        // ── Poll step sections ────────────────────────────────────────────────
        StepSection::PollUrl | StepSection::PollIntervalMs | StepSection::PollTimeoutSecs | StepSection::PollUntilVar => {
            if key.code == KeyCode::Enter {
                let step = &app.campaign.steps[step_idx];
                let buf = match section {
                    StepSection::PollUrl         => step.url.clone(),
                    StepSection::PollIntervalMs  => step.interval_ms.to_string(),
                    StepSection::PollTimeoutSecs => step.timeout_secs.to_string(),
                    StepSection::PollUntilVar    => step.until.as_ref().map(|u| u.var.clone()).unwrap_or_default(),
                    _ => unreachable!(),
                };
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
            }
        }
        StepSection::PollMethod => {
            if matches!(key.code, KeyCode::Left | KeyCode::Right | KeyCode::Enter) {
                let step = &mut app.campaign.steps[step_idx];
                let idx = METHODS.iter().position(|&m| m == step.method).unwrap_or(0);
                let next = if key.code == KeyCode::Left { (idx + METHODS.len() - 1) % METHODS.len() }
                           else { (idx + 1) % METHODS.len() };
                step.method = METHODS[next].to_string();
                app.modified = true;
            }
        }
        StepSection::PollUntilCond => {
            if matches!(key.code, KeyCode::Enter | KeyCode::Left | KeyCode::Right) {
                let step = &mut app.campaign.steps[step_idx];
                let until = step.until.get_or_insert(crate::campaign::StepCondition {
                    var: "STATUS".into(), eq: Some("done".into()), ne: None, exists: None, lt: None, lte: None,
                });
                let idx = if until.exists == Some(false) { 0 }
                    else if until.exists == Some(true) { 1 }
                    else if until.eq.is_some() { 2 }
                    else if until.ne.is_some() { 3 }
                    else { 2 };
                let next = if key.code == KeyCode::Left { (idx + 3) % 4 } else { (idx + 1) % 4 };
                until.eq = None; until.ne = None; until.exists = None; until.lt = None; until.lte = None;
                match next {
                    0 => until.exists = Some(false),
                    1 => until.exists = Some(true),
                    2 => until.eq     = Some(String::new()),
                    _ => until.ne     = Some(String::new()),
                }
                app.modified = true;
            }
        }
        StepSection::PollHeaders => match key.code {
            KeyCode::Char('a') => {
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::AddPairStage1 { target: PairTarget::Headers, buffer: String::new() });
            }
            KeyCode::Char('d') => {
                let keys = sorted_keys(&app.campaign.steps[step_idx].headers);
                if let Some(k) = keys.get(sub_cursor) {
                    let k = k.clone();
                    app.campaign.steps[step_idx].headers.remove(&k);
                    app.modified = true;
                }
            }
            _ => {}
        },
        StepSection::PollExtract => match key.code {
            KeyCode::Char('a') => {
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::AddPairStage1 { target: PairTarget::Extract, buffer: String::new() });
            }
            KeyCode::Char('d') => {
                let keys = sorted_keys(&app.campaign.steps[step_idx].extract);
                if let Some(k) = keys.get(sub_cursor) {
                    let k = k.clone();
                    app.campaign.steps[step_idx].extract.remove(&k);
                    app.modified = true;
                }
            }
            KeyCode::Enter => {
                let keys = sorted_keys(&app.campaign.steps[step_idx].extract);
                if let Some(k) = keys.get(sub_cursor) {
                    let k = k.clone();
                    let v = app.campaign.steps[step_idx].extract.get(&k).cloned().unwrap_or_default();
                    let cursor = v.chars().count();
                    set_focus(app, step_idx, section_cursor, sub_cursor,
                        StepEditorMode::AddPairStage2 { target: PairTarget::Extract, key: k, buffer: v, cursor });
                }
            }
            _ => {}
        },
        StepSection::PollContinueOnError => {
            if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                let cur = app.campaign.steps[step_idx].continue_on_error.unwrap_or(false);
                app.campaign.steps[step_idx].continue_on_error = Some(!cur);
                app.modified = true;
            }
        }

        // ── Parallel step sections ────────────────────────────────────────────
        StepSection::ParallelSteps => match key.code {
            KeyCode::Char('a') | KeyCode::Enter => {
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::AddParallelStep { cursor: 0 });
            }
            KeyCode::Char('d') => {
                let step = &mut app.campaign.steps[step_idx];
                if !step.parallel_steps.is_empty() {
                    let remove_idx = sub_cursor.min(step.parallel_steps.len() - 1);
                    step.parallel_steps.remove(remove_idx);
                    app.modified = true;
                    let new_len = step.parallel_steps.len();
                    let new_sub = if new_len == 0 { 0 } else { sub_cursor.min(new_len - 1) };
                    set_focus(app, step_idx, section_cursor, new_sub, StepEditorMode::Browse);
                }
            }
            _ => {}
        },

        // ── Notify step sections ──────────────────────────────────────────────
        StepSection::NotifyUrl => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].url.clone();
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
            }
        }
        StepSection::NotifyMethod => {
            if matches!(key.code, KeyCode::Enter | KeyCode::Left | KeyCode::Right) {
                let step = &mut app.campaign.steps[step_idx];
                let methods = ["POST", "GET", "PUT", "PATCH", "DELETE"];
                let idx = methods.iter().position(|&m| m == step.method).unwrap_or(0);
                step.method = methods[if key.code == KeyCode::Left {
                    (idx + methods.len() - 1) % methods.len()
                } else {
                    (idx + 1) % methods.len()
                }].to_string();
                app.modified = true;
            }
        }
        StepSection::NotifyMessage => {
            if key.code == KeyCode::Enter {
                let buf = app.campaign.steps[step_idx].message.clone().unwrap_or_default();
                set_focus(app, step_idx, section_cursor, sub_cursor,
                    StepEditorMode::EditText { cursor: buf.chars().count(), buffer: buf });
            }
        }

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
    mut cursor: usize,
) -> Result<()> {
    let kind = app.campaign.steps[step_idx].kind.clone();
    let sections = sections_for(&kind);
    let section = sections[section_cursor].clone();

    match key.code {
        KeyCode::Esc => {
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
            return Ok(());
        }
        KeyCode::Enter => {
            apply_text_edit(app, step_idx, &section, &buffer);
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
            return Ok(());
        }
        KeyCode::Left => {
            cursor = cursor.saturating_sub(1);
        }
        KeyCode::Right => {
            cursor = (cursor + 1).min(buffer.chars().count());
        }
        KeyCode::Home => {
            cursor = 0;
        }
        KeyCode::End => {
            cursor = buffer.chars().count();
        }
        KeyCode::Backspace => {
            if cursor > 0 {
                let byte_idx = buffer.char_indices().nth(cursor - 1).map(|(i, _)| i).unwrap_or(0);
                buffer.remove(byte_idx);
                cursor -= 1;
            }
        }
        KeyCode::Delete => {
            let len = buffer.chars().count();
            if cursor < len {
                let byte_idx = buffer.char_indices().nth(cursor).map(|(i, _)| i).unwrap_or(0);
                buffer.remove(byte_idx);
            }
        }
        KeyCode::Char(c) => {
            let byte_idx = buffer.char_indices().nth(cursor).map(|(i, _)| i).unwrap_or(buffer.len());
            buffer.insert(byte_idx, c);
            cursor += 1;
        }
        _ => {}
    }
    set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::EditText { buffer, cursor });
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
            // Loop sections
            StepSection::LoopUrl    => step.url         = value.to_string(),
            StepSection::LoopUntilVar => {
                let u = step.until.get_or_insert(crate::campaign::StepCondition {
                    var: String::new(), eq: None, ne: None, exists: Some(false), lt: None, lte: None,
                });
                u.var = value.to_string();
            }
            StepSection::LoopAccumulateVar => {
                let a = step.accumulate.get_or_insert(crate::campaign::AccumulateConfig {
                    var: String::new(), from: String::new(),
                });
                a.var = value.to_string();
            }
            StepSection::LoopAccumulateFrom => {
                let a = step.accumulate.get_or_insert(crate::campaign::AccumulateConfig {
                    var: String::new(), from: String::new(),
                });
                a.from = value.to_string();
            }
            // JQ sections
            StepSection::JqInput      => step.jq_input      = Some(value.to_string()),
            StepSection::JqExpression => step.jq_expression = Some(value.to_string()),
            StepSection::JqOutput     => step.jq_output     = Some(value.to_string()),
            // Poll sections
            StepSection::PollUrl         => step.url          = value.to_string(),
            StepSection::PollIntervalMs  => step.interval_ms  = value.parse().unwrap_or(1000),
            StepSection::PollTimeoutSecs => step.timeout_secs = value.parse().unwrap_or(60),
            StepSection::PollUntilVar => {
                let u = step.until.get_or_insert(crate::campaign::StepCondition {
                    var: String::new(), eq: Some("done".into()), ne: None, exists: None, lt: None, lte: None,
                });
                u.var = value.to_string();
            }
            // Notify sections
            StepSection::NotifyUrl     => step.url     = value.to_string(),
            StepSection::NotifyMessage => step.message = if value.is_empty() { None } else { Some(value.to_string()) },
            // Search sections
            StepSection::SearchInput | StepSection::SearchPath | StepSection::SearchMatch | StepSection::SearchOutput => {
                let cfg = step.search.get_or_insert(crate::campaign::SearchConfig {
                    input: String::new(), path: String::new(),
                    pattern: String::new(), output: "RESULTS".into(), first_only: false,
                });
                match section {
                    StepSection::SearchInput  => cfg.input   = value.to_string(),
                    StepSection::SearchPath   => cfg.path    = value.to_string(),
                    StepSection::SearchMatch  => cfg.pattern = value.to_string(),
                    StepSection::SearchOutput => cfg.output  = value.to_string(),
                    _ => {}
                }
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
                StepEditorMode::AddPairStage2 { target, key: key_str, buffer: String::new(), cursor: 0 });
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
    mut cursor: usize,
) -> Result<()> {
    // Tab on Extract value opens the JSON path picker (only when preview result exists)
    if key.code == KeyCode::Tab {
        if target == PairTarget::Extract {
            if let Some(ref result) = app.step_preview_result {
                if let Some(ref body) = result.body_json {
                    let paths = collect_json_paths(body);
                    let filter = buffer.clone();
                    set_focus(app, step_idx, section_cursor, sub_cursor,
                        StepEditorMode::ExtractPicker { key: pair_key, paths, filter, cursor: 0 });
                    return Ok(());
                }
            }
        }
        return Ok(());
    }

    match key.code {
        KeyCode::Esc => {
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
            return Ok(());
        }
        KeyCode::Enter => {
            let step = &mut app.campaign.steps[step_idx];
            match target {
                PairTarget::Headers          => { step.headers.insert(pair_key, buffer.trim().to_string()); }
                PairTarget::Extract          => { step.extract.insert(pair_key, buffer.trim().to_string()); }
                PairTarget::GraphqlVariables => { step.graphql_variables.insert(pair_key, buffer.trim().to_string()); }
                PairTarget::Vars             => { step.vars.insert(pair_key, buffer.trim().to_string()); }
                PairTarget::ParallelSteps    => unreachable!("ParallelSteps is single-stage, never reaches stage 2"),
            }
            app.modified = true;
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
            return Ok(());
        }
        KeyCode::Left  => { cursor = cursor.saturating_sub(1); }
        KeyCode::Right => { cursor = (cursor + 1).min(buffer.chars().count()); }
        KeyCode::Home  => { cursor = 0; }
        KeyCode::End   => { cursor = buffer.chars().count(); }
        KeyCode::Backspace => {
            if cursor > 0 {
                let byte_idx = buffer.char_indices().nth(cursor - 1).map(|(i, _)| i).unwrap_or(0);
                buffer.remove(byte_idx);
                cursor -= 1;
            }
        }
        KeyCode::Delete => {
            if cursor < buffer.chars().count() {
                let byte_idx = buffer.char_indices().nth(cursor).map(|(i, _)| i).unwrap_or(0);
                buffer.remove(byte_idx);
            }
        }
        KeyCode::Char(c) => {
            let byte_idx = buffer.char_indices().nth(cursor).map(|(i, _)| i).unwrap_or(buffer.len());
            buffer.insert(byte_idx, c);
            cursor += 1;
        }
        _ => {}
    }
    set_focus(app, step_idx, section_cursor, sub_cursor,
        StepEditorMode::AddPairStage2 { target, key: pair_key, buffer, cursor });
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
            let len = filter.chars().count();
            set_focus(app, step_idx, section_cursor, sub_cursor,
                StepEditorMode::AddPairStage2 {
                    target: PairTarget::Extract,
                    key: pair_key,
                    cursor: len,
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
            set_focus(app, step_idx, section_cursor, sub_cursor,
                StepEditorMode::ExtractPicker { key: pair_key, paths, filter, cursor: 0 });
        }
        KeyCode::Char(c) => {
            filter.push(c);
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
        eq: None, ne: None, exists: None, lt: None, lte: None,
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
        // Loop sections
        StepSection::LoopUrl            => step.url.clone(),
        StepSection::LoopMethod         => step.method.clone(),
        StepSection::LoopHeaders        => format!("({} items)", step.headers.len()),
        StepSection::LoopExtract        => format!("({} items)", step.extract.len()),
        StepSection::LoopContinueOnError =>
            if step.continue_on_error.unwrap_or(false) { "[x]".into() } else { "[ ]".into() },
        StepSection::LoopUntilVar       => step.until.as_ref().map(|u| u.var.clone()).unwrap_or_default(),
        StepSection::LoopUntilCond      => step.until.as_ref().map(|u| {
            if u.exists == Some(false) { "not exists".into() }
            else if u.exists == Some(true) { "exists".into() }
            else if let Some(eq) = &u.eq { format!("== \"{}\"", eq) }
            else if let Some(ne) = &u.ne { format!("!= \"{}\"", ne) }
            else if let Some(lt) = &u.lt { format!("< {}", lt) }
            else { "not exists".into() }
        }).unwrap_or_else(|| "not exists".into()),
        StepSection::LoopAccumulateVar  => step.accumulate.as_ref().map(|a| a.var.clone()).unwrap_or_default(),
        StepSection::LoopAccumulateFrom => step.accumulate.as_ref().map(|a| a.from.clone()).unwrap_or_default(),
        // Search sections
        StepSection::SearchInput    => step.search.as_ref().map(|c| c.input.clone()).unwrap_or_default(),
        StepSection::SearchPath     => step.search.as_ref().map(|c| c.path.clone()).unwrap_or_else(|| String::new()),
        StepSection::SearchMatch    => step.search.as_ref().map(|c| c.pattern.clone()).unwrap_or_default(),
        StepSection::SearchOutput   => step.search.as_ref().map(|c| c.output.clone()).unwrap_or_else(|| "RESULTS".into()),
        StepSection::SearchFirstOnly => if step.search.as_ref().map_or(false, |c| c.first_only) { "[x] first match only".into() } else { "[ ] all matches (array)".into() },
        // JQ sections
        StepSection::JqInput      => step.jq_input.clone().unwrap_or_else(|| "{{RESPONSE}}".into()),
        StepSection::JqExpression => step.jq_expression.clone().unwrap_or_else(|| ".".into()),
        StepSection::JqOutput     => step.jq_output.clone().unwrap_or_else(|| "JQ_RESULT".into()),
        StepSection::JqRaw        => if step.jq_raw { "[x] raw string (-r)".into() } else { "[ ] compact JSON".into() },
        // Set sections
        StepSection::SetVars => format!("({} vars)", step.vars.len()),
        // Poll sections
        StepSection::PollUrl            => step.url.clone(),
        StepSection::PollMethod         => step.method.clone(),
        StepSection::PollHeaders        => format!("({} items)", step.headers.len()),
        StepSection::PollExtract        => format!("({} items)", step.extract.len()),
        StepSection::PollUntilVar       => step.until.as_ref().map(|u| u.var.clone()).unwrap_or_default(),
        StepSection::PollUntilCond      => step.until.as_ref().map(|u| {
            if u.exists == Some(false) { "not exists".into() }
            else if u.exists == Some(true) { "exists".into() }
            else if let Some(eq) = &u.eq { format!("== \"{}\"", eq) }
            else if let Some(ne) = &u.ne { format!("!= \"{}\"", ne) }
            else { "not exists".into() }
        }).unwrap_or_else(|| "== \"done\"".into()),
        StepSection::PollIntervalMs     => step.interval_ms.to_string(),
        StepSection::PollTimeoutSecs    => step.timeout_secs.to_string(),
        StepSection::PollContinueOnError =>
            if step.continue_on_error.unwrap_or(false) { "[x]".into() } else { "[ ]".into() },
        // Parallel sections
        StepSection::ParallelSteps => format!("({} steps)", step.parallel_steps.len()),
        // Notify sections
        StepSection::NotifyUrl     => step.url.clone(),
        StepSection::NotifyMethod  => if step.method.is_empty() { "POST".into() } else { step.method.clone() },
        StepSection::NotifyMessage => step.message.clone().unwrap_or_default(),
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

// ── Parallel step picker ───────────────────────────────────────────────────────

fn handle_add_parallel_step(
    app: &mut BuilderApp,
    key: KeyEvent,
    step_idx: usize,
    section_cursor: usize,
    sub_cursor: usize,
    mut cursor: usize,
) -> Result<()> {
    // Build list of candidate step names (all steps except self and already-added)
    let already: std::collections::HashSet<String> = app.campaign.steps[step_idx]
        .parallel_steps.iter().cloned().collect();
    let candidates: Vec<String> = app.campaign.steps.iter().enumerate()
        .filter(|(i, s)| *i != step_idx
            && matches!(s.kind.as_str(), "http" | "graphql" | "seed" | "poll" | "loop")
            && !already.contains(&s.name))
        .map(|(_, s)| s.name.clone())
        .collect();

    match key.code {
        KeyCode::Esc => {
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
        }
        KeyCode::Up => {
            cursor = cursor.saturating_sub(1);
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::AddParallelStep { cursor });
        }
        KeyCode::Down => {
            if !candidates.is_empty() { cursor = (cursor + 1).min(candidates.len() - 1); }
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::AddParallelStep { cursor });
        }
        KeyCode::Enter if !candidates.is_empty() => {
            let name = candidates[cursor.min(candidates.len() - 1)].clone();
            app.campaign.steps[step_idx].parallel_steps.push(name);
            app.modified = true;
            set_focus(app, step_idx, section_cursor, sub_cursor, StepEditorMode::Browse);
        }
        _ => {}
    }
    Ok(())
}
