use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use super::BuilderApp;
use super::step_editor::{current_value, sections_for, sorted_keys};
use super::types::{BRICK_KINDS, BuilderFocus, CampaignSettingsMode, CheckLevel, StepEditorMode, StepSection};

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
}

// ── Pipeline ─────────────────────────────────────────────────────────────────

fn render_pipeline(frame: &mut Frame, app: &BuilderApp, area: Rect) {
    let env_label = app.campaign.env_file.as_deref().unwrap_or("—");
    let title = if app.modified {
        format!(" Pipeline · {} [{}] * ", app.campaign.campaign.name, env_label)
    } else {
        format!(" Pipeline · {} [{}] ", app.campaign.campaign.name, env_label)
    };

    let in_pipeline = matches!(app.focus, BuilderFocus::Pipeline);
    let border_style = if in_pipeline {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.campaign.steps.is_empty() {
        let hint = Paragraph::new("No steps — n: add")
            .style(Style::default().fg(Color::Indexed(242)));
        frame.render_widget(hint, inner);
        return;
    }

    let mut items: Vec<ListItem> = Vec::new();
    let mut step_number = 0usize; // only counts non-comment steps
    for (idx, step) in app.campaign.steps.iter().enumerate() {
        let selected = idx == app.cursor;
        let cursor_char = if selected { "▶ " } else { "  " };

        if step.kind == "comment" {
            let style = if selected {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Indexed(238))
            };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{}# {}", cursor_char, truncate(&step.name, 42)),
                    style,
                ),
            ])));
            continue;
        }

        step_number += 1;
        let (badge, badge_color) = step_badge(&step.kind);

        let num_span = Span::styled(
            format!("{}[{}] ", cursor_char, step_number),
            if selected { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) }
            else         { Style::default().fg(Color::Indexed(242)) },
        );
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
            num_span, badge_span, method_span, summary_span,
        ])));

        if let Some(foreach) = &step.foreach {
            items.push(ListItem::new(Line::from(vec![
                Span::raw("         "),
                Span::styled(format!("↻ foreach: {}", foreach), Style::default().fg(Color::Indexed(242))),
            ])));
        }
        if let Some(when) = &step.when {
            let label = when_label(when);
            items.push(ListItem::new(Line::from(vec![
                Span::raw("         "),
                Span::styled(label, Style::default().fg(Color::Indexed(242))),
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
                Span::styled(label, Style::default().fg(Color::Indexed(242))),
            ])));
        }
    }

    let coe_flag = if app.campaign.continue_on_error {
        Span::styled(" ↷ continue-on-error", Style::default().fg(Color::Yellow))
    } else {
        Span::raw("")
    };
    items.push(ListItem::new(Line::from(vec![Span::raw(""), coe_flag])));

    let list = List::new(items);
    frame.render_widget(list, inner);
}

fn step_badge(kind: &str) -> (&'static str, Color) {
    match kind {
        "transform" => ("TRSF", Color::Yellow),
        "pause"     => ("WAIT", Color::Indexed(242)),
        "seed"      => ("SEED", Color::Blue),
        "comment"   => ("#   ", Color::Indexed(238)),
        _           => ("HTTP", Color::Cyan),
    }
}

fn step_summary(step: &crate::campaign::Step) -> String {
    match step.kind.as_str() {
        "pause"     => format!("{}ms", step.wait_ms),
        "transform" => {
            if let Some(t) = step.transforms.first() {
                format!("{} → {}", t.kind, t.output)
            } else {
                step.name.clone()
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
        BuilderFocus::StepEditor { step_idx, section_cursor, sub_cursor, mode } =>
            render_step_editor(frame, app, *step_idx, *section_cursor, *sub_cursor, mode, area),
        BuilderFocus::CollectionBrowser { col_cursor, expanded, .. } =>
            render_collection_browser(frame, app, *col_cursor, expanded, area),
        BuilderFocus::CampaignSettings { cursor, mode } =>
            render_campaign_settings(frame, app, *cursor, mode, area),
        BuilderFocus::Checker { results } => render_checker(frame, results, area),
        BuilderFocus::TomlPreview { scroll } => render_toml_preview(frame, app, *scroll, area),
        BuilderFocus::Variables { cursor } => render_variables(frame, app, *cursor, area),
    }
}

fn render_pipeline_hint(frame: &mut Frame, app: &BuilderApp, area: Rect) {
    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Indexed(242)));

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
        hint_line("w",       "Save"),
        hint_line("q",       "Quit"),
        Line::from(""),
        Line::from(Span::styled(
            format!("{} step{}", step_count, if step_count != 1 { "s" } else { "" }),
            Style::default().fg(Color::Indexed(242)),
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
        let desc_style = Style::default().fg(Color::Indexed(242));
        ListItem::new(Line::from(vec![
            Span::styled(format!("{}{:<14}", prefix, brick.label()), style),
            Span::styled(brick.description(), desc_style),
        ]))
    }).collect();

    let hint = ListItem::new(Line::from(vec![
        Span::styled("  ↑↓: choose  Enter: create  Esc: cancel", Style::default().fg(Color::Indexed(242))),
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
    ];

    let mut rows: Vec<ListItem> = Vec::new();
    rows.push(ListItem::new(Line::from("")));

    for &(label, idx) in fields {
        let is_cursor = idx == cursor;
        let cursor_char = if is_cursor { "▶ " } else { "  " };
        let label_style = if is_cursor {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Indexed(242))
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
        Span::styled(hints, Style::default().fg(Color::Indexed(242)))
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
                Span::styled("—", Style::default().fg(Color::Indexed(242)))
            } else {
                Span::styled(d.clone(), Style::default().fg(color))
            }
        }
        2 => {
            if app.campaign.continue_on_error {
                Span::styled("[x] enabled", Style::default().fg(Color::Green))
            } else {
                Span::styled("[ ] disabled", Style::default().fg(Color::Indexed(242)))
            }
        }
        3 => {
            let env = app.campaign.env_file.as_deref().unwrap_or("— none —");
            Span::styled(format!("[ {} ▾ ]", env), Style::default().fg(Color::Yellow))
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
    all.push(Line::from(Span::styled("Esc: close", Style::default().fg(Color::Indexed(242)))));

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
    let lines: Vec<Line> = toml_str.lines()
        .skip(scroll)
        .map(|l| Line::from(Span::styled(l.to_string(), Style::default().fg(Color::White))))
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_variables(frame: &mut Frame, app: &BuilderApp, cursor: usize, area: Rect) {
    let block = Block::default()
        .title(" Variables [env] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut vars: Vec<(&String, &String)> = app.campaign.env.iter().collect();
    vars.sort_by_key(|(k, _)| k.as_str());

    if vars.is_empty() {
        let hint = Paragraph::new("No variables — a: add")
            .style(Style::default().fg(Color::Indexed(242)));
        frame.render_widget(hint, inner);
        return;
    }

    let items: Vec<ListItem> = vars.iter().enumerate().map(|(i, (k, v))| {
        let selected = i == cursor;
        let prefix = if selected { "▶ " } else { "  " };
        let key_style = Style::default().fg(Color::Yellow).add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() });
        let val_style = Style::default().fg(if selected { Color::White } else { Color::Indexed(250) });
        ListItem::new(Line::from(vec![
            Span::styled(format!("{}{:<20}", prefix, k), key_style),
            Span::styled(v.as_str().to_string(), val_style),
        ]))
    }).collect();

    let mut all = items;
    all.push(ListItem::new(Line::from("")));
    all.push(ListItem::new(Line::from(
        Span::styled("a: add  d: del  Enter: edit  Esc: close", Style::default().fg(Color::Indexed(242)))
    )));

    frame.render_widget(List::new(all), inner);
}

fn render_step_editor(
    frame: &mut Frame,
    app: &BuilderApp,
    step_idx: usize,
    section_cursor: usize,
    _sub_cursor: usize,
    mode: &StepEditorMode,
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
            Style::default().fg(Color::Indexed(242))
        };

        let cursor_char = if is_cursor { "▶ " } else { "  " };
        let label = format!("{}{:<17}", cursor_char, section.label());

        let value_span = if is_cursor {
            match mode {
                StepEditorMode::EditText { buffer } => {
                    Span::styled(
                        format!("[ {}_]", buffer),
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    )
                }
                StepEditorMode::AddPairStage1 { .. } | StepEditorMode::AddPairStage2 { .. }
                    if section.is_list() =>
                {
                    Span::styled(
                        format!("({} items)", list_count(app, step_idx, section)),
                        Style::default().fg(Color::Indexed(242)),
                    )
                }
                _ => value_span_for(app, step_idx, section, is_cursor),
            }
        } else {
            value_span_for(app, step_idx, section, is_cursor)
        };

        let hint_span = if is_cursor && section.is_list() && matches!(mode, StepEditorMode::Browse) {
            Span::styled("  a: add  d: del", Style::default().fg(Color::Indexed(242)))
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
            for item_str in &items {
                rows.push(ListItem::new(Line::from(vec![
                    Span::raw("     "),
                    Span::styled(
                        format!("  {}", item_str),
                        Style::default().fg(if is_cursor { Color::Indexed(250) } else { Color::Indexed(242) }),
                    ),
                ])));
            }
        }

        if is_cursor {
            match mode {
                StepEditorMode::AddPairStage1 { buffer, .. } => {
                    rows.push(ListItem::new(Line::from(vec![
                        Span::raw("     "),
                        Span::styled(
                            format!("  Key : [ {}_]", buffer),
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                        ),
                    ])));
                }
                StepEditorMode::AddPairStage2 { key, buffer, .. } => {
                    rows.push(ListItem::new(Line::from(vec![
                        Span::raw("     "),
                        Span::styled(
                            format!("  {} : [ {}_]", key, buffer),
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                        ),
                    ])));
                }
                _ => {}
            }
        }
    }

    rows.push(ListItem::new(Line::from("")));
    let hints = match mode {
        StepEditorMode::Browse =>
            "↑↓: field  Enter: edit  ←/→: cycle  a/d: list  Esc: back",
        StepEditorMode::EditText { .. } =>
            "Type to edit  Enter: confirm  Esc: cancel",
        StepEditorMode::AddPairStage1 { .. } =>
            "Key name  Enter: next  Esc: cancel",
        StepEditorMode::AddPairStage2 { .. } =>
            "Value  Enter: add  Esc: cancel",
    };
    rows.push(ListItem::new(Line::from(
        Span::styled(hints, Style::default().fg(Color::Indexed(242)))
    )));

    frame.render_widget(List::new(rows), inner);
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
        StepSection::TransformKind => {
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
            Span::styled("—", Style::default().fg(Color::Indexed(242)))
        }
        _ => {
            Span::styled(truncate(&val, 38), Style::default().fg(color))
        }
    }
}

fn list_count(app: &BuilderApp, step_idx: usize, section: &StepSection) -> usize {
    let step = &app.campaign.steps[step_idx];
    match section {
        StepSection::Headers    => step.headers.len(),
        StepSection::Extract    => step.extract.len(),
        StepSection::Assertions => step.assert.len(),
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
            .style(Style::default().fg(Color::Indexed(242)));
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
            Style::default().fg(Color::Indexed(242)),
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
        _        => Color::Indexed(242),
    }
}

// ── Status bar ────────────────────────────────────────────────────────────────

fn render_status(frame: &mut Frame, app: &BuilderApp, area: Rect) {
    let focus_label = match &app.focus {
        BuilderFocus::Pipeline              => "Builder › Pipeline",
        BuilderFocus::Catalog { .. }        => "Builder › Catalog",
        BuilderFocus::StepEditor { .. }     => "Builder › Step editor",
        BuilderFocus::CollectionBrowser { .. } => "Builder › Collections",
        BuilderFocus::CampaignSettings { .. }  => "Builder › Campaign settings",
        BuilderFocus::Variables { .. }      => "Builder › Variables",
        BuilderFocus::Checker { .. }        => "Builder › Check report",
        BuilderFocus::TomlPreview { .. }    => "Builder › TOML preview",
    };

    let hints: &str = match &app.focus {
        BuilderFocus::Pipeline =>
            "n: new  i: insert  d: del  K/J: move  Enter: edit  s: settings  v: vars  c: check  p: preview  w: save  q: quit",
        BuilderFocus::Catalog { .. } =>
            "↑↓: choose  Enter: create  Esc: cancel",
        BuilderFocus::StepEditor { mode, .. } => match mode {
            StepEditorMode::Browse =>
                "↑↓: field  Enter: edit  ←/→: cycle  a/d: list  Esc: back to pipeline",
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
        BuilderFocus::Checker { .. } | BuilderFocus::TomlPreview { .. } | BuilderFocus::Variables { .. } =>
            "↑↓: navigate  Esc: close",
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
    let line2 = Line::from(Span::styled(hints, Style::default().fg(Color::Indexed(242))));

    let status = Paragraph::new(vec![line1, line2])
        .block(Block::default().borders(Borders::TOP));
    frame.render_widget(status, area);
}

// ── TOML generation (preview) ─────────────────────────────────────────────────

fn generate_toml_preview(app: &BuilderApp) -> String {
    let mut out = String::new();
    let m = &app.campaign.campaign;
    out.push_str(&format!("[campaign]\nname        = \"{}\"\ndescription = \"{}\"\n", m.name, m.description));
    if app.campaign.continue_on_error {
        out.push_str("continue_on_error = true\n");
    }
    if let Some(ref env) = app.campaign.env_file {
        out.push_str(&format!("env_file = \"{}\"\n", env));
    }

    if !app.campaign.env.is_empty() {
        out.push_str("\n[env]\n");
        let mut vars: Vec<_> = app.campaign.env.iter().collect();
        vars.sort_by_key(|(k, _)| k.as_str());
        for (k, v) in vars {
            out.push_str(&format!("{} = \"{}\"\n", k, v));
        }
    }

    for step in &app.campaign.steps {
        if step.kind == "comment" {
            out.push_str(&format!("\n# {}\n", step.name));
            continue;
        }
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
    }

    out
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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
