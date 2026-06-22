use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, TableState, Tabs},
    Frame,
};

use crate::app::{
    flatten_stored, sorted_vars, App, BodyMode, EnvFocus, InputField, ModalState, RequestFocus,
    RequestTab, ResponseView, Tab, VarField, METHODS,
};
use crate::json_highlight::{self, ValueType};

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    render_tabs(frame, app, chunks[0]);
    render_body(frame, app, chunks[1]);
    render_status(frame, app, chunks[2]);

    if app.modal.is_some() {
        render_modal(frame, app);
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
        Tab::History => render_placeholder(frame, area, "History", "Recent requests will appear here."),
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
            Constraint::Fill(1),
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

    let (title, msg) = match app.active_request_tab {
        RequestTab::Description => ("Description", "Add a description for this request."),
        RequestTab::Headers | RequestTab::Body => unreachable!(),
        RequestTab::UrlParams => ("URL Params", "Add query parameters (key=value)."),
        RequestTab::Auth => ("Auth", "Configure authentication (Bearer, API Key, OAuth2…)."),
        RequestTab::Options => ("Options", "Timeout, redirects, SSL verification…"),
    };

    let content = Paragraph::new(msg)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {title} "))
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);

    frame.render_widget(content, area);
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
    let json_active = app.response_view == ResponseView::Json;

    let json_style = if json_active {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Indexed(244))
    };
    let raw_style = if !json_active {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Indexed(244))
    };

    let title = if app.request_loading {
        Line::from(vec![
            Span::raw(" "),
            Span::styled("⟳ sending…", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  r: toggle  -/=: resize "),
        ])
    } else {
        let mut spans = vec![
            Span::raw(" "),
            Span::styled("JSON", json_style),
            Span::styled(" · ", Style::default().fg(Color::DarkGray)),
            Span::styled("Raw", raw_style),
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
        spans.push(Span::raw("  r: toggle  -/=: resize "));
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
        ResponseView::Raw => render_response_raw(frame, app, inner),
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

        None => {}
    }
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
    let status = Paragraph::new(app.status_message.as_str())
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(status, area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect { x, y, width: width.min(area.width), height: height.min(area.height) }
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
