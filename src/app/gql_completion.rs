use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use super::*;

impl App {
    /// Open the GQL completion overlay at the current cursor position.
    pub(super) fn open_gql_completion(&mut self) {
        let items = self.build_completion_items();
        if items.is_empty() {
            self.status_message =
                "No schema loaded — go to Schema tab and press f to introspect".into();
            return;
        }
        let prefix = self.gql_word_before_cursor();
        self.gql_completion = Some(GqlCompletionState { items, prefix, cursor: 0 });
    }

    pub(super) fn handle_gql_completion_key(&mut self, key: KeyEvent) -> Result<()> {
        let Some(mut state) = self.gql_completion.take() else { return Ok(()); };
        let filtered = Self::filter_completions(&state.items, &state.prefix);

        match key.code {
            KeyCode::Esc => {}

            KeyCode::Up => {
                if state.cursor > 0 { state.cursor -= 1; }
                self.gql_completion = Some(state);
            }
            KeyCode::Down => {
                if state.cursor + 1 < filtered.len() { state.cursor += 1; }
                self.gql_completion = Some(state);
            }

            KeyCode::Enter | KeyCode::Tab => {
                if let Some(item) = filtered.get(state.cursor) {
                    let label = item.label.clone();
                    let prefix_len = state.prefix.len();
                    self.insert_gql_completion(&label, prefix_len);
                }
            }

            KeyCode::Backspace => {
                if state.prefix.is_empty() {
                    self.graphql_query_textarea.delete_char();
                } else {
                    state.prefix.pop();
                    state.cursor = 0;
                    self.gql_completion = Some(state);
                }
            }

            KeyCode::Char(c) => {
                let mut new_prefix = state.prefix.clone();
                new_prefix.push(c);
                let matches = Self::filter_completions(&state.items, &new_prefix);
                if !matches.is_empty() {
                    state.prefix = new_prefix;
                    state.cursor = 0;
                    self.gql_completion = Some(state);
                } else {
                    // No matches — close and pass the char through to the textarea
                    self.graphql_query_textarea.insert_str(&c.to_string());
                }
            }

            _ => { self.gql_completion = Some(state); }
        }
        Ok(())
    }

    pub fn filter_completions<'a>(
        items: &'a [GqlCompletionItem],
        prefix: &str,
    ) -> Vec<&'a GqlCompletionItem> {
        if prefix.is_empty() {
            items.iter().collect()
        } else {
            items.iter()
                .filter(|i| i.label.to_lowercase().starts_with(&prefix.to_lowercase()))
                .collect()
        }
    }

    /// Extract the partial identifier being typed just before the cursor.
    fn gql_word_before_cursor(&self) -> String {
        let (row, col) = self.graphql_query_textarea.cursor();
        let lines = self.graphql_query_textarea.lines();
        let line = lines.get(row).map(|l| l.as_str()).unwrap_or("");
        let before: &str = &line[..col.min(line.len())];
        before
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>()
            .chars()
            .rev()
            .collect()
    }

    /// Insert `label` into the query textarea, replacing `prefix_len` chars before cursor.
    fn insert_gql_completion(&mut self, label: &str, prefix_len: usize) {
        for _ in 0..prefix_len {
            self.graphql_query_textarea.delete_char();
        }
        self.graphql_query_textarea.insert_str(label);
    }

    /// Build completion items from the current schema state.
    fn build_completion_items(&self) -> Vec<GqlCompletionItem> {
        match &self.schema_state {
            SchemaState::Ready { types, detail } => {
                // If a type detail is loaded, prefer its fields
                if let SchemaDetail::Loaded(d) = detail {
                    let mut items: Vec<GqlCompletionItem> = d.fields.iter().map(|f| {
                        GqlCompletionItem {
                            label: f.name.clone(),
                            detail: f.type_str.clone(),
                        }
                    }).collect();
                    // Also add input_fields for InputObject types
                    for f in &d.input_fields {
                        items.push(GqlCompletionItem {
                            label: f.name.clone(),
                            detail: f.type_str.clone(),
                        });
                    }
                    if !items.is_empty() {
                        return items;
                    }
                }
                // Fall back to type names (useful at root level)
                types.iter()
                    .filter(|t| matches!(t.kind.as_str(), "OBJECT" | "INTERFACE" | "INPUT_OBJECT"))
                    .map(|t| GqlCompletionItem {
                        label: t.name.clone(),
                        detail: t.kind.clone(),
                    })
                    .collect()
            }
            _ => vec![],
        }
    }
}
