use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table, TableState, Tabs},
    Frame,
};

use crate::app::{flatten_collections, App, RequestTab, Tab};
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
    render_response_table(frame, app, chunks[3]);
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

fn render_response_table(frame: &mut Frame, app: &App, area: Rect) {
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
            ValueType::Object(_) | ValueType::Array(_) => Color::Cyan,
            _ => Color::White,
        };

        let key_cell = Cell::from(Line::from(vec![
            Span::raw(format!("{}{}", indent, icon)),
            Span::styled(r.key.clone(), Style::default().fg(key_color)),
        ]));

        let (type_color, type_label) = match r.value_type {
            ValueType::Object(_) => (Color::Cyan,    "Object "),
            ValueType::Array(_)  => (Color::Blue,    "Array  "),
            ValueType::Str       => (Color::Green,   "String "),
            ValueType::Number    => (Color::Yellow,  "Number "),
            ValueType::Boolean   => (Color::Magenta, "Boolean"),
            ValueType::Null      => (Color::DarkGray,"Null   "),
        };

        let type_cell = Cell::from(Span::styled(type_label, Style::default().fg(type_color)));

        let value_color = match r.value_type {
            ValueType::Object(_) | ValueType::Array(_) => Color::White,
            ValueType::Str     => Color::Green,
            ValueType::Number  => Color::Yellow,
            ValueType::Boolean => Color::Magenta,
            ValueType::Null    => Color::DarkGray,
        };

        let value_cell = Cell::from(Span::styled(
            r.value_preview.clone(),
            Style::default().fg(value_color),
        ));

        Row::new(vec![key_cell, type_cell, value_cell])
    }).collect();

    let header = Row::new(vec![
        Cell::from(Span::styled("Key", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Type", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Value", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
    ])
    .style(Style::default().bg(Color::Indexed(236)))
    .height(1);

    let widths = [
        Constraint::Length(app.key_col_width),
        Constraint::Length(8),
        Constraint::Min(10),
    ];

    let hint = format!(" Response  -/=: resize key col ({}) ", app.key_col_width);
    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(hint)
                .border_style(Style::default().fg(Color::Green)),
        )
        .highlight_style(Style::default().bg(Color::Indexed(237)))
        .column_spacing(1);

    let mut state = TableState::default()
        .with_selected(Some(app.response_cursor))
        .with_offset(app.response_scroll as usize);

    frame.render_stateful_widget(table, area, &mut state);
}

fn render_collections_panel(frame: &mut Frame, app: &App, area: Rect) {
    let flat = flatten_collections(&app.collections);

    let items: Vec<ListItem> = flat
        .iter()
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
                let method_color = match method {
                    "GET" => Color::Green,
                    "POST" => Color::Blue,
                    "PUT" => Color::Yellow,
                    "PATCH" => Color::Magenta,
                    "DELETE" => Color::Red,
                    _ => Color::White,
                };
                Line::from(vec![
                    Span::raw(format!("{indent}{icon}")),
                    Span::styled(format!("{method:<7}"), Style::default().fg(method_color)),
                    Span::styled(node.name.clone(), Style::default().fg(Color::White)),
                ])
            };

            let style = if i == app.collection_cursor {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Collections ")
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(list, area);
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
