use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use super::*;

impl App {
    pub fn active_env_vars(&self) -> Vec<(String, String)> {
        let mut vars: Vec<(String, String)> = self.active_env_idx
            .and_then(|i| self.environments.get(i))
            .map(|e| e.vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        vars.sort_by(|a, b| a.0.cmp(&b.0));
        vars
    }

    pub fn filtered_var_names(&self, prefix: &str) -> Vec<String> {
        self.active_env_vars()
            .into_iter()
            .filter(|(k, _)| k.to_lowercase().starts_with(&prefix.to_lowercase()))
            .map(|(k, _)| k)
            .collect()
    }

    pub(super) fn open_var_picker(&mut self, target: VarPickerTarget) {
        if self.active_env_idx.is_none() {
            self.status_message = "No active environment — activate one in the Env tab first".into();
            return;
        }
        if self.active_env_vars().is_empty() {
            self.status_message = "Active environment has no variables".into();
            return;
        }
        self.var_picker = Some(VarPickerState { target, prefix: String::new(), cursor: 0 });
    }

    pub(super) fn handle_var_picker_key(&mut self, key: KeyEvent) -> Result<()> {
        let Some(mut picker) = self.var_picker.take() else { return Ok(()); };
        let vars = self.filtered_var_names(&picker.prefix);

        match key.code {
            KeyCode::Esc => {}
            KeyCode::Up => {
                if picker.cursor > 0 { picker.cursor -= 1; }
                self.var_picker = Some(picker);
            }
            KeyCode::Down => {
                if picker.cursor + 1 < vars.len() { picker.cursor += 1; }
                self.var_picker = Some(picker);
            }
            KeyCode::Enter => {
                if !vars.is_empty() {
                    let var_name = vars[picker.cursor].clone();
                    self.insert_var_into_target(&picker.target, &var_name, &picker.prefix);
                }
            }
            KeyCode::Backspace => {
                if picker.prefix.is_empty() {
                    self.backspace_in_target(&picker.target);
                } else {
                    picker.prefix.pop();
                    picker.cursor = 0;
                    self.var_picker = Some(picker);
                }
            }
            KeyCode::Char(c) => {
                picker.prefix.push(c);
                picker.cursor = 0;
                let still_matches = !self.filtered_var_names(&picker.prefix).is_empty();
                if still_matches {
                    self.var_picker = Some(picker);
                } else {
                    self.push_char_to_target(&picker.target, c);
                }
            }
            _ => { self.var_picker = Some(picker); }
        }
        Ok(())
    }

    fn insert_var_into_target(&mut self, target: &VarPickerTarget, var_name: &str, prefix: &str) {
        let remove_count = 2 + prefix.len();
        let insert = format!("{{{{{}}}}}", var_name);
        match target {
            VarPickerTarget::Url => {
                for _ in 0..remove_count {
                    self.url_textarea.delete_char();
                }
                self.url_textarea.insert_str(&insert);
            }
            VarPickerTarget::ModalValue => {
                if let Some(modal) = &mut self.modal {
                    let val = match modal {
                        ModalState::NewHeader { value, .. } => Some(value),
                        ModalState::UrlParam { value, .. } => Some(value),
                        ModalState::BodyPair { value, .. } => Some(value),
                        _ => None,
                    };
                    if let Some(v) = val {
                        let new_len = v.len().saturating_sub(remove_count);
                        v.truncate(new_len);
                        v.push_str(&insert);
                    }
                }
            }
            VarPickerTarget::BodyText => {
                if self.graphql_mode {
                    for _ in 0..remove_count {
                        self.graphql_query_textarea.delete_char();
                    }
                    self.graphql_query_textarea.insert_str(&insert);
                } else {
                    for _ in 0..remove_count {
                        self.body_textarea.delete_char();
                    }
                    self.body_textarea.insert_str(&insert);
                }
            }
        }
    }

    fn push_char_to_target(&mut self, target: &VarPickerTarget, c: char) {
        match target {
            VarPickerTarget::Url => { self.url_textarea.insert_str(&c.to_string()); }
            VarPickerTarget::ModalValue => {
                if let Some(modal) = &mut self.modal {
                    match modal {
                        ModalState::NewHeader { value, .. }
                        | ModalState::UrlParam { value, .. }
                        | ModalState::BodyPair { value, .. } => { value.push(c); }
                        _ => {}
                    }
                }
            }
            VarPickerTarget::BodyText => {
                if self.graphql_mode {
                    self.graphql_query_textarea.insert_str(&c.to_string());
                } else {
                    self.body_textarea.insert_str(&c.to_string());
                }
            }
        }
    }

    fn backspace_in_target(&mut self, target: &VarPickerTarget) {
        match target {
            VarPickerTarget::Url => { self.url_textarea.delete_char(); }
            VarPickerTarget::ModalValue => {
                if let Some(modal) = &mut self.modal {
                    match modal {
                        ModalState::NewHeader { value, .. }
                        | ModalState::UrlParam { value, .. }
                        | ModalState::BodyPair { value, .. } => { value.pop(); }
                        _ => {}
                    }
                }
            }
            VarPickerTarget::BodyText => {
                if self.graphql_mode {
                    self.graphql_query_textarea.delete_char();
                } else {
                    self.body_textarea.delete_char();
                }
            }
        }
    }
}
