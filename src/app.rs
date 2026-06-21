use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    Request,
    Collections,
    History,
}

impl Tab {
    pub fn title(&self) -> &'static str {
        match self {
            Tab::Request => "Request",
            Tab::Collections => "Collections",
            Tab::History => "History",
        }
    }

    pub fn all() -> Vec<Tab> {
        vec![Tab::Request, Tab::Collections, Tab::History]
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

#[derive(Debug, Clone)]
pub enum CollectionNode {
    Folder {
        name: String,
        children: Vec<CollectionNode>,
        expanded: bool,
    },
    Request {
        name: String,
        method: String,
        url: String,
    },
}

pub struct FlatNode {
    pub depth: usize,
    pub name: String,
    pub is_folder: bool,
    pub expanded: bool,
    pub method: Option<String>,
}

pub fn flatten_collections(nodes: &[CollectionNode]) -> Vec<FlatNode> {
    let mut result = Vec::new();
    flatten_recursive(nodes, 0, &mut result);
    result
}

fn flatten_recursive(nodes: &[CollectionNode], depth: usize, result: &mut Vec<FlatNode>) {
    for node in nodes {
        match node {
            CollectionNode::Folder { name, children, expanded } => {
                result.push(FlatNode {
                    depth,
                    name: name.clone(),
                    is_folder: true,
                    expanded: *expanded,
                    method: None,
                });
                if *expanded {
                    flatten_recursive(children, depth + 1, result);
                }
            }
            CollectionNode::Request { name, method, .. } => {
                result.push(FlatNode {
                    depth,
                    name: name.clone(),
                    is_folder: false,
                    expanded: false,
                    method: Some(method.clone()),
                });
            }
        }
    }
}

fn toggle_at_cursor(nodes: &mut [CollectionNode], cursor: usize) {
    let mut count = 0;
    toggle_recursive(nodes, cursor, &mut count);
}

fn toggle_recursive(nodes: &mut [CollectionNode], cursor: usize, count: &mut usize) -> bool {
    for node in nodes.iter_mut() {
        let current = *count;
        *count += 1;

        if current == cursor {
            if let CollectionNode::Folder { expanded, .. } = node {
                *expanded = !*expanded;
            }
            return true;
        }

        if let CollectionNode::Folder { children, expanded, .. } = node {
            if *expanded && toggle_recursive(children, cursor, count) {
                return true;
            }
        }
    }
    false
}

const SAMPLE_RESPONSE: &str = r#"{
  "id": 42,
  "name": "Alice Dupont",
  "active": true,
  "score": 98.5,
  "role": null,
  "address": {
    "street": "12 rue de la Paix",
    "city": "Paris",
    "zip": "75001"
  },
  "tags": ["rust", "tui", "graphql"],
  "permissions": [
    { "resource": "users", "action": "read" },
    { "resource": "users", "action": "write" }
  ]
}"#;

pub struct App {
    pub running: bool,
    pub active_tab: Tab,
    pub active_request_tab: RequestTab,
    pub collections: Vec<CollectionNode>,
    pub collection_cursor: usize,
    pub response_body: Option<String>,
    pub response_cursor: usize,
    pub response_scroll: u16,
    pub response_folds: HashSet<String>,
    pub status_message: String,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            active_tab: Tab::Request,
            active_request_tab: RequestTab::Description,
            collections: Self::sample_collections(),
            collection_cursor: 0,
            response_body: Some(SAMPLE_RESPONSE.to_string()),
            response_cursor: 0,
            response_scroll: 0,
            response_folds: HashSet::new(),
            status_message: String::from("Tab: switch panel  ←/→: section  ↑/↓: cursor  Enter: fold  q: quit"),
        }
    }

    fn sample_collections() -> Vec<CollectionNode> {
        vec![
            CollectionNode::Folder {
                name: "Public APIs".into(),
                expanded: true,
                children: vec![
                    CollectionNode::Folder {
                        name: "Auth".into(),
                        expanded: false,
                        children: vec![
                            CollectionNode::Request {
                                name: "Login".into(),
                                method: "POST".into(),
                                url: "https://api.example.com/auth/login".into(),
                            },
                            CollectionNode::Request {
                                name: "Refresh token".into(),
                                method: "POST".into(),
                                url: "https://api.example.com/auth/refresh".into(),
                            },
                        ],
                    },
                    CollectionNode::Request {
                        name: "List users".into(),
                        method: "GET".into(),
                        url: "https://api.example.com/users".into(),
                    },
                    CollectionNode::Request {
                        name: "Create user".into(),
                        method: "POST".into(),
                        url: "https://api.example.com/users".into(),
                    },
                    CollectionNode::Request {
                        name: "Delete user".into(),
                        method: "DELETE".into(),
                        url: "https://api.example.com/users/{id}".into(),
                    },
                ],
            },
            CollectionNode::Folder {
                name: "GraphQL".into(),
                expanded: false,
                children: vec![
                    CollectionNode::Request {
                        name: "Introspection".into(),
                        method: "POST".into(),
                        url: "https://api.example.com/graphql".into(),
                    },
                    CollectionNode::Request {
                        name: "Get users".into(),
                        method: "POST".into(),
                        url: "https://api.example.com/graphql".into(),
                    },
                ],
            },
        ]
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.running = false;
            }
            KeyCode::Tab => {
                self.active_tab = match self.active_tab {
                    Tab::Request => Tab::Collections,
                    Tab::Collections => Tab::History,
                    Tab::History => Tab::Request,
                };
                self.status_message = match self.active_tab {
                    Tab::Request => "Tab: switch panel  ←/→: switch section  q: quit".into(),
                    Tab::Collections => "Tab: switch panel  ↑/↓: navigate  Enter: expand/collapse  q: quit".into(),
                    Tab::History => "Tab: switch panel  q: quit".into(),
                };
            }
            KeyCode::Right if self.active_tab == Tab::Request => {
                self.active_request_tab = self.active_request_tab.next();
            }
            KeyCode::Left if self.active_tab == Tab::Request => {
                self.active_request_tab = self.active_request_tab.prev();
            }
            KeyCode::Up if self.active_tab == Tab::Request => {
                self.response_cursor = self.response_cursor.saturating_sub(1);
                self.sync_scroll();
            }
            KeyCode::Down if self.active_tab == Tab::Request => {
                let len = self.response_line_count();
                if self.response_cursor + 1 < len {
                    self.response_cursor += 1;
                }
                self.sync_scroll();
            }
            KeyCode::Enter if self.active_tab == Tab::Request => {
                self.toggle_response_fold();
            }
            KeyCode::Up if self.active_tab == Tab::Collections => {
                if self.collection_cursor > 0 {
                    self.collection_cursor -= 1;
                }
            }
            KeyCode::Down if self.active_tab == Tab::Collections => {
                let flat = flatten_collections(&self.collections);
                if self.collection_cursor + 1 < flat.len() {
                    self.collection_cursor += 1;
                }
            }
            KeyCode::Enter if self.active_tab == Tab::Collections => {
                toggle_at_cursor(&mut self.collections, self.collection_cursor);
            }
            _ => {}
        }
        Ok(())
    }

    pub fn tick(&mut self) {}

    fn response_line_count(&self) -> usize {
        crate::json_highlight::render(
            self.response_body.as_deref().unwrap_or(""),
            &self.response_folds,
            0,
        )
        .len()
    }

    fn sync_scroll(&mut self) {
        self.response_scroll = (self.response_cursor as u16).saturating_sub(3);
    }

    fn toggle_response_fold(&mut self) {
        let json = self.response_body.as_deref().unwrap_or("");
        let infos =
            crate::json_highlight::render(json, &self.response_folds, self.response_cursor);

        if let Some(path) = infos.get(self.response_cursor).and_then(|i| i.fold_path.clone()) {
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
