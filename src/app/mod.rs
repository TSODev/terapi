use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::collections::HashSet;
use tokio::sync::mpsc;
use tui_textarea::TextArea;

use crate::storage::{HistoryEntry, StoredCollection, StoredEnv, StoredRequest};

mod collections;
mod envs;
mod http;
mod request;
mod response;
mod types;
mod var_picker;

pub use types::*;

// ── App state ─────────────────────────────────────────────────────────────────

pub struct App {
    pub running: bool,
    pub active_tab: Tab,
    pub active_request_tab: RequestTab,
    // Collections
    pub stored_collections: Vec<StoredCollection>,
    pub expanded_nodes: HashSet<String>,
    pub collection_cursor: usize,
    // Environments
    pub environments: Vec<StoredEnv>,
    pub active_env_idx: Option<usize>,
    pub env_cursor: usize,
    pub env_var_cursor: usize,
    pub env_focus: EnvFocus,
    // Modal
    pub modal: Option<ModalState>,
    pub var_picker: Option<VarPickerState>,
    // Request builder
    pub request_url: String,
    pub request_method_idx: usize,
    pub request_url_params: Vec<(String, String)>,
    pub url_params_cursor: usize,
    pub request_headers: Vec<(String, String)>,
    pub header_cursor: usize,
    pub body_mode: BodyMode,
    pub body_textarea: TextArea<'static>,
    pub body_json_pairs: Vec<(String, String)>,
    pub body_json_cursor: usize,
    pub request_focus: RequestFocus,
    pub request_loading: bool,
    // Response
    pub skip_tls_verify: bool,
    pub last_request_raw: Option<RawRequest>,
    pub response_body: Option<String>,
    pub response_status: Option<u16>,
    pub response_elapsed_ms: Option<u64>,
    pub response_headers: Vec<(String, String)>,
    pub response_view: ResponseView,
    pub response_cursor: usize,
    pub response_scroll: u16,
    pub response_folds: HashSet<String>,
    pub key_col_width: u16,
    pub status_message: String,
    // History
    pub history: Vec<HistoryEntry>,
    pub history_cursor: usize,
    // Async channel — receives HTTP results from spawned tasks
    pub(super) response_rx: mpsc::UnboundedReceiver<HttpOutcome>,
    pub(super) response_tx: mpsc::UnboundedSender<HttpOutcome>,
}

impl App {
    pub fn new(response_body: Option<String>) -> Self {
        let stored_collections = match crate::storage::load_collections() {
            Ok(cols) if !cols.is_empty() => cols,
            _ => Self::sample_stored_collections(),
        };
        let mut expanded_nodes = HashSet::new();
        if !stored_collections.is_empty() {
            expanded_nodes.insert("c0".to_string());
        }
        let environments = crate::storage::load_envs().unwrap_or_default();
        let history = crate::storage::load_history().unwrap_or_default();
        let (response_tx, response_rx) = mpsc::unbounded_channel();
        Self {
            running: true,
            active_tab: Tab::Request,
            active_request_tab: RequestTab::Description,
            stored_collections,
            expanded_nodes,
            collection_cursor: 0,
            environments,
            active_env_idx: None,
            env_cursor: 0,
            env_var_cursor: 0,
            env_focus: EnvFocus::Envs,
            modal: None,
            var_picker: None,
            request_url: String::new(),
            request_method_idx: 0,
            request_url_params: Vec::new(),
            url_params_cursor: 0,
            request_headers: Vec::new(),
            header_cursor: 0,
            body_mode: BodyMode::Text,
            body_textarea: TextArea::default(),
            body_json_pairs: Vec::new(),
            body_json_cursor: 0,
            request_focus: RequestFocus::Response,
            request_loading: false,
            skip_tls_verify: false,
            last_request_raw: None,
            response_body,
            response_status: None,
            response_elapsed_ms: None,
            response_headers: Vec::new(),
            response_view: ResponseView::Json,
            response_cursor: 0,
            response_scroll: 0,
            response_folds: HashSet::new(),
            key_col_width: 22,
            status_message: "Tab: panels  e: edit URL  s: send  S: save  n: new  m: method  ←/→: section  ↑/↓: cursor  r: raw  q: quit".into(),
            history,
            history_cursor: 0,
            response_rx,
            response_tx,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if self.var_picker.is_some() {
            return self.handle_var_picker_key(key);
        }

        if self.modal.is_some() {
            return self.handle_modal_key(key);
        }

        // Body editor intercepts all keys when focused
        if self.active_tab == Tab::Request && self.request_focus == RequestFocus::Body {
            return self.handle_body_key(key);
        }

        match key.code {
            // URL edit mode Esc must come before the global quit handler
            KeyCode::Esc
                if self.active_tab == Tab::Request
                    && self.request_focus == RequestFocus::Url =>
            {
                self.request_focus = RequestFocus::Response;
                self.status_message = "Tab: panels  e: edit URL  s: send  m: method  ←/→: section  ↑/↓: cursor  r: raw  q: quit".into();
            }

            KeyCode::Char('q') | KeyCode::Esc => {
                self.running = false;
            }
            KeyCode::Tab => {
                self.active_tab = match self.active_tab {
                    Tab::Request => Tab::Collections,
                    Tab::Collections => Tab::Env,
                    Tab::Env => Tab::History,
                    Tab::History => Tab::Request,
                };
                self.status_message = match self.active_tab {
                    Tab::Request => "Tab: switch panel  ←/→: section  q: quit".into(),
                    Tab::Collections => "Tab: switch panel  ↑/↓: navigate  Enter: expand/load  n: new  f: folder  a: add  e: edit  d: delete  q: quit".into(),
                    Tab::Env => "Tab: switch panel  ←/→: switch focus  ↑/↓: navigate  Enter: activate  n: new env  a: add var  d: delete  q: quit".into(),
                    Tab::History => "Tab: switch panel  ↑/↓: navigate  Enter: load  d: delete  q: quit".into(),
                };
            }

            // ── Request panel — URL edit mode ──────────────────────────────
            KeyCode::Enter
                if self.active_tab == Tab::Request
                    && self.request_focus == RequestFocus::Url =>
            {
                self.send_request();
            }
            KeyCode::Up
                if self.active_tab == Tab::Request
                    && self.request_focus == RequestFocus::Url =>
            {
                self.request_method_idx = if self.request_method_idx == 0 {
                    METHODS.len() - 1
                } else {
                    self.request_method_idx - 1
                };
            }
            KeyCode::Down
                if self.active_tab == Tab::Request
                    && self.request_focus == RequestFocus::Url =>
            {
                self.request_method_idx = (self.request_method_idx + 1) % METHODS.len();
            }
            KeyCode::Left
                if self.active_tab == Tab::Request
                    && self.request_focus == RequestFocus::Url =>
            {
                self.request_focus = RequestFocus::Response;
                self.active_request_tab = self.active_request_tab.prev();
                self.update_request_status_hint();
            }
            KeyCode::Right
                if self.active_tab == Tab::Request
                    && self.request_focus == RequestFocus::Url =>
            {
                self.request_focus = RequestFocus::Response;
                self.active_request_tab = self.active_request_tab.next();
                self.update_request_status_hint();
            }
            KeyCode::Backspace
                if self.active_tab == Tab::Request
                    && self.request_focus == RequestFocus::Url =>
            {
                self.request_url.pop();
            }
            KeyCode::Char(c)
                if self.active_tab == Tab::Request
                    && self.request_focus == RequestFocus::Url =>
            {
                self.request_url.push(c);
                if self.request_url.ends_with("{{") {
                    self.open_var_picker(VarPickerTarget::Url);
                }
            }

            // ── Request panel — response navigation mode ───────────────────
            KeyCode::Char('n') if self.active_tab == Tab::Request => {
                self.new_request();
            }
            KeyCode::Char('S') if self.active_tab == Tab::Request => {
                if self.stored_collections.is_empty() {
                    self.status_message = "No collections — create one first in the Collections tab".into();
                } else {
                    self.modal = Some(ModalState::SaveRequest {
                        name: String::new(),
                        collection_idx: 0,
                        folder_display_idx: 0,
                        active_field: SaveField::Name,
                    });
                }
            }
            KeyCode::Char('e') if self.active_tab == Tab::Request => {
                self.request_focus = RequestFocus::Url;
                self.status_message = "URL: type address  ↑/↓: method  ←/→: section  Enter: send  Esc: done".into();
            }
            KeyCode::Char('i')
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::Body =>
            {
                self.request_focus = RequestFocus::Body;
                self.status_message = match self.body_mode {
                    BodyMode::Text => "Body [Text]: editing  Esc: exit editor".into(),
                    BodyMode::Json => "Body [JSON]: ↑↓: navigate  a: add  d: delete  Enter: edit  Esc: exit".into(),
                };
            }
            KeyCode::Char('t')
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::Body
                    && self.request_focus != RequestFocus::Body =>
            {
                self.toggle_body_mode();
            }
            KeyCode::Char('m') if self.active_tab == Tab::Request => {
                self.request_method_idx = (self.request_method_idx + 1) % METHODS.len();
            }
            KeyCode::Char('s') if self.active_tab == Tab::Request => {
                self.send_request();
            }
            KeyCode::Char('a')
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::UrlParams =>
            {
                self.modal = Some(ModalState::UrlParam {
                    key: String::new(),
                    value: String::new(),
                    active_field: VarField::Key,
                    edit_idx: None,
                });
            }
            KeyCode::Char('d')
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::UrlParams
                    && !self.request_url_params.is_empty() =>
            {
                self.request_url_params.remove(self.url_params_cursor);
                if self.url_params_cursor > 0 && self.url_params_cursor >= self.request_url_params.len() {
                    self.url_params_cursor -= 1;
                }
            }
            KeyCode::Enter
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::UrlParams
                    && !self.request_url_params.is_empty() =>
            {
                let (k, v) = self.request_url_params[self.url_params_cursor].clone();
                self.modal = Some(ModalState::UrlParam {
                    key: k,
                    value: v,
                    active_field: VarField::Key,
                    edit_idx: Some(self.url_params_cursor),
                });
            }
            KeyCode::Up
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::UrlParams =>
            {
                if self.url_params_cursor > 0 {
                    self.url_params_cursor -= 1;
                }
            }
            KeyCode::Down
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::UrlParams =>
            {
                if self.url_params_cursor + 1 < self.request_url_params.len() {
                    self.url_params_cursor += 1;
                }
            }
            KeyCode::Char(' ') | KeyCode::Enter
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::Options =>
            {
                self.skip_tls_verify = !self.skip_tls_verify;
                let state = if self.skip_tls_verify { "enabled" } else { "disabled" };
                self.status_message = format!("Skip TLS verify: {}  —  Space/Enter: toggle  ←/→: section  s: send  q: quit", state);
            }
            KeyCode::Char('a')
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::Headers =>
            {
                self.modal = Some(ModalState::HeaderPicker { cursor: 0 });
            }
            KeyCode::Char('d')
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::Headers
                    && !self.request_headers.is_empty() =>
            {
                self.request_headers.remove(self.header_cursor);
                if self.header_cursor > 0 && self.header_cursor >= self.request_headers.len() {
                    self.header_cursor -= 1;
                }
            }
            KeyCode::Up
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::Headers =>
            {
                if self.header_cursor > 0 {
                    self.header_cursor -= 1;
                }
            }
            KeyCode::Down
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::Headers =>
            {
                if self.header_cursor + 1 < self.request_headers.len() {
                    self.header_cursor += 1;
                }
            }
            KeyCode::Right if self.active_tab == Tab::Request => {
                self.active_request_tab = self.active_request_tab.next();
                self.update_request_status_hint();
            }
            KeyCode::Left if self.active_tab == Tab::Request => {
                self.active_request_tab = self.active_request_tab.prev();
                self.update_request_status_hint();
            }
            KeyCode::Char('r') if self.active_tab == Tab::Request => {
                self.response_view = match self.response_view {
                    ResponseView::Json => ResponseView::Raw,
                    ResponseView::Raw => ResponseView::Http,
                    ResponseView::Http => ResponseView::Json,
                };
                self.response_cursor = 0;
                self.response_scroll = 0;
            }
            KeyCode::Up if self.active_tab == Tab::Request => {
                match self.response_view {
                    ResponseView::Json => {
                        self.response_cursor = self.response_cursor.saturating_sub(1);
                        self.sync_scroll();
                    }
                    ResponseView::Raw | ResponseView::Http => {
                        self.response_scroll = self.response_scroll.saturating_sub(1);
                    }
                }
            }
            KeyCode::Down if self.active_tab == Tab::Request => {
                match self.response_view {
                    ResponseView::Json => {
                        let len = self.response_line_count();
                        if self.response_cursor + 1 < len {
                            self.response_cursor += 1;
                        }
                        self.sync_scroll();
                    }
                    ResponseView::Raw | ResponseView::Http => {
                        self.response_scroll = self.response_scroll.saturating_add(1);
                    }
                }
            }
            KeyCode::Enter if self.active_tab == Tab::Request => {
                if self.response_view == ResponseView::Json {
                    self.toggle_response_fold();
                }
            }
            KeyCode::Char('-') if self.active_tab == Tab::Request => {
                self.key_col_width = self.key_col_width.saturating_sub(2).max(8);
            }
            KeyCode::Char('=') if self.active_tab == Tab::Request => {
                self.key_col_width = (self.key_col_width + 2).min(50);
            }

            // ── Collections panel ──────────────────────────────────────────
            KeyCode::Up if self.active_tab == Tab::Collections => {
                if self.collection_cursor > 0 {
                    self.collection_cursor -= 1;
                }
            }
            KeyCode::Down if self.active_tab == Tab::Collections => {
                let flat = flatten_stored(&self.stored_collections, &self.expanded_nodes);
                if self.collection_cursor + 1 < flat.len() {
                    self.collection_cursor += 1;
                }
            }
            KeyCode::Enter if self.active_tab == Tab::Collections => {
                self.toggle_collection_cursor();
            }
            KeyCode::Char('n') if self.active_tab == Tab::Collections => {
                self.modal = Some(ModalState::NewCollection { input: String::new() });
            }
            KeyCode::Char('f') if self.active_tab == Tab::Collections => {
                if let Some((ci, _)) = self.cursor_insertion_context() {
                    self.modal = Some(ModalState::NewFolder { input: String::new(), collection_idx: ci });
                } else {
                    self.status_message = "No collection selected — press n to create one first.".into();
                }
            }
            KeyCode::Char('a') if self.active_tab == Tab::Collections => {
                if let Some((ci, fi)) = self.cursor_insertion_context() {
                    self.modal = Some(ModalState::NewRequest {
                        name: String::new(),
                        method_idx: 0,
                        url: String::new(),
                        active_field: InputField::Name,
                        collection_idx: ci,
                        folder_idx: fi,
                    });
                } else {
                    self.status_message = "No collection selected — press n to create one first.".into();
                }
            }
            KeyCode::Char('e') if self.active_tab == Tab::Collections => {
                let flat = flatten_stored(&self.stored_collections, &self.expanded_nodes);
                if let Some(node) = flat.get(self.collection_cursor) {
                    if !node.is_folder {
                        let (ci, fi, ri) = match &node.address {
                            NodeAddress::RootRequest(ci, ri) => (*ci, None, *ri),
                            NodeAddress::FolderRequest(ci, fi, ri) => (*ci, Some(*fi), *ri),
                            _ => return Ok(()),
                        };
                        let req = if let Some(fi) = fi {
                            &self.stored_collections[ci].folders[fi].requests[ri]
                        } else {
                            &self.stored_collections[ci].requests[ri]
                        };
                        let method_idx = METHODS.iter().position(|&m| m == req.method).unwrap_or(0);
                        self.modal = Some(ModalState::EditRequest {
                            name: req.name.clone(),
                            method_idx,
                            url: req.url.clone(),
                            active_field: InputField::Name,
                            collection_idx: ci,
                            folder_idx: fi,
                            request_idx: ri,
                        });
                    }
                }
            }
            KeyCode::Char('d') if self.active_tab == Tab::Collections => {
                self.open_delete_modal();
            }

            // ── Env panel ──────────────────────────────────────────────────
            KeyCode::Left if self.active_tab == Tab::Env => {
                self.env_focus = EnvFocus::Envs;
            }
            KeyCode::Right if self.active_tab == Tab::Env => {
                self.env_focus = EnvFocus::Vars;
            }
            KeyCode::Up if self.active_tab == Tab::Env => {
                match self.env_focus {
                    EnvFocus::Envs => {
                        if self.env_cursor > 0 {
                            self.env_cursor -= 1;
                            self.env_var_cursor = 0;
                        }
                    }
                    EnvFocus::Vars => {
                        if self.env_var_cursor > 0 {
                            self.env_var_cursor -= 1;
                        }
                    }
                }
            }
            KeyCode::Down if self.active_tab == Tab::Env => {
                match self.env_focus {
                    EnvFocus::Envs => {
                        if self.env_cursor + 1 < self.environments.len() {
                            self.env_cursor += 1;
                            self.env_var_cursor = 0;
                        }
                    }
                    EnvFocus::Vars => {
                        let count = self.environments
                            .get(self.env_cursor)
                            .map_or(0, |e| e.vars.len());
                        if self.env_var_cursor + 1 < count {
                            self.env_var_cursor += 1;
                        }
                    }
                }
            }
            KeyCode::Enter if self.active_tab == Tab::Env && self.env_focus == EnvFocus::Envs => {
                if self.env_cursor < self.environments.len() {
                    self.active_env_idx = Some(self.env_cursor);
                    self.status_message = format!(
                        "Active env: {}",
                        self.environments[self.env_cursor].env.name
                    );
                }
            }
            KeyCode::Char('n') if self.active_tab == Tab::Env => {
                self.modal = Some(ModalState::NewEnv { input: String::new() });
            }
            KeyCode::Char('a') if self.active_tab == Tab::Env => {
                if !self.environments.is_empty() {
                    self.modal = Some(ModalState::NewVar {
                        key: String::new(),
                        value: String::new(),
                        active_field: VarField::Key,
                        env_idx: self.env_cursor,
                    });
                } else {
                    self.status_message = "No environment — press n to create one first.".into();
                }
            }
            KeyCode::Char('d') if self.active_tab == Tab::Env => {
                self.open_env_delete_modal();
            }

            // ── History panel ──────────────────────────────────────────────
            KeyCode::Up if self.active_tab == Tab::History => {
                if self.history_cursor > 0 { self.history_cursor -= 1; }
            }
            KeyCode::Down if self.active_tab == Tab::History => {
                if self.history_cursor + 1 < self.history.len() {
                    self.history_cursor += 1;
                }
            }
            KeyCode::Enter if self.active_tab == Tab::History => {
                self.load_from_history(self.history_cursor);
            }
            KeyCode::Char('d') if self.active_tab == Tab::History => {
                self.delete_history_entry(self.history_cursor);
            }

            _ => {}
        }
        Ok(())
    }

    fn handle_modal_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.modal.take() {
            Some(ModalState::NewCollection { mut input }) => match key.code {
                KeyCode::Char(c) => {
                    input.push(c);
                    self.modal = Some(ModalState::NewCollection { input });
                }
                KeyCode::Backspace => {
                    input.pop();
                    self.modal = Some(ModalState::NewCollection { input });
                }
                KeyCode::Enter if !input.trim().is_empty() => {
                    self.create_collection(input.trim().to_string())?;
                }
                KeyCode::Esc => {}
                _ => { self.modal = Some(ModalState::NewCollection { input }); }
            },

            Some(ModalState::NewFolder { mut input, collection_idx }) => match key.code {
                KeyCode::Char(c) => {
                    input.push(c);
                    self.modal = Some(ModalState::NewFolder { input, collection_idx });
                }
                KeyCode::Backspace => {
                    input.pop();
                    self.modal = Some(ModalState::NewFolder { input, collection_idx });
                }
                KeyCode::Enter if !input.trim().is_empty() => {
                    self.create_folder(input.trim().to_string(), collection_idx)?;
                }
                KeyCode::Esc => {}
                _ => { self.modal = Some(ModalState::NewFolder { input, collection_idx }); }
            },

            Some(ModalState::NewRequest {
                mut name, mut method_idx, mut url, mut active_field, collection_idx, folder_idx,
            }) => match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter if !name.trim().is_empty() && !url.trim().is_empty() => {
                    let req = StoredRequest::new(name.trim(), METHODS[method_idx], url.trim());
                    self.add_request(req, collection_idx, folder_idx)?;
                }
                KeyCode::Tab => {
                    active_field = match active_field {
                        InputField::Name => InputField::Url,
                        InputField::Url => InputField::Name,
                    };
                    self.modal = Some(ModalState::NewRequest { name, method_idx, url, active_field, collection_idx, folder_idx });
                }
                KeyCode::Left => {
                    method_idx = if method_idx == 0 { METHODS.len() - 1 } else { method_idx - 1 };
                    self.modal = Some(ModalState::NewRequest { name, method_idx, url, active_field, collection_idx, folder_idx });
                }
                KeyCode::Right => {
                    method_idx = (method_idx + 1) % METHODS.len();
                    self.modal = Some(ModalState::NewRequest { name, method_idx, url, active_field, collection_idx, folder_idx });
                }
                KeyCode::Char(c) => {
                    match active_field { InputField::Name => name.push(c), InputField::Url => url.push(c) }
                    self.modal = Some(ModalState::NewRequest { name, method_idx, url, active_field, collection_idx, folder_idx });
                }
                KeyCode::Backspace => {
                    match active_field { InputField::Name => { name.pop(); } InputField::Url => { url.pop(); } }
                    self.modal = Some(ModalState::NewRequest { name, method_idx, url, active_field, collection_idx, folder_idx });
                }
                _ => { self.modal = Some(ModalState::NewRequest { name, method_idx, url, active_field, collection_idx, folder_idx }); }
            },

            Some(ModalState::EditRequest {
                mut name, mut method_idx, mut url, mut active_field,
                collection_idx, folder_idx, request_idx,
            }) => match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter if !name.trim().is_empty() && !url.trim().is_empty() => {
                    self.edit_request(name.trim().to_string(), method_idx, url.trim().to_string(), collection_idx, folder_idx, request_idx)?;
                    self.status_message = "Request updated.  e: edit  Enter: load  d: delete  q: quit".into();
                }
                KeyCode::Tab => {
                    active_field = match active_field {
                        InputField::Name => InputField::Url,
                        InputField::Url => InputField::Name,
                    };
                    self.modal = Some(ModalState::EditRequest { name, method_idx, url, active_field, collection_idx, folder_idx, request_idx });
                }
                KeyCode::Left => {
                    method_idx = if method_idx == 0 { METHODS.len() - 1 } else { method_idx - 1 };
                    self.modal = Some(ModalState::EditRequest { name, method_idx, url, active_field, collection_idx, folder_idx, request_idx });
                }
                KeyCode::Right => {
                    method_idx = (method_idx + 1) % METHODS.len();
                    self.modal = Some(ModalState::EditRequest { name, method_idx, url, active_field, collection_idx, folder_idx, request_idx });
                }
                KeyCode::Char(c) => {
                    match active_field { InputField::Name => name.push(c), InputField::Url => url.push(c) }
                    self.modal = Some(ModalState::EditRequest { name, method_idx, url, active_field, collection_idx, folder_idx, request_idx });
                }
                KeyCode::Backspace => {
                    match active_field { InputField::Name => { name.pop(); } InputField::Url => { url.pop(); } }
                    self.modal = Some(ModalState::EditRequest { name, method_idx, url, active_field, collection_idx, folder_idx, request_idx });
                }
                _ => { self.modal = Some(ModalState::EditRequest { name, method_idx, url, active_field, collection_idx, folder_idx, request_idx }); }
            },

            Some(ModalState::NewEnv { mut input }) => match key.code {
                KeyCode::Char(c) => {
                    input.push(c);
                    self.modal = Some(ModalState::NewEnv { input });
                }
                KeyCode::Backspace => {
                    input.pop();
                    self.modal = Some(ModalState::NewEnv { input });
                }
                KeyCode::Enter if !input.trim().is_empty() => {
                    self.create_env(input.trim().to_string())?;
                }
                KeyCode::Esc => {}
                _ => { self.modal = Some(ModalState::NewEnv { input }); }
            },

            Some(ModalState::NewVar { key: mut var_key, value: mut var_value, mut active_field, env_idx }) => match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter if !var_key.trim().is_empty() => {
                    self.add_var(var_key.trim().to_string(), var_value.trim().to_string(), env_idx)?;
                }
                KeyCode::Tab => {
                    active_field = match active_field {
                        VarField::Key => VarField::Value,
                        VarField::Value => VarField::Key,
                    };
                    self.modal = Some(ModalState::NewVar { key: var_key, value: var_value, active_field, env_idx });
                }
                KeyCode::Char(c) => {
                    match active_field { VarField::Key => var_key.push(c), VarField::Value => var_value.push(c) }
                    self.modal = Some(ModalState::NewVar { key: var_key, value: var_value, active_field, env_idx });
                }
                KeyCode::Backspace => {
                    match active_field { VarField::Key => { var_key.pop(); } VarField::Value => { var_value.pop(); } }
                    self.modal = Some(ModalState::NewVar { key: var_key, value: var_value, active_field, env_idx });
                }
                _ => { self.modal = Some(ModalState::NewVar { key: var_key, value: var_value, active_field, env_idx }); }
            },

            Some(ModalState::HeaderPicker { mut cursor }) => {
                let total = COMMON_HEADERS.len() + 1;
                match key.code {
                    KeyCode::Esc => {}
                    KeyCode::Up => {
                        cursor = if cursor == 0 { total - 1 } else { cursor - 1 };
                        self.modal = Some(ModalState::HeaderPicker { cursor });
                    }
                    KeyCode::Down => {
                        cursor = (cursor + 1) % total;
                        self.modal = Some(ModalState::HeaderPicker { cursor });
                    }
                    KeyCode::Enter => {
                        if cursor < COMMON_HEADERS.len() {
                            let (k, _) = COMMON_HEADERS[cursor];
                            if k == "Content-Type" {
                                self.modal = Some(ModalState::ContentTypePicker { cursor: 0 });
                            } else {
                                let (k, v) = COMMON_HEADERS[cursor];
                                self.modal = Some(ModalState::NewHeader {
                                    key: k.to_string(),
                                    value: v.to_string(),
                                    active_field: VarField::Value,
                                });
                            }
                        } else {
                            self.modal = Some(ModalState::NewHeader {
                                key: String::new(),
                                value: String::new(),
                                active_field: VarField::Key,
                            });
                        }
                    }
                    _ => { self.modal = Some(ModalState::HeaderPicker { cursor }); }
                }
            }

            Some(ModalState::ContentTypePicker { mut cursor }) => {
                let total = COMMON_CONTENT_TYPES.len() + 1;
                match key.code {
                    KeyCode::Esc => { self.modal = Some(ModalState::HeaderPicker { cursor: 1 }); }
                    KeyCode::Up => {
                        cursor = if cursor == 0 { total - 1 } else { cursor - 1 };
                        self.modal = Some(ModalState::ContentTypePicker { cursor });
                    }
                    KeyCode::Down => {
                        cursor = (cursor + 1) % total;
                        self.modal = Some(ModalState::ContentTypePicker { cursor });
                    }
                    KeyCode::Enter => {
                        let value = if cursor < COMMON_CONTENT_TYPES.len() {
                            COMMON_CONTENT_TYPES[cursor].to_string()
                        } else {
                            String::new()
                        };
                        self.modal = Some(ModalState::NewHeader {
                            key: "Content-Type".to_string(),
                            value,
                            active_field: if cursor < COMMON_CONTENT_TYPES.len() {
                                VarField::Value
                            } else {
                                VarField::Key
                            },
                        });
                    }
                    _ => { self.modal = Some(ModalState::ContentTypePicker { cursor }); }
                }
            }

            Some(ModalState::NewHeader { key: mut hdr_key, value: mut hdr_val, mut active_field }) => match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter if !hdr_key.trim().is_empty() => {
                    self.request_headers.push((hdr_key.trim().to_string(), hdr_val.trim().to_string()));
                    self.header_cursor = self.request_headers.len() - 1;
                }
                KeyCode::Tab => {
                    active_field = match active_field {
                        VarField::Key => VarField::Value,
                        VarField::Value => VarField::Key,
                    };
                    self.modal = Some(ModalState::NewHeader { key: hdr_key, value: hdr_val, active_field });
                }
                KeyCode::Char(c) => {
                    match active_field { VarField::Key => hdr_key.push(c), VarField::Value => hdr_val.push(c) }
                    let trigger = active_field == VarField::Value && hdr_val.ends_with("{{");
                    self.modal = Some(ModalState::NewHeader { key: hdr_key, value: hdr_val, active_field });
                    if trigger { self.open_var_picker(VarPickerTarget::ModalValue); }
                }
                KeyCode::Backspace => {
                    match active_field { VarField::Key => { hdr_key.pop(); } VarField::Value => { hdr_val.pop(); } }
                    self.modal = Some(ModalState::NewHeader { key: hdr_key, value: hdr_val, active_field });
                }
                _ => { self.modal = Some(ModalState::NewHeader { key: hdr_key, value: hdr_val, active_field }); }
            },

            Some(ModalState::UrlParam { key: mut up_key, value: mut up_val, mut active_field, edit_idx }) => match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter if !up_key.trim().is_empty() => {
                    if let Some(idx) = edit_idx {
                        self.request_url_params[idx] = (up_key.trim().to_string(), up_val.trim().to_string());
                    } else {
                        self.request_url_params.push((up_key.trim().to_string(), up_val.trim().to_string()));
                        self.url_params_cursor = self.request_url_params.len() - 1;
                    }
                }
                KeyCode::Tab => {
                    active_field = match active_field { VarField::Key => VarField::Value, VarField::Value => VarField::Key };
                    self.modal = Some(ModalState::UrlParam { key: up_key, value: up_val, active_field, edit_idx });
                }
                KeyCode::Char(c) => {
                    match active_field { VarField::Key => up_key.push(c), VarField::Value => up_val.push(c) }
                    let trigger = active_field == VarField::Value && up_val.ends_with("{{");
                    self.modal = Some(ModalState::UrlParam { key: up_key, value: up_val, active_field, edit_idx });
                    if trigger { self.open_var_picker(VarPickerTarget::ModalValue); }
                }
                KeyCode::Backspace => {
                    match active_field { VarField::Key => { up_key.pop(); } VarField::Value => { up_val.pop(); } }
                    self.modal = Some(ModalState::UrlParam { key: up_key, value: up_val, active_field, edit_idx });
                }
                _ => { self.modal = Some(ModalState::UrlParam { key: up_key, value: up_val, active_field, edit_idx }); }
            },

            Some(ModalState::BodyPair { key: mut bp_key, value: mut bp_val, mut active_field, edit_idx }) => match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter if !bp_key.trim().is_empty() => {
                    if let Some(idx) = edit_idx {
                        self.body_json_pairs[idx] = (bp_key.trim().to_string(), bp_val.trim().to_string());
                    } else {
                        self.body_json_pairs.push((bp_key.trim().to_string(), bp_val.trim().to_string()));
                        self.body_json_cursor = self.body_json_pairs.len() - 1;
                    }
                }
                KeyCode::Tab => {
                    active_field = match active_field { VarField::Key => VarField::Value, VarField::Value => VarField::Key };
                    self.modal = Some(ModalState::BodyPair { key: bp_key, value: bp_val, active_field, edit_idx });
                }
                KeyCode::Char(c) => {
                    match active_field { VarField::Key => bp_key.push(c), VarField::Value => bp_val.push(c) }
                    let trigger = active_field == VarField::Value && bp_val.ends_with("{{");
                    self.modal = Some(ModalState::BodyPair { key: bp_key, value: bp_val, active_field, edit_idx });
                    if trigger { self.open_var_picker(VarPickerTarget::ModalValue); }
                }
                KeyCode::Backspace => {
                    match active_field { VarField::Key => { bp_key.pop(); } VarField::Value => { bp_val.pop(); } }
                    self.modal = Some(ModalState::BodyPair { key: bp_key, value: bp_val, active_field, edit_idx });
                }
                _ => { self.modal = Some(ModalState::BodyPair { key: bp_key, value: bp_val, active_field, edit_idx }); }
            },

            Some(ModalState::SaveRequest { mut name, mut collection_idx, mut folder_display_idx, mut active_field }) => {
                let n_cols = self.stored_collections.len();
                let n_folders = self.stored_collections.get(collection_idx).map_or(0, |c| c.folders.len());
                match key.code {
                    KeyCode::Esc => {}
                    KeyCode::Enter if !name.trim().is_empty() && n_cols > 0 => {
                        let fi = if folder_display_idx == 0 { None } else { Some(folder_display_idx - 1) };
                        self.save_request_to_collection(name.trim().to_string(), collection_idx, fi)?;
                    }
                    KeyCode::Tab => {
                        active_field = match active_field {
                            SaveField::Name => SaveField::Collection,
                            SaveField::Collection => if n_folders > 0 { SaveField::Folder } else { SaveField::Name },
                            SaveField::Folder => SaveField::Name,
                        };
                        self.modal = Some(ModalState::SaveRequest { name, collection_idx, folder_display_idx, active_field });
                    }
                    KeyCode::Up if active_field == SaveField::Collection && n_cols > 0 => {
                        collection_idx = if collection_idx == 0 { n_cols - 1 } else { collection_idx - 1 };
                        folder_display_idx = 0;
                        self.modal = Some(ModalState::SaveRequest { name, collection_idx, folder_display_idx, active_field });
                    }
                    KeyCode::Down if active_field == SaveField::Collection && n_cols > 0 => {
                        collection_idx = (collection_idx + 1) % n_cols;
                        folder_display_idx = 0;
                        self.modal = Some(ModalState::SaveRequest { name, collection_idx, folder_display_idx, active_field });
                    }
                    KeyCode::Up if active_field == SaveField::Folder => {
                        folder_display_idx = if folder_display_idx == 0 { n_folders } else { folder_display_idx - 1 };
                        self.modal = Some(ModalState::SaveRequest { name, collection_idx, folder_display_idx, active_field });
                    }
                    KeyCode::Down if active_field == SaveField::Folder => {
                        folder_display_idx = (folder_display_idx + 1) % (n_folders + 1);
                        self.modal = Some(ModalState::SaveRequest { name, collection_idx, folder_display_idx, active_field });
                    }
                    KeyCode::Char(c) if active_field == SaveField::Name => {
                        name.push(c);
                        self.modal = Some(ModalState::SaveRequest { name, collection_idx, folder_display_idx, active_field });
                    }
                    KeyCode::Backspace if active_field == SaveField::Name => {
                        name.pop();
                        self.modal = Some(ModalState::SaveRequest { name, collection_idx, folder_display_idx, active_field });
                    }
                    _ => { self.modal = Some(ModalState::SaveRequest { name, collection_idx, folder_display_idx, active_field }); }
                }
            }

            Some(ModalState::ConfirmDelete { label, address }) => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.delete_node(address)?;
                }
                KeyCode::Char('n') | KeyCode::Esc => {}
                _ => { self.modal = Some(ModalState::ConfirmDelete { label, address }); }
            },

            None => {}
        }
        Ok(())
    }
}
