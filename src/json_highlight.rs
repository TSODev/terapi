use std::collections::HashSet;

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use serde_json::Value;

const INDENT: &str = "  ";

pub struct LineInfo {
    pub line: Line<'static>,
    /// Path of the foldable node on this line (JSON Pointer format), if any.
    pub fold_path: Option<String>,
}

/// Render a JSON string as syntax-highlighted ratatui Lines with fold support.
///
/// - `folds`  — set of JSON Pointer paths that are currently collapsed
/// - `cursor` — index of the currently selected line (highlighted background)
pub fn render(json: &str, folds: &HashSet<String>, cursor: usize) -> Vec<LineInfo> {
    match serde_json::from_str::<Value>(json) {
        Ok(value) => {
            let mut infos = Vec::new();
            let mut idx = 0;
            render_value(&value, 0, false, None, "", folds, cursor, &mut idx, &mut infos);
            infos
        }
        Err(e) => vec![LineInfo {
            line: Line::from(Span::styled(
                format!("Parse error: {e}"),
                Style::default().fg(Color::Red),
            )),
            fold_path: None,
        }],
    }
}

fn fg(color: Color) -> Style {
    Style::default().fg(color)
}

fn cursor_style(idx: usize, cursor: usize) -> Style {
    if idx == cursor {
        Style::default().bg(Color::Indexed(237))
    } else {
        Style::default()
    }
}

fn emit(
    spans: Vec<Span<'static>>,
    fold_path: Option<String>,
    idx: usize,
    cursor: usize,
    infos: &mut Vec<LineInfo>,
) {
    infos.push(LineInfo {
        line: Line::from(spans).style(cursor_style(idx, cursor)),
        fold_path,
    });
}

#[allow(clippy::too_many_arguments)]
fn render_value(
    value: &Value,
    depth: usize,
    trailing_comma: bool,
    key_prefix: Option<Vec<Span<'static>>>,
    path: &str,
    folds: &HashSet<String>,
    cursor: usize,
    idx: &mut usize,
    infos: &mut Vec<LineInfo>,
) {
    let indent = INDENT.repeat(depth);
    let comma = if trailing_comma { "," } else { "" };

    match value {
        Value::Null => {
            let i = *idx;
            *idx += 1;
            let mut spans = key_prefix.unwrap_or_else(|| vec![Span::raw(indent)]);
            spans.push(Span::styled("null", fg(Color::DarkGray)));
            if trailing_comma {
                spans.push(Span::raw(","));
            }
            emit(spans, None, i, cursor, infos);
        }

        Value::Bool(b) => {
            let i = *idx;
            *idx += 1;
            let mut spans = key_prefix.unwrap_or_else(|| vec![Span::raw(indent)]);
            spans.push(Span::styled(
                if *b { "true" } else { "false" },
                fg(Color::Magenta),
            ));
            if trailing_comma {
                spans.push(Span::raw(","));
            }
            emit(spans, None, i, cursor, infos);
        }

        Value::Number(n) => {
            let i = *idx;
            *idx += 1;
            let mut spans = key_prefix.unwrap_or_else(|| vec![Span::raw(indent)]);
            spans.push(Span::styled(n.to_string(), fg(Color::Yellow)));
            if trailing_comma {
                spans.push(Span::raw(","));
            }
            emit(spans, None, i, cursor, infos);
        }

        Value::String(s) => {
            let i = *idx;
            *idx += 1;
            let mut spans = key_prefix.unwrap_or_else(|| vec![Span::raw(indent)]);
            spans.push(Span::styled(format!("\"{}\"", s), fg(Color::Green)));
            if trailing_comma {
                spans.push(Span::raw(","));
            }
            emit(spans, None, i, cursor, infos);
        }

        Value::Array(arr) if arr.is_empty() => {
            let i = *idx;
            *idx += 1;
            let mut spans = key_prefix.unwrap_or_else(|| vec![Span::raw(indent)]);
            spans.push(Span::styled("[]", fg(Color::White)));
            if trailing_comma {
                spans.push(Span::raw(","));
            }
            emit(spans, None, i, cursor, infos);
        }

        Value::Array(arr) => {
            let is_folded = folds.contains(path);
            let i = *idx;
            *idx += 1;
            let icon = if is_folded { "▶ " } else { "▼ " };

            if is_folded {
                let mut spans = key_prefix.unwrap_or_else(|| vec![Span::raw(indent)]);
                spans.push(Span::styled(icon, fg(Color::DarkGray)));
                spans.push(Span::styled("[", fg(Color::White)));
                spans.push(Span::styled(
                    format!(" {} ", arr.len()),
                    fg(Color::DarkGray),
                ));
                spans.push(Span::styled("]", fg(Color::White)));
                if trailing_comma {
                    spans.push(Span::raw(","));
                }
                emit(spans, Some(path.to_string()), i, cursor, infos);
            } else {
                let mut open = key_prefix.unwrap_or_else(|| vec![Span::raw(indent.clone())]);
                open.push(Span::styled(icon, fg(Color::DarkGray)));
                open.push(Span::styled("[", fg(Color::White)));
                emit(open, Some(path.to_string()), i, cursor, infos);

                let last = arr.len() - 1;
                for (j, item) in arr.iter().enumerate() {
                    render_value(
                        item,
                        depth + 1,
                        j < last,
                        None,
                        &format!("{}/{}", path, j),
                        folds,
                        cursor,
                        idx,
                        infos,
                    );
                }

                let ci = *idx;
                *idx += 1;
                emit(
                    vec![Span::raw(format!("{indent}]{comma}"))],
                    None,
                    ci,
                    cursor,
                    infos,
                );
            }
        }

        Value::Object(map) if map.is_empty() => {
            let i = *idx;
            *idx += 1;
            let mut spans = key_prefix.unwrap_or_else(|| vec![Span::raw(indent)]);
            spans.push(Span::styled("{}", fg(Color::White)));
            if trailing_comma {
                spans.push(Span::raw(","));
            }
            emit(spans, None, i, cursor, infos);
        }

        Value::Object(map) => {
            let is_folded = folds.contains(path);
            let i = *idx;
            *idx += 1;
            let icon = if is_folded { "▶ " } else { "▼ " };

            if is_folded {
                let mut spans = key_prefix.unwrap_or_else(|| vec![Span::raw(indent)]);
                spans.push(Span::styled(icon, fg(Color::DarkGray)));
                spans.push(Span::styled("{", fg(Color::White)));
                spans.push(Span::styled(
                    format!(" {} ", map.len()),
                    fg(Color::DarkGray),
                ));
                spans.push(Span::styled("}", fg(Color::White)));
                if trailing_comma {
                    spans.push(Span::raw(","));
                }
                emit(spans, Some(path.to_string()), i, cursor, infos);
            } else {
                let mut open = key_prefix.unwrap_or_else(|| vec![Span::raw(indent.clone())]);
                open.push(Span::styled(icon, fg(Color::DarkGray)));
                open.push(Span::styled("{", fg(Color::White)));
                emit(open, Some(path.to_string()), i, cursor, infos);

                let last = map.len() - 1;
                for (j, (key, val)) in map.iter().enumerate() {
                    let prefix = vec![
                        Span::raw(INDENT.repeat(depth + 1)),
                        Span::styled(format!("\"{}\"", key), fg(Color::Cyan)),
                        Span::styled(": ", fg(Color::White)),
                    ];
                    render_value(
                        val,
                        depth + 1,
                        j < last,
                        Some(prefix),
                        &format!("{}/{}", path, key),
                        folds,
                        cursor,
                        idx,
                        infos,
                    );
                }

                let ci = *idx;
                *idx += 1;
                emit(
                    vec![Span::raw(format!("{indent}}}{comma}"))],
                    None,
                    ci,
                    cursor,
                    infos,
                );
            }
        }
    }
}
