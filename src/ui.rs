use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, TableState, Tabs},
    Frame,
};

use crate::app::{
    flatten_stored, sorted_vars, App, BodyMode, EnvFocus, InputField, ModalState,
    RequestFocus, RequestTab, ResponseView, SaveField, Tab, VarField,
    COMMON_CONTENT_TYPES, COMMON_HEADERS, METHODS,
};
use crate::json_highlight::{self, ValueType};

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(area);

    render_tabs(frame, app, chunks[0]);
    render_body(frame, app, chunks[1]);
    render_status(frame, app, chunks[2]);

    if app.modal.is_some() {
        render_modal(frame, app);
    }
    if app.var_picker.is_some() {
        render_var_picker(frame, app);
    }
}

fn render_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let tabs: Vec<Line> = Tab::all().into_iter().map(|t| Line::from(t.title())).collect();
    let selected = Tab::all().into_iter().position(|t| t == app.active_tab).unwrap_or(0);

    let tabs_widget = Tabs::new(tabs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" terapi ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .select(selected)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .divider(Span::raw(" | "));

    frame.render_widget(tabs_widget, area);
}

fn render_body(frame: &mut Frame, app: &App, area: Rect) {
    match app.active_tab {
        Tab::Request => render_request_panel(frame, app, area),
        Tab::Collections => render_collections_panel(frame, app, area),
        Tab::Env => render_env_panel(frame, app, area),
        Tab::History => render_history_panel(frame, app, area),
    }
}

// ── Request panel ────────────────────────────────────────────────────────────

fn render_request_panel(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Fill(2),
        ])
        .split(area);

    let env_badge = app.active_env_idx
        .and_then(|i| app.environments.get(i))
        .map(|e| format!(" · env: {}", e.env.name))
        .unwrap_or_default();
    let url_title = format!(" URL{} ", env_badge);

    let editing = app.request_focus == RequestFocus::Url;
    let method = app.active_method();
    let url_cursor = if editing { "_" } else { "" };
    let url_text = Line::from(vec![
        Span::raw(" "),
        if editing {
            Span::styled("◀ ", Style::default().fg(Color::DarkGray))
        } else {
            Span::raw("  ")
        },
        Span::styled(method, Style::default().fg(method_color(method)).add_modifier(Modifier::BOLD)),
        if editing {
            Span::styled(" ▶  ", Style::default().fg(Color::DarkGray))
        } else {
            Span::raw("  ")
        },
        Span::styled(
            format!("{}{}", app.request_url, url_cursor),
            if editing {
                Style::default().fg(Color::Yellow)
            } else if app.request_url.is_empty() {
                Style::default().fg(Color::Indexed(244))
            } else {
                Style::default().fg(Color::White)
            },
        ),
    ]);

    let url_border_color = if app.request_loading {
        Color::Cyan
    } else if editing {
        Color::Yellow
    } else {
        Color::Yellow
    };

    let url_bar = Paragraph::new(url_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(url_title)
                .border_style(Style::default().fg(url_border_color)),
        );
    frame.render_widget(url_bar, chunks[0]);

    render_request_subtabs(frame, app, chunks[1]);
    render_request_content(frame, app, chunks[2]);
    render_response(frame, app, chunks[3]);
}

fn render_request_subtabs(frame: &mut Frame, app: &App, area: Rect) {
    let tabs: Vec<Line> = RequestTab::all().into_iter().map(|t| Line::from(t.title())).collect();
    let selected = RequestTab::all().into_iter().position(|t| t == app.active_request_tab).unwrap_or(0);

    let sub_tabs = Tabs::new(tabs)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Indexed(238))))
        .select(selected)
        .style(Style::default().fg(Color::Indexed(244)))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .divider(Span::raw(" | "));

    frame.render_widget(sub_tabs, area);
}

fn render_request_content(frame: &mut Frame, app: &App, area: Rect) {
    if app.active_request_tab == RequestTab::Headers {
        render_headers_editor(frame, app, area);
        return;
    }
    if app.active_request_tab == RequestTab::Body {
        render_body_editor(frame, app, area);
        return;
    }
    if app.active_request_tab == RequestTab::UrlParams {
        render_url_params_editor(frame, app, area);
        return;
    }
    if app.active_request_tab == RequestTab::Options {
        render_options_editor(frame, app, area);
        return;
    }
    if app.active_request_tab == RequestTab::Auth {
        render_auth_editor(frame, app, area);
        return;
    }

    let editing = app.request_focus == RequestFocus::Description;
    let border_style = if editing {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Yellow)
    };
    let title = if editing { " Description — editing (Esc: done) " } else { " Description — i: edit " };
    let mut textarea = app.description_textarea.clone();
    textarea.set_block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    );
    textarea.set_style(Style::default().fg(Color::White));
    textarea.set_cursor_line_style(Style::default());
    if !editing {
        textarea.set_cursor_style(Style::default());
    }
    frame.render_widget(&textarea, area);
}

fn render_options_editor(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Options ")
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let cursor = app.options_cursor;

    let option_line = |idx: usize, checked: bool, label: &str, detail: &str| -> Line<'static> {
        let selected = idx == cursor;
        let cursor_marker = if selected { "▶ " } else { "  " };
        let checkbox = if checked { "[x]" } else { "[ ]" };
        let (check_style, label_style) = if checked {
            (Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
             Style::default().fg(Color::Yellow))
        } else {
            (Style::default().fg(Color::Gray),
             if selected { Style::default().fg(Color::White).add_modifier(Modifier::BOLD) }
             else        { Style::default().fg(Color::White) })
        };
        let cursor_style = if selected {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        Line::from(vec![
            Span::styled(cursor_marker.to_string(), cursor_style),
            Span::styled(format!("{} ", checkbox), check_style),
            Span::styled(label.to_string(), label_style),
            Span::styled(format!("  {}", detail), Style::default().fg(Color::Indexed(244))),
        ])
    };

    // Timeout row (numeric, cycles through presets)
    let timeout_selected = cursor == 2;
    let timeout_cursor_style = if timeout_selected { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) };
    let timeout_label_style = if timeout_selected {
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let timeout_line = Line::from(vec![
        Span::styled(if timeout_selected { "▶ " } else { "  " }.to_string(), timeout_cursor_style),
        Span::styled(format!("[{}s]", app.request_timeout_secs), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled("  Timeout".to_string(), timeout_label_style),
        Span::styled("  (Space/Enter cycles: 5→10→15→20→30→45→60→90→120→300 s)".to_string(), Style::default().fg(Color::Indexed(244))),
    ]);

    let hint = Line::from(Span::styled(
        "  ↑/↓: navigate   Space/Enter: toggle / cycle timeout",
        Style::default().fg(Color::Indexed(238)),
    ));

    let text = vec![
        Line::from(""),
        option_line(0, app.skip_tls_verify, "Skip TLS verification", "(accept self-signed / mismatched certificates)"),
        Line::from(""),
        option_line(1, app.follow_redirects, "Follow redirects",     "(automatically follow 3xx responses, up to 10)"),
        Line::from(""),
        timeout_line,
        Line::from(""),
        option_line(3, app.cookie_jar,       "Cookie jar",           "(store & send cookies across requests — cleared on new request)"),
        Line::from(""),
        hint,
    ];
    frame.render_widget(Paragraph::new(text), inner);
}

fn render_auth_editor(frame: &mut Frame, app: &App, area: Rect) {
    use crate::app::{AuthType, ApiKeyLocation};

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Auth ")
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let auth = &app.auth_config;
    let cursor = app.auth_field_cursor;

    // ── Row 0: type selector ─────────────────────────────────────────────────
    let types = [AuthType::None, AuthType::Bearer, AuthType::Basic, AuthType::ApiKey];
    let type_spans: Vec<Span> = types.iter().enumerate().flat_map(|(i, t)| {
        let active = &auth.auth_type == t;
        let label = format!(" {} ", t.label());
        let style = if active {
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Indexed(244))
        };
        let mut spans = vec![Span::styled(label, style)];
        if i < types.len() - 1 {
            spans.push(Span::styled("  ", Style::default()));
        }
        spans
    }).collect();

    let row0_bg = if cursor == 0 {
        Style::default().bg(Color::Indexed(237))
    } else {
        Style::default()
    };
    let mut type_line_spans = vec![Span::styled(" Type   ", Style::default().fg(Color::Indexed(244)))];
    type_line_spans.extend(type_spans);
    let type_line = Line::from(type_line_spans).style(row0_bg);

    // ── Field rows ──────────────────────────────────────────────────────────
    let field_rows: Vec<Line> = match &auth.auth_type {
        AuthType::None => vec![
            Line::from(Span::styled(
                " No authentication header will be sent.",
                Style::default().fg(Color::Indexed(238)),
            )),
        ],
        AuthType::Bearer => {
            let row_style = if cursor == 1 { Style::default().bg(Color::Indexed(237)) } else { Style::default() };
            let value = if auth.bearer_token.is_empty() {
                Span::styled(" <enter token>", Style::default().fg(Color::Indexed(238)))
            } else {
                Span::styled(format!(" {}", &auth.bearer_token), Style::default().fg(Color::Green))
            };
            vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled(" Token   ", Style::default().fg(Color::Indexed(244))),
                    value,
                ]).style(row_style),
            ]
        }
        AuthType::Basic => {
            let user_style = if cursor == 1 { Style::default().bg(Color::Indexed(237)) } else { Style::default() };
            let pass_style = if cursor == 2 { Style::default().bg(Color::Indexed(237)) } else { Style::default() };
            let user_val = if auth.basic_username.is_empty() {
                Span::styled(" <enter username>", Style::default().fg(Color::Indexed(238)))
            } else {
                Span::styled(format!(" {}", &auth.basic_username), Style::default().fg(Color::Green))
            };
            let pass_val = if auth.basic_password.is_empty() {
                Span::styled(" <enter password>", Style::default().fg(Color::Indexed(238)))
            } else {
                Span::styled(format!(" {}", "•".repeat(auth.basic_password.len())), Style::default().fg(Color::Yellow))
            };
            vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled(" Username", Style::default().fg(Color::Indexed(244))),
                    user_val,
                ]).style(user_style),
                Line::from(vec![
                    Span::styled(" Password", Style::default().fg(Color::Indexed(244))),
                    pass_val,
                ]).style(pass_style),
            ]
        }
        AuthType::ApiKey => {
            let name_style  = if cursor == 1 { Style::default().bg(Color::Indexed(237)) } else { Style::default() };
            let value_style = if cursor == 2 { Style::default().bg(Color::Indexed(237)) } else { Style::default() };
            let loc_style   = if cursor == 3 { Style::default().bg(Color::Indexed(237)) } else { Style::default() };
            let name_val = if auth.api_key_name.is_empty() {
                Span::styled(" <enter key name>", Style::default().fg(Color::Indexed(238)))
            } else {
                Span::styled(format!(" {}", &auth.api_key_name), Style::default().fg(Color::Cyan))
            };
            let key_val = if auth.api_key_value.is_empty() {
                Span::styled(" <enter key value>", Style::default().fg(Color::Indexed(238)))
            } else {
                Span::styled(format!(" {}", &auth.api_key_value), Style::default().fg(Color::Green))
            };
            let (hdr_style, qp_style) = match auth.api_key_location {
                ApiKeyLocation::Header => (
                    Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
                    Style::default().fg(Color::Indexed(244)),
                ),
                ApiKeyLocation::QueryParam => (
                    Style::default().fg(Color::Indexed(244)),
                    Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
            };
            vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled(" Key Name", Style::default().fg(Color::Indexed(244))),
                    name_val,
                ]).style(name_style),
                Line::from(vec![
                    Span::styled(" Key Value", Style::default().fg(Color::Indexed(244))),
                    key_val,
                ]).style(value_style),
                Line::from(vec![
                    Span::styled(" Location ", Style::default().fg(Color::Indexed(244))),
                    Span::styled(" Header ", hdr_style),
                    Span::styled("  ", Style::default()),
                    Span::styled(" Query Param ", qp_style),
                ]).style(loc_style),
            ]
        }
    };

    let hint = Line::from(Span::styled(
        " ↑/↓: navigate  Space/Enter: cycle type or edit field",
        Style::default().fg(Color::Indexed(238)),
    ));

    let mut lines = vec![Line::from(""), type_line, Line::from("")];
    lines.extend(field_rows);
    lines.push(Line::from(""));
    lines.push(hint);

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_url_params_editor(frame: &mut Frame, app: &App, area: Rect) {
    let count = app.request_url_params.len();
    let title = if count == 0 {
        " URL Params ".to_string()
    } else {
        format!(" URL Params ({}) ", count)
    };

    let items: Vec<ListItem> = if app.request_url_params.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  No params — press a to add one",
            Style::default().fg(Color::Gray),
        )))]
    } else {
        app.request_url_params.iter().enumerate().map(|(i, (k, v))| {
            let line = Line::from(vec![
                Span::styled(format!("  {:<28}", k), Style::default().fg(Color::Cyan)),
                Span::styled("= ", Style::default().fg(Color::DarkGray)),
                Span::styled(v.clone(), Style::default().fg(Color::Yellow)),
            ]);
            let style = if i == app.url_params_cursor {
                Style::default().bg(Color::Indexed(237)).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        }).collect()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(list, area);
}

fn render_body_editor(frame: &mut Frame, app: &App, area: Rect) {
    match app.body_mode {
        BodyMode::Text => render_body_text(frame, app, area),
        BodyMode::Json => render_body_json(frame, app, area),
    }
}

fn render_body_text(frame: &mut Frame, app: &App, area: Rect) {
    let editing = app.request_focus == RequestFocus::Body;
    let border_style = if editing {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Yellow)
    };
    let title = if editing {
        " Body  [Text — Esc: exit] ".to_string()
    } else {
        let lines = app.body_textarea.lines().iter().filter(|l| !l.trim().is_empty()).count();
        if lines == 0 {
            " Body  [Text]  i: edit  t: JSON mode ".to_string()
        } else {
            format!(" Body  [Text]  ({} lines)  i: edit  t: JSON mode ", lines)
        }
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(&app.body_textarea, inner);
}

fn render_body_json(frame: &mut Frame, app: &App, area: Rect) {
    let editing = app.request_focus == RequestFocus::Body;
    let border_style = if editing {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Yellow)
    };
    let n = app.body_json_pairs.len();
    let title = if editing {
        " Body  [JSON — a: add  d: delete  Enter: edit  Esc: exit] ".to_string()
    } else if n == 0 {
        " Body  [JSON]  i: edit  t: text mode ".to_string()
    } else {
        format!(" Body  [JSON]  ({} fields)  i: edit  t: text mode ", n)
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.body_json_pairs.is_empty() {
        let hint = if editing { "  a: add a field" } else { "  No fields — press i then a to add" };
        frame.render_widget(
            Paragraph::new(hint).style(Style::default().fg(Color::Gray)),
            inner,
        );
        return;
    }

    let rows: Vec<Row> = app.body_json_pairs.iter().enumerate().map(|(i, (k, v))| {
        let value_color = json_value_color(v);
        let style = if editing && i == app.body_json_cursor {
            Style::default().bg(Color::Indexed(237)).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        Row::new(vec![
            Cell::from(Span::styled(format!(" {}", k), Style::default().fg(Color::Cyan))),
            Cell::from(Span::styled(v.clone(), Style::default().fg(value_color))),
        ]).style(style)
    }).collect();

    let header = Row::new(vec![
        Cell::from(Span::styled(" Key", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Value", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
    ])
    .style(Style::default().bg(Color::Indexed(236)))
    .height(1);

    let widths = [Constraint::Percentage(40), Constraint::Percentage(60)];
    let table = Table::new(rows, widths).header(header).column_spacing(1);
    frame.render_widget(table, inner);
}

fn json_value_color(v: &str) -> Color {
    if v == "null" { Color::DarkGray }
    else if v == "true" || v == "false" { Color::Magenta }
    else if v.parse::<f64>().is_ok() { Color::Yellow }
    else { Color::Green }
}

fn render_headers_editor(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = if app.request_headers.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  No headers — press a to add one",
            Style::default().fg(Color::Gray),
        )))]
    } else {
        app.request_headers.iter().enumerate().map(|(i, (k, v))| {
            let line = Line::from(vec![
                Span::styled(format!("  {:<28}", k), Style::default().fg(Color::Cyan)),
                Span::styled(": ", Style::default().fg(Color::DarkGray)),
                Span::styled(v.clone(), Style::default().fg(Color::White)),
            ]);
            let style = if i == app.header_cursor {
                Style::default().bg(Color::Indexed(237)).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        }).collect()
    };

    let count = app.request_headers.len();
    let title = if count == 0 {
        " Headers ".to_string()
    } else {
        format!(" Headers ({}) ", count)
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(list, area);
}

fn render_response(frame: &mut Frame, app: &App, area: Rect) {
    let title = if app.request_loading {
        Line::from(vec![
            Span::raw(" "),
            Span::styled("⟳ sending…", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  r: cycle view  -/=: resize "),
        ])
    } else {
        let style_for = |v: &ResponseView| {
            if *v == app.response_view {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Indexed(244))
            }
        };
        let sep = Span::styled(" · ", Style::default().fg(Color::DarkGray));
        let mut spans = vec![
            Span::raw(" "),
            Span::styled("JSON", style_for(&ResponseView::Json)),
            sep.clone(),
            Span::styled("Raw", style_for(&ResponseView::Raw)),
            sep.clone(),
            Span::styled("HTTP", style_for(&ResponseView::Http)),
        ];
        if let Some(status) = app.response_status {
            let status_color = match status {
                200..=299 => Color::Green,
                300..=399 => Color::Cyan,
                400..=499 => Color::Yellow,
                _         => Color::Red,
            };
            spans.push(Span::styled("  ·  ", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                format!("{}", status),
                Style::default().fg(status_color).add_modifier(Modifier::BOLD),
            ));
            if let Some(ms) = app.response_elapsed_ms {
                spans.push(Span::styled(
                    format!("  {}ms", ms),
                    Style::default().fg(Color::Indexed(244)),
                ));
            }
        }
        spans.push(Span::raw("  r: cycle  -/=: resize "));
        Line::from(spans)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    match app.response_view {
        ResponseView::Json => render_response_json(frame, app, inner),
        ResponseView::Raw  => render_response_raw(frame, app, inner),
        ResponseView::Http => render_response_http(frame, app, inner),
    }
}

fn render_response_json(frame: &mut Frame, app: &App, area: Rect) {
    let json_rows = match &app.response_body {
        Some(json) => json_highlight::rows(json, &app.response_folds),
        None => vec![],
    };

    let rows: Vec<Row> = json_rows.iter().map(|r| {
        let indent = "  ".repeat(r.depth);
        let icon = match r.fold_path {
            Some(_) if r.is_folded => "▶ ",
            Some(_) => "▼ ",
            None => "  ",
        };
        let key_color = match r.value_type {
            ValueType::Object | ValueType::Array => Color::Cyan,
            _ => Color::White,
        };
        let key_cell = Cell::from(Line::from(vec![
            Span::raw(format!("{}{}", indent, icon)),
            Span::styled(r.key.clone(), Style::default().fg(key_color)),
        ]));
        let (type_color, type_label) = match r.value_type {
            ValueType::Object  => (Color::Cyan,    "Object "),
            ValueType::Array   => (Color::Blue,    "Array  "),
            ValueType::Str     => (Color::Green,   "String "),
            ValueType::Number  => (Color::Yellow,  "Number "),
            ValueType::Boolean => (Color::Magenta, "Boolean"),
            ValueType::Null    => (Color::DarkGray,"Null   "),
        };
        let value_color = match r.value_type {
            ValueType::Object | ValueType::Array => Color::White,
            ValueType::Str     => Color::Green,
            ValueType::Number  => Color::Yellow,
            ValueType::Boolean => Color::Magenta,
            ValueType::Null    => Color::DarkGray,
        };
        Row::new(vec![
            key_cell,
            Cell::from(Span::styled(type_label, Style::default().fg(type_color))),
            Cell::from(Span::styled(r.value_preview.clone(), Style::default().fg(value_color))),
        ])
    }).collect();

    let header = Row::new(vec![
        Cell::from(Span::styled("Key",   Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Type",  Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Value", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
    ])
    .style(Style::default().bg(Color::Indexed(236)))
    .height(1);

    let widths = [
        Constraint::Length(app.key_col_width),
        Constraint::Length(8),
        Constraint::Min(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(Style::default().bg(Color::Indexed(237)))
        .column_spacing(1);

    let mut state = TableState::default()
        .with_selected(Some(app.response_cursor))
        .with_offset(app.response_scroll as usize);

    frame.render_stateful_widget(table, area, &mut state);
}

fn render_response_raw(frame: &mut Frame, app: &App, area: Rect) {
    let text = app.response_body.as_deref().unwrap_or("No response.");
    let para = Paragraph::new(text)
        .style(Style::default().fg(Color::White))
        .scroll((app.response_scroll, 0));
    frame.render_widget(para, area);
}

fn render_response_http(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    let sep_style    = Style::default().fg(Color::Indexed(238));
    let header_key   = Style::default().fg(Color::Yellow);
    let header_val   = Style::default().fg(Color::White);
    let body_style   = Style::default().fg(Color::White);
    let hint_style   = Style::default().fg(Color::Indexed(244));

    // ── Request ───────────────────────────────────────────────────────────
    lines.push(Line::from(Span::styled("── Request ──────────────────────────────────────────", sep_style)));

    match &app.last_request_raw {
        None => {
            lines.push(Line::from(Span::styled("No request sent yet.", hint_style)));
        }
        Some(req) => {
            // Extract host and path from URL with simple string splitting
            let (path_query, host_str) = {
                let url = &req.url;
                // Strip scheme (https:// or http://)
                let after_scheme = url.find("://").map(|i| &url[i+3..]).unwrap_or(url);
                let slash_pos = after_scheme.find('/').unwrap_or(after_scheme.len());
                let host = &after_scheme[..slash_pos];
                let path = if slash_pos < after_scheme.len() {
                    after_scheme[slash_pos..].to_string()
                } else {
                    "/".to_string()
                };
                (path, host.to_string())
            };

            lines.push(Line::from(vec![
                Span::styled(format!("{} ", req.method), Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                Span::styled(path_query, header_val),
                Span::styled(" HTTP/1.1", hint_style),
            ]));
            if !host_str.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("Host: ", header_key),
                    Span::styled(host_str, header_val),
                ]));
            }
            for (k, v) in &req.headers {
                lines.push(Line::from(vec![
                    Span::styled(format!("{}: ", k), header_key),
                    Span::styled(v.clone(), header_val),
                ]));
            }
            if let Some(body) = &req.body {
                lines.push(Line::from(vec![
                    Span::styled("Content-Length: ", header_key),
                    Span::styled(body.len().to_string(), header_val),
                ]));
                lines.push(Line::from(Span::raw("")));
                for l in body.lines() {
                    lines.push(Line::from(Span::styled(l.to_string(), body_style)));
                }
            } else {
                lines.push(Line::from(Span::raw("")));
            }
        }
    }

    lines.push(Line::from(Span::raw("")));

    // ── Response ──────────────────────────────────────────────────────────
    lines.push(Line::from(Span::styled("── Response ─────────────────────────────────────────", sep_style)));

    match app.response_status {
        None => {
            lines.push(Line::from(Span::styled("No response yet.", hint_style)));
        }
        Some(status) => {
            let status_color = match status {
                200..=299 => Color::Green,
                300..=399 => Color::Cyan,
                400..=499 => Color::Yellow,
                _         => Color::Red,
            };
            let reason = http_reason(status);
            lines.push(Line::from(vec![
                Span::styled("HTTP/1.1 ", hint_style),
                Span::styled(
                    format!("{} {}", status, reason),
                    Style::default().fg(status_color).add_modifier(Modifier::BOLD),
                ),
            ]));
            for (k, v) in &app.response_headers {
                lines.push(Line::from(vec![
                    Span::styled(format!("{}: ", k), header_key),
                    Span::styled(v.clone(), header_val),
                ]));
            }
            lines.push(Line::from(Span::raw("")));
            let body = app.response_body.as_deref().unwrap_or("");
            for l in body.lines() {
                lines.push(Line::from(Span::styled(l.to_string(), body_style)));
            }
        }
    }

    frame.render_widget(Paragraph::new(lines).scroll((app.response_scroll, 0)), area);
}

fn http_reason(status: u16) -> &'static str {
    match status {
        200 => "OK", 201 => "Created", 202 => "Accepted", 204 => "No Content",
        301 => "Moved Permanently", 302 => "Found", 304 => "Not Modified",
        400 => "Bad Request", 401 => "Unauthorized", 403 => "Forbidden",
        404 => "Not Found", 405 => "Method Not Allowed", 409 => "Conflict",
        422 => "Unprocessable Entity", 429 => "Too Many Requests",
        500 => "Internal Server Error", 502 => "Bad Gateway",
        503 => "Service Unavailable", 504 => "Gateway Timeout",
        _ => "",
    }
}

// ── Collections panel ────────────────────────────────────────────────────────

fn render_collections_panel(frame: &mut Frame, app: &App, area: Rect) {
    let flat = flatten_stored(&app.stored_collections, &app.expanded_nodes);

    let items: Vec<ListItem> = if flat.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  No collections — press n to create one",
            Style::default().fg(Color::Gray),
        )))]
    } else {
        flat.iter().enumerate().map(|(i, node)| {
            let indent = "  ".repeat(node.depth);
            let icon = if node.is_folder {
                if node.expanded { "▼ " } else { "▶ " }
            } else {
                "  "
            };
            let line = if node.is_folder {
                Line::from(vec![
                    Span::raw(format!("{indent}{icon}")),
                    Span::styled(node.name.clone(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ])
            } else {
                let method = node.method.as_deref().unwrap_or("GET");
                Line::from(vec![
                    Span::raw(format!("{indent}{icon}")),
                    Span::styled(format!("{method:<7}"), Style::default().fg(method_color(method))),
                    Span::styled(node.name.clone(), Style::default().fg(Color::White)),
                ])
            };
            let style = if i == app.collection_cursor {
                Style::default().bg(Color::Indexed(237)).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        }).collect()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Collections ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(list, area);
}

// ── Env panel ────────────────────────────────────────────────────────────────

fn render_env_panel(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    render_env_list(frame, app, chunks[0]);
    render_env_vars(frame, app, chunks[1]);
}

fn render_env_list(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.env_focus == EnvFocus::Envs;
    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Indexed(238))
    };

    let items: Vec<ListItem> = if app.environments.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  No environments",
            Style::default().fg(Color::Gray),
        ))),
        ListItem::new(Line::from(Span::styled(
            "  Press n to create one",
            Style::default().fg(Color::Gray),
        )))]
    } else {
        app.environments.iter().enumerate().map(|(i, env)| {
            let active = app.active_env_idx == Some(i);
            let indicator = if active { "● " } else { "  " };
            let line = Line::from(vec![
                Span::styled(indicator, Style::default().fg(Color::Green)),
                Span::styled(env.env.name.clone(), Style::default().fg(if active { Color::Green } else { Color::White })),
            ]);
            let style = if i == app.env_cursor && focused {
                Style::default().bg(Color::Indexed(237)).add_modifier(Modifier::BOLD)
            } else if i == app.env_cursor {
                Style::default().bg(Color::Indexed(235))
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        }).collect()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Environments ")
            .border_style(border_style),
    );
    frame.render_widget(list, area);
}

fn render_env_vars(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.env_focus == EnvFocus::Vars;
    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Indexed(238))
    };

    let Some(env) = app.environments.get(app.env_cursor) else {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Variables ")
            .border_style(border_style);
        frame.render_widget(
            Paragraph::new("Select an environment.")
                .block(block)
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center),
            area,
        );
        return;
    };

    let vars = sorted_vars(env);

    let items: Vec<ListItem> = if vars.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  No variables — press a to add one",
            Style::default().fg(Color::Gray),
        )))]
    } else {
        vars.iter().enumerate().map(|(i, (k, v))| {
            let line = Line::from(vec![
                Span::styled(format!("  {:<22}", k), Style::default().fg(Color::Cyan)),
                Span::styled("= ", Style::default().fg(Color::DarkGray)),
                Span::styled(v.clone(), Style::default().fg(Color::Green)),
            ]);
            let style = if i == app.env_var_cursor && focused {
                Style::default().bg(Color::Indexed(237)).add_modifier(Modifier::BOLD)
            } else if i == app.env_var_cursor {
                Style::default().bg(Color::Indexed(235))
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        }).collect()
    };

    let title = format!(" {} — Variables ", env.env.name);
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    );
    frame.render_widget(list, area);
}

// ── Modals ───────────────────────────────────────────────────────────────────

fn render_modal(frame: &mut Frame, app: &App) {
    match &app.modal {
        Some(ModalState::HeaderPicker { cursor }) => {
            let total = COMMON_HEADERS.len() + 1;
            let inner_h = total as u16;
            let area = centered_rect(58, inner_h + 4, frame.area());
            frame.render_widget(Clear, area);

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Add header ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Cyan));
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let hint_area = Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };
            frame.render_widget(
                Paragraph::new("↑/↓: navigate  Enter: select  Esc: cancel")
                    .style(Style::default().fg(Color::Indexed(244))),
                hint_area,
            );

            let list_area = Rect { x: inner.x, y: inner.y, width: inner.width, height: inner.height.saturating_sub(1) };

            let mut items: Vec<ListItem> = COMMON_HEADERS.iter().enumerate().map(|(i, (name, default_val))| {
                let selected = i == *cursor;
                let style = if selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };
                let val_style = if selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Indexed(244))
                };
                let pad = 22usize.saturating_sub(name.len());
                let preview: String = default_val.chars().take(20).collect();
                let line = Line::from(vec![
                    Span::styled(format!(" {}{} ", name, " ".repeat(pad)), style),
                    Span::styled(preview, val_style),
                ]);
                ListItem::new(line)
            }).collect();

            // Custom... entry
            let custom_selected = *cursor == COMMON_HEADERS.len();
            let custom_style = if custom_selected {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::Yellow)
            };
            items.push(ListItem::new(Line::from(Span::styled(" Custom…", custom_style))));

            frame.render_widget(List::new(items), list_area);
        }
        Some(ModalState::ContentTypePicker { cursor }) => {
            let total = COMMON_CONTENT_TYPES.len() + 1;
            let area = centered_rect(52, total as u16 + 4, frame.area());
            frame.render_widget(Clear, area);

            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Content-Type ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Cyan));
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let hint_area = Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };
            frame.render_widget(
                Paragraph::new("↑/↓: navigate  Enter: select  Esc: back")
                    .style(Style::default().fg(Color::Indexed(244))),
                hint_area,
            );

            let list_area = Rect { x: inner.x, y: inner.y, width: inner.width, height: inner.height.saturating_sub(1) };

            let mut items: Vec<ListItem> = COMMON_CONTENT_TYPES.iter().enumerate().map(|(i, ct)| {
                let selected = i == *cursor;
                let style = if selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(Line::from(Span::styled(format!(" {} ", ct), style)))
            }).collect();

            let custom_selected = *cursor == COMMON_CONTENT_TYPES.len();
            items.push(ListItem::new(Line::from(Span::styled(
                " Custom…",
                if custom_selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Yellow)
                },
            ))));

            frame.render_widget(List::new(items), list_area);
        }
        Some(ModalState::NewCollection { input }) => {
            let area = centered_rect(52, 7, frame.area());
            frame.render_widget(Clear, area);
            let text = vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Name: "),
                    Span::styled(format!("{}_", input), Style::default().fg(Color::Yellow)),
                ]),
                Line::from(""),
                Line::from(Span::styled("  Enter: save   Esc: cancel", Style::default().fg(Color::Gray))),
            ];
            frame.render_widget(
                Paragraph::new(text).block(
                    Block::default().borders(Borders::ALL)
                        .title(" New Collection ").title_alignment(Alignment::Center)
                        .border_style(Style::default().fg(Color::Cyan)),
                ),
                area,
            );
        }

        Some(ModalState::NewFolder { input, .. }) => {
            let area = centered_rect(52, 7, frame.area());
            frame.render_widget(Clear, area);
            let text = vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Name: "),
                    Span::styled(format!("{}_", input), Style::default().fg(Color::Cyan)),
                ]),
                Line::from(""),
                Line::from(Span::styled("  Enter: save   Esc: cancel", Style::default().fg(Color::Gray))),
            ];
            frame.render_widget(
                Paragraph::new(text).block(
                    Block::default().borders(Borders::ALL)
                        .title(" New Folder ").title_alignment(Alignment::Center)
                        .border_style(Style::default().fg(Color::Cyan)),
                ),
                area,
            );
        }

        Some(ModalState::NewRequest { name, method_idx, url, active_field, .. }) => {
            let area = centered_rect(60, 11, frame.area());
            frame.render_widget(Clear, area);

            let name_style = if *active_field == InputField::Name { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::White) };
            let url_style  = if *active_field == InputField::Url  { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::White) };
            let name_cursor = if *active_field == InputField::Name { "_" } else { "" };
            let url_cursor  = if *active_field == InputField::Url  { "_" } else { "" };

            let method = METHODS[*method_idx];
            let max_url = 44usize;
            let url_display = if url.len() > max_url {
                format!("…{}", &url[url.len() - max_url..])
            } else {
                url.clone()
            };

            let text = vec![
                Line::from(""),
                Line::from(vec![Span::raw("  Name:   "), Span::styled(format!("{}{}", name, name_cursor), name_style)]),
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Method: "),
                    Span::styled("◀ ", Style::default().fg(Color::DarkGray)),
                    Span::styled(method, Style::default().fg(method_color(method)).add_modifier(Modifier::BOLD)),
                    Span::styled(" ▶", Style::default().fg(Color::DarkGray)),
                    Span::styled("  (←/→ to change)", Style::default().fg(Color::Gray)),
                ]),
                Line::from(""),
                Line::from(vec![Span::raw("  URL:    "), Span::styled(format!("{}{}", url_display, url_cursor), url_style)]),
                Line::from(""),
                Line::from(Span::styled("  Tab: next field   Enter: save   Esc: cancel", Style::default().fg(Color::Gray))),
            ];
            frame.render_widget(
                Paragraph::new(text).block(
                    Block::default().borders(Borders::ALL)
                        .title(" New Request ").title_alignment(Alignment::Center)
                        .border_style(Style::default().fg(Color::Yellow)),
                ),
                area,
            );
        }

        Some(ModalState::NewEnv { input }) => {
            let area = centered_rect(52, 7, frame.area());
            frame.render_widget(Clear, area);
            let text = vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Name: "),
                    Span::styled(format!("{}_", input), Style::default().fg(Color::Yellow)),
                ]),
                Line::from(""),
                Line::from(Span::styled("  Enter: save   Esc: cancel", Style::default().fg(Color::Gray))),
            ];
            frame.render_widget(
                Paragraph::new(text).block(
                    Block::default().borders(Borders::ALL)
                        .title(" New Environment ").title_alignment(Alignment::Center)
                        .border_style(Style::default().fg(Color::Yellow)),
                ),
                area,
            );
        }

        Some(ModalState::NewVar { key, value, active_field, .. }) => {
            let area = centered_rect(60, 9, frame.area());
            frame.render_widget(Clear, area);

            let key_style   = if *active_field == VarField::Key   { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::White) };
            let val_style   = if *active_field == VarField::Value { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::White) };
            let key_cursor  = if *active_field == VarField::Key   { "_" } else { "" };
            let val_cursor  = if *active_field == VarField::Value { "_" } else { "" };

            let text = vec![
                Line::from(""),
                Line::from(vec![Span::raw("  Key:   "), Span::styled(format!("{}{}", key, key_cursor), key_style)]),
                Line::from(""),
                Line::from(vec![Span::raw("  Value: "), Span::styled(format!("{}{}", value, val_cursor), val_style)]),
                Line::from(""),
                Line::from(Span::styled("  Tab: next field   Enter: save   Esc: cancel", Style::default().fg(Color::Gray))),
            ];
            frame.render_widget(
                Paragraph::new(text).block(
                    Block::default().borders(Borders::ALL)
                        .title(" New Variable ").title_alignment(Alignment::Center)
                        .border_style(Style::default().fg(Color::Yellow)),
                ),
                area,
            );
        }

        Some(ModalState::NewHeader { key, value, active_field }) => {
            let area = centered_rect(64, 9, frame.area());
            frame.render_widget(Clear, area);

            let key_style   = if *active_field == VarField::Key   { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::White) };
            let val_style   = if *active_field == VarField::Value { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::White) };
            let key_cursor  = if *active_field == VarField::Key   { "_" } else { "" };
            let val_cursor  = if *active_field == VarField::Value { "_" } else { "" };

            let text = vec![
                Line::from(""),
                Line::from(vec![Span::raw("  Key:   "), Span::styled(format!("{}{}", key, key_cursor), key_style)]),
                Line::from(""),
                Line::from(vec![Span::raw("  Value: "), Span::styled(format!("{}{}", value, val_cursor), val_style)]),
                Line::from(""),
                Line::from(Span::styled("  Tab: next field   Enter: add   Esc: cancel", Style::default().fg(Color::Gray))),
            ];
            frame.render_widget(
                Paragraph::new(text).block(
                    Block::default().borders(Borders::ALL)
                        .title(" New Header ").title_alignment(Alignment::Center)
                        .border_style(Style::default().fg(Color::Cyan)),
                ),
                area,
            );
        }

        Some(ModalState::UrlParam { key, value, active_field, edit_idx }) => {
            let area = centered_rect(64, 9, frame.area());
            frame.render_widget(Clear, area);
            let key_style  = if *active_field == VarField::Key   { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::White) };
            let val_style  = if *active_field == VarField::Value { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::White) };
            let key_cursor = if *active_field == VarField::Key   { "_" } else { "" };
            let val_cursor = if *active_field == VarField::Value { "_" } else { "" };
            let modal_title = if edit_idx.is_some() { " Edit Param " } else { " Add Param " };
            let text = vec![
                Line::from(""),
                Line::from(vec![Span::raw("  Key:   "), Span::styled(format!("{}{}", key, key_cursor), key_style)]),
                Line::from(""),
                Line::from(vec![Span::raw("  Value: "), Span::styled(format!("{}{}", value, val_cursor), val_style)]),
                Line::from(""),
                Line::from(Span::styled("  Tab: next field   Enter: save   Esc: cancel", Style::default().fg(Color::Gray))),
            ];
            frame.render_widget(
                Paragraph::new(text).block(
                    Block::default().borders(Borders::ALL)
                        .title(modal_title).title_alignment(Alignment::Center)
                        .border_style(Style::default().fg(Color::Cyan)),
                ),
                area,
            );
        }

        Some(ModalState::BodyPair { key, value, active_field, edit_idx }) => {
            let area = centered_rect(64, 9, frame.area());
            frame.render_widget(Clear, area);
            let key_style  = if *active_field == VarField::Key   { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::White) };
            let val_style  = if *active_field == VarField::Value { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::White) };
            let key_cursor = if *active_field == VarField::Key   { "_" } else { "" };
            let val_cursor = if *active_field == VarField::Value { "_" } else { "" };
            let modal_title = if edit_idx.is_some() { " Edit Field " } else { " Add Field " };
            let text = vec![
                Line::from(""),
                Line::from(vec![Span::raw("  Key:   "), Span::styled(format!("{}{}", key, key_cursor), key_style)]),
                Line::from(""),
                Line::from(vec![Span::raw("  Value: "), Span::styled(format!("{}{}", value, val_cursor), val_style)]),
                Line::from(""),
                Line::from(Span::styled("  Tab: next field   Enter: save   Esc: cancel", Style::default().fg(Color::Gray))),
            ];
            frame.render_widget(
                Paragraph::new(text).block(
                    Block::default().borders(Borders::ALL)
                        .title(modal_title).title_alignment(Alignment::Center)
                        .border_style(Style::default().fg(Color::Yellow)),
                ),
                area,
            );
        }

        Some(ModalState::SaveRequest { name, collection_idx, folder_display_idx, active_field }) => {
            let area = centered_rect(66, 13, frame.area());
            frame.render_widget(Clear, area);

            let name_style = if *active_field == SaveField::Name {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };
            let name_cursor = if *active_field == SaveField::Name { "_" } else { "" };

            let col_name = app.stored_collections
                .get(*collection_idx)
                .map(|c| c.collection.name.as_str())
                .unwrap_or("—");
            let n_cols = app.stored_collections.len();
            let col_style = if *active_field == SaveField::Collection {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let col_nav = if *active_field == SaveField::Collection && n_cols > 1 {
                format!("↑ {} ↓  ({}/{})", col_name, collection_idx + 1, n_cols)
            } else {
                col_name.to_string()
            };

            let n_folders = app.stored_collections
                .get(*collection_idx)
                .map_or(0, |c| c.folders.len());
            let folder_label = if *folder_display_idx == 0 {
                "(root)".to_string()
            } else {
                app.stored_collections[*collection_idx]
                    .folders
                    .get(folder_display_idx - 1)
                    .map(|f| f.name.clone())
                    .unwrap_or_else(|| "(root)".to_string())
            };
            let folder_style = if *active_field == SaveField::Folder {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let folder_nav = if *active_field == SaveField::Folder {
                format!("↑ {} ↓  ({}/{})", folder_label, folder_display_idx, n_folders + 1)
            } else {
                folder_label.clone()
            };

            let is_edit_mode = app.editing_request_origin.is_some();
            let hint = if is_edit_mode {
                "  Tab: next field   ↑/↓: navigate   Enter: save   Esc: cancel  (change location → save as new)"
            } else {
                "  Tab: next field   ↑/↓: navigate   Enter: save   Esc: cancel"
            };
            let modal_title = if is_edit_mode { " Update Request " } else { " Save Request " };

            let text = vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Name:        "),
                    Span::styled(format!("{}{}", name, name_cursor), name_style),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Collection:  "),
                    Span::styled(col_nav, col_style),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Folder:      "),
                    Span::styled(folder_nav, folder_style),
                ]),
                Line::from(""),
                Line::from(Span::styled(hint, Style::default().fg(Color::Gray))),
            ];
            frame.render_widget(
                Paragraph::new(text).block(
                    Block::default().borders(Borders::ALL)
                        .title(modal_title).title_alignment(Alignment::Center)
                        .border_style(Style::default().fg(Color::Green)),
                ),
                area,
            );
        }

        Some(ModalState::ConfirmDelete { label, .. }) => {
            let area = centered_rect(52, 7, frame.area());
            frame.render_widget(Clear, area);
            let text = vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Delete "),
                    Span::styled(format!("\"{}\"", label), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                    Span::raw("?"),
                ]),
                Line::from(""),
                Line::from(Span::styled("  y / Enter: confirm   n / Esc: cancel", Style::default().fg(Color::Gray))),
            ];
            frame.render_widget(
                Paragraph::new(text).block(
                    Block::default().borders(Borders::ALL)
                        .title(" Delete ").title_alignment(Alignment::Center)
                        .border_style(Style::default().fg(Color::Red)),
                ),
                area,
            );
        }

        Some(ModalState::EditAuthField { kind, value }) => {
            let area = centered_rect(60, 7, frame.area());
            frame.render_widget(Clear, area);
            let label = kind.label();
            let display = if kind == &crate::app::AuthFieldKind::BasicPassword && !value.is_empty() {
                "•".repeat(value.len())
            } else {
                value.clone()
            };
            let text = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled(format!("  {}:  ", label), Style::default().fg(Color::Gray)),
                    Span::styled(&display, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled("█", Style::default().fg(Color::Yellow)),
                ]),
                Line::from(""),
                Line::from(Span::styled("  Enter: confirm   Esc: cancel", Style::default().fg(Color::Gray))),
            ];
            frame.render_widget(
                Paragraph::new(text).block(
                    Block::default().borders(Borders::ALL)
                        .title(format!(" Edit {} ", label)).title_alignment(Alignment::Center)
                        .border_style(Style::default().fg(Color::Cyan)),
                ),
                area,
            );
        }

        None => {}
    }
}

// ── History panel ────────────────────────────────────────────────────────────

fn render_history_panel(frame: &mut Frame, app: &App, area: Rect) {
    if app.history.is_empty() {
        render_placeholder(frame, area, "History", "No requests yet — send one to start recording history.");
        return;
    }

    let items: Vec<ListItem> = app.history.iter().enumerate().map(|(i, entry)| {
        let ts = crate::storage::format_timestamp(entry.timestamp_secs);
        let status_str = match entry.status {
            Some(s) => format!("{}", s),
            None => "ERR".to_string(),
        };
        let status_color = match entry.status {
            Some(s) if s < 300 => Color::Green,
            Some(s) if s < 500 => Color::Yellow,
            Some(_) => Color::Red,
            None => Color::Gray,
        };
        let elapsed_str = match entry.elapsed_ms {
            Some(ms) => format!("  {}ms", ms),
            None => String::new(),
        };
        let method_col = method_color(&entry.method);
        let url_max = area.width.saturating_sub(42) as usize;
        let url_display = if entry.url.len() > url_max {
            format!("{}…", &entry.url[..url_max.saturating_sub(1)])
        } else {
            entry.url.clone()
        };

        let selected = i == app.history_cursor;
        let bg = if selected { Color::Indexed(236) } else { Color::Reset };

        let line = Line::from(vec![
            Span::styled(format!("  {}", ts), Style::default().fg(Color::DarkGray).bg(bg)),
            Span::styled("  ", Style::default().bg(bg)),
            Span::styled(format!("{:<6}", entry.method), Style::default().fg(method_col).add_modifier(Modifier::BOLD).bg(bg)),
            Span::styled(format!("{:<3}", status_str), Style::default().fg(status_color).bg(bg)),
            Span::styled(format!("{:<7}", elapsed_str), Style::default().fg(Color::DarkGray).bg(bg)),
            Span::styled(format!("  {}", url_display), Style::default().fg(if selected { Color::White } else { Color::Gray }).bg(bg)),
        ]);
        ListItem::new(line)
    }).collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" History ({}) ", app.history.len()))
                .border_style(Style::default().fg(Color::Cyan)),
        );

    frame.render_widget(list, area);
}

// ── Shared helpers ───────────────────────────────────────────────────────────

fn render_placeholder(frame: &mut Frame, area: Rect, title: &str, msg: &str) {
    let widget = Paragraph::new(msg)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {title} "))
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    frame.render_widget(widget, area);
}

fn render_status(frame: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // ── Context bar (top row) ──────────────────────────────────────────────
    let ctx_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(32)])
        .split(rows[0]);

    let breadcrumb = context_breadcrumb(app);
    frame.render_widget(
        Paragraph::new(breadcrumb).style(Style::default().fg(Color::Cyan)),
        ctx_cols[0],
    );

    let (env_str, env_color) = env_indicator(app);
    frame.render_widget(
        Paragraph::new(env_str)
            .style(Style::default().fg(env_color))
            .alignment(Alignment::Right),
        ctx_cols[1],
    );

    // ── Hints bar (bottom row) ─────────────────────────────────────────────
    frame.render_widget(
        Paragraph::new(app.status_message.as_str()).style(Style::default().fg(Color::Gray)),
        rows[1],
    );
}

fn context_breadcrumb(app: &App) -> String {
    match &app.active_tab {
        Tab::Request => {
            let sub = app.active_request_tab.title();
            let mode_suffix = if app.active_request_tab == RequestTab::Body {
                match app.body_mode {
                    BodyMode::Text => "  ›  Text",
                    BodyMode::Json => "  ›  JSON",
                }
            } else {
                ""
            };
            let focus_suffix = match app.request_focus {
                RequestFocus::Url => "  ›  URL edit",
                RequestFocus::Body | RequestFocus::Description => "  ›  editing",
                RequestFocus::Response => "",
            };
            format!("Request  ›  {}{}{}", sub, mode_suffix, focus_suffix)
        }
        Tab::Collections => "Collections".to_string(),
        Tab::Env => match app.env_focus {
            EnvFocus::Envs => "Env  ›  Environments".to_string(),
            EnvFocus::Vars => "Env  ›  Variables".to_string(),
        },
        Tab::History => "History".to_string(),
    }
}

fn env_indicator(app: &App) -> (String, Color) {
    match app.active_env_idx.and_then(|i| app.environments.get(i)) {
        Some(env) => (format!("● env: {}", env.env.name), Color::Green),
        None => ("○ no active env".to_string(), Color::Indexed(238)),
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect { x, y, width: width.min(area.width), height: height.min(area.height) }
}

fn render_var_picker(frame: &mut Frame, app: &App) {
    let Some(picker) = &app.var_picker else { return };
    let vars = app.filtered_var_names(&picker.prefix);

    let inner_h = (vars.len().max(1) as u16).min(10);
    let total_h = inner_h + 4; // border (2) + title (1) + hint (1)
    let width: u16 = 44;
    let area = centered_rect(width, total_h, frame.area());

    let title = if picker.prefix.is_empty() {
        " Insert variable ".to_string()
    } else {
        format!(" Insert variable · filter: {} ", picker.prefix)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    // Hint line at the bottom
    let hint_area = Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };
    let hint = Paragraph::new("↑/↓: navigate  Enter: insert  Esc: cancel")
        .style(Style::default().fg(Color::Indexed(244)));
    frame.render_widget(hint, hint_area);

    // Var list
    let list_area = Rect { x: inner.x, y: inner.y, width: inner.width, height: inner.height.saturating_sub(1) };

    if vars.is_empty() {
        let no_match = Paragraph::new("No matching variables")
            .style(Style::default().fg(Color::Indexed(244)));
        frame.render_widget(no_match, list_area);
        return;
    }

    let items: Vec<ListItem> = vars.iter().enumerate().map(|(i, name)| {
        let style = if i == picker.cursor {
            Style::default().fg(Color::Black).bg(Color::Cyan)
        } else {
            Style::default().fg(Color::White)
        };
        let env_val = app.active_env_vars()
            .into_iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v)
            .unwrap_or_default();
        let preview: String = env_val.chars().take(16).collect();
        let label = if env_val.is_empty() {
            format!("{{{{{}}}}}  ", name)
        } else {
            format!("{{{{{}}}}}  = {}", name, preview)
        };
        ListItem::new(label).style(style)
    }).collect();

    let list = List::new(items);
    frame.render_widget(list, list_area);
}

fn method_color(method: &str) -> Color {
    match method {
        "GET"    => Color::Green,
        "POST"   => Color::Blue,
        "PUT"    => Color::Yellow,
        "PATCH"  => Color::Magenta,
        "DELETE" => Color::Red,
        _        => Color::White,
    }
}
