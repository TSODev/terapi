use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use serde_json::Value;

const INDENT: &str = "  ";

/// Parse and syntax-highlight a JSON string into ratatui Lines.
/// Returns an error line on invalid input.
pub fn highlight(json: &str) -> Vec<Line<'static>> {
    match serde_json::from_str::<Value>(json) {
        Ok(value) => {
            let mut lines = Vec::new();
            render_value(&value, 0, false, None, &mut lines);
            lines
        }
        Err(e) => vec![Line::from(Span::styled(
            format!("Parse error: {e}"),
            Style::default().fg(Color::Red),
        ))],
    }
}

fn style(color: Color) -> Style {
    Style::default().fg(color)
}

/// Render a JSON value as ratatui Lines.
///
/// - `depth`          — current indentation level
/// - `trailing_comma` — append a comma after the value
/// - `key_prefix`     — spans to prepend on the opening line (object key + colon)
fn render_value(
    value: &Value,
    depth: usize,
    trailing_comma: bool,
    key_prefix: Option<Vec<Span<'static>>>,
    lines: &mut Vec<Line<'static>>,
) {
    let indent = INDENT.repeat(depth);
    let comma = if trailing_comma { "," } else { "" };

    match value {
        Value::Null => {
            let mut spans = key_prefix.unwrap_or_else(|| vec![Span::raw(indent)]);
            spans.push(Span::styled("null", style(Color::DarkGray)));
            if trailing_comma {
                spans.push(Span::raw(","));
            }
            lines.push(Line::from(spans));
        }

        Value::Bool(b) => {
            let mut spans = key_prefix.unwrap_or_else(|| vec![Span::raw(indent)]);
            spans.push(Span::styled(
                if *b { "true" } else { "false" },
                style(Color::Magenta),
            ));
            if trailing_comma {
                spans.push(Span::raw(","));
            }
            lines.push(Line::from(spans));
        }

        Value::Number(n) => {
            let mut spans = key_prefix.unwrap_or_else(|| vec![Span::raw(indent)]);
            spans.push(Span::styled(n.to_string(), style(Color::Yellow)));
            if trailing_comma {
                spans.push(Span::raw(","));
            }
            lines.push(Line::from(spans));
        }

        Value::String(s) => {
            let mut spans = key_prefix.unwrap_or_else(|| vec![Span::raw(indent)]);
            spans.push(Span::styled(format!("\"{}\"", s), style(Color::Green)));
            if trailing_comma {
                spans.push(Span::raw(","));
            }
            lines.push(Line::from(spans));
        }

        Value::Array(arr) => {
            let mut open = key_prefix.unwrap_or_else(|| vec![Span::raw(indent.clone())]);
            open.push(Span::styled("[", style(Color::White)));
            lines.push(Line::from(open));

            let last = arr.len().saturating_sub(1);
            for (i, item) in arr.iter().enumerate() {
                render_value(item, depth + 1, i < last, None, lines);
            }

            lines.push(Line::from(Span::raw(format!("{indent}]{comma}"))));
        }

        Value::Object(map) => {
            let mut open = key_prefix.unwrap_or_else(|| vec![Span::raw(indent.clone())]);
            open.push(Span::styled("{", style(Color::White)));
            lines.push(Line::from(open));

            let last = map.len().saturating_sub(1);
            for (i, (key, val)) in map.iter().enumerate() {
                let inner = INDENT.repeat(depth + 1);
                let prefix = vec![
                    Span::raw(inner),
                    Span::styled(format!("\"{}\"", key), style(Color::Cyan)),
                    Span::styled(": ", style(Color::White)),
                ];
                render_value(val, depth + 1, i < last, Some(prefix), lines);
            }

            lines.push(Line::from(Span::raw(format!("{indent}}}{comma}"))));
        }
    }
}
