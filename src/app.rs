use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::collections::{HashMap, HashSet};

use crate::storage::{
    CollectionMeta, EnvMeta, StoredCollection, StoredEnv, StoredFolder, StoredRequest,
};

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
    // Response
    pub response_body: Option<String>,
    pub response_view: ResponseView,
    pub response_cursor: usize,
    pub response_scroll: u16,
    pub response_folds: HashSet<String>,
    pub key_col_width: u16,
    pub status_message: String,
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
            response_body,
            response_view: ResponseView::Json,
            response_cursor: 0,
            response_scroll: 0,
            response_folds: HashSet::new(),
            key_col_width: 22,
            status_message: "Tab: panels  ←/→: section  ↑/↓: cursor  Enter: fold  r: raw/json  -/=: resize  q: quit".into(),
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

        match key.code {
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

            // ── Request panel ──────────────────────────────────────────────
            KeyCode::Right if self.active_tab == Tab::Request => {
                self.active_request_tab = self.active_request_tab.next();
            }
            KeyCode::Left if self.active_tab == Tab::Request => {
                self.active_request_tab = self.active_request_tab.prev();
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
            }
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

    pub fn tick(&mut self) {}

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
