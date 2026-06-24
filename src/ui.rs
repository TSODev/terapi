use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, TableState, Tabs, Wrap},
    Frame,
};

use crate::app::{
    flatten_stored, sorted_vars, App, BodyMode, EnvFocus, GqlField,
    GraphqlTab, InputField, ModalState, OAuth2WaitState, RequestFocus, RequestTab, ResponseView,
    SaveField, SchemaDetail, SchemaState, Tab, VarField, COMMON_CONTENT_TYPES, COMMON_HEADERS,
    METHODS,
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
    if app.gql_completion.is_some() {
        render_gql_completion(frame, app);
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
        Tab::Request     => render_request_panel(frame, app, area),
        Tab::Collections => render_collections_panel(frame, app, area),
        Tab::Env         => render_env_panel(frame, app, area),
        Tab::History     => render_history_panel(frame, app, area),
        Tab::Campaigns   => render_campaigns_panel(frame, app, area),
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
    let method_label = if app.graphql_mode { "GQL" } else { app.active_method() };
    let method_col = method_color(method_label);
    let url_cursor = if editing { "_" } else { "" };
    // In read mode, show the full URL with query params reconstructed from the params list.
    // In edit mode, show only what the user is typing (params are parsed on Esc/Enter).
    let url_display = if editing || app.request_url_params.is_empty() {
        app.request_url.clone()
    } else {
        let query = app.request_url_params.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        format!("{}?{}", app.request_url, query)
    };
    let url_text = Line::from(vec![
        Span::raw(" "),
        if editing && !app.graphql_mode {
            Span::styled("◀ ", Style::default().fg(Color::Indexed(242)))
        } else {
            Span::raw("  ")
        },
        Span::styled(method_label, Style::default().fg(method_col).add_modifier(Modifier::BOLD)),
        if editing && !app.graphql_mode {
            Span::styled(" ▶  ", Style::default().fg(Color::Indexed(242)))
        } else {
            Span::raw("  ")
        },
        Span::styled(
            format!("{}{}", url_display, url_cursor),
            if editing {
                Style::default().fg(Color::Yellow)
            } else if url_display.is_empty() {
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
    if app.graphql_mode {
        let tabs: Vec<Line> = GraphqlTab::all().into_iter().map(|t| Line::from(t.title())).collect();
        let selected = GraphqlTab::all().into_iter().position(|t| t == app.active_graphql_tab).unwrap_or(0);
        let sub_tabs = Tabs::new(tabs)
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Magenta)))
            .select(selected)
            .style(Style::default().fg(Color::Indexed(244)))
            .highlight_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
            .divider(Span::raw(" | "));
        frame.render_widget(sub_tabs, area);
    } else {
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
}

fn render_request_content(frame: &mut Frame, app: &App, area: Rect) {
    if app.graphql_mode {
        match app.active_graphql_tab {
            GraphqlTab::Query     => render_graphql_query_editor(frame, app, area),
            GraphqlTab::Variables => render_graphql_vars_editor(frame, app, area),
            GraphqlTab::Headers   => render_headers_editor(frame, app, area),
            GraphqlTab::Schema    => render_graphql_schema(frame, app, area),
            GraphqlTab::Options   => render_options_editor(frame, app, area),
        }
        return;
    }
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

fn render_graphql_query_editor(frame: &mut Frame, app: &App, area: Rect) {
    let editing = app.request_focus == RequestFocus::Body;
    let border_style = if editing {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Magenta)
    };
    let title = if editing { " Query — editing (Esc: done) " } else { " Query — i: edit " };
    let mut textarea = app.graphql_query_textarea.clone();
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

fn render_graphql_vars_editor(frame: &mut Frame, app: &App, area: Rect) {
    let count = app.graphql_vars.len();
    let title = if count == 0 {
        " Variables ".to_string()
    } else {
        format!(" Variables ({}) ", count)
    };

    let items: Vec<ListItem> = if app.graphql_vars.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  No variables — press a to add one",
            Style::default().fg(Color::Indexed(238)),
        )))]
    } else {
        app.graphql_vars.iter().enumerate().map(|(i, (k, v))| {
            let selected = i == app.graphql_vars_cursor;
            let cursor = if selected {
                Span::styled("▶ ", Style::default().fg(Color::Cyan))
            } else {
                Span::raw("  ")
            };
            let key_style = if selected {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };
            let val_style = if selected {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(vec![
                cursor,
                Span::styled(k.clone(), key_style),
                Span::styled("  =  ", Style::default().fg(Color::Indexed(244))),
                Span::styled(v.clone(), val_style),
            ]))
        }).collect()
    };

    let hint = Line::from(Span::styled(
        "  a: add  d: delete  Enter: edit  ↑/↓: navigate",
        Style::default().fg(Color::Indexed(238)),
    ));

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    frame.render_widget(List::new(items), chunks[0]);
    frame.render_widget(Paragraph::new(hint), chunks[1]);
}

fn render_graphql_schema(frame: &mut Frame, app: &App, area: Rect) {
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" Schema ")
        .border_style(Style::default().fg(Color::Magenta));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    match &app.schema_state {
        SchemaState::Idle => {
            let hint = if app.request_url.is_empty() {
                "Set an endpoint URL first (e), then press f".to_string()
            } else {
                format!("{}  — press f to fetch schema", app.request_url)
            };
            let text = vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("  {}", hint),
                    Style::default().fg(Color::Indexed(244)),
                )),
            ];
            frame.render_widget(Paragraph::new(text), inner);
        }

        SchemaState::LoadingList => {
            let text = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  ⟳ Fetching type list…",
                    Style::default().fg(Color::Yellow),
                )),
            ];
            frame.render_widget(Paragraph::new(text), inner);
        }

        SchemaState::Error(msg) => {
            let mut lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Schema error:",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];
            for line in msg.lines() {
                lines.push(Line::from(Span::styled(
                    format!("  {}", line),
                    Style::default().fg(Color::Indexed(244)),
                )));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Press f to retry",
                Style::default().fg(Color::Indexed(238)),
            )));
            frame.render_widget(Paragraph::new(lines), inner);
        }

        SchemaState::Ready { types, detail } => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
                .split(inner);

            // ── Left: type list ───────────────────────────────────────────
            let left_block = Block::default()
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(Color::Indexed(238)));
            let left_inner = left_block.inner(chunks[0]);
            frame.render_widget(left_block, chunks[0]);

            let items: Vec<ListItem> = types.iter().enumerate().map(|(i, t)| {
                let (kind_abbr, kind_color) = match t.kind.as_str() {
                    "OBJECT"       => ("OBJ", Color::Cyan),
                    "INTERFACE"    => ("INT", Color::Blue),
                    "UNION"        => ("UNI", Color::Magenta),
                    "ENUM"         => ("ENM", Color::Yellow),
                    "INPUT_OBJECT" => ("INP", Color::Green),
                    "SCALAR"       => ("SCL", Color::Indexed(244)),
                    _              => ("???", Color::Indexed(244)),
                };
                let selected = i == app.schema_type_cursor;
                let name_style = if selected {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(Line::from(vec![
                    Span::raw(if selected { "► " } else { "  " }),
                    Span::styled(kind_abbr, Style::default().fg(kind_color)),
                    Span::raw("  "),
                    Span::styled(t.name.clone(), name_style),
                ]))
            }).collect();

            let mut list_state = ratatui::widgets::ListState::default();
            list_state.select(Some(app.schema_type_cursor));
            frame.render_stateful_widget(
                List::new(items).highlight_style(Style::default()),
                left_inner,
                &mut list_state,
            );

            // ── Right: type detail ────────────────────────────────────────
            let right_area = chunks[1];
            match detail {
                SchemaDetail::None => {
                    let t = types.get(app.schema_type_cursor);
                    let name = t.map(|t| t.name.as_str()).unwrap_or("");
                    let lines = vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            format!("  {}", name),
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            "  Press Enter to load fields",
                            Style::default().fg(Color::Indexed(244)),
                        )),
                    ];
                    frame.render_widget(Paragraph::new(lines), right_area);
                }
                SchemaDetail::Loading => {
                    let lines = vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            "  ⟳ Loading fields…",
                            Style::default().fg(Color::Yellow),
                        )),
                    ];
                    frame.render_widget(Paragraph::new(lines), right_area);
                }
                SchemaDetail::Error(msg) => {
                    let lines = vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            format!("  Error: {}", msg),
                            Style::default().fg(Color::Red),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            "  Press Enter to retry",
                            Style::default().fg(Color::Indexed(238)),
                        )),
                    ];
                    frame.render_widget(Paragraph::new(lines), right_area);
                }
                SchemaDetail::Loaded(t) => {
                    let mut lines: Vec<Line> = Vec::new();

                    lines.push(Line::from(vec![
                        Span::styled(
                            t.name.clone(),
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("  {}", t.kind),
                            Style::default().fg(Color::Indexed(244)),
                        ),
                    ]));
                    if let Some(desc) = &t.description {
                        lines.push(Line::from(Span::styled(
                            desc.chars().take(100).collect::<String>(),
                            Style::default().fg(Color::Indexed(238)),
                        )));
                    }
                    lines.push(Line::from(""));

                    let fields_to_show: &[GqlField] = if !t.fields.is_empty() {
                        &t.fields
                    } else if !t.input_fields.is_empty() {
                        &t.input_fields
                    } else {
                        &[]
                    };

                    if !fields_to_show.is_empty() {
                        for f in fields_to_show {
                            let padded = format!("{:<24}", f.name);
                            lines.push(Line::from(vec![
                                Span::raw("  "),
                                Span::styled(padded, Style::default().fg(Color::White)),
                                Span::styled(
                                    f.type_str.clone(),
                                    Style::default().fg(Color::Magenta),
                                ),
                            ]));
                            if !f.args.is_empty() {
                                let args_str = f.args.iter()
                                    .map(|a| format!("{}: {}", a.name, a.type_str))
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                lines.push(Line::from(Span::styled(
                                    format!("    ↳ ({})", args_str),
                                    Style::default().fg(Color::Indexed(244)),
                                )));
                            }
                        }
                    } else if !t.enum_values.is_empty() {
                        for val in &t.enum_values {
                            lines.push(Line::from(vec![
                                Span::raw("  "),
                                Span::styled(val.clone(), Style::default().fg(Color::Yellow)),
                            ]));
                        }
                    } else {
                        lines.push(Line::from(Span::styled(
                            "  (no fields)",
                            Style::default().fg(Color::Indexed(238)),
                        )));
                    }

                    frame.render_widget(
                        Paragraph::new(lines).scroll((app.schema_field_scroll, 0)),
                        right_area,
                    );
                }
            }
        }
    }
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
            Style::default().fg(Color::Indexed(242))
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
    let timeout_cursor_style = if timeout_selected { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::Indexed(242)) };
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
    let types: &[(AuthType, &str)] = &[
        (AuthType::None,                    "No Auth"),
        (AuthType::Bearer,                  "Bearer"),
        (AuthType::Basic,                   "Basic"),
        (AuthType::ApiKey,                  "API Key"),
        (AuthType::OAuth2ClientCredentials, "OAuth2 CC"),
        (AuthType::OAuth2AuthorizationCode, "OAuth2 AC"),
    ];
    let type_spans: Vec<Span> = types.iter().enumerate().flat_map(|(i, (t, label))| {
        let active = &auth.auth_type == t;
        let styled_label = format!(" {} ", label);
        let style = if active {
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Indexed(244))
        };
        let mut spans = vec![Span::styled(styled_label, style)];
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
        AuthType::OAuth2ClientCredentials | AuthType::OAuth2AuthorizationCode => {
            let is_auth_code = auth.auth_type == AuthType::OAuth2AuthorizationCode;
            let field_style = |idx: usize| if cursor == idx { Style::default().bg(Color::Indexed(237)) } else { Style::default() };
            let val_span = |s: &str, placeholder: &'static str| -> Span<'static> {
                if s.is_empty() {
                    Span::styled(format!(" {}", placeholder), Style::default().fg(Color::Indexed(238)))
                } else {
                    Span::styled(format!(" {}", s.to_string()), Style::default().fg(Color::Cyan))
                }
            };
            let secret_span = |s: &str| -> Span<'static> {
                if s.is_empty() {
                    Span::styled(" <enter secret>", Style::default().fg(Color::Indexed(238)))
                } else {
                    Span::styled(format!(" {}", "•".repeat(s.len())), Style::default().fg(Color::Yellow))
                }
            };

            // Token status line
            let token_status = if app.oauth2_token_cache.contains_key(&auth.oauth2_cache_key()) {
                Span::styled(" ● token cached", Style::default().fg(Color::Green))
            } else {
                Span::styled(" ○ no token  (f to fetch)", Style::default().fg(Color::Indexed(244)))
            };

            // Wait state banner
            let wait_line = match &app.oauth2_wait_state {
                OAuth2WaitState::FetchingToken =>
                    Some(Line::from(Span::styled(" ⟳ fetching token…", Style::default().fg(Color::Yellow)))),
                OAuth2WaitState::WaitingForBrowser { port } =>
                    Some(Line::from(Span::styled(
                        format!(" ⟳ waiting for browser callback on port {}… (Esc to cancel)", port),
                        Style::default().fg(Color::Yellow),
                    ))),
                OAuth2WaitState::Error(msg) =>
                    Some(Line::from(Span::styled(format!(" ✗ {}", msg), Style::default().fg(Color::Red)))),
                OAuth2WaitState::Idle => None,
            };

            let mut rows = vec![
                Line::from(""),
                Line::from(vec![
                    Span::styled(" Token URL   ", Style::default().fg(Color::Indexed(244))),
                    val_span(&auth.oauth2_token_url, "<enter token URL>"),
                ]).style(field_style(1)),
                Line::from(vec![
                    Span::styled(" Client ID   ", Style::default().fg(Color::Indexed(244))),
                    val_span(&auth.oauth2_client_id, "<enter client ID>"),
                ]).style(field_style(2)),
                Line::from(vec![
                    Span::styled(" Client Secret", Style::default().fg(Color::Indexed(244))),
                    secret_span(&auth.oauth2_client_secret),
                ]).style(field_style(3)),
                Line::from(vec![
                    Span::styled(" Scope       ", Style::default().fg(Color::Indexed(244))),
                    val_span(&auth.oauth2_scope, "<optional scope>"),
                ]).style(field_style(4)),
            ];
            if is_auth_code {
                rows.push(Line::from(vec![
                    Span::styled(" Auth URL    ", Style::default().fg(Color::Indexed(244))),
                    val_span(&auth.oauth2_auth_url, "<enter authorization URL>"),
                ]).style(field_style(5)));
                let port_str = auth.oauth2_redirect_port.to_string();
                rows.push(Line::from(vec![
                    Span::styled(" Redirect Port", Style::default().fg(Color::Indexed(244))),
                    val_span(&port_str, "9876"),
                ]).style(field_style(6)));
            }
            rows.push(Line::from(""));
            rows.push(Line::from(vec![
                Span::styled(" Status      ", Style::default().fg(Color::Indexed(244))),
                token_status,
            ]));
            if let Some(wl) = wait_line { rows.push(wl); }
            rows
        }
    };

    let hint = Line::from(Span::styled(
        " ↑/↓: navigate  Space/Enter: cycle type or edit field  f: fetch token",
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
                Span::styled("= ", Style::default().fg(Color::Indexed(242))),
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
    if v == "null" { Color::Indexed(242) }
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
                Span::styled(": ", Style::default().fg(Color::Indexed(242))),
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
        let sep = Span::styled(" · ", Style::default().fg(Color::Indexed(242)));
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
            spans.push(Span::styled("  ·  ", Style::default().fg(Color::Indexed(242))));
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
    // Reserve bottom lines: 1 for path bar (always), +1 for search bar when active
    let has_body = app.response_body.is_some();
    let has_search = app.json_search.is_some();
    let bottom_lines = if has_body && has_search { 2 } else if has_body { 1 } else { 0 };

    let (table_area, path_area, search_area) = if bottom_lines == 2 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1), Constraint::Length(1)])
            .split(area);
        (chunks[0], Some(chunks[1]), Some(chunks[2]))
    } else if bottom_lines == 1 && has_search {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        (chunks[0], None, Some(chunks[1]))
    } else if bottom_lines == 1 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        (chunks[0], Some(chunks[1]), None)
    } else {
        (area, None, None)
    };

    let term = app.json_search.as_deref().unwrap_or("").to_lowercase();
    let has_term = !term.is_empty();

    let json_rows = match &app.response_body {
        Some(json) => json_highlight::rows(json, &app.response_folds),
        None => vec![],
    };

    let match_count = if has_term {
        json_rows.iter().filter(|r| {
            r.key.to_lowercase().contains(&term) || r.value_preview.to_lowercase().contains(&term)
        }).count()
    } else {
        0
    };

    let rows: Vec<Row> = json_rows.iter().map(|r| {
        let is_match = has_term && (
            r.key.to_lowercase().contains(&term) ||
            r.value_preview.to_lowercase().contains(&term)
        );
        let indent = "  ".repeat(r.depth);
        let icon = match r.fold_path {
            Some(_) if r.is_folded => "▶ ",
            Some(_) => "▼ ",
            None => "  ",
        };
        let key_color = if is_match {
            Color::Yellow
        } else {
            match r.value_type {
                ValueType::Object | ValueType::Array => Color::Cyan,
                _ => Color::White,
            }
        };
        let key_mod = if is_match { Modifier::BOLD } else { Modifier::empty() };
        let key_cell = Cell::from(Line::from(vec![
            Span::raw(format!("{}{}", indent, icon)),
            Span::styled(r.key.clone(), Style::default().fg(key_color).add_modifier(key_mod)),
        ]));
        let (type_color, type_label) = match r.value_type {
            ValueType::Object  => (Color::Cyan,         "Object "),
            ValueType::Array   => (Color::Blue,         "Array  "),
            ValueType::Str     => (Color::Green,        "String "),
            ValueType::Number  => (Color::Yellow,       "Number "),
            ValueType::Boolean => (Color::Magenta,      "Boolean"),
            ValueType::Null    => (Color::Indexed(242), "Null   "),
        };
        let value_color = if is_match {
            Color::Yellow
        } else {
            match r.value_type {
                ValueType::Object | ValueType::Array => Color::White,
                ValueType::Str     => Color::Green,
                ValueType::Number  => Color::Yellow,
                ValueType::Boolean => Color::Magenta,
                ValueType::Null    => Color::Indexed(242),
            }
        };
        let value_mod = if is_match { Modifier::BOLD } else { Modifier::empty() };
        Row::new(vec![
            key_cell,
            Cell::from(Span::styled(type_label, Style::default().fg(type_color))),
            Cell::from(Span::styled(r.value_preview.clone(), Style::default().fg(value_color).add_modifier(value_mod))),
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

    frame.render_stateful_widget(table, table_area, &mut state);

    // ── path bar (always visible when there is a response) ────────────────────
    if let Some(pbar_area) = path_area {
        let dot = json_rows.get(app.response_cursor)
            .map(|r| r.dot_path.as_str())
            .unwrap_or("");
        let path_display = if dot.is_empty() { "(root)".to_string() } else { dot.to_string() };
        let path_line = Line::from(vec![
            Span::styled(" ↳ ", Style::default().fg(Color::Indexed(244))),
            Span::styled(path_display, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]);
        frame.render_widget(
            Paragraph::new(path_line).style(Style::default().bg(Color::Indexed(234))),
            pbar_area,
        );
    }

    // ── search bar ────────────────────────────────────────────────────────────
    if let Some(bar_area) = search_area {
        let search_term = app.json_search.as_deref().unwrap_or("");
        let (count_text, count_color) = if !search_term.is_empty() {
            if match_count == 0 {
                (" no match ".to_string(), Color::Red)
            } else {
                (format!(" {} match{} ", match_count, if match_count == 1 { "" } else { "es" }), Color::Green)
            }
        } else {
            (String::new(), Color::Green)
        };

        let bar = Line::from(vec![
            Span::styled(" /", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(search_term.to_string(), Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::Yellow)),
            Span::styled(count_text, Style::default().fg(count_color).add_modifier(Modifier::BOLD)),
            Span::styled("  >: next  <: prev  Esc: close", Style::default().fg(Color::Indexed(244))),
        ]);
        frame.render_widget(
            Paragraph::new(bar).style(Style::default().bg(Color::Indexed(234))),
            bar_area,
        );
    }
}

fn render_response_raw(frame: &mut Frame, app: &App, area: Rect) {
    let text = app.response_body.as_deref().unwrap_or("No response.");
    let lines = highlight_raw(text);
    let para = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.response_scroll, 0));
    frame.render_widget(para, area);
}

/// JSON-aware syntax highlighter for the Raw response view.
/// Tokenises line by line; degrades gracefully for non-JSON text.
fn highlight_raw(text: &str) -> Vec<Line<'static>> {
    let s_key    = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let s_str    = Style::default().fg(Color::Green);
    let s_num    = Style::default().fg(Color::Yellow);
    let s_bool   = Style::default().fg(Color::Magenta);
    let s_null   = Style::default().fg(Color::Indexed(245));
    let s_punct  = Style::default().fg(Color::Indexed(240));
    let s_plain  = Style::default().fg(Color::White);

    text.lines().map(|line| {
        let chars: Vec<char> = line.chars().collect();
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];
            match c {
                // ── whitespace ────────────────────────────────────────────
                ' ' | '\t' => {
                    let start = i;
                    while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') { i += 1; }
                    spans.push(Span::raw(chars[start..i].iter().collect::<String>()));
                }
                // ── quoted string ─────────────────────────────────────────
                '"' => {
                    let mut s = String::from('"');
                    i += 1;
                    let mut escaped = false;
                    while i < chars.len() {
                        let ch = chars[i];
                        s.push(ch);
                        if escaped { escaped = false; }
                        else if ch == '\\' { escaped = true; }
                        else if ch == '"' { i += 1; break; }
                        i += 1;
                    }
                    // key if next non-space char is ':'
                    let mut j = i;
                    while j < chars.len() && chars[j] == ' ' { j += 1; }
                    let style = if j < chars.len() && chars[j] == ':' { s_key } else { s_str };
                    spans.push(Span::styled(s, style));
                }
                // ── number ────────────────────────────────────────────────
                '0'..='9' | '-' => {
                    let start = i;
                    while i < chars.len() && matches!(chars[i], '0'..='9' | '.' | '-' | 'e' | 'E' | '+') { i += 1; }
                    spans.push(Span::styled(chars[start..i].iter().collect::<String>(), s_num));
                }
                // ── literals: true / false / null ─────────────────────────
                'a'..='z' | 'A'..='Z' => {
                    let start = i;
                    while i < chars.len() && chars[i].is_ascii_alphabetic() { i += 1; }
                    let word: String = chars[start..i].iter().collect();
                    let style = match word.as_str() {
                        "true" | "false" => s_bool,
                        "null"           => s_null,
                        _                => s_plain,
                    };
                    spans.push(Span::styled(word, style));
                }
                // ── structural punctuation ────────────────────────────────
                '{' | '}' | '[' | ']' => {
                    spans.push(Span::styled(c.to_string(), s_punct.add_modifier(Modifier::BOLD)));
                    i += 1;
                }
                ':' | ',' => {
                    spans.push(Span::styled(c.to_string(), s_punct));
                    i += 1;
                }
                // ── anything else (plain) ─────────────────────────────────
                _ => {
                    spans.push(Span::styled(c.to_string(), s_plain));
                    i += 1;
                }
            }
        }

        if spans.is_empty() {
            Line::from(Span::raw(""))
        } else {
            Line::from(spans)
        }
    }).collect()
}

fn render_response_http(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    let sep_style    = Style::default().fg(Color::Indexed(238));
    let header_key   = Style::default().fg(Color::Yellow);
    let header_val   = Style::default().fg(Color::White);
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
            // Show cookies that were sent from the cookie jar (cookie_jar_enabled).
            if app.cookie_jar && !app.response_cookies.is_empty() {
                let cookie_str = app.response_cookies.iter()
                    .map(|(name, rest)| {
                        let val = rest.split(';').next().unwrap_or(rest).trim();
                        format!("{}={}", name, val)
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                lines.push(Line::from(vec![
                    Span::styled("Cookie: ", header_key),
                    Span::styled(cookie_str, header_val),
                ]));
            }
            if let Some(body) = &req.body {
                lines.push(Line::from(vec![
                    Span::styled("Content-Length: ", header_key),
                    Span::styled(body.len().to_string(), header_val),
                ]));
                lines.push(Line::from(Span::raw("")));
                lines.extend(highlight_raw(body));
            } else {
                lines.push(Line::from(Span::raw("")));
            }
        }
    }

    lines.push(Line::from(Span::raw("")));

    // ── Response ──────────────────────────────────────────────────────────
    lines.push(Line::from(Span::styled("── Response ─────────────────────────────────────────", sep_style)));

    let diag_label = Style::default().fg(Color::Indexed(245));
    let diag_val   = Style::default().fg(Color::White);

    match app.response_status {
        None if app.last_request_raw.is_none() => {
            lines.push(Line::from(Span::styled("No response yet.", hint_style)));
        }
        None => {
            // Transport error — request was sent but no HTTP response received.
            lines.push(Line::from(Span::styled(
                "⚠  Transport error",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::raw("")));
            let raw = app.response_body.as_deref().unwrap_or("Unknown error");
            let msg = raw.strip_prefix("Error: ").unwrap_or(raw);
            for l in msg.lines() {
                let t = l.trim();
                if t.is_empty() { continue; }
                let style = if t.starts_with("caused by:") {
                    Style::default().fg(Color::Indexed(245))
                } else {
                    Style::default().fg(Color::Red)
                };
                lines.push(Line::from(Span::styled(format!("  {t}"), style)));
            }

            // Still show timing if we have it (e.g. timeout after partial connect).
            if let Some(elapsed) = app.response_elapsed_ms {
                lines.push(Line::from(Span::raw("")));
                lines.push(Line::from(Span::styled("── Diagnostics ──────────────────────────────────────", sep_style)));
                lines.push(Line::from(vec![
                    Span::styled("  Elapsed     ", diag_label),
                    Span::styled(format!("{} ms", elapsed), Style::default().fg(Color::Red)),
                ]));
            }
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
            lines.extend(highlight_raw(body));

            // ── Redirect chain ────────────────────────────────────────────
            if !app.response_redirects.is_empty() {
                lines.push(Line::from(Span::raw("")));
                lines.push(Line::from(Span::styled("── Redirects ────────────────────────────────────────", sep_style)));
                for (i, (code, url)) in app.response_redirects.iter().enumerate() {
                    let code_color = match code {
                        301 | 308 => Color::Yellow,
                        302 | 303 => Color::Cyan,
                        307       => Color::Blue,
                        _         => Color::White,
                    };
                    lines.push(Line::from(vec![
                        Span::styled(format!("  {}  ", i + 1), hint_style),
                        Span::styled(format!("{} ", code),
                            Style::default().fg(code_color).add_modifier(Modifier::BOLD)),
                        Span::styled("→ ", hint_style),
                        Span::styled(url.clone(), header_val),
                    ]));
                }
            }

            // ── Cookies set ───────────────────────────────────────────────
            if !app.response_cookies.is_empty() {
                lines.push(Line::from(Span::raw("")));
                lines.push(Line::from(Span::styled("── Cookies ──────────────────────────────────────────", sep_style)));
                for (name, rest) in &app.response_cookies {
                    let mut parts = rest.splitn(2, ';');
                    let value = parts.next().unwrap_or(rest).trim();
                    let attrs  = rest.find(';').map(|i| &rest[i..]).unwrap_or("").trim();
                    lines.push(Line::from(vec![
                        Span::styled(format!("  {}=", name), Style::default().fg(Color::Yellow)),
                        Span::styled(value.to_string(), header_val),
                        Span::styled(
                            if attrs.is_empty() { String::new() } else { format!("  {}", attrs) },
                            hint_style,
                        ),
                    ]));
                }
            }

            // ── Diagnostics ───────────────────────────────────────────────
            lines.push(Line::from(Span::raw("")));
            lines.push(Line::from(Span::styled("── Diagnostics ──────────────────────────────────────", sep_style)));

            // Elapsed + color-coded threshold
            if let Some(elapsed) = app.response_elapsed_ms {
                let t_color = if elapsed < 300 { Color::Green }
                    else if elapsed < 1000 { Color::Yellow }
                    else { Color::Red };
                lines.push(Line::from(vec![
                    Span::styled("  Elapsed     ", diag_label),
                    Span::styled(format!("{} ms", elapsed),
                        Style::default().fg(t_color).add_modifier(Modifier::BOLD)),
                ]));
            }

            // Response size (decompressed — reqwest decompresses automatically)
            let size = body.len();
            let compressed = app.response_headers.iter()
                .any(|(k, _)| k.to_lowercase() == "content-encoding");
            let size_str = if size < 1024 {
                format!("{} B", size)
            } else if size < 1024 * 1024 {
                format!("{:.1} KB  ({} B)", size as f64 / 1024.0, size)
            } else {
                format!("{:.1} MB  ({} B)", size as f64 / (1024.0 * 1024.0), size)
            };
            let size_suffix = if compressed { "  (decompressed)" } else { "" };
            lines.push(Line::from(vec![
                Span::styled("  Size        ", diag_label),
                Span::styled(format!("{}{}", size_str, size_suffix), diag_val),
            ]));

            // Content-Type
            if let Some((_, ct)) = app.response_headers.iter()
                .find(|(k, _)| k.to_lowercase() == "content-type")
            {
                lines.push(Line::from(vec![
                    Span::styled("  Type        ", diag_label),
                    Span::styled(ct.clone(), Style::default().fg(Color::Cyan)),
                ]));
            }

            // Content-Encoding (if present)
            if let Some((_, enc)) = app.response_headers.iter()
                .find(|(k, _)| k.to_lowercase() == "content-encoding")
            {
                lines.push(Line::from(vec![
                    Span::styled("  Encoding    ", diag_label),
                    Span::styled(enc.clone(), Style::default().fg(Color::Cyan)),
                ]));
            }

            // Server
            if let Some((_, srv)) = app.response_headers.iter()
                .find(|(k, _)| k.to_lowercase() == "server")
            {
                lines.push(Line::from(vec![
                    Span::styled("  Server      ", diag_label),
                    Span::styled(srv.clone(), diag_val),
                ]));
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
                Span::styled("= ", Style::default().fg(Color::Indexed(242))),
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
                    Span::styled("◀ ", Style::default().fg(Color::Indexed(242))),
                    Span::styled(method, Style::default().fg(method_color(method)).add_modifier(Modifier::BOLD)),
                    Span::styled(" ▶", Style::default().fg(Color::Indexed(242))),
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

        Some(ModalState::CampaignParams { campaign_idx, params, cursor, editing, input }) => {
            let height = (params.len() as u16 + 6).max(8);
            let area = centered_rect(70, height, frame.area());
            frame.render_widget(Clear, area);

            let campaign_name = app.campaigns.get(*campaign_idx)
                .map(|e| e.name.as_str())
                .unwrap_or("Campaign");

            let mut lines: Vec<Line> = vec![Line::from("")];
            for (i, (name, description, value)) in params.iter().enumerate() {
                let selected = i == *cursor;
                let bg = if selected { Color::Indexed(236) } else { Color::Reset };
                let display_value = if selected && *editing {
                    format!("{}█", input)
                } else {
                    value.clone()
                };
                let name_style = Style::default().fg(Color::Cyan).bg(bg);
                let value_style = if selected {
                    Style::default().fg(Color::Yellow).bg(bg).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White).bg(bg)
                };
                let desc_style = Style::default().fg(Color::Indexed(245)).bg(bg);

                let mut spans = vec![
                    Span::styled(format!("  {:<20}", name), name_style),
                    Span::styled(format!(" {:<24}", display_value), value_style),
                ];
                if !description.is_empty() {
                    spans.push(Span::styled(format!("  {}", description), desc_style));
                }
                lines.push(Line::from(spans));
            }
            lines.push(Line::from(""));
            if *editing {
                lines.push(Line::from(Span::styled(
                    "  Enter: confirm   Esc: cancel edit",
                    Style::default().fg(Color::Gray),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    "  Enter: edit value   r: run   Esc: cancel",
                    Style::default().fg(Color::Gray),
                )));
            }

            frame.render_widget(
                Paragraph::new(lines).block(
                    Block::default().borders(Borders::ALL)
                        .title(format!(" Parameters — {} ", campaign_name))
                        .title_alignment(Alignment::Center)
                        .border_style(Style::default().fg(Color::Magenta)),
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
        let url_max = area.width.saturating_sub(46) as usize;
        let url_display = if entry.url.chars().count() > url_max {
            format!("{}…", entry.url.chars().take(url_max.saturating_sub(1)).collect::<String>())
        } else {
            entry.url.clone()
        };

        let selected = i == app.history_cursor;
        let bg = if selected { Color::Indexed(236) } else { Color::Reset };

        let mode_span = if entry.graphql {
            Span::styled(" GQL  ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD).bg(bg))
        } else {
            let col = method_color(&entry.method);
            Span::styled(format!("{:<6}", entry.method), Style::default().fg(col).add_modifier(Modifier::BOLD).bg(bg))
        };

        let line = Line::from(vec![
            Span::styled(format!("  {}", ts), Style::default().fg(Color::Indexed(250)).bg(bg)),
            Span::styled("  ", Style::default().bg(bg)),
            mode_span,
            Span::styled(format!("{:<3}", status_str), Style::default().fg(status_color).bg(bg)),
            Span::styled(format!("{:<7}", elapsed_str), Style::default().fg(Color::Indexed(250)).bg(bg)),
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

    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(app.history_cursor));
    frame.render_stateful_widget(list, area, &mut state);
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
    let (hint_text, hint_color) = if app.confirm_quit {
        (app.status_message.as_str(), Color::Yellow)
    } else {
        (app.status_message.as_str(), Color::Gray)
    };
    frame.render_widget(
        Paragraph::new(hint_text).style(Style::default().fg(hint_color)),
        rows[1],
    );
}

fn context_breadcrumb(app: &App) -> String {
    match &app.active_tab {
        Tab::Request => {
            if app.graphql_mode {
                let sub = app.active_graphql_tab.title();
                let focus_suffix = match app.request_focus {
                    RequestFocus::Url  => "  ›  URL edit",
                    RequestFocus::Body => "  ›  editing",
                    _                  => "",
                };
                format!("GraphQL  ›  {}{}", sub, focus_suffix)
            } else {
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
        }
        Tab::Collections => "Collections".to_string(),
        Tab::Env => match app.env_focus {
            EnvFocus::Envs => "Env  ›  Environments".to_string(),
            EnvFocus::Vars => "Env  ›  Variables".to_string(),
        },
        Tab::History   => "History".to_string(),
        Tab::Campaigns => {
            let run_label = match &app.campaign_run_state {
                crate::campaign::CampaignRunState::Idle    => String::new(),
                crate::campaign::CampaignRunState::Running { name, .. } => format!("  ›  Running: {}", name),
                crate::campaign::CampaignRunState::Done { name, .. }    => format!("  ›  Done: {}", name),
            };
            format!("Campaigns{}", run_label)
        }
    }
}

fn env_indicator(app: &App) -> (String, Color) {
    if app.active_tab == Tab::Request
        && app.active_env_idx.is_none()
        && app.has_unresolved_vars()
    {
        return ("⚠ {{VAR}} not resolved".to_string(), Color::Yellow);
    }
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

fn render_gql_completion(frame: &mut Frame, app: &App) {
    let Some(state) = &app.gql_completion else { return };
    use crate::app::App as A;
    let filtered = A::filter_completions(&state.items, &state.prefix);

    let inner_h = (filtered.len().max(1) as u16).min(12);
    let total_h = inner_h + 4;
    let width: u16 = 52;
    let area = centered_rect(width, total_h, frame.area());

    let title = if state.prefix.is_empty() {
        " GQL completion ".to_string()
    } else {
        format!(" GQL completion · {} ", state.prefix)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    let hint_area = Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };
    let hint = Paragraph::new("↑/↓: navigate  Enter/Tab: insert  Esc: cancel")
        .style(Style::default().fg(Color::Indexed(245)));
    frame.render_widget(hint, hint_area);

    let list_area = Rect { x: inner.x, y: inner.y, width: inner.width, height: inner.height.saturating_sub(1) };

    if filtered.is_empty() {
        let msg = Paragraph::new("No matches")
            .style(Style::default().fg(Color::Indexed(245)));
        frame.render_widget(msg, list_area);
        return;
    }

    let max_label = filtered.iter().map(|i| i.label.len()).max().unwrap_or(0).min(28);
    let items: Vec<ListItem> = filtered.iter().enumerate().map(|(i, item)| {
        let selected = i == state.cursor;
        let (label_style, detail_style) = if selected {
            (Style::default().fg(Color::Black).bg(Color::Magenta).add_modifier(Modifier::BOLD),
             Style::default().fg(Color::Black).bg(Color::Magenta))
        } else {
            (Style::default().fg(Color::White),
             Style::default().fg(Color::Indexed(245)))
        };
        let line = Line::from(vec![
            Span::styled(format!("{:<width$}", item.label, width = max_label + 1), label_style),
            Span::styled(format!("  {}", item.detail), detail_style),
        ]);
        ListItem::new(line)
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
        "GQL"    => Color::Magenta,
        _        => Color::White,
    }
}

// ── Campaigns panel ───────────────────────────────────────────────────────────

fn render_campaigns_panel(frame: &mut Frame, app: &App, area: Rect) {
    use crate::campaign::CampaignRunState;
    use crate::app::CampaignFocus;

    if app.campaigns.is_empty() {
        render_placeholder(
            frame, area, "Campaigns",
            "No campaigns found — place .toml files in <terapi_dir>/campaigns/",
        );
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    // ── Left: campaign list ───────────────────────────────────────────────────
    let dim = Color::Indexed(250);
    let hint = Color::Indexed(245);

    let list_focused = app.campaign_focus == CampaignFocus::List;
    let left_border_color = if list_focused { Color::Cyan } else { Color::Indexed(240) };

    let list_items: Vec<ListItem> = app.campaigns.iter().enumerate().map(|(i, entry)| {
        let selected = i == app.campaign_cursor;
        let bg = if selected { Color::Indexed(236) } else { Color::Reset };
        let fg = if selected { Color::White } else { Color::Gray };
        let steps = entry.campaign.steps.len();
        let line = Line::from(vec![
            Span::styled(
                format!("  {}", entry.name),
                Style::default().fg(fg).bg(bg).add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() }),
            ),
            Span::styled(
                format!("  ({} steps)", steps),
                Style::default().fg(dim).bg(bg),
            ),
        ]);
        ListItem::new(line)
    }).collect();

    let list = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Campaigns ({}) ", app.campaigns.len()))
                .border_style(Style::default().fg(left_border_color)),
        );
    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(app.campaign_cursor));
    frame.render_stateful_widget(list, chunks[0], &mut list_state);

    // ── Right: run state ──────────────────────────────────────────────────────
    match &app.campaign_run_state {
        CampaignRunState::Idle => {
            if let Some(entry) = app.campaigns.get(app.campaign_cursor) {
                let c = &entry.campaign;
                let mut lines: Vec<Line> = vec![
                    Line::from(vec![
                        Span::styled("  Name       ", Style::default().fg(dim)),
                        Span::styled(c.campaign.name.clone(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    ]),
                ];
                if !c.campaign.description.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled("  Description", Style::default().fg(dim)),
                        Span::styled(format!("  {}", c.campaign.description), Style::default().fg(Color::White)),
                    ]));
                }
                if let Some(ref ef) = c.env_file {
                    lines.push(Line::from(vec![
                        Span::styled("  Env file   ", Style::default().fg(dim)),
                        Span::styled(format!("  {}", ef), Style::default().fg(Color::White)),
                    ]));
                }
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled("  Steps", Style::default().fg(dim)),
                ]));
                for step in &c.steps {
                    let (method_str, method_color) = if step.kind == "transform" {
                        ("TRSF".to_string(), Color::Magenta)
                    } else if step.kind == "pause" {
                        ("WAIT".to_string(), Color::Indexed(245))
                    } else if step.body.as_deref()
                        .and_then(|b| serde_json::from_str::<serde_json::Value>(b).ok())
                        .and_then(|v| v.get("query").cloned())
                        .is_some()
                    {
                        ("GQL".to_string(), Color::Magenta)
                    } else {
                        (step.method.clone(), method_color(&step.method))
                    };
                    let foreach_badge = if step.foreach.is_some() {
                        Span::styled("↻ ", Style::default().fg(Color::Cyan))
                    } else {
                        Span::raw("  ")
                    };
                    lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        foreach_badge,
                        Span::styled(format!("{:<6}", method_str), Style::default().fg(method_color).add_modifier(Modifier::BOLD)),
                        Span::styled(step.name.clone(), Style::default().fg(Color::White)),
                    ]));
                    for a in &step.assert {
                        let label = crate::campaign::assertion_label(a);
                        lines.push(Line::from(vec![
                            Span::styled("          ? ", Style::default().fg(hint)),
                            Span::styled(label, Style::default().fg(hint)),
                        ]));
                    }
                }
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled("  r", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(" to run this campaign", Style::default().fg(dim)),
                ]));
                let right_border_color = if !list_focused { Color::Cyan } else { hint };
                let p = Paragraph::new(lines)
                    .block(Block::default().borders(Borders::ALL)
                        .title(format!(" {} ", entry.name))
                        .border_style(Style::default().fg(right_border_color)))
                    .scroll((app.campaign_result_scroll, 0));
                frame.render_widget(p, chunks[1]);
            }
        }

        CampaignRunState::Running { name, step_results, current_step } => {
            let mut lines: Vec<Line> = vec![
                Line::from(vec![
                    Span::styled("  Running: ", Style::default().fg(dim)),
                    Span::styled(name.clone(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(""),
            ];
            for sr in step_results.iter() {
                lines.push(render_step_result_line(sr));
                for (desc, ok) in &sr.assertion_results {
                    let (sym, color) = if *ok { ("✓", Color::Green) } else { ("✗", Color::Red) };
                    lines.push(Line::from(vec![
                        Span::styled(format!("      {} ", sym), Style::default().fg(color)),
                        Span::styled(desc.clone(), Style::default().fg(color)),
                    ]));
                }
            }
            if let Some(ref step) = current_step {
                lines.push(Line::from(vec![
                    Span::styled("  ⟳ ", Style::default().fg(Color::Yellow)),
                    Span::styled(step.clone(), Style::default().fg(Color::Yellow)),
                    Span::styled("…", Style::default().fg(hint)),
                ]));
            }
            let visible = chunks[1].height.saturating_sub(2) as usize;
            let auto_scroll = (lines.len().saturating_sub(visible)) as u16;
            let p = Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL)
                    .title(format!(" Running: {} ", name))
                    .border_style(Style::default().fg(Color::Yellow)))
                .scroll((auto_scroll, 0));
            frame.render_widget(p, chunks[1]);
        }

        CampaignRunState::Done { name, results } => {
            let total_steps: usize = results.iter().map(|r| r.steps.len()).sum();
            let total_ok:    usize = results.iter().map(|r| r.ok_count()).sum();
            let total_fail:  usize = results.iter().map(|r| r.fail_count()).sum();
            let total_ms:    u64   = results.iter().map(|r| r.total_ms()).sum();
            let verdict_color = if total_fail == 0 { Color::Green } else { Color::Red };
            let verdict = if total_fail == 0 { "✓  ALL PASSED" } else { "✗  SOME STEPS FAILED" };

            let mut lines: Vec<Line> = vec![
                Line::from(vec![
                    Span::styled(format!("  {}", verdict), Style::default().fg(verdict_color).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("  Steps: ", Style::default().fg(dim)),
                    Span::styled(format!("{} ok", total_ok), Style::default().fg(Color::Green)),
                    Span::styled("  /  ", Style::default().fg(dim)),
                    Span::styled(format!("{} failed", total_fail), Style::default().fg(if total_fail > 0 { Color::Red } else { dim })),
                    Span::styled(format!("  ({} total)  {}ms", total_steps, total_ms), Style::default().fg(dim)),
                ]),
                Line::from(""),
            ];

            for iter in results {
                if results.len() > 1 {
                    let row_label = iter.row_vars.iter()
                        .map(|(k, v)| format!("{}={}", k, if v.chars().count() > 15 { v.chars().take(15).collect::<String>() } else { v.clone() }))
                        .collect::<Vec<_>>().join("  ");
                    lines.push(Line::from(vec![
                        Span::styled(format!("  Row {} — ", iter.row_index.map_or(0, |i| i + 1)), Style::default().fg(dim)),
                        Span::styled(row_label, Style::default().fg(Color::White)),
                    ]));
                }
                for sr in &iter.steps {
                    lines.push(render_step_result_line(sr));
                    for (var, val) in &sr.extracted {
                        let v = if val.chars().count() > 40 { format!("{}…", val.chars().take(40).collect::<String>()) } else { val.clone() };
                        lines.push(Line::from(vec![
                            Span::styled(format!("      ↳ {} = {}", var, v), Style::default().fg(hint)),
                        ]));
                    }
                    for (desc, ok) in &sr.assertion_results {
                        let (sym, color) = if *ok { ("✓", Color::Green) } else { ("✗", Color::Red) };
                        lines.push(Line::from(vec![
                            Span::styled(format!("      {} ", sym), Style::default().fg(color)),
                            Span::styled(desc.clone(), Style::default().fg(color)),
                        ]));
                    }
                }
            }

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  Esc", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(" to clear  r to re-run", Style::default().fg(dim)),
            ]));

            let right_border_color = if !list_focused { Color::Cyan } else { verdict_color };
            let p = Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL)
                    .title(format!(" Done: {} ", name))
                    .border_style(Style::default().fg(right_border_color)))
                .scroll((app.campaign_result_scroll, 0))
                .wrap(Wrap { trim: false });
            frame.render_widget(p, chunks[1]);
        }
    }
}

fn render_step_result_line(sr: &crate::campaign::StepResult) -> Line<'static> {
    let (mark, mark_color) = if sr.success { ("✓", Color::Green) } else { ("✗", Color::Red) };
    let status_str = sr.status
        .map(|s| format!("{}", s))
        .unwrap_or_else(|| if sr.error.is_some() { "ERR".into() } else { "-".into() });
    let status_color = sr.status.map(|s| if s < 400 { Color::Green } else { Color::Red })
        .unwrap_or(if sr.error.is_some() { Color::Red } else { Color::Indexed(250) });
    let (method_display, method_c) = if sr.graphql {
        ("GQL".to_string(), Color::Magenta)
    } else {
        (sr.method.clone(), method_color(&sr.method))
    };
    let name = if sr.name.chars().count() > 22 {
        format!("{}…", sr.name.chars().take(21).collect::<String>())
    } else {
        sr.name.clone()
    };
    let err = sr.error.as_deref().unwrap_or("").chars().take(28).collect::<String>();

    let mut spans = vec![
        Span::styled(format!("  {} ", mark), Style::default().fg(mark_color)),
        Span::styled(format!("{:<23}", name), Style::default().fg(Color::White)),
        Span::styled(format!("{:<7}", method_display), Style::default().fg(method_c).add_modifier(Modifier::BOLD)),
        Span::styled(format!("{:<5}", status_str), Style::default().fg(status_color)),
        Span::styled(format!("{:>6}ms  ", sr.duration_ms), Style::default().fg(Color::Indexed(250))),
        Span::styled(err, Style::default().fg(Color::Red)),
    ];
    if !sr.success && sr.non_blocking {
        spans.push(Span::styled("  [↷]", Style::default().fg(Color::Indexed(245))));
    }
    Line::from(spans)
}
