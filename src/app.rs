use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;
use tui_textarea::TextArea;

use crate::storage::{
    CollectionMeta, EnvMeta, StoredCollection, StoredEnv, StoredFolder, StoredRequest,
};

// ── HTTP types ────────────────────────────────────────────────────────────────

pub struct HttpResult {
    pub status: u16,
    pub body: String,
    pub headers: Vec<(String, String)>,
    pub elapsed_ms: u64,
}

pub type HttpOutcome = Result<HttpResult, String>;

// ── Request focus ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum RequestFocus {
    Url,
    Body,
    Response,
}

pub const METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE"];

#[derive(Debug, Clone, PartialEq)]
pub enum ResponseView {
    Json,
    Raw,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    Request,
    Collections,
    Env,
    History,
}

impl Tab {
    pub fn title(&self) -> &'static str {
        match self {
            Tab::Request => "Request",
            Tab::Collections => "Collections",
            Tab::Env => "Env",
            Tab::History => "History",
        }
    }

    pub fn all() -> Vec<Tab> {
        vec![Tab::Request, Tab::Collections, Tab::Env, Tab::History]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RequestTab {
    Description,
    Headers,
    UrlParams,
    Body,
    Auth,
    Options,
}

impl RequestTab {
    pub fn title(&self) -> &'static str {
        match self {
            RequestTab::Description => "Description",
            RequestTab::Headers => "Headers",
            RequestTab::UrlParams => "URL Params",
            RequestTab::Body => "Body",
            RequestTab::Auth => "Auth",
            RequestTab::Options => "Options",
        }
    }

    pub fn all() -> Vec<RequestTab> {
        vec![
            RequestTab::Description,
            RequestTab::Headers,
            RequestTab::UrlParams,
            RequestTab::Body,
            RequestTab::Auth,
            RequestTab::Options,
        ]
    }

    fn next(&self) -> RequestTab {
        let all = RequestTab::all();
        let pos = all.iter().position(|t| t == self).unwrap_or(0);
        all.into_iter().nth((pos + 1) % 6).unwrap_or(RequestTab::Description)
    }

    fn prev(&self) -> RequestTab {
        let all = RequestTab::all();
        let pos = all.iter().position(|t| t == self).unwrap_or(0);
        all.into_iter().nth(if pos == 0 { 5 } else { pos - 1 }).unwrap_or(RequestTab::Options)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputField {
    Name,
    Url,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VarField {
    Key,
    Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EnvFocus {
    Envs,
    Vars,
}

#[derive(Debug, Clone)]
pub enum NodeAddress {
    // Collections
    Collection(usize),
    Folder(usize, usize),
    RootRequest(usize, usize),
    FolderRequest(usize, usize, usize),
    // Environments
    Env(usize),
    EnvVar { env_idx: usize, key: String },
}

#[derive(Debug, Clone)]
pub enum ModalState {
    NewCollection {
        input: String,
    },
    NewFolder {
        input: String,
        collection_idx: usize,
    },
    NewRequest {
        name: String,
        method_idx: usize,
        url: String,
        active_field: InputField,
        collection_idx: usize,
        folder_idx: Option<usize>,
    },
    NewEnv {
        input: String,
    },
    NewVar {
        key: String,
        value: String,
        active_field: VarField,
        env_idx: usize,
    },
    NewHeader {
        key: String,
        value: String,
        active_field: VarField,
    },
    ConfirmDelete {
        label: String,
        address: NodeAddress,
    },
}

pub struct FlatNode {
    pub depth: usize,
    pub name: String,
    pub is_folder: bool,
    pub expanded: bool,
    pub method: Option<String>,
    pub address: NodeAddress,
}

pub fn flatten_stored(cols: &[StoredCollection], expanded: &HashSet<String>) -> Vec<FlatNode> {
    let mut result = Vec::new();
    for (ci, col) in cols.iter().enumerate() {
        let col_key = format!("c{}", ci);
        let col_expanded = expanded.contains(&col_key);
        result.push(FlatNode {
            depth: 0,
            name: col.collection.name.clone(),
            is_folder: true,
            expanded: col_expanded,
            method: None,
            address: NodeAddress::Collection(ci),
        });
        if col_expanded {
            for (fi, folder) in col.folders.iter().enumerate() {
                let folder_key = format!("c{}f{}", ci, fi);
                let folder_expanded = expanded.contains(&folder_key);
                result.push(FlatNode {
                    depth: 1,
                    name: folder.name.clone(),
                    is_folder: true,
                    expanded: folder_expanded,
                    method: None,
                    address: NodeAddress::Folder(ci, fi),
                });
                if folder_expanded {
                    for (ri, req) in folder.requests.iter().enumerate() {
                        result.push(FlatNode {
                            depth: 2,
                            name: req.name.clone(),
                            is_folder: false,
                            expanded: false,
                            method: Some(req.method.clone()),
                            address: NodeAddress::FolderRequest(ci, fi, ri),
                        });
                    }
                }
            }
            for (ri, req) in col.requests.iter().enumerate() {
                result.push(FlatNode {
                    depth: 1,
                    name: req.name.clone(),
                    is_folder: false,
                    expanded: false,
                    method: Some(req.method.clone()),
                    address: NodeAddress::RootRequest(ci, ri),
                });
            }
        }
    }
    result
}

/// Returns the sorted list of (key, value) pairs for an environment.
pub fn sorted_vars(env: &StoredEnv) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = env.vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    pairs
}

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
    // Request builder
    pub request_url: String,
    pub request_method_idx: usize,
    pub request_headers: Vec<(String, String)>,
    pub header_cursor: usize,
    pub body_textarea: TextArea<'static>,
    pub request_focus: RequestFocus,
    pub request_loading: bool,
    // Response
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
    // Async channel — receives HTTP results from spawned tasks
    response_rx: mpsc::UnboundedReceiver<HttpOutcome>,
    response_tx: mpsc::UnboundedSender<HttpOutcome>,
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
            request_url: String::new(),
            request_method_idx: 0,
            request_headers: Vec::new(),
            header_cursor: 0,
            body_textarea: TextArea::default(),
            request_focus: RequestFocus::Response,
            request_loading: false,
            response_body,
            response_status: None,
            response_elapsed_ms: None,
            response_headers: Vec::new(),
            response_view: ResponseView::Json,
            response_cursor: 0,
            response_scroll: 0,
            response_folds: HashSet::new(),
            key_col_width: 22,
            status_message: "Tab: panels  e: edit URL  s: send  m: method  ←/→: section  ↑/↓: cursor  r: raw  q: quit".into(),
            response_rx,
            response_tx,
        }
    }

    fn sample_stored_collections() -> Vec<StoredCollection> {
        vec![
            StoredCollection {
                collection: CollectionMeta { name: "Public APIs".into(), description: String::new() },
                folders: vec![
                    StoredFolder {
                        name: "Auth".into(),
                        requests: vec![
                            StoredRequest::new("Login", "POST", "https://api.example.com/auth/login"),
                            StoredRequest::new("Refresh token", "POST", "https://api.example.com/auth/refresh"),
                        ],
                    },
                ],
                requests: vec![
                    StoredRequest::new("List users", "GET", "https://api.example.com/users"),
                    StoredRequest::new("Create user", "POST", "https://api.example.com/users"),
                    StoredRequest::new("Delete user", "DELETE", "https://api.example.com/users/{id}"),
                ],
            },
            StoredCollection {
                collection: CollectionMeta { name: "GraphQL".into(), description: String::new() },
                folders: vec![],
                requests: vec![
                    StoredRequest::new("Introspection", "POST", "https://api.example.com/graphql"),
                    StoredRequest::new("Get users", "POST", "https://api.example.com/graphql"),
                ],
            },
        ]
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
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
                    Tab::Collections => "Tab: switch panel  ↑/↓: navigate  Enter: expand  n: new collection  f: new folder  a: add request  d: delete  q: quit".into(),
                    Tab::Env => "Tab: switch panel  ←/→: switch focus  ↑/↓: navigate  Enter: activate  n: new env  a: add var  d: delete  q: quit".into(),
                    Tab::History => "Tab: switch panel  q: quit".into(),
                };
            }

            // ── Request panel — URL edit mode ──────────────────────────────
            KeyCode::Enter
                if self.active_tab == Tab::Request
                    && self.request_focus == RequestFocus::Url =>
            {
                self.send_request();
            }
            KeyCode::Left
                if self.active_tab == Tab::Request
                    && self.request_focus == RequestFocus::Url =>
            {
                self.request_method_idx = if self.request_method_idx == 0 {
                    METHODS.len() - 1
                } else {
                    self.request_method_idx - 1
                };
            }
            KeyCode::Right
                if self.active_tab == Tab::Request
                    && self.request_focus == RequestFocus::Url =>
            {
                self.request_method_idx = (self.request_method_idx + 1) % METHODS.len();
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
            }

            // ── Request panel — response navigation mode ───────────────────
            KeyCode::Char('e') if self.active_tab == Tab::Request => {
                self.request_focus = RequestFocus::Url;
                self.status_message = "URL: type address  ←/→: method  Enter: send  Esc: cancel".into();
            }
            KeyCode::Char('i')
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::Body =>
            {
                self.request_focus = RequestFocus::Body;
                self.status_message = "Body: editing JSON  Esc: exit editor".into();
            }
            KeyCode::Char('m') if self.active_tab == Tab::Request => {
                self.request_method_idx = (self.request_method_idx + 1) % METHODS.len();
            }
            KeyCode::Char('s') if self.active_tab == Tab::Request => {
                self.send_request();
            }
            KeyCode::Char('a')
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::Headers =>
            {
                self.modal = Some(ModalState::NewHeader {
                    key: String::new(),
                    value: String::new(),
                    active_field: VarField::Key,
                });
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
                    ResponseView::Raw => ResponseView::Json,
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
                    ResponseView::Raw => {
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
                    ResponseView::Raw => {
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
                    self.modal = Some(ModalState::NewHeader { key: hdr_key, value: hdr_val, active_field });
                }
                KeyCode::Backspace => {
                    match active_field { VarField::Key => { hdr_key.pop(); } VarField::Value => { hdr_val.pop(); } }
                    self.modal = Some(ModalState::NewHeader { key: hdr_key, value: hdr_val, active_field });
                }
                _ => { self.modal = Some(ModalState::NewHeader { key: hdr_key, value: hdr_val, active_field }); }
            },

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

    // ── Collections helpers ────────────────────────────────────────────────

    fn toggle_collection_cursor(&mut self) {
        let flat = flatten_stored(&self.stored_collections, &self.expanded_nodes);
        if let Some(node) = flat.get(self.collection_cursor) {
            if node.is_folder {
                let key = match &node.address {
                    NodeAddress::Collection(ci) => format!("c{}", ci),
                    NodeAddress::Folder(ci, fi) => format!("c{}f{}", ci, fi),
                    _ => return,
                };
                if !self.expanded_nodes.remove(&key) {
                    self.expanded_nodes.insert(key);
                }
            } else {
                let address = node.address.clone();
                self.load_collection_request(&address);
            }
        }
    }

    fn load_collection_request(&mut self, address: &NodeAddress) {
        let req = match address {
            NodeAddress::RootRequest(ci, ri) => {
                self.stored_collections.get(*ci).and_then(|c| c.requests.get(*ri))
            }
            NodeAddress::FolderRequest(ci, fi, ri) => {
                self.stored_collections.get(*ci)
                    .and_then(|c| c.folders.get(*fi))
                    .and_then(|f| f.requests.get(*ri))
            }
            _ => None,
        };

        if let Some(req) = req {
            self.request_method_idx = METHODS.iter()
                .position(|&m| m == req.method)
                .unwrap_or(0);
            self.request_url = req.url.clone();
            self.request_headers = req.headers.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            self.request_headers.sort_by(|a, b| a.0.cmp(&b.0));
            self.header_cursor = 0;
            self.body_textarea = if let Some(body) = &req.body {
                let lines: Vec<String> = body.lines().map(|l| l.to_string()).collect();
                TextArea::from(lines)
            } else {
                TextArea::default()
            };
            self.request_focus = RequestFocus::Response;
            self.response_body = None;
            self.response_status = None;
            self.response_elapsed_ms = None;
            self.response_cursor = 0;
            self.response_scroll = 0;
            self.response_folds = HashSet::new();
            self.active_tab = Tab::Request;
            self.active_request_tab = RequestTab::Description;
            self.status_message = format!(
                "Loaded: {}  —  e: edit URL  s: send  q: quit",
                req.name
            );
        }
    }

    fn cursor_insertion_context(&self) -> Option<(usize, Option<usize>)> {
        let flat = flatten_stored(&self.stored_collections, &self.expanded_nodes);
        let node = flat.get(self.collection_cursor)?;
        let ctx = match &node.address {
            NodeAddress::Collection(ci) => (*ci, None),
            NodeAddress::Folder(ci, fi) => (*ci, Some(*fi)),
            NodeAddress::RootRequest(ci, _) => (*ci, None),
            NodeAddress::FolderRequest(ci, fi, _) => (*ci, Some(*fi)),
            _ => return None,
        };
        Some(ctx)
    }

    fn open_delete_modal(&mut self) {
        let flat = flatten_stored(&self.stored_collections, &self.expanded_nodes);
        if let Some(node) = flat.get(self.collection_cursor) {
            self.modal = Some(ModalState::ConfirmDelete {
                label: node.name.clone(),
                address: node.address.clone(),
            });
        }
    }

    fn create_collection(&mut self, name: String) -> Result<()> {
        let col = StoredCollection {
            collection: CollectionMeta { name, description: String::new() },
            folders: vec![],
            requests: vec![],
        };
        crate::storage::save_collection(&col)?;
        let ci = self.stored_collections.len();
        self.stored_collections.push(col);
        self.expanded_nodes.insert(format!("c{}", ci));
        let flat = flatten_stored(&self.stored_collections, &self.expanded_nodes);
        self.collection_cursor = flat.len().saturating_sub(1);
        Ok(())
    }

    fn create_folder(&mut self, name: String, ci: usize) -> Result<()> {
        let fi = self.stored_collections[ci].folders.len();
        self.stored_collections[ci].folders.push(StoredFolder { name, requests: vec![] });
        crate::storage::save_collection(&self.stored_collections[ci])?;
        self.expanded_nodes.insert(format!("c{}f{}", ci, fi));
        let flat = flatten_stored(&self.stored_collections, &self.expanded_nodes);
        if let Some(pos) = flat.iter().position(|n| {
            matches!(&n.address, NodeAddress::Folder(c, f) if *c == ci && *f == fi)
        }) {
            self.collection_cursor = pos;
        }
        Ok(())
    }

    fn add_request(&mut self, req: StoredRequest, ci: usize, fi: Option<usize>) -> Result<()> {
        if let Some(fi) = fi {
            self.stored_collections[ci].folders[fi].requests.push(req);
        } else {
            self.stored_collections[ci].requests.push(req);
        }
        crate::storage::save_collection(&self.stored_collections[ci])?;
        Ok(())
    }

    fn delete_node(&mut self, address: NodeAddress) -> Result<()> {
        match address {
            NodeAddress::Collection(ci) => {
                let name = self.stored_collections[ci].collection.name.clone();
                crate::storage::delete_collection(&name)?;
                self.stored_collections.remove(ci);
                self.expanded_nodes.clear();
                if !self.stored_collections.is_empty() {
                    self.expanded_nodes.insert("c0".to_string());
                }
                self.collection_cursor = self.collection_cursor.saturating_sub(1);
            }
            NodeAddress::Folder(ci, fi) => {
                self.stored_collections[ci].folders.remove(fi);
                crate::storage::save_collection(&self.stored_collections[ci])?;
                self.rebuild_expanded_after_folder_remove(ci, fi);
            }
            NodeAddress::RootRequest(ci, ri) => {
                self.stored_collections[ci].requests.remove(ri);
                crate::storage::save_collection(&self.stored_collections[ci])?;
            }
            NodeAddress::FolderRequest(ci, fi, ri) => {
                self.stored_collections[ci].folders[fi].requests.remove(ri);
                crate::storage::save_collection(&self.stored_collections[ci])?;
            }
            NodeAddress::Env(ei) => {
                let name = self.environments[ei].env.name.clone();
                crate::storage::delete_env(&name)?;
                self.environments.remove(ei);
                if self.active_env_idx == Some(ei) {
                    self.active_env_idx = None;
                } else if let Some(active) = self.active_env_idx {
                    if active > ei {
                        self.active_env_idx = Some(active - 1);
                    }
                }
                if self.env_cursor >= self.environments.len() && !self.environments.is_empty() {
                    self.env_cursor = self.environments.len() - 1;
                }
            }
            NodeAddress::EnvVar { env_idx, key } => {
                self.environments[env_idx].vars.remove(&key);
                crate::storage::save_env(&self.environments[env_idx])?;
                let count = self.environments[env_idx].vars.len();
                if self.env_var_cursor >= count && count > 0 {
                    self.env_var_cursor = count - 1;
                }
            }
        }
        let flat_len = flatten_stored(&self.stored_collections, &self.expanded_nodes).len();
        if self.collection_cursor >= flat_len && flat_len > 0 {
            self.collection_cursor = flat_len - 1;
        }
        Ok(())
    }

    fn rebuild_expanded_after_folder_remove(&mut self, ci: usize, removed_fi: usize) {
        let old = std::mem::take(&mut self.expanded_nodes);
        for key in old {
            let prefix = format!("c{}", ci);
            if let Some(rest) = key.strip_prefix(&prefix) {
                if rest.is_empty() {
                    self.expanded_nodes.insert(key);
                } else if let Some(fi_str) = rest.strip_prefix('f') {
                    if let Ok(fi) = fi_str.parse::<usize>() {
                        if fi < removed_fi {
                            self.expanded_nodes.insert(key);
                        } else if fi > removed_fi {
                            self.expanded_nodes.insert(format!("c{}f{}", ci, fi - 1));
                        }
                    }
                }
            } else {
                self.expanded_nodes.insert(key);
            }
        }
    }

    // ── Env helpers ────────────────────────────────────────────────────────

    fn create_env(&mut self, name: String) -> Result<()> {
        let env = StoredEnv {
            env: EnvMeta { name },
            vars: HashMap::new(),
        };
        crate::storage::save_env(&env)?;
        self.environments.push(env);
        self.env_cursor = self.environments.len() - 1;
        self.env_var_cursor = 0;
        Ok(())
    }

    fn add_var(&mut self, key: String, value: String, env_idx: usize) -> Result<()> {
        self.environments[env_idx].vars.insert(key, value);
        crate::storage::save_env(&self.environments[env_idx])?;
        Ok(())
    }

    fn open_env_delete_modal(&mut self) {
        match self.env_focus {
            EnvFocus::Envs => {
                if let Some(env) = self.environments.get(self.env_cursor) {
                    self.modal = Some(ModalState::ConfirmDelete {
                        label: env.env.name.clone(),
                        address: NodeAddress::Env(self.env_cursor),
                    });
                }
            }
            EnvFocus::Vars => {
                if let Some(env) = self.environments.get(self.env_cursor) {
                    let vars = sorted_vars(env);
                    if let Some((key, _)) = vars.get(self.env_var_cursor) {
                        self.modal = Some(ModalState::ConfirmDelete {
                            label: key.clone(),
                            address: NodeAddress::EnvVar {
                                env_idx: self.env_cursor,
                                key: key.clone(),
                            },
                        });
                    }
                }
            }
        }
    }

    // ── Response helpers ───────────────────────────────────────────────────

    pub fn tick(&mut self) {
        // Poll for HTTP results from spawned tasks (non-blocking)
        if let Ok(outcome) = self.response_rx.try_recv() {
            self.request_loading = false;
            match outcome {
                Ok(http) => {
                    self.response_status = Some(http.status);
                    self.response_elapsed_ms = Some(http.elapsed_ms);
                    self.response_headers = http.headers;
                    self.response_body = Some(http.body);
                    self.response_cursor = 0;
                    self.response_scroll = 0;
                    self.response_folds = HashSet::new();
                    self.status_message = format!(
                        "{}  {}ms  —  Tab: panels  e: edit URL  s: send  m: method  ←/→: section  r: raw  q: quit",
                        http_status_label(self.response_status.unwrap_or(0)),
                        self.response_elapsed_ms.unwrap_or(0),
                    );
                }
                Err(msg) => {
                    self.response_status = None;
                    self.response_body = Some(format!("Error: {}", msg));
                    self.response_cursor = 0;
                    self.response_scroll = 0;
                    self.response_folds = HashSet::new();
                    self.status_message = format!("Error: {}  —  e: edit URL  s: retry  q: quit", msg);
                }
            }
        }
    }

    // ── HTTP ───────────────────────────────────────────────────────────────

    fn send_request(&mut self) {
        if self.request_loading {
            return;
        }
        let url = self.request_url.trim().to_string();
        if url.is_empty() {
            self.status_message = "No URL — press e to enter one".into();
            return;
        }

        // Resolve {{VAR}} from the active environment
        let env_vars = self.active_env_idx
            .and_then(|i| self.environments.get(i))
            .map(|e| e.vars.clone())
            .unwrap_or_default();

        let resolved_url = crate::storage::resolve_vars(&url, &env_vars);

        let resolved_headers: Vec<(String, String)> = self.request_headers.iter()
            .map(|(k, v)| (
                crate::storage::resolve_vars(k, &env_vars),
                crate::storage::resolve_vars(v, &env_vars),
            ))
            .collect();

        let method = METHODS[self.request_method_idx].to_string();
        let tx = self.response_tx.clone();

        // Extract body text; None if empty
        let body_text = self.body_textarea.lines().join("\n");
        let body = if body_text.trim().is_empty() { None } else { Some(body_text) };

        self.request_loading = true;
        self.request_focus = RequestFocus::Response;
        self.status_message = format!("Sending {} {}…", method, resolved_url);

        tokio::spawn(async move {
            let result = execute_http(&method, &resolved_url, &resolved_headers, body).await;
            let _ = tx.send(result);
        });
    }

    fn response_line_count(&self) -> usize {
        crate::json_highlight::rows(
            self.response_body.as_deref().unwrap_or(""),
            &self.response_folds,
        )
        .len()
    }

    fn sync_scroll(&mut self) {
        self.response_scroll = (self.response_cursor as u16).saturating_sub(3);
    }

    pub fn active_method(&self) -> &'static str {
        METHODS[self.request_method_idx]
    }

    fn update_request_status_hint(&mut self) {
        self.status_message = match self.active_request_tab {
            RequestTab::Headers => "Tab: panels  a: add  d: delete  ↑/↓: navigate  ←/→: section  e: edit URL  s: send  q: quit".into(),
            RequestTab::Body => "Tab: panels  i: edit body  ←/→: section  e: edit URL  s: send  q: quit".into(),
            _ => "Tab: panels  e: edit URL  s: send  m: method  ←/→: section  ↑/↓: cursor  r: raw  q: quit".into(),
        };
    }

    fn handle_body_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.code == KeyCode::Esc {
            self.request_focus = RequestFocus::Response;
            self.update_request_status_hint();
            return Ok(());
        }
        self.body_textarea.input(tui_textarea::Input::from(key));
        Ok(())
    }

    fn toggle_response_fold(&mut self) {
        let json = self.response_body.as_deref().unwrap_or("");
        let json_rows = crate::json_highlight::rows(json, &self.response_folds);

        if let Some(path) = json_rows
            .get(self.response_cursor)
            .and_then(|r| r.fold_path.clone())
        {
            if !self.response_folds.remove(&path) {
                self.response_folds.insert(path);
            }
            let new_len = self.response_line_count();
            if self.response_cursor >= new_len && new_len > 0 {
                self.response_cursor = new_len - 1;
            }
            self.sync_scroll();
        }
    }
}

// ── HTTP execution ────────────────────────────────────────────────────────────

async fn execute_http(method: &str, url: &str, headers: &[(String, String)], body: Option<String>) -> HttpOutcome {
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
    use std::str::FromStr;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let t0 = std::time::Instant::now();

    let mut req = match method {
        "GET"    => client.get(url),
        "POST"   => client.post(url),
        "PUT"    => client.put(url),
        "PATCH"  => client.patch(url),
        "DELETE" => client.delete(url),
        _        => client.get(url),
    };

    if !headers.is_empty() {
        let mut hmap = HeaderMap::new();
        for (k, v) in headers {
            if let (Ok(name), Ok(val)) = (
                HeaderName::from_str(k),
                HeaderValue::from_str(v),
            ) {
                hmap.insert(name, val);
            }
        }
        req = req.headers(hmap);
    }

    if let Some(b) = body {
        req = req.body(b);
    }

    let resp = req.send().await.map_err(|e| e.to_string())?;
    let elapsed_ms = t0.elapsed().as_millis() as u64;
    let status = resp.status().as_u16();

    let headers: Vec<(String, String)> = resp
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let body = resp.text().await.map_err(|e| e.to_string())?;

    Ok(HttpResult { status, body, headers, elapsed_ms })
}

fn http_status_label(status: u16) -> String {
    let text = match status {
        200 => "200 OK",
        201 => "201 Created",
        204 => "204 No Content",
        301 => "301 Moved Permanently",
        302 => "302 Found",
        304 => "304 Not Modified",
        400 => "400 Bad Request",
        401 => "401 Unauthorized",
        403 => "403 Forbidden",
        404 => "404 Not Found",
        405 => "405 Method Not Allowed",
        409 => "409 Conflict",
        422 => "422 Unprocessable Entity",
        429 => "429 Too Many Requests",
        500 => "500 Internal Server Error",
        502 => "502 Bad Gateway",
        503 => "503 Service Unavailable",
        _   => return format!("{}", status),
    };
    text.to_string()
}
