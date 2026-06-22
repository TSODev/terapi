use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, TableState, Tabs},
    Frame,
};

use crate::app::{flatten_stored, App, InputField, ModalState, RequestTab, ResponseView, Tab, METHODS};
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
    let tabs: Vec<Line> = Tab::all()
        .into_iter()
        .map(|t| Line::from(t.title()))
        .collect();

    let selected = Tab::all()
        .into_iter()
        .position(|t| t == app.active_tab)
        .unwrap_or(0);

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
        Tab::History => render_placeholder(frame, area, "History", "Recent requests will appear here."),
    }
}

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

    let url_bar = Paragraph::new("GET  https://")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" URL ")
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .style(Style::default().fg(Color::White));
    frame.render_widget(url_bar, chunks[0]);

    render_request_subtabs(frame, app, chunks[1]);
    render_request_content(frame, app, chunks[2]);
    render_response(frame, app, chunks[3]);
}

fn render_request_subtabs(frame: &mut Frame, app: &App, area: Rect) {
    let tabs: Vec<Line> = RequestTab::all()
        .into_iter()
        .map(|t| Line::from(t.title()))
        .collect();

    let selected = RequestTab::all()
        .into_iter()
        .position(|t| t == app.active_request_tab)
        .unwrap_or(0);

    let sub_tabs = Tabs::new(tabs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .select(selected)
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .divider(Span::raw(" | "));

    frame.render_widget(sub_tabs, area);
}

fn render_request_content(frame: &mut Frame, app: &App, area: Rect) {
    let (title, msg) = match app.active_request_tab {
        RequestTab::Description => ("Description", "Add a description for this request."),
        RequestTab::Headers => ("Headers", "Add request headers (key: value)."),
        RequestTab::UrlParams => ("URL Params", "Add query parameters (key=value)."),
        RequestTab::Body => ("Body", "Enter the raw JSON body."),
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
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

    frame.render_widget(content, area);
}

fn render_response(frame: &mut Frame, app: &App, area: Rect) {
    let json_active = app.response_view == ResponseView::Json;

    let json_style = if json_active {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let raw_style = if !json_active {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled("JSON", json_style),
        Span::styled(" · ", Style::default().fg(Color::DarkGray)),
        Span::styled("Raw", raw_style),
        Span::raw("  r: toggle  -/=: resize "),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    match app.response_view {
        ResponseView::Json => render_response_json(frame, app, inner),
        ResponseView::Raw  => render_response_raw(frame, app, inner),
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

fn render_collections_panel(frame: &mut Frame, app: &App, area: Rect) {
    let flat = flatten_stored(&app.stored_collections, &app.expanded_nodes);

    let items: Vec<ListItem> = if flat.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  No collections — press n to create one",
            Style::default().fg(Color::DarkGray),
        )))]
    } else {
        flat.iter()
            .enumerate()
            .map(|(i, node)| {
                let indent = "  ".repeat(node.depth);
                let icon = if node.is_folder {
                    if node.expanded { "▼ " } else { "▶ " }
                } else {
                    "  "
                };

                let line = if node.is_folder {
                    Line::from(vec![
                        Span::raw(format!("{indent}{icon}")),
                        Span::styled(
                            node.name.clone(),
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        ),
                    ])
                } else {
                    let method = node.method.as_deref().unwrap_or("GET");
                    let method_color = method_color(method);
                    Line::from(vec![
                        Span::raw(format!("{indent}{icon}")),
                        Span::styled(format!("{method:<7}"), Style::default().fg(method_color)),
                        Span::styled(node.name.clone(), Style::default().fg(Color::White)),
                    ])
                };

                let style = if i == app.collection_cursor {
                    Style::default().bg(Color::Indexed(237)).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                ListItem::new(line).style(style)
            })
            .collect()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Collections ")
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(list, area);
}

fn render_modal(frame: &mut Frame, app: &App) {
    match &app.modal {
        Some(ModalState::NewCollection { input }) => {
            let area = centered_rect(52, 7, frame.area());
            frame.render_widget(Clear, area);

            let text = vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Name: "),
                    Span::styled(
                        format!("{}_", input),
                        Style::default().fg(Color::Yellow),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "  Enter: save   Esc: cancel",
                    Style::default().fg(Color::DarkGray),
                )),
            ];

            let modal = Paragraph::new(text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" New Collection ")
                    .title_alignment(Alignment::Center)
                    .border_style(Style::default().fg(Color::Cyan)),
            );
            frame.render_widget(modal, area);
        }

        Some(ModalState::NewRequest { name, method_idx, url, active_field, .. }) => {
            let area = centered_rect(60, 11, frame.area());
            frame.render_widget(Clear, area);

            let name_style = if *active_field == InputField::Name {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };
            let url_style = if *active_field == InputField::Url {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };
            let name_cursor = if *active_field == InputField::Name { "_" } else { "" };
            let url_cursor  = if *active_field == InputField::Url  { "_" } else { "" };

            let method = METHODS[*method_idx];
            let method_color = method_color(method);

            let max_url = 44usize;
            let url_display = if url.len() > max_url {
                format!("…{}", &url[url.len() - max_url..])
            } else {
                url.clone()
            };

            let text = vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Name:   "),
                    Span::styled(format!("{}{}", name, name_cursor), name_style),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Method: "),
                    Span::styled("◀ ", Style::default().fg(Color::DarkGray)),
                    Span::styled(method, Style::default().fg(method_color).add_modifier(Modifier::BOLD)),
                    Span::styled(" ▶", Style::default().fg(Color::DarkGray)),
                    Span::styled("  (←/→ to change)", Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::raw("  URL:    "),
                    Span::styled(format!("{}{}", url_display, url_cursor), url_style),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "  Tab: next field   Enter: save   Esc: cancel",
                    Style::default().fg(Color::DarkGray),
                )),
            ];

            let modal = Paragraph::new(text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" New Request ")
                    .title_alignment(Alignment::Center)
                    .border_style(Style::default().fg(Color::Yellow)),
            );
            frame.render_widget(modal, area);
        }

        Some(ModalState::ConfirmDelete { label, .. }) => {
            let area = centered_rect(52, 7, frame.area());
            frame.render_widget(Clear, area);

            let text = vec![
                Line::from(""),
                Line::from(vec![
                    Span::raw("  Delete "),
                    Span::styled(
                        format!("\"{}\"", label),
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("?"),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "  y / Enter: confirm   n / Esc: cancel",
                    Style::default().fg(Color::DarkGray),
                )),
            ];

            let modal = Paragraph::new(text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Delete ")
                    .title_alignment(Alignment::Center)
                    .border_style(Style::default().fg(Color::Red)),
            );
            frame.render_widget(modal, area);
        }

        None => {}
    }
}

fn render_placeholder(frame: &mut Frame, area: Rect, title: &str, msg: &str) {
    let widget = Paragraph::new(msg)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {title} "))
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

    frame.render_widget(widget, area);
}

fn render_status(frame: &mut Frame, app: &App, area: Rect) {
    let status = Paragraph::new(app.status_message.as_str())
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(status, area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
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
