use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

use crate::app::{App, Tab};

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Root layout: header / body / footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tabs
            Constraint::Min(0),    // content
            Constraint::Length(1), // status bar
        ])
        .split(area);

    render_tabs(frame, app, chunks[0].into());
    render_body(frame, app, chunks[1].into());
    render_status(frame, app, chunks[2].into());
}

fn render_tabs(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
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
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::raw(" | "));

    frame.render_widget(tabs_widget, area);
}

fn render_body(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    match app.active_tab {
        Tab::Request => render_request_panel(frame, area),
        Tab::Collections => render_placeholder(frame, area, "Collections", "Your saved request collections will appear here."),
        Tab::History => render_placeholder(frame, area, "History", "Recent requests will appear here."),
    }
}

fn render_request_panel(frame: &mut Frame, area: ratatui::layout::Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // URL bar
            Constraint::Min(0),    // response
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

    let response = Paragraph::new("Response will appear here…")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Response ")
                .border_style(Style::default().fg(Color::Green)),
        )
        .style(Style::default().fg(Color::DarkGray));

    frame.render_widget(response, chunks[1]);
}

fn render_placeholder(frame: &mut Frame, area: ratatui::layout::Rect, title: &str, msg: &str) {
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

fn render_status(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let status = Paragraph::new(app.status_message.as_str())
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(status, area);
}
