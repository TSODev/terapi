use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use crate::campaign::{CampaignRunState, StepResult};

use super::BuilderApp;
use super::step_editor::{current_value, sections_for, sorted_keys};
use super::types::{ASSERT_OPS, BRICK_KINDS, BuilderFocus, CampaignSettingsMode, CheckLevel, IoEditorMode, ParamEditorMode, StepEditorMode, StepSection, VariablesMode, WHEN_OPS};

pub fn render(frame: &mut Frame, app: &BuilderApp) {
    let area = frame.area();

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(area);

    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(outer[0]);

    render_pipeline(frame, app, panels[0]);
    render_context(frame, app, panels[1]);
    render_status(frame, app, outer[1]);

    if app.quit_confirm {
        render_quit_confirm(frame, area);
    }
}

// ── Quit confirmation overlay ─────────────────────────────────────────────────

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

fn render_quit_confirm(frame: &mut Frame, area: Rect) {
    let dialog = centered_rect(54, 6, area);
    frame.render_widget(Clear, dialog);
    let block = Block::default()
        .title(" Unsaved changes ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Save before quitting?",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("[y]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" save & quit    "),
            Span::styled("[n]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(" quit without saving    "),
            Span::styled("[Esc]", Style::default().fg(Color::Indexed(244))),
            Span::raw(" cancel"),
        ]),
    ];
    frame.render_widget(Paragraph::new(text).block(block), dialog);
}

// ── Pipeline ─────────────────────────────────────────────────────────────────

fn render_pipeline(frame: &mut Frame, app: &BuilderApp, area: Rect) {
    let env_label = app.campaign.env_file.as_deref().unwrap_or("—");
    let title = if app.modified {
        format!(" Pipeline · {} [{}] * ", app.campaign.campaign.name, env_label)
    } else {
        format!(" Pipeline · {} [{}] ", app.campaign.campaign.name, env_label)
    };

    let in_pipeline = matches!(app.focus,
        BuilderFocus::Pipeline | BuilderFocus::PipelineConnectors { .. } | BuilderFocus::PipelineOutputs { .. }
    );
    let border_style = if in_pipeline {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::Indexed(244))
    };
    let conn_cursor = if let BuilderFocus::PipelineConnectors { cursor } = app.focus { Some(cursor) } else { None };
    let out_cursor  = if let BuilderFocus::PipelineOutputs  { cursor } = app.focus { Some(cursor) } else { None };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut items: Vec<ListItem> = Vec::new();

    // Header comment block (from top of TOML file)
    if !app.header_comment.is_empty() {
        for line in app.header_comment.lines() {
            items.push(ListItem::new(Line::from(vec![
                Span::styled(
                    format!("# {}", truncate(line, 50)),
                    Style::default().fg(Color::Indexed(240)).add_modifier(Modifier::ITALIC),
                ),
            ])));
        }
        items.push(ListItem::new(Line::from(
            Span::styled("─".repeat(50), Style::default().fg(Color::Indexed(240))),
        )));
    }

    // ── Inputs section ────────────────────────────────────────────────────────
    if !app.campaign.connectors.is_empty() {
        items.push(ListItem::new(Line::from(vec![
            Span::styled("── Inputs ", Style::default().fg(Color::Indexed(240))),
            Span::styled("─".repeat(38), Style::default().fg(Color::Indexed(240))),
        ])));
        for (ci, c) in app.campaign.connectors.iter().enumerate() {
            let selected = conn_cursor == Some(ci);
            let prefix = if selected { "▶ " } else { "  " };
            let kind_color = if c.kind == "json" { Color::Yellow } else { Color::Green };
            let path_or_step = if let Some(ref fs) = c.from_step {
                format!("from:{}", fs)
            } else {
                truncate(&c.path, 34)
            };
            let row_style = if selected { Style::default().fg(Color::White).add_modifier(Modifier::BOLD) } else { Style::default() };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(prefix, row_style),
                Span::styled(format!("[{:<3}] ", c.kind.to_uppercase()), Style::default().fg(kind_color).add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() })),
                Span::styled(path_or_step, if selected { Style::default().fg(Color::White) } else { Style::default().fg(Color::Indexed(250)) }),
                if let Some(ref s) = c.select {
                    Span::styled(format!("  select:{}", s), Style::default().fg(Color::Indexed(246)))
                } else { Span::raw("") },
                if selected { Span::styled("  Enter:edit  d:del", Style::default().fg(Color::Indexed(242))) } else { Span::raw("") },
            ])));
        }
        items.push(ListItem::new(Line::from(
            Span::styled("─".repeat(48), Style::default().fg(Color::Indexed(240))),
        )));
    }

    if app.campaign.steps.is_empty() {
        items.push(ListItem::new(Line::from(
            Span::styled("No steps — n: add from catalog", Style::default().fg(Color::Indexed(246))),
        )));
        frame.render_widget(List::new(items), inner);
        return;
    }
    let in_steps = matches!(app.focus, BuilderFocus::Pipeline);
    let mut step_number = 0usize; // only counts non-comment steps
    for (idx, step) in app.campaign.steps.iter().enumerate() {
        let selected = in_steps && idx == app.cursor;
        let cursor_char = if selected { "▶ " } else { "  " };

        if step.kind == "comment" {
            let style = if selected {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Indexed(242))
            };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{}# {}", cursor_char, truncate(&step.name, 42)),
                    style,
                ),
            ])));
            continue;
        }

        // Show step_comments above the step row if present
        if let Some(comment) = app.step_comments.get(idx) {
            if !comment.is_empty() {
                for line in comment.lines().take(3) {
                    let comment_style = if selected {
                        Style::default().fg(Color::Indexed(250)).add_modifier(Modifier::ITALIC)
                    } else {
                        Style::default().fg(Color::Indexed(240))
                    };
                    items.push(ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("  # {}", truncate(line, 48)),
                            comment_style,
                        ),
                    ])));
                }
            }
        }

        step_number += 1;
        let (badge, badge_color) = step_badge(&step.kind);

        let run_mark = run_marker_for(&app.run_state, idx);

        let num_span = Span::styled(
            format!("{}[{}] ", cursor_char, step_number),
            if selected { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) }
            else         { Style::default().fg(Color::Indexed(246)) },
        );
        let run_span = if let Some((icon, color)) = run_mark {
            Span::styled(format!("{} ", icon), Style::default().fg(color).add_modifier(Modifier::BOLD))
        } else {
            Span::raw("  ")
        };
        let badge_span = Span::styled(
            format!("{badge:<4} "),
            Style::default().fg(badge_color).add_modifier(Modifier::BOLD),
        );
        let method_span = if !step.method.is_empty() && step.kind != "transform" && step.kind != "pause" {
            Span::styled(
                format!("{:<6}", step.method),
                Style::default().fg(Color::Yellow),
            )
        } else {
            Span::raw("      ")
        };
        let summary = step_summary(step);
        let summary_span = Span::styled(
            truncate(&summary, 30),
            if selected { Style::default().fg(Color::White) }
            else         { Style::default().fg(Color::Indexed(250)) },
        );

        items.push(ListItem::new(Line::from(vec![
            num_span, run_span, badge_span, method_span, summary_span,
        ])));

        if let Some(foreach) = &step.foreach {
            items.push(ListItem::new(Line::from(vec![
                Span::raw("         "),
                Span::styled(format!("↻ foreach: {}", foreach), Style::default().fg(Color::Indexed(246))),
            ])));
        }
        if let Some(when) = &step.when {
            let label = when_label(when);
            items.push(ListItem::new(Line::from(vec![
                Span::raw("         "),
                Span::styled(label, Style::default().fg(Color::Indexed(246))),
            ])));
        }
        if !step.assert.is_empty() {
            let n = step.assert.len();
            let preview: Vec<String> = step.assert.iter().take(2)
                .map(|a| format!("? {}", a.on))
                .collect();
            let mut label = preview.join("  ·  ");
            if n > 2 { label.push_str(&format!("  +{}", n - 2)); }
            items.push(ListItem::new(Line::from(vec![
                Span::raw("         "),
                Span::styled(label, Style::default().fg(Color::Indexed(246))),
            ])));
        }
    }

    let coe_flag = if app.campaign.continue_on_error {
        Span::styled(" ↷ continue-on-error", Style::default().fg(Color::Yellow))
    } else {
        Span::raw("")
    };
    items.push(ListItem::new(Line::from(vec![Span::raw(""), coe_flag])));

    // ── Outputs section ───────────────────────────────────────────────────────
    if !app.campaign.outputs.is_empty() {
        items.push(ListItem::new(Line::from(
            Span::styled("─".repeat(48), Style::default().fg(Color::Indexed(240))),
        )));
        items.push(ListItem::new(Line::from(vec![
            Span::styled("── Outputs ", Style::default().fg(Color::Indexed(240))),
            Span::styled("─".repeat(37), Style::default().fg(Color::Indexed(240))),
        ])));
        for (oi, o) in app.campaign.outputs.iter().enumerate() {
            let selected = out_cursor == Some(oi);
            let prefix = if selected { "▶ " } else { "  " };
            let badge_style = Style::default().fg(Color::Magenta).add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() });
            items.push(ListItem::new(Line::from(vec![
                Span::styled(prefix, if selected { Style::default().fg(Color::White).add_modifier(Modifier::BOLD) } else { Style::default() }),
                Span::styled("[OUT] ", badge_style),
                Span::styled(format!("{:<18}", truncate(&o.from_step, 18)), if selected { Style::default().fg(Color::White) } else { Style::default().fg(Color::Indexed(250)) }),
                Span::styled("→ ", Style::default().fg(Color::Indexed(246))),
                Span::styled(truncate(&o.path, 18), Style::default().fg(Color::Yellow)),
                if selected { Span::styled("  Enter:edit  d:del", Style::default().fg(Color::Indexed(242))) } else { Span::raw("") },
            ])));
            if !o.include_vars.is_empty() {
                items.push(ListItem::new(Line::from(Span::styled(
                    format!("         vars: {}", o.include_vars.join(", ")),
                    Style::default().fg(Color::Indexed(246)),
                ))));
            }
        }
    }

    let list = List::new(items);
    frame.render_widget(list, inner);
}

fn step_badge(kind: &str) -> (&'static str, Color) {
    match kind {
        "transform" => ("TRSF", Color::Yellow),
        "pause"     => ("WAIT", Color::Indexed(246)),
        "seed"      => ("SEED", Color::Blue),
        "comment"   => ("#   ", Color::Indexed(242)),
        "file"      => ("FILE", Color::Magenta),
        "graphql"   => ("GQL ", Color::Magenta),
        "loop"      => ("LOOP", Color::Green),
        "poll"      => ("POLL", Color::Yellow),
        "set"       => ("SET ", Color::Blue),
        "jq"        => ("JQ  ", Color::Green),
        "search"    => ("SRCH", Color::Cyan),
        _           => ("HTTP", Color::Cyan),
    }
}

fn step_summary(step: &crate::campaign::Step) -> String {
    match step.kind.as_str() {
        "pause"     => format!("{}ms", step.wait_ms),
        "file"      => {
            let path = step.file_path.as_deref().unwrap_or("");
            let out  = step.file_output.as_deref().unwrap_or("FILE_DATA");
            let enc  = step.file_encoding.as_deref().unwrap_or("base64");
            if path.is_empty() { format!("→ {} ({})", out, enc) } else { format!("{} → {} ({})", path, out, enc) }
        }
        "transform" => {
            if let Some(t) = step.transforms.first() {
                format!("{} → {}", t.kind, t.output)
            } else {
                step.name.clone()
            }
        }
        "graphql" => {
            let url = &step.url;
            if url.is_empty() { "no URL".into() } else { url.clone() }
        }
        "loop" => {
            let acc = step.accumulate.as_ref().map(|a| a.var.clone()).unwrap_or_default();
            let until_var = step.until.as_ref().map(|u| u.var.clone()).unwrap_or_default();
            if acc.is_empty() { format!("{} until {}", step.url, until_var) }
            else { format!("{} → {} until {}", step.url, acc, until_var) }
        }
        "search" => {
            if let Some(ref cfg) = step.search {
                let mode = if cfg.first_only { "first" } else { "all" };
                format!("{{{}}} .{} ~ /{}/  → {} ({})", cfg.input, cfg.path, cfg.pattern, cfg.output, mode)
            } else {
                "search (unconfigured)".into()
            }
        }
        _ => {
            let url = if step.url.len() > 30 { format!("…{}", &step.url[step.url.len().saturating_sub(27)..]) }
                      else { step.url.clone() };
            url
        }
    }
}

fn when_label(when: &crate::campaign::StepCondition) -> String {
    if let Some(eq) = &when.eq {
        format!("⊘ if {} == \"{}\"", when.var, eq)
    } else if let Some(ne) = &when.ne {
        format!("⊘ if {} != \"{}\"", when.var, ne)
    } else if let Some(exists) = when.exists {
        if exists { format!("⊘ if {} exists", when.var) }
        else       { format!("⊘ if {} not set", when.var) }
    } else {
        format!("⊘ if {} non-empty", when.var)
    }
}

// ── Context panel ─────────────────────────────────────────────────────────────

fn render_context(frame: &mut Frame, app: &BuilderApp, area: Rect) {
    match &app.focus {
        BuilderFocus::Pipeline => render_pipeline_hint(frame, app, area),
        BuilderFocus::Catalog { cursor, .. } => render_catalog(frame, *cursor, area),
        BuilderFocus::StepEditor { step_idx, section_cursor, sub_cursor, mode, desc_active } => {
            if app.step_preview_result.is_some() || app.step_preview_running {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
                    .split(area);
                render_step_editor(frame, app, *step_idx, *section_cursor, *sub_cursor, mode, *desc_active, chunks[0]);
                render_step_preview(frame, app, chunks[1]);
            } else {
                render_step_editor(frame, app, *step_idx, *section_cursor, *sub_cursor, mode, *desc_active, area);
            }
        }
        BuilderFocus::CollectionBrowser { col_cursor, expanded, .. } =>
            render_collection_browser(frame, app, *col_cursor, expanded, area),
        BuilderFocus::CampaignSettings { cursor, mode } =>
            render_campaign_settings(frame, app, *cursor, mode, area),
        BuilderFocus::Checker { results } => render_checker(frame, results, area),
        BuilderFocus::TomlPreview { scroll } => render_toml_preview(frame, app, *scroll, area),
        BuilderFocus::Variables { cursor, mode } => render_variables(frame, app, *cursor, mode, area),
        BuilderFocus::Run { scroll } => render_run_view(frame, app, *scroll, area),
        BuilderFocus::ParamsEditor { cursor, mode }     => render_params_editor(frame, app, *cursor, mode, area),
        BuilderFocus::ConnectorsEditor { cursor, mode } => render_connectors_editor(frame, app, *cursor, mode, area),
        BuilderFocus::OutputsEditor { cursor, mode }    => render_outputs_editor(frame, app, *cursor, mode, area),
        BuilderFocus::PipelineConnectors { .. }         => render_pipeline_hint(frame, app, area),
        BuilderFocus::PipelineOutputs { .. }            => render_pipeline_hint(frame, app, area),
        BuilderFocus::OutputStepPicker { step_cursor, f1, f2, f3, .. } => {
            render_output_step_picker(frame, app, *step_cursor, f1, f2, f3, area)
        }
    }
}

fn render_pipeline_hint(frame: &mut Frame, app: &BuilderApp, area: Rect) {
    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Indexed(246)));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let step_count = app.campaign.steps.len();
    let lines = vec![
        Line::from(Span::styled("Keybindings — Pipeline", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        hint_line("n",       "New step (append)"),
        hint_line("i",       "Insert step after cursor"),
        hint_line("Enter",   "Edit selected step"),
        hint_line("d",       "Delete selected step"),
        hint_line("K / J",   "Move step up / down"),
        hint_line("s",       "Campaign settings"),
        hint_line("v",       "Variables [env]"),
        hint_line("c",       "Check pipeline"),
        hint_line("p",       "TOML preview"),
        hint_line("r",       "Run campaign"),
        hint_line("w",       "Save"),
        hint_line("q",       "Quit"),
        Line::from(""),
        Line::from(Span::styled(
            format!("{} step{}", step_count, if step_count != 1 { "s" } else { "" }),
            Style::default().fg(Color::Indexed(246)),
        )),
    ];

    let para = Paragraph::new(lines);
    frame.render_widget(para, inner);
}

fn render_catalog(frame: &mut Frame, cursor: usize, area: Rect) {
    let block = Block::default()
        .title(" Catalog — choose a brick ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = BRICK_KINDS.iter().enumerate().map(|(i, brick)| {
        let selected = i == cursor;
        let prefix = if selected { "▶ " } else { "  " };
        let style = if selected {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Indexed(250))
        };
        let desc_style = Style::default().fg(Color::Indexed(246));
        ListItem::new(Line::from(vec![
            Span::styled(format!("{}{:<14}", prefix, brick.label()), style),
            Span::styled(brick.description(), desc_style),
        ]))
    }).collect();

    let hint = ListItem::new(Line::from(vec![
        Span::styled("  ↑↓: choose  Enter: create  Esc: cancel", Style::default().fg(Color::Indexed(246))),
    ]));

    let mut all = items;
    all.push(ListItem::new(Line::from("")));
    all.push(hint);

    frame.render_widget(List::new(all), inner);
}

fn render_campaign_settings(
    frame: &mut Frame,
    app: &BuilderApp,
    cursor: usize,
    mode: &CampaignSettingsMode,
    area: Rect,
) {
    let block = Block::default()
        .title(" Campaign Settings ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let fields: &[(&str, usize)] = &[
        ("Name",              0),
        ("Description",       1),
        ("Continue on error", 2),
        ("Env",               3),
        ("Params",            4),
    ];

    let mut rows: Vec<ListItem> = Vec::new();
    rows.push(ListItem::new(Line::from("")));

    for &(label, idx) in fields {
        let is_cursor = idx == cursor;
        let cursor_char = if is_cursor { "▶ " } else { "  " };
        let label_style = if is_cursor {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Indexed(246))
        };

        let value_span = if is_cursor {
            match mode {
                CampaignSettingsMode::EditText { buffer } => Span::styled(
                    format!("[ {}_]", buffer),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                CampaignSettingsMode::Browse => settings_value_span(app, idx, is_cursor),
            }
        } else {
            settings_value_span(app, idx, is_cursor)
        };

        rows.push(ListItem::new(Line::from(vec![
            Span::styled(format!("{}{:<18}", cursor_char, label), label_style),
            value_span,
        ])));
    }

    rows.push(ListItem::new(Line::from("")));
    let hints = match mode {
        CampaignSettingsMode::Browse =>
            "↑↓: field  Enter: edit/toggle  ←/→: cycle env  Esc: back",
        CampaignSettingsMode::EditText { .. } =>
            "Type to edit  Enter: confirm  Esc: cancel",
    };
    rows.push(ListItem::new(Line::from(
        Span::styled(hints, Style::default().fg(Color::Indexed(246)))
    )));

    frame.render_widget(List::new(rows), inner);
}

fn settings_value_span(app: &BuilderApp, field_idx: usize, is_cursor: bool) -> Span<'static> {
    let color = if is_cursor { Color::White } else { Color::Indexed(250) };
    match field_idx {
        0 => Span::styled(app.campaign.campaign.name.clone(), Style::default().fg(color)),
        1 => {
            let d = &app.campaign.campaign.description;
            if d.is_empty() {
                Span::styled("—", Style::default().fg(Color::Indexed(246)))
            } else {
                Span::styled(d.clone(), Style::default().fg(color))
            }
        }
        2 => {
            if app.campaign.continue_on_error {
                Span::styled("[x] enabled", Style::default().fg(Color::Green))
            } else {
                Span::styled("[ ] disabled", Style::default().fg(Color::Indexed(246)))
            }
        }
        3 => {
            let env = app.campaign.env_file.as_deref().unwrap_or("— none —");
            Span::styled(format!("[ {} ▾ ]", env), Style::default().fg(Color::Yellow))
        }
        4 => {
            let n = app.campaign.params.len();
            if n == 0 { Span::styled("(none)  Enter: manage", Style::default().fg(Color::Indexed(246))) }
            else       { Span::styled(format!("({})  Enter: manage", n), Style::default().fg(Color::Cyan)) }
        }
        _ => Span::raw(""),
    }
}

fn render_checker(frame: &mut Frame, results: &[super::types::CheckResult], area: Rect) {
    let block = Block::default()
        .title(" Check Report ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines: Vec<Line> = results.iter().map(|r| {
        let (icon, color) = match r.level {
            CheckLevel::Ok      => ("✓ ", Color::Green),
            CheckLevel::Warning => ("⚠  ", Color::Yellow),
            CheckLevel::Error   => ("✗  ", Color::Red),
        };
        Line::from(vec![
            Span::styled(icon, Style::default().fg(color).add_modifier(Modifier::BOLD)),
            Span::styled(r.message.clone(), Style::default().fg(Color::White)),
        ])
    }).collect();

    let mut all = lines;
    all.push(Line::from(""));
    all.push(Line::from(Span::styled("Esc: close", Style::default().fg(Color::Indexed(246)))));

    frame.render_widget(Paragraph::new(all), inner);
}

fn render_toml_preview(frame: &mut Frame, app: &BuilderApp, scroll: usize, area: Rect) {
    let block = Block::default()
        .title(" TOML Preview ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let toml_str = generate_toml_preview(app);
    let all_lines = highlight_toml(&toml_str);
    let lines: Vec<Line> = all_lines.into_iter().skip(scroll).collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_variables(frame: &mut Frame, app: &BuilderApp, cursor: usize, mode: &VariablesMode, area: Rect) {
    let block = Block::default()
        .title(" Variables [env] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut vars: Vec<(&String, &String)> = app.campaign.env.iter().collect();
    vars.sort_by_key(|(k, _)| k.as_str());

    let mut rows: Vec<ListItem> = Vec::new();

    if vars.is_empty() && matches!(mode, VariablesMode::Browse) {
        rows.push(ListItem::new(Line::from(Span::styled("No variables — a: add", Style::default().fg(Color::Indexed(246))))));
    }

    for (i, (k, v)) in vars.iter().enumerate() {
        let selected = matches!(mode, VariablesMode::Browse) && i == cursor;
        let prefix = if selected { "▶ " } else { "  " };
        let key_style = Style::default().fg(Color::Yellow).add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() });
        let val_style = Style::default().fg(if selected { Color::White } else { Color::Indexed(250) });
        rows.push(ListItem::new(Line::from(vec![
            Span::styled(format!("{}{:<20}", prefix, k), key_style),
            Span::styled(v.as_str().to_string(), val_style),
        ])));
    }

    if let VariablesMode::Edit { original_key, key: var_key, value: var_value, field } = mode {
        rows.push(ListItem::new(Line::from("")));
        let title = if original_key.is_some() { "── Edit variable ────────────────────" } else { "── New variable ─────────────────────" };
        rows.push(ListItem::new(Line::from(Span::styled(title, Style::default().fg(Color::Cyan)))));

        for (fi, label) in ["Key", "Value"].iter().enumerate() {
            let fi = fi as u8;
            let is_active = fi == *field;
            let val = if fi == 0 { var_key.as_str() } else { var_value.as_str() };
            let label_style = if is_active { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Indexed(246)) };
            let val_span = if is_active {
                Span::styled(format!("[ {}_]", val), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            } else {
                Span::styled(if val.is_empty() { "—".to_string() } else { val.to_string() }, Style::default().fg(Color::Indexed(250)))
            };
            rows.push(ListItem::new(Line::from(vec![
                Span::styled(format!("  {:<8}", label), label_style),
                val_span,
            ])));
        }
        rows.push(ListItem::new(Line::from("")));
        rows.push(ListItem::new(Line::from(Span::styled(
            "Tab/Enter: next field / save  Esc: cancel",
            Style::default().fg(Color::Indexed(246)),
        ))));
    } else {
        rows.push(ListItem::new(Line::from("")));
        rows.push(ListItem::new(Line::from(
            Span::styled("a: add  d: del  Enter: edit  Esc: close", Style::default().fg(Color::Indexed(246)))
        )));
    }

    frame.render_widget(List::new(rows), inner);
}

fn render_body_editor(frame: &mut Frame, app: &BuilderApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(area);

    let mut ta = app.description_textarea.clone();
    ta.set_block(
        Block::default()
            .title(" Body — multi-line editor ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    ta.set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
    frame.render_widget(&ta, chunks[0]);

    let hints = vec![
        Span::styled("Esc", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(": save & close   "),
        Span::styled("Enter", Style::default().fg(Color::Indexed(246))),
        Span::raw(": new line   "),
        Span::styled("Ctrl+H / Backspace", Style::default().fg(Color::Indexed(246))),
        Span::raw(": delete"),
    ];
    frame.render_widget(
        Paragraph::new(Line::from(hints)).style(Style::default().fg(Color::Indexed(246))),
        chunks[1],
    );
}

fn render_graphql_query_editor(frame: &mut Frame, app: &BuilderApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(area);

    let mut ta = app.description_textarea.clone();
    ta.set_block(
        Block::default()
            .title(" GQL Query — multi-line editor ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta)),
    );
    ta.set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
    frame.render_widget(&ta, chunks[0]);

    let hints = vec![
        Span::styled("Esc", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(": save & close   "),
        Span::styled("Enter", Style::default().fg(Color::Indexed(246))),
        Span::raw(": new line   "),
        Span::styled("Ctrl+H / Backspace", Style::default().fg(Color::Indexed(246))),
        Span::raw(": delete"),
    ];
    frame.render_widget(
        Paragraph::new(Line::from(hints)).style(Style::default().fg(Color::Indexed(246))),
        chunks[1],
    );
}

fn render_step_preview(frame: &mut Frame, app: &BuilderApp, area: Rect) {
    if app.step_preview_running {
        let block = Block::default()
            .title(" Run result ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Indexed(246)));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "⟳ running…",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ))),
            inner,
        );
        return;
    }

    let Some(result) = &app.step_preview_result else { return };

    let (title_icon, border_color) = if result.success { ("✓", Color::Green) } else { ("✗", Color::Red) };
    let status_color = match result.status {
        Some(s) if s < 300 => Color::Green,
        Some(s) if s < 400 => Color::Yellow,
        Some(_)             => Color::Red,
        None                => if result.success { Color::Green } else { Color::Red },
    };

    let block = Block::default()
        .title(format!(" {} Run result ", title_icon))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    // Status + duration + url
    let status_str = result.status
        .map(|s| s.to_string())
        .unwrap_or_else(|| result.method.clone());
    let url_display = if result.url.chars().count() > 35 {
        format!("…{}", result.url.chars().rev().take(34).collect::<String>().chars().rev().collect::<String>())
    } else {
        result.url.clone()
    };
    lines.push(Line::from(vec![
        Span::styled(status_str, Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(format!("{} ms", result.duration_ms), Style::default().fg(Color::Indexed(246))),
        Span::raw("  "),
        Span::styled(url_display, Style::default().fg(Color::Indexed(246))),
    ]));

    // Error
    if let Some(ref err) = result.error {
        lines.push(Line::from(Span::styled(
            format!("⚠ {}", err),
            Style::default().fg(Color::Red),
        )));
    }

    // Assertions
    for (label, passed) in &result.assertion_results {
        let (icon, color) = if *passed { ("✓", Color::Green) } else { ("✗", Color::Red) };
        lines.push(Line::from(vec![
            Span::styled(icon, Style::default().fg(color)),
            Span::raw(format!(" {}", label)),
        ]));
    }

    // Extracted vars
    for (key, val) in &result.extracted {
        let v = if val.chars().count() > 38 {
            format!("{}…", val.chars().take(38).collect::<String>())
        } else {
            val.clone()
        };
        lines.push(Line::from(vec![
            Span::styled("↳ ", Style::default().fg(Color::Cyan)),
            Span::styled(key.clone(), Style::default().fg(Color::Cyan)),
            Span::raw(format!(" = {}", v)),
        ]));
    }

    // Body preview (first 6 lines)
    if let Some(ref body) = result.body_json {
        lines.push(Line::from(""));
        let body_str = serde_json::to_string_pretty(body).unwrap_or_default();
        let total = body_str.lines().count();
        for line in body_str.lines().take(6) {
            lines.push(Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::Indexed(246)),
            )));
        }
        if total > 6 {
            lines.push(Line::from(Span::styled(
                format!("… ({} more lines)", total - 6),
                Style::default().fg(Color::Indexed(242)),
            )));
        }
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn render_step_editor(
    frame: &mut Frame,
    app: &BuilderApp,
    step_idx: usize,
    section_cursor: usize,
    sub_cursor: usize,
    mode: &StepEditorMode,
    desc_active: bool,
    area: Rect,
) {
    let step = &app.campaign.steps[step_idx];
    let (badge, _badge_color) = step_badge(&step.kind);
    let title = if step.kind == "comment" {
        format!(" # Comment — {} ", step.name)
    } else {
        format!(" {} step — {} ", badge, step.name)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Body textarea takes the full inner area
    if matches!(mode, StepEditorMode::EditBody) {
        render_body_editor(frame, app, inner);
        return;
    }

    // GraphQL query textarea takes the full inner area
    if matches!(mode, StepEditorMode::EditGraphqlQuery) {
        render_graphql_query_editor(frame, app, inner);
        return;
    }

    // Split: description textarea (7 lines) + sections list
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)])
        .split(inner);

    render_description_area(frame, app, step_idx, desc_active, chunks[0]);
    let sections_area = chunks[1];

    let sections = sections_for(&step.kind);
    let mut rows: Vec<ListItem> = Vec::new();

    for (i, section) in sections.iter().enumerate() {
        if *section == StepSection::LoadFromCollection {
            rows.push(ListItem::new(Line::from("")));
        }

        let is_cursor = i == section_cursor;
        let label_style = if is_cursor {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Indexed(246))
        };

        let cursor_char = if is_cursor { "▶ " } else { "  " };
        let label = format!("{}{:<17}", cursor_char, section.label());

        let value_span = if is_cursor {
            match mode {
                StepEditorMode::EditText { buffer, cursor } => {
                    let cursor = (*cursor).min(buffer.chars().count());
                    let before: String = buffer.chars().take(cursor).collect();
                    let after: String  = buffer.chars().skip(cursor).collect();
                    Span::styled(
                        format!("[ {}▌{}]", before, after),
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    )
                }
                StepEditorMode::AddPairStage1 { .. } | StepEditorMode::AddPairStage2 { .. }
                    if section.is_list() =>
                    Span::styled(format!("({} items)", list_count(app, step_idx, section)), Style::default().fg(Color::Indexed(246))),
                StepEditorMode::AddAssertPath { .. } | StepEditorMode::AddAssertOp { .. } | StepEditorMode::AddAssertValue { .. }
                    if *section == StepSection::Assertions =>
                    Span::styled(format!("({} items)  +", list_count(app, step_idx, section)), Style::default().fg(Color::Indexed(246))),
                StepEditorMode::AddMultipart { .. }
                    if *section == StepSection::MultipartParts =>
                    Span::styled(format!("({} parts)  +", list_count(app, step_idx, section)), Style::default().fg(Color::Indexed(246))),
                StepEditorMode::EditWhenVar { .. } | StepEditorMode::EditWhenOp { .. } | StepEditorMode::EditWhenValue { .. }
                    if *section == StepSection::When =>
                    Span::styled("editing…", Style::default().fg(Color::Yellow)),
                _ => value_span_for(app, step_idx, section, is_cursor),
            }
        } else {
            value_span_for(app, step_idx, section, is_cursor)
        };

        let hint_span = if is_cursor && matches!(mode, StepEditorMode::Browse) {
            if matches!(section, StepSection::Extract | StepSection::LoopExtract) {
                let has_items = !app.campaign.steps[step_idx].extract.is_empty();
                if has_items {
                    Span::styled("  a: add  d: del  Enter: edit  ↑↓: navigate", Style::default().fg(Color::Indexed(246)))
                } else {
                    Span::styled("  a: add", Style::default().fg(Color::Indexed(246)))
                }
            } else if section.is_list() {
                Span::styled("  a: add  d: del", Style::default().fg(Color::Indexed(246)))
            } else if *section == StepSection::When {
                Span::styled("  Enter: edit  d: clear", Style::default().fg(Color::Indexed(246)))
            } else if *section == StepSection::GraphqlQuery {
                Span::styled("  Enter: edit query", Style::default().fg(Color::Indexed(246)))
            } else {
                Span::raw("")
            }
        } else {
            Span::raw("")
        };

        rows.push(ListItem::new(Line::from(vec![
            Span::styled(label, label_style),
            value_span,
            hint_span,
        ])));

        if section.is_list() {
            let items = list_items_for(app, step_idx, section);
            for (item_idx, item_str) in items.iter().enumerate() {
                let item_active = is_cursor && item_idx == sub_cursor;
                rows.push(ListItem::new(Line::from(vec![
                    Span::raw("     "),
                    Span::styled(
                        format!("{} {}", if item_active { "▶" } else { " " }, item_str),
                        if item_active {
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(if is_cursor { Color::Indexed(250) } else { Color::Indexed(246) })
                        },
                    ),
                ])));
            }
        }

        if is_cursor {
            match mode {
                StepEditorMode::AddPairStage1 { buffer, .. } => {
                    rows.push(sub_row(format!("Key : [ {}_]", buffer), Color::Yellow));
                }
                StepEditorMode::AddPairStage2 { key, buffer, cursor, .. } => {
                    let cur = (*cursor).min(buffer.chars().count());
                    let before: String = buffer.chars().take(cur).collect();
                    let after: String  = buffer.chars().skip(cur).collect();
                    rows.push(sub_row(format!("{} : [ {}▌{}]", key, before, after), Color::Yellow));
                }
                StepEditorMode::AddAssertPath { buffer } if *section == StepSection::Assertions => {
                    rows.push(sub_row(format!("Path : [ {}_]", buffer), Color::Yellow));
                }
                StepEditorMode::AddAssertOp { path, op } if *section == StepSection::Assertions => {
                    rows.push(sub_row(format!(
                        "Path: {}  Op: [ {} ▾ ]  ←/→ cycle  Enter: confirm",
                        path, ASSERT_OPS[*op].0
                    ), Color::Yellow));
                }
                StepEditorMode::AddAssertValue { path, op, buffer } if *section == StepSection::Assertions => {
                    rows.push(sub_row(format!(
                        "Path: {}  {}  Value: [ {}_]",
                        path, ASSERT_OPS[*op].0, buffer
                    ), Color::Yellow));
                }
                StepEditorMode::AddMultipart { name, value, content_type, stage, .. }
                    if *section == StepSection::MultipartParts =>
                {
                    let (label, buf) = match stage {
                        0 => ("Name        ", name),
                        1 => ("Value/@file ", value),
                        _ => ("Content-Type", content_type),
                    };
                    rows.push(sub_row(format!("{}: [ {}_]", label, buf), Color::Yellow));
                    if *stage == 1 {
                        rows.push(sub_row(
                            "  Tip: prefix value with @ for binary file  (e.g. @/path/to/file.png)".into(),
                            Color::Indexed(246),
                        ));
                    }
                }
                StepEditorMode::EditWhenVar { buffer } if *section == StepSection::When => {
                    rows.push(sub_row(format!("Var : [ {}_]", buffer), Color::Yellow));
                }
                StepEditorMode::EditWhenOp { var, op } if *section == StepSection::When => {
                    rows.push(sub_row(format!(
                        "Var: {}  Op: [ {} ▾ ]  ←/→ cycle  Enter: confirm",
                        var, WHEN_OPS[*op].0
                    ), Color::Yellow));
                }
                StepEditorMode::EditWhenValue { var, op, buffer } if *section == StepSection::When => {
                    rows.push(sub_row(format!(
                        "Var: {}  {}  Value: [ {}_]",
                        var, WHEN_OPS[*op].0, buffer
                    ), Color::Yellow));
                }
                _ => {}
            }
        }
    }

    rows.push(ListItem::new(Line::from("")));
    let hints = match mode {
        StepEditorMode::Browse =>
            "↑↓: field  Enter: edit  ←/→: cycle  a/d: list  d: clear when  Esc: back",
        StepEditorMode::EditText { .. } =>
            "Type  Enter: confirm  Esc: cancel",
        StepEditorMode::AddPairStage1 { .. } =>
            "Key name  Enter: next  Esc: cancel",
        StepEditorMode::AddPairStage2 { .. } =>
            "Value  Enter: add  Esc: cancel",
        StepEditorMode::AddAssertPath { .. } =>
            "Path (dot-notation)  Enter: next  Esc: cancel",
        StepEditorMode::AddAssertOp { .. } =>
            "←/→: operator  Enter: confirm  Esc: cancel",
        StepEditorMode::AddAssertValue { .. } =>
            "Value  Enter: add assertion  Esc: cancel",
        StepEditorMode::EditWhenVar { .. } =>
            "Variable name  Enter: next  Esc: cancel",
        StepEditorMode::EditWhenOp { .. } =>
            "←/→: operator  Enter: confirm  Esc: cancel",
        StepEditorMode::EditWhenValue { .. } =>
            "Value  Enter: save condition  Esc: cancel",
        StepEditorMode::AddMultipart { stage, .. } => match stage {
            0 => "Part name  Enter/Tab: next  Esc: cancel",
            1 => "Value or @/path/to/file  Enter/Tab: next  Esc: cancel",
            _ => "Content-Type (optional)  Enter: save  Esc: cancel",
        },
        StepEditorMode::EditBody => "", // full-screen, hints rendered by render_body_editor
        StepEditorMode::EditGraphqlQuery => "", // full-screen, hints rendered by render_graphql_query_editor
        StepEditorMode::ExtractPicker { .. } => "", // overlay rendered below
    };
    rows.push(ListItem::new(Line::from(
        Span::styled(hints, Style::default().fg(Color::Indexed(246)))
    )));

    frame.render_widget(List::new(rows), sections_area);

    // Picker overlay (drawn last so it sits on top)
    if let StepEditorMode::ExtractPicker { paths, filter, cursor, .. } = mode {
        render_extract_picker(frame, paths, filter, *cursor, inner);
    }
}

fn render_extract_picker(
    frame: &mut Frame,
    paths: &[String],
    filter: &str,
    cursor: usize,
    area: Rect,
) {
    let filtered: Vec<&String> = paths.iter()
        .filter(|p| p.to_lowercase().contains(&filter.to_lowercase()))
        .collect();

    let popup_height = (filtered.len().min(12) + 4) as u16;
    let popup = centered_rect(area.width.saturating_sub(4), popup_height, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Extract path picker — Tab/Esc: close  ↑↓: navigate  Enter: insert ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    if inner.height == 0 { return; }

    // Filter input line
    let filter_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("filter: ", Style::default().fg(Color::Indexed(246))),
            Span::styled(filter, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled("█", Style::default().fg(Color::Magenta)),
        ])),
        filter_chunks[0],
    );

    // Path list
    let list_area = filter_chunks[1];
    let items: Vec<ListItem> = filtered.iter().enumerate()
        .take(list_area.height as usize)
        .map(|(i, path)| {
            if i == cursor {
                ListItem::new(Line::from(vec![
                    Span::styled("▶ ", Style::default().fg(Color::Magenta)),
                    Span::styled(path.as_str(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                ]))
            } else {
                ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(path.as_str(), Style::default().fg(Color::Indexed(246))),
                ]))
            }
        })
        .collect();

    if items.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("(no matches)", Style::default().fg(Color::Indexed(242)))),
            list_area,
        );
    } else {
        frame.render_widget(List::new(items), list_area);
    }
}

fn render_description_area(
    frame: &mut Frame,
    app: &BuilderApp,
    step_idx: usize,
    desc_active: bool,
    area: Rect,
) {
    let border_style = if desc_active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Indexed(242))
    };
    let title = if desc_active {
        " Comments — Esc: done "
    } else {
        " Comments — ↑ to edit "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    if desc_active {
        let mut ta = app.description_textarea.clone();
        ta.set_block(block);
        ta.set_style(Style::default().fg(Color::White));
        ta.set_cursor_line_style(Style::default());
        frame.render_widget(&ta, area);
    } else {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let comment = app.step_comments.get(step_idx).map(|s| s.as_str()).unwrap_or("");
        if comment.is_empty() {
            frame.render_widget(
                Paragraph::new("(none — ↑ to add)")
                    .style(Style::default().fg(Color::Indexed(242))),
                inner,
            );
        } else {
            let lines: Vec<Line> = comment
                .lines()
                .take(5)
                .map(|l| Line::from(Span::styled(
                    format!("# {}", l),
                    Style::default().fg(Color::Indexed(246)).add_modifier(Modifier::ITALIC),
                )))
                .collect();
            frame.render_widget(Paragraph::new(lines), inner);
        }
    }
}

fn value_span_for<'a>(app: &'a BuilderApp, step_idx: usize, section: &StepSection, is_cursor: bool) -> Span<'a> {
    let val = current_value(app, step_idx, section);
    let color = if is_cursor { Color::White } else { Color::Indexed(250) };
    match section {
        StepSection::Method => {
            Span::styled(
                format!("[ {} ▾ ]", val),
                Style::default().fg(Color::Yellow).add_modifier(if is_cursor { Modifier::BOLD } else { Modifier::empty() }),
            )
        }
        StepSection::TransformKind | StepSection::FileEncoding => {
            Span::styled(
                format!("[ {} ▾ ]", val),
                Style::default().fg(Color::Yellow).add_modifier(if is_cursor { Modifier::BOLD } else { Modifier::empty() }),
            )
        }
        StepSection::ContinueOnError => {
            let checked = val.contains('x');
            Span::styled(val, Style::default().fg(if checked { Color::Green } else { color }))
        }
        StepSection::LoadFromCollection => {
            Span::styled("Enter / L", Style::default().fg(Color::Cyan))
        }
        _ if val.is_empty() => {
            Span::styled("—", Style::default().fg(Color::Indexed(246)))
        }
        _ => {
            Span::styled(truncate(&val, 38), Style::default().fg(color))
        }
    }
}

fn list_count(app: &BuilderApp, step_idx: usize, section: &StepSection) -> usize {
    let step = &app.campaign.steps[step_idx];
    match section {
        StepSection::Headers          => step.headers.len(),
        StepSection::Extract          => step.extract.len(),
        StepSection::Assertions       => step.assert.len(),
        StepSection::MultipartParts   => step.multipart_parts.len(),
        StepSection::GraphqlVariables => step.graphql_variables.len(),
        _ => 0,
    }
}

fn list_items_for(app: &BuilderApp, step_idx: usize, section: &StepSection) -> Vec<String> {
    let step = &app.campaign.steps[step_idx];
    match section {
        StepSection::Headers => {
            sorted_keys(&step.headers).into_iter()
                .map(|k| format!("{}: {}", k, step.headers.get(&k).cloned().unwrap_or_default()))
                .collect()
        }
        StepSection::Extract => {
            sorted_keys(&step.extract).into_iter()
                .map(|k| format!("{} = {}", k, step.extract.get(&k).cloned().unwrap_or_default()))
                .collect()
        }
        StepSection::Assertions => {
            step.assert.iter()
                .map(|a| format!("{} {}", a.on, assertion_op_label(a)))
                .collect()
        }
        StepSection::GraphqlVariables => {
            sorted_keys(&step.graphql_variables).into_iter()
                .map(|k| format!("{} = {}", k, step.graphql_variables.get(&k).cloned().unwrap_or_default()))
                .collect()
        }
        StepSection::MultipartParts => {
            step.multipart_parts.iter().map(|p| {
                let label = if p.value.starts_with('@') {
                    format!("{} = {} (file)", p.name, p.value)
                } else {
                    format!("{} = {}", p.name, p.value)
                };
                if let Some(ref ct) = p.content_type {
                    format!("{} [{}]", label, ct)
                } else {
                    label
                }
            }).collect()
        }
        StepSection::LoopHeaders => {
            sorted_keys(&step.headers).into_iter()
                .map(|k| format!("{}: {}", k, step.headers.get(&k).cloned().unwrap_or_default()))
                .collect()
        }
        StepSection::LoopExtract => {
            sorted_keys(&step.extract).into_iter()
                .map(|k| format!("{} = {}", k, step.extract.get(&k).cloned().unwrap_or_default()))
                .collect()
        }
        _ => vec![],
    }
}

fn assertion_op_label(a: &crate::campaign::Assertion) -> String {
    if let Some(eq) = &a.eq  { return format!("eq {}", eq); }
    if let Some(ne) = &a.ne  { return format!("ne {}", ne); }
    if a.exists == Some(true) { return "exists".into(); }
    if a.exists == Some(false){ return "not exists".into(); }
    if let Some(c) = &a.contains { return format!("contains {}", c); }
    if let Some(m) = &a.matches  { return format!("matches {}", m); }
    String::new()
}

fn render_collection_browser(
    frame: &mut Frame,
    app: &BuilderApp,
    col_cursor: usize,
    expanded: &std::collections::HashSet<String>,
    area: Rect,
) {
    let block = Block::default()
        .title(" Collections — select a request ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.stored_collections.is_empty() {
        let hint = Paragraph::new("No collections — Esc: cancel")
            .style(Style::default().fg(Color::Indexed(246)));
        frame.render_widget(hint, inner);
        return;
    }

    let nodes = super::browser::flatten(&app.stored_collections, expanded);
    let mut items: Vec<ListItem> = Vec::new();

    for (i, node) in nodes.iter().enumerate() {
        let is_cursor = i == col_cursor;
        let indent = "  ".repeat(node.depth);

        if node.is_folder {
            let arrow = if node.is_expanded { "▼ " } else { "▶ " };
            let cursor_mark = if is_cursor { "▶ " } else { "  " };
            let style = if is_cursor {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Indexed(250))
            };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{cursor_mark}{indent}{arrow}{}", node.label),
                    style,
                ),
            ])));
        } else {
            let cursor_mark = if is_cursor { "▶ " } else { "  " };
            let mc = method_color(&node.method);
            let label_style = if is_cursor {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Indexed(250))
            };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!("{cursor_mark}{indent}"), label_style),
                Span::styled(
                    format!("{:<6} ", node.method),
                    Style::default().fg(mc),
                ),
                Span::styled(node.label.clone(), label_style),
            ])));
        }
    }

    items.push(ListItem::new(Line::from("")));
    items.push(ListItem::new(Line::from(
        Span::styled(
            "↑↓: navigate  Space: expand/collapse  Enter: load  Esc: cancel",
            Style::default().fg(Color::Indexed(246)),
        ),
    )));

    frame.render_widget(List::new(items), inner);
}

fn method_color(method: &str) -> Color {
    match method {
        "GET"    => Color::Green,
        "POST"   => Color::Yellow,
        "PUT"    => Color::Blue,
        "PATCH"  => Color::Magenta,
        "DELETE" => Color::Red,
        _        => Color::Indexed(246),
    }
}

// ── Params editor ─────────────────────────────────────────────────────────────

fn render_params_editor(
    frame: &mut Frame,
    app: &BuilderApp,
    cursor: usize,
    mode: &ParamEditorMode,
    area: Rect,
) {
    let block = Block::default()
        .title(" Input Parameters [[params]] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut rows: Vec<ListItem> = Vec::new();
    rows.push(ListItem::new(Line::from(Span::styled(
        "Parameters passed at runtime (terapi run … --param KEY=val)",
        Style::default().fg(Color::Indexed(246)),
    ))));
    rows.push(ListItem::new(Line::from("")));

    let params = &app.campaign.params;

    if params.is_empty() && matches!(mode, ParamEditorMode::Browse) {
        rows.push(ListItem::new(Line::from(
            Span::styled("No parameters — a: add", Style::default().fg(Color::Indexed(246))),
        )));
    }

    for (i, p) in params.iter().enumerate() {
        let is_cursor = i == cursor && matches!(mode, ParamEditorMode::Browse);
        let prefix = if is_cursor { "▶ " } else { "  " };
        let name_style = if is_cursor {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let default_str = p.default.as_deref().unwrap_or("—");
        rows.push(ListItem::new(Line::from(vec![
            Span::styled(format!("{}{:<20}", prefix, p.name), name_style),
            Span::styled(truncate(default_str, 20), Style::default().fg(Color::Yellow)),
            if !p.description.is_empty() {
                Span::styled(format!("  {}", truncate(&p.description, 24)), Style::default().fg(Color::Indexed(246)))
            } else {
                Span::raw("")
            },
        ])));
    }

    // Inline add/edit form
    let (form_rows, hint) = match mode {
        ParamEditorMode::Browse => {
            let h = if params.is_empty() {
                "a: add  Esc: back"
            } else {
                "↑↓: navigate  Enter: edit  a: add  d: del  Esc: back"
            };
            (vec![], h)
        }
        ParamEditorMode::AddParam { name, desc, default_val, field } => {
            let form = param_form_rows("New parameter", name, desc, default_val, *field, None);
            (form, "Tab/Enter: next field  Esc: cancel")
        }
        ParamEditorMode::EditParam { idx, name, desc, default_val, field, .. } => {
            let label = format!("Edit param #{}", idx + 1);
            let form = param_form_rows(&label, name, desc, default_val, *field, None);
            (form, "Tab/Enter: next field  Esc: cancel")
        }
    };

    if !form_rows.is_empty() {
        rows.push(ListItem::new(Line::from("")));
        for r in form_rows {
            rows.push(r);
        }
    }

    rows.push(ListItem::new(Line::from("")));
    rows.push(ListItem::new(Line::from(
        Span::styled(hint, Style::default().fg(Color::Indexed(246))),
    )));

    frame.render_widget(List::new(rows), inner);
}

fn param_form_rows(
    title: &str,
    name: &str,
    desc: &str,
    default_val: &str,
    active_field: u8,
    _idx: Option<usize>,
) -> Vec<ListItem<'static>> {
    let mut rows = Vec::new();
    rows.push(ListItem::new(Line::from(Span::styled(
        format!("── {} ─────────────", title),
        Style::default().fg(Color::Cyan),
    ))));

    let fields = [
        ("Name",        name,        0u8),
        ("Description", desc,        1),
        ("Default",     default_val, 2),
    ];

    for (label, value, field_idx) in &fields {
        let is_active = *field_idx == active_field;
        let label_style = if is_active {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Indexed(246))
        };
        let value_span = if is_active {
            Span::styled(
                format!("[ {}_]", value),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                if value.is_empty() { "—".to_string() } else { value.to_string() },
                Style::default().fg(Color::Indexed(250)),
            )
        };
        rows.push(ListItem::new(Line::from(vec![
            Span::styled(format!("  {:<14}", label), label_style),
            value_span,
        ])));
    }

    rows
}

// ── Output step picker ────────────────────────────────────────────────────────

fn render_output_step_picker(frame: &mut Frame, app: &BuilderApp, step_cursor: usize, f1: &str, f2: &str, f3: &str, area: Rect) {
    let block = Block::default()
        .title(" Output — choose source step ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut rows: Vec<ListItem> = Vec::new();
    rows.push(ListItem::new(Line::from(Span::styled(
        "Select the step whose response will be collected in the output:",
        Style::default().fg(Color::Indexed(246)),
    ))));
    rows.push(ListItem::new(Line::from("")));

    let steps: Vec<&crate::campaign::Step> = app.campaign.steps.iter()
        .filter(|s| s.kind != "comment" && s.kind != "transform" && s.kind != "pause" && s.kind != "file")
        .collect();

    if steps.is_empty() {
        rows.push(ListItem::new(Line::from(Span::styled(
            "No HTTP steps available — add a step first",
            Style::default().fg(Color::Red),
        ))));
    } else {
        for (i, step) in steps.iter().enumerate() {
            let selected = i == step_cursor;
            let prefix = if selected { "▶ " } else { "  " };
            let (badge, badge_color) = step_badge(&step.kind);
            let method_str = if !step.method.is_empty() {
                format!("{:<6} ", step.method)
            } else {
                "       ".to_string()
            };
            let style = if selected {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Indexed(250))
            };
            rows.push(ListItem::new(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{badge:<4} "), Style::default().fg(badge_color).add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() })),
                Span::styled(method_str, Style::default().fg(Color::Yellow)),
                Span::styled(truncate(&step.name, 28), style),
                if !step.url.is_empty() {
                    Span::styled(format!("  {}", truncate(&step.url, 18)), Style::default().fg(Color::Indexed(246)))
                } else { Span::raw("") },
            ])));
        }
    }

    // Show already-filled fields as preview
    if !f1.is_empty() || !f2.is_empty() || !f3.is_empty() {
        rows.push(ListItem::new(Line::from("")));
        rows.push(ListItem::new(Line::from(Span::styled("── Current values ─────────────────", Style::default().fg(Color::Indexed(240))))));
        if !f1.is_empty() { rows.push(ListItem::new(Line::from(Span::styled(format!("  Path:         {}", f1), Style::default().fg(Color::Indexed(250)))))); }
        if !f2.is_empty() { rows.push(ListItem::new(Line::from(Span::styled(format!("  Select:       {}", f2), Style::default().fg(Color::Indexed(250)))))); }
        if !f3.is_empty() { rows.push(ListItem::new(Line::from(Span::styled(format!("  Include vars: {}", f3), Style::default().fg(Color::Indexed(250)))))); }
    }

    rows.push(ListItem::new(Line::from("")));
    rows.push(ListItem::new(Line::from(Span::styled(
        "↑↓: navigate  Enter: select  Esc: cancel",
        Style::default().fg(Color::Indexed(246)),
    ))));

    frame.render_widget(List::new(rows), inner);
}

// ── Connectors editor ─────────────────────────────────────────────────────────

fn render_connectors_editor(frame: &mut Frame, app: &BuilderApp, cursor: usize, mode: &IoEditorMode, area: Rect) {
    let block = Block::default()
        .title(" Input Connectors [[connectors]] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut rows: Vec<ListItem> = Vec::new();
    rows.push(ListItem::new(Line::from(Span::styled(
        "type: csv (iterate rows) | json (iterate array)",
        Style::default().fg(Color::Indexed(246)),
    ))));
    rows.push(ListItem::new(Line::from("")));

    let connectors = &app.campaign.connectors;
    if connectors.is_empty() && matches!(mode, IoEditorMode::Browse) {
        rows.push(ListItem::new(Line::from(Span::styled("No connectors — a: add", Style::default().fg(Color::Indexed(246))))));
    }

    for (i, c) in connectors.iter().enumerate() {
        let is_cursor = i == cursor && matches!(mode, IoEditorMode::Browse);
        let prefix = if is_cursor { "▶ " } else { "  " };
        let kind_color = if c.kind == "json" { Color::Yellow } else { Color::Green };
        let path_or_step = if let Some(ref fs) = c.from_step {
            format!("from:{}", fs)
        } else {
            truncate(&c.path, 30)
        };
        rows.push(ListItem::new(Line::from(vec![
            Span::styled(format!("{}{:<6}", prefix, c.kind), Style::default().fg(kind_color).add_modifier(if is_cursor { Modifier::BOLD } else { Modifier::empty() })),
            Span::styled(path_or_step, Style::default().fg(Color::Indexed(250))),
            if let Some(ref s) = c.select {
                Span::styled(format!("  select:{}", s), Style::default().fg(Color::Indexed(246)))
            } else { Span::raw("") },
        ])));
    }

    if let IoEditorMode::Edit { f0, f1, f2, f3, field, .. } = mode {
        let is_json = f0 == "json";
        rows.push(ListItem::new(Line::from("")));
        rows.push(ListItem::new(Line::from(Span::styled("── Edit connector ─────────────────", Style::default().fg(Color::Cyan)))));
        let labels = ["Type", "Path", "Select (opt.)", "From step (json)"];
        for (fi, label) in labels.iter().enumerate() {
            if fi == 3 && !is_json { continue; }
            let fi = fi as u8;
            let is_active = fi == *field;
            let val = match fi { 0 => f0.as_str(), 1 => f1.as_str(), 2 => f2.as_str(), _ => f3.as_str() };
            let label_style = if is_active { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Indexed(246)) };
            let val_span = if fi == 0 {
                Span::styled(format!("[ {} ▾ ]  ←/→", f0), Style::default().fg(Color::Yellow).add_modifier(if is_active { Modifier::BOLD } else { Modifier::empty() }))
            } else if is_active {
                Span::styled(format!("[ {}_]", val), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            } else {
                Span::styled(if val.is_empty() { "—".to_string() } else { val.to_string() }, Style::default().fg(Color::Indexed(250)))
            };
            rows.push(ListItem::new(Line::from(vec![
                Span::styled(format!("  {:<16}", label), label_style),
                val_span,
            ])));
        }
        rows.push(ListItem::new(Line::from("")));
        rows.push(ListItem::new(Line::from(Span::styled(
            "field 0: ←/→ cycle type  Tab/Enter: next  Esc: cancel",
            Style::default().fg(Color::Indexed(246)),
        ))));
    } else {
        rows.push(ListItem::new(Line::from("")));
        rows.push(ListItem::new(Line::from(Span::styled(
            "↑↓: navigate  Enter: edit  a: add  d: del  Esc: back",
            Style::default().fg(Color::Indexed(246)),
        ))));
    }

    frame.render_widget(List::new(rows), inner);
}

// ── Outputs editor ────────────────────────────────────────────────────────────

fn render_outputs_editor(frame: &mut Frame, app: &BuilderApp, cursor: usize, mode: &IoEditorMode, area: Rect) {
    let block = Block::default()
        .title(" Outputs [[outputs]] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut rows: Vec<ListItem> = Vec::new();
    rows.push(ListItem::new(Line::from(Span::styled(
        "Collect step responses into a JSON file",
        Style::default().fg(Color::Indexed(246)),
    ))));
    rows.push(ListItem::new(Line::from("")));

    let outputs = &app.campaign.outputs;
    if outputs.is_empty() && matches!(mode, IoEditorMode::Browse) {
        rows.push(ListItem::new(Line::from(Span::styled("No outputs — a: add", Style::default().fg(Color::Indexed(246))))));
    }

    for (i, o) in outputs.iter().enumerate() {
        let is_cursor = i == cursor && matches!(mode, IoEditorMode::Browse);
        let prefix = if is_cursor { "▶ " } else { "  " };
        let name_style = Style::default().fg(if is_cursor { Color::White } else { Color::Indexed(250) }).add_modifier(if is_cursor { Modifier::BOLD } else { Modifier::empty() });
        rows.push(ListItem::new(Line::from(vec![
            Span::styled(format!("{}{:<18}", prefix, o.from_step), name_style),
            Span::styled("→ ", Style::default().fg(Color::Indexed(246))),
            Span::styled(truncate(&o.path, 28), Style::default().fg(Color::Yellow)),
            if let Some(ref s) = o.select {
                Span::styled(format!("  [{}]", s), Style::default().fg(Color::Indexed(246)))
            } else { Span::raw("") },
        ])));
        if !o.include_vars.is_empty() {
            rows.push(ListItem::new(Line::from(Span::styled(
                format!("     vars: {}", o.include_vars.join(", ")),
                Style::default().fg(Color::Indexed(246)),
            ))));
        }
    }

    if let IoEditorMode::Edit { f0, f1, f2, f3, field, .. } = mode {
        rows.push(ListItem::new(Line::from("")));
        rows.push(ListItem::new(Line::from(Span::styled("── Edit output ───────────────────────", Style::default().fg(Color::Cyan)))));
        let labels = ["From step", "Path", "Select (opt.)", "Include vars"];
        let vals = [f0.as_str(), f1.as_str(), f2.as_str(), f3.as_str()];
        let descs = ["", "", "dot-path into response", "comma-separated VAR names"];
        for (fi, label) in labels.iter().enumerate() {
            let fi = fi as u8;
            let is_active = fi == *field;
            let val = vals[fi as usize];
            let label_style = if is_active { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Indexed(246)) };
            let val_span = if is_active {
                Span::styled(format!("[ {}_]", val), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            } else {
                Span::styled(if val.is_empty() { "—".to_string() } else { val.to_string() }, Style::default().fg(Color::Indexed(250)))
            };
            let desc = descs[fi as usize];
            rows.push(ListItem::new(Line::from(vec![
                Span::styled(format!("  {:<16}", label), label_style),
                val_span,
                if !desc.is_empty() { Span::styled(format!("  ({})", desc), Style::default().fg(Color::Indexed(242))) } else { Span::raw("") },
            ])));
        }
        rows.push(ListItem::new(Line::from("")));
        rows.push(ListItem::new(Line::from(Span::styled(
            "Tab/Enter: next field  Esc: cancel",
            Style::default().fg(Color::Indexed(246)),
        ))));
    } else {
        rows.push(ListItem::new(Line::from("")));
        rows.push(ListItem::new(Line::from(Span::styled(
            "↑↓: navigate  Enter: edit  a: add  d: del  Esc: back",
            Style::default().fg(Color::Indexed(246)),
        ))));
    }

    frame.render_widget(List::new(rows), inner);
}

// ── Run view ──────────────────────────────────────────────────────────────────

fn render_run_view(frame: &mut Frame, app: &BuilderApp, scroll: usize, area: Rect) {
    let (title, border_color, step_results, current_step, done) = match &app.run_state {
        CampaignRunState::Idle => {
            let block = Block::default().title(" Run ").borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Indexed(246)));
            frame.render_widget(block, area);
            return;
        }
        CampaignRunState::Running { name, step_results, current_step } => (
            format!(" ⟳ Running: {} ", name),
            Color::Yellow,
            step_results.as_slice(),
            current_step.as_deref(),
            false,
        ),
        CampaignRunState::Done { name, results } => {
            let flat: Vec<&StepResult> = results.iter().flat_map(|r| r.steps.iter()).collect();
            let ok = flat.iter().filter(|s| s.success && !s.skipped).count();
            let fail = flat.iter().filter(|s| !s.success).count();
            let color = if fail > 0 { Color::Red } else { Color::Green };
            let title = format!(" {} Done: {}  ✓ {}  ✗ {} ", if fail > 0 { "✗" } else { "✓" }, name, ok, fail);
            // Render done state inline
            let block = Block::default().title(title).borders(Borders::ALL)
                .border_style(Style::default().fg(color));
            let inner = block.inner(area);
            frame.render_widget(block, area);
            render_run_results(frame, app, &flat, None, true, scroll, inner);
            return;
        }
    };

    let block = Block::default().title(title).borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let flat: Vec<&StepResult> = step_results.iter().collect();
    render_run_results(frame, app, &flat, current_step, done, scroll, inner);
}

fn render_run_results(
    frame: &mut Frame,
    _app: &BuilderApp,
    results: &[&StepResult],
    current_step: Option<&str>,
    done: bool,
    scroll: usize,
    area: Rect,
) {
    let mut lines: Vec<Line> = Vec::new();

    // Cumulative extracted variables
    let mut all_vars: Vec<(String, String)> = Vec::new();

    for sr in results {
        // Step header line
        let (icon, icon_color) = if sr.skipped {
            ("⊘", Color::Indexed(246))
        } else if sr.success {
            ("✓", Color::Green)
        } else {
            ("✗", Color::Red)
        };

        let status_str = sr.status.map(|s| format!(" {}", s)).unwrap_or_default();
        let dur_str = if sr.duration_ms > 0 { format!(" {}ms", sr.duration_ms) } else { String::new() };

        lines.push(Line::from(vec![
            Span::styled(format!("{} ", icon), Style::default().fg(icon_color).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{:<6}", if sr.method.is_empty() { sr.name.chars().take(6).collect::<String>() } else { sr.method.clone() }),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                truncate(&sr.name, 28),
                Style::default().fg(if sr.success { Color::White } else { Color::Red }),
            ),
            Span::styled(status_str, Style::default().fg(status_color(sr.status))),
            Span::styled(dur_str, Style::default().fg(Color::Indexed(246))),
        ]));

        // Error message if any
        if let Some(ref err) = sr.error {
            for chunk in err.chars().collect::<Vec<_>>().chunks(50) {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(chunk.iter().collect::<String>(), Style::default().fg(Color::Red)),
                ]));
            }
        }

        // Assertion results
        for (desc, passed) in &sr.assertion_results {
            let (a_icon, a_color) = if *passed { ("  ✓", Color::Green) } else { ("  ✗", Color::Red) };
            lines.push(Line::from(vec![
                Span::styled(format!("{} ", a_icon), Style::default().fg(a_color)),
                Span::styled(truncate(desc, 52), Style::default().fg(Color::Indexed(246))),
            ]));
        }

        // Extracted vars from this step
        if !sr.extracted.is_empty() {
            let mut pairs: Vec<_> = sr.extracted.iter().collect();
            pairs.sort_by_key(|(k, _)| k.as_str());
            for (k, v) in &pairs {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("↳ {:<18}", k), Style::default().fg(Color::Cyan)),
                    Span::styled(truncate(v, 30), Style::default().fg(Color::Indexed(250))),
                ]));
                all_vars.push((k.to_string(), v.to_string()));
            }
        }

        lines.push(Line::from(""));
    }

    // "Currently running" indicator
    if let Some(name) = current_step {
        lines.push(Line::from(vec![
            Span::styled("⟳ ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(name, Style::default().fg(Color::Yellow)),
            Span::styled(" …", Style::default().fg(Color::Indexed(246))),
        ]));
        lines.push(Line::from(""));
    }

    // Variable summary (deduped, last value wins)
    if !all_vars.is_empty() {
        // Dedup: keep last value per key
        let mut seen: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for (k, v) in &all_vars { seen.insert(k.clone(), v.clone()); }
        let mut sorted: Vec<_> = seen.into_iter().collect();
        sorted.sort_by_key(|(k, _)| k.clone());

        lines.push(Line::from(Span::styled(
            "─ Variables ─────────────────────────────────────",
            Style::default().fg(Color::Indexed(240)),
        )));
        for (k, v) in &sorted {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<20}", k), Style::default().fg(Color::Cyan)),
                Span::styled(truncate(v, 32), Style::default().fg(Color::Indexed(250))),
            ]));
        }
        lines.push(Line::from(""));
    }

    if done {
        lines.push(Line::from(Span::styled(
            "r: re-run  Esc: back to pipeline",
            Style::default().fg(Color::Indexed(246)),
        )));
    }

    let para = Paragraph::new(lines)
        .scroll((scroll as u16, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

fn run_marker_for(run_state: &CampaignRunState, step_idx: usize) -> Option<(&'static str, Color)> {
    match run_state {
        CampaignRunState::Idle => None,
        CampaignRunState::Running { step_results, current_step, .. } => {
            if step_idx < step_results.len() {
                let sr = &step_results[step_idx];
                Some(if sr.skipped { ("⊘", Color::Indexed(246)) }
                     else if sr.success { ("✓", Color::Green) }
                     else { ("✗", Color::Red) })
            } else if current_step.is_some() && step_idx == step_results.len() {
                Some(("⟳", Color::Yellow))
            } else {
                Some(("·", Color::Indexed(240)))
            }
        }
        CampaignRunState::Done { results, .. } => {
            let flat: Vec<&StepResult> = results.iter().flat_map(|r| r.steps.iter()).collect();
            flat.get(step_idx).map(|sr| {
                if sr.skipped { ("⊘", Color::Indexed(246)) }
                else if sr.success { ("✓", Color::Green) }
                else { ("✗", Color::Red) }
            })
        }
    }
}

fn status_color(status: Option<u16>) -> Color {
    match status {
        Some(s) if s < 300 => Color::Green,
        Some(s) if s < 400 => Color::Yellow,
        Some(_) => Color::Red,
        None => Color::Indexed(246),
    }
}

// ── Status bar ────────────────────────────────────────────────────────────────

fn render_status(frame: &mut Frame, app: &BuilderApp, area: Rect) {
    let focus_label = match &app.focus {
        BuilderFocus::Pipeline              => "Builder › Pipeline",
        BuilderFocus::Catalog { .. }        => "Builder › Catalog",
        BuilderFocus::StepEditor { desc_active: true, .. } => "Builder › Step editor (comments)",
        BuilderFocus::StepEditor { .. }     => "Builder › Step editor",
        BuilderFocus::CollectionBrowser { .. } => "Builder › Collections",
        BuilderFocus::CampaignSettings { .. }  => "Builder › Campaign settings",
        BuilderFocus::Variables { .. }      => "Builder › Variables",
        BuilderFocus::Checker { .. }        => "Builder › Check report",
        BuilderFocus::TomlPreview { .. }    => "Builder › TOML preview",
        BuilderFocus::Run { .. }            => "Builder › Run",
        BuilderFocus::ParamsEditor { .. }      => "Builder › Parameters",
        BuilderFocus::ConnectorsEditor { .. }    => "Builder › Input Connectors",
        BuilderFocus::OutputsEditor { .. }       => "Builder › Outputs",
        BuilderFocus::PipelineConnectors { .. }  => "Builder › Pipeline [Inputs]",
        BuilderFocus::PipelineOutputs { .. }     => "Builder › Pipeline [Outputs]",
        BuilderFocus::OutputStepPicker { .. }    => "Builder › Output — step picker",
    };

    let hints: &str = match &app.focus {
        BuilderFocus::Pipeline =>
            "n: catalog(end)  i: catalog(after)  d: del  K/J: move  Enter: edit  r: run  s: settings  v: vars  c: check  p: preview  w: save  q: quit",
        BuilderFocus::Catalog { .. } =>
            "↑↓: choose  Enter: create  Esc: cancel",
        BuilderFocus::StepEditor { desc_active: true, .. } =>
            "Type comments  Enter: new line  Esc: save & close",
        BuilderFocus::StepEditor { mode: StepEditorMode::EditBody, .. } =>
            "Type body  Enter: new line  Esc: save & close",
        BuilderFocus::StepEditor { mode: StepEditorMode::EditGraphqlQuery, .. } =>
            "Type GQL query  Enter: new line  Esc: save & close",
        BuilderFocus::StepEditor { mode, .. } => match mode {
            StepEditorMode::Browse =>
                "↑↓: field  ↑ at top: description  Enter: edit  ←/→: cycle  a/d: list  r: run step  Esc: back",
            StepEditorMode::ExtractPicker { .. } =>
                "↑↓: navigate  Type: filter  Enter: insert  Tab/Esc: close",
            StepEditorMode::EditText { .. } =>
                "Type to edit  Enter: confirm  Esc: cancel",
            _ =>
                "Type  Enter: next/confirm  Esc: cancel",
        },
        BuilderFocus::CollectionBrowser { .. } =>
            "↑↓: navigate  Space: expand/collapse  Enter: load request  Esc: cancel",
        BuilderFocus::CampaignSettings { mode, .. } => match mode {
            CampaignSettingsMode::Browse =>
                "↑↓: field  Enter: edit/toggle  ←/→: cycle env  Esc: back",
            CampaignSettingsMode::EditText { .. } =>
                "Type to edit  Enter: confirm  Esc: cancel",
        },
        BuilderFocus::Checker { .. } | BuilderFocus::TomlPreview { .. } =>
            "↑↓: navigate  Esc: close",
        BuilderFocus::Variables { mode, .. } => match mode {
            VariablesMode::Browse => "↑↓: navigate  a: add  d: del  Enter: edit  Esc: close",
            VariablesMode::Edit { .. } => "Tab/Enter: next field / save  Esc: cancel",
        },
        BuilderFocus::Run { .. } =>
            "↑↓: scroll  r: re-run  Esc: back to pipeline",
        BuilderFocus::ParamsEditor { mode, .. } => match mode {
            ParamEditorMode::Browse =>
                "↑↓: navigate  Enter: edit  a: add  d: del  Esc: back",
            _ =>
                "Tab/Enter: next field  Esc: cancel",
        },
        BuilderFocus::ConnectorsEditor { mode, .. } => match mode {
            IoEditorMode::Browse =>
                "↑↓: navigate  Enter: edit  a: add  d: del  Esc: back",
            IoEditorMode::Edit { field: 0, .. } =>
                "←/→: cycle type  Tab/Enter: next field  Esc: cancel",
            _ =>
                "Type  Tab/Enter: next field  Esc: cancel",
        },
        BuilderFocus::OutputsEditor { mode, .. } => match mode {
            IoEditorMode::Browse =>
                "↑↓: navigate  Enter: edit  a: add  d: del  Esc: back",
            _ =>
                "Type  Tab/Enter: next field  Esc: cancel",
        },
        BuilderFocus::PipelineConnectors { .. } =>
            "↑↓: navigate  Enter: edit  d: delete  ↓ (last): back to steps  Esc: back",
        BuilderFocus::PipelineOutputs { .. } =>
            "↑↓: navigate  Enter: edit  d: delete  ↑ (first): back to steps  Esc: back",
        BuilderFocus::OutputStepPicker { .. } =>
            "↑↓: navigate  Enter: select step  Esc: cancel",
    };

    let status_msg = if app.status_message.is_empty() { "" } else { &app.status_message };
    let modified_flag = if app.modified {
        Span::styled(" [modified]", Style::default().fg(Color::Yellow))
    } else {
        Span::raw("")
    };

    let line1 = Line::from(vec![
        Span::styled(focus_label, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        modified_flag,
        if !status_msg.is_empty() {
            Span::styled(format!("  — {}", status_msg), Style::default().fg(Color::Green))
        } else {
            Span::raw("")
        },
    ]);
    let line2 = Line::from(Span::styled(hints, Style::default().fg(Color::Indexed(246))));

    let status = Paragraph::new(vec![line1, line2])
        .block(Block::default().borders(Borders::TOP));
    frame.render_widget(status, area);
}

// ── TOML generation (preview) ─────────────────────────────────────────────────

fn highlight_toml(text: &str) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut in_multiline = false; // inside ''' or """ block

    for raw in text.lines() {
        let trimmed = raw.trim_start();

        // Track multi-line literal/basic strings
        if in_multiline {
            let style = Style::default().fg(Color::Green);
            if trimmed.contains("'''") || trimmed.contains("\"\"\"") {
                in_multiline = false;
            }
            lines.push(Line::from(Span::styled(raw.to_string(), style)));
            continue;
        }

        // Empty line
        if trimmed.is_empty() {
            lines.push(Line::from(""));
            continue;
        }

        // Comment line
        if trimmed.starts_with('#') {
            lines.push(Line::from(Span::styled(
                raw.to_string(),
                Style::default().fg(Color::Indexed(246)),
            )));
            continue;
        }

        // [[array.table]] header
        if trimmed.starts_with("[[") {
            lines.push(Line::from(Span::styled(
                raw.to_string(),
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            )));
            continue;
        }

        // [table] header
        if trimmed.starts_with('[') {
            lines.push(Line::from(Span::styled(
                raw.to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )));
            continue;
        }

        // key = value
        if let Some(eq_pos) = trimmed.find(" = ") {
            let indent = " ".repeat(raw.len() - trimmed.len());
            let key = &trimmed[..eq_pos];
            let value = trimmed[eq_pos + 3..].trim();

            // Detect start of multi-line string
            if value.starts_with("'''") && !value[3..].contains("'''") {
                in_multiline = true;
            } else if value.starts_with("\"\"\"") && !value[3..].contains("\"\"\"") {
                in_multiline = true;
            }

            let val_style = toml_value_style(value);
            lines.push(Line::from(vec![
                Span::raw(indent),
                Span::styled(key.to_string(), Style::default().fg(Color::White)),
                Span::styled(" = ", Style::default().fg(Color::Indexed(246))),
                Span::styled(value.to_string(), val_style),
            ]));
            continue;
        }

        // Fallback (continuation of inline tables, etc.)
        lines.push(Line::from(Span::styled(
            raw.to_string(),
            Style::default().fg(Color::Indexed(246)),
        )));
    }

    lines
}

fn toml_value_style(value: &str) -> Style {
    if value.starts_with("'''") || value.starts_with("\"\"\"")
        || value.starts_with('"')  || value.starts_with('\'') {
        Style::default().fg(Color::Green)
    } else if value == "true" || value == "false" {
        Style::default().fg(Color::Yellow)
    } else if value.starts_with(|c: char| c.is_ascii_digit() || c == '-') {
        Style::default().fg(Color::Yellow)
    } else if value.starts_with('[') || value.starts_with('{') {
        Style::default().fg(Color::Indexed(246))
    } else {
        Style::default().fg(Color::White)
    }
}

fn generate_toml_preview(app: &BuilderApp) -> String {
    super::generate_toml(&app.campaign, &app.step_comments, &app.header_comment)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn sub_row(text: String, color: Color) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        Span::raw("     "),
        Span::styled(format!("  {}", text), Style::default().fg(color).add_modifier(Modifier::BOLD)),
    ]))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}

fn hint_line<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {:<12}", key), Style::default().fg(Color::Yellow)),
        Span::styled(desc, Style::default().fg(Color::Indexed(250))),
    ])
}
