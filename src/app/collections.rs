use anyhow::Result;
use std::collections::HashSet;
use tui_textarea::TextArea;

use super::*;
use super::http::split_url_params;
use crate::storage::{CollectionMeta, StoredCollection, StoredFolder, StoredRequest};

impl App {
    pub(super) fn sample_stored_collections() -> Vec<StoredCollection> {
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
                path: String::new(),
            },
            StoredCollection {
                collection: CollectionMeta { name: "GraphQL".into(), description: String::new() },
                folders: vec![],
                requests: vec![
                    StoredRequest::new("Introspection", "POST", "https://api.example.com/graphql"),
                    StoredRequest::new("Get users", "POST", "https://api.example.com/graphql"),
                ],
                path: String::new(),
            },
        ]
    }

    pub(super) fn toggle_collection_cursor(&mut self) {
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
                let (ci, fi, ri) = match &node.address {
                    NodeAddress::RootRequest(ci, ri)       => (*ci, None, *ri),
                    NodeAddress::FolderRequest(ci, fi, ri) => (*ci, Some(*fi), *ri),
                    _ => return,
                };
                let req_name = if let Some(fi) = fi {
                    self.stored_collections[ci].folders[fi].requests[ri].name.clone()
                } else {
                    self.stored_collections[ci].requests[ri].name.clone()
                };
                let address = node.address.clone();
                self.load_collection_request(&address);
                self.editing_request_origin = Some((ci, fi, ri));
                self.editing_request_name = req_name;
            }
        }
    }

    pub(super) fn load_collection_request(&mut self, address: &NodeAddress) {
        // Clone everything out of the stored request so we hold no borrow
        // when mutating self (needed for rebuild_http_client).
        let loaded = match address {
            NodeAddress::RootRequest(ci, ri) => {
                self.stored_collections.get(*ci).and_then(|c| c.requests.get(*ri)).cloned()
            }
            NodeAddress::FolderRequest(ci, fi, ri) => {
                self.stored_collections.get(*ci)
                    .and_then(|c| c.folders.get(*fi))
                    .and_then(|f| f.requests.get(*ri))
                    .cloned()
            }
            _ => None,
        };

        if let Some(req) = loaded {
            self.request_method_idx = METHODS.iter()
                .position(|&m| m == req.method.as_str())
                .unwrap_or(0);
            let (base_url, params) = split_url_params(&req.url);
            self.set_url(&base_url);
            self.request_url_params = params;
            self.url_params_cursor = 0;
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
            self.body_mode = BodyMode::Text;
            self.body_json_pairs = Vec::new();
            self.body_json_cursor = 0;
            self.description_textarea = if let Some(desc) = &req.description {
                let lines: Vec<String> = desc.lines().map(|l| l.to_string()).collect();
                TextArea::from(lines)
            } else {
                TextArea::default()
            };
            self.header_cursor = 0;
            self.auth_config = AuthConfig {
                auth_type: AuthType::from_str(&req.auth.auth_type),
                bearer_token: req.auth.bearer_token.clone(),
                basic_username: req.auth.basic_username.clone(),
                basic_password: req.auth.basic_password.clone(),
                api_key_name: req.auth.api_key_name.clone(),
                api_key_value: req.auth.api_key_value.clone(),
                api_key_location: ApiKeyLocation::from_str(&req.auth.api_key_location),
                oauth2_token_url: req.auth.oauth2_token_url.clone(),
                oauth2_client_id: req.auth.oauth2_client_id.clone(),
                oauth2_client_secret: req.auth.oauth2_client_secret.clone(),
                oauth2_scope: req.auth.oauth2_scope.clone(),
                oauth2_auth_url: req.auth.oauth2_auth_url.clone(),
                oauth2_redirect_port: req.auth.oauth2_redirect_port,
            };
            self.auth_field_cursor = 0;
            self.skip_tls_verify = req.skip_tls_verify;
            self.follow_redirects = req.follow_redirects;
            self.request_timeout_secs = req.timeout_secs;
            self.cookie_jar = req.cookie_jar;
            self.options_cursor = 0;
            self.cookie_jar_store = std::sync::Arc::new(reqwest::cookie::Jar::default());
            self.rebuild_http_client();
            self.editing_request_origin = None;
            // GraphQL
            self.graphql_mode = req.graphql;
            self.graphql_query_textarea = if let Some(q) = &req.graphql_query {
                let lines: Vec<String> = q.lines().map(|l| l.to_string()).collect();
                TextArea::from(lines)
            } else {
                TextArea::default()
            };
            let mut gql_vars: Vec<(String, String)> = req.graphql_variables.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            gql_vars.sort_by(|a, b| a.0.cmp(&b.0));
            self.graphql_vars = gql_vars;
            self.graphql_vars_cursor = 0;
            self.active_graphql_tab = GraphqlTab::Query;
            let req_name = req.name.clone();
            self.request_focus = RequestFocus::Response;
            self.response_body = None;
            self.response_status = None;
            self.response_elapsed_ms = None;
            self.response_cursor = 0;
            self.response_scroll = 0;
            self.response_folds = HashSet::new();
            self.response_rows = Vec::new();
            self.response_expanded = false;
            self.active_tab = Tab::Request;
            self.active_request_tab = RequestTab::Description;
            self.status_message = if req.graphql {
                format!("Loaded: {}  —  i: edit query  s: send  g: REST mode  q: quit", req_name)
            } else {
                format!("Loaded: {}  —  e: edit URL  s: send  q: quit", req_name)
            };
        }
    }

    pub(super) fn cursor_insertion_context(&self) -> Option<(usize, Option<usize>)> {
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

    pub(super) fn open_delete_modal(&mut self) {
        let flat = flatten_stored(&self.stored_collections, &self.expanded_nodes);
        if let Some(node) = flat.get(self.collection_cursor) {
            self.modal = Some(ModalState::ConfirmDelete {
                label: node.name.clone(),
                address: node.address.clone(),
            });
        }
    }

    pub(super) fn create_collection(&mut self, name: String) -> Result<()> {
        let col = StoredCollection {
            collection: CollectionMeta { name, description: String::new() },
            folders: vec![],
            requests: vec![],
            path: String::new(),
        };
        crate::storage::save_collection(&col)?;
        let ci = self.stored_collections.len();
        self.stored_collections.push(col);
        self.expanded_nodes.insert(format!("c{}", ci));
        let flat = flatten_stored(&self.stored_collections, &self.expanded_nodes);
        if let Some(pos) = flat.iter().position(|n| matches!(&n.address, NodeAddress::Collection(c) if *c == ci)) {
            self.collection_cursor = pos;
        }
        Ok(())
    }

    pub(super) fn create_folder(&mut self, name: String, ci: usize) -> Result<()> {
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

    pub(super) fn overwrite_request(&mut self, name: String, ci: usize, fi: Option<usize>, ri: usize) -> Result<()> {
        use std::collections::HashMap as HMap;
        // Compute all values from self before taking a mutable borrow on stored_collections
        let base_url = self.url_text();
        let url = if self.request_url_params.is_empty() {
            base_url.clone()
        } else {
            let sep = if base_url.contains('?') { '&' } else { '?' };
            let query = self.request_url_params.iter()
                .filter(|(k, _)| !k.is_empty())
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            format!("{}{}{}", base_url, sep, query)
        };
        let desc_text = self.description_textarea.lines().join("\n");
        let description = if desc_text.trim().is_empty() { None } else { Some(desc_text) };
        let method = METHODS[self.request_method_idx].to_string();
        let headers: HMap<String, String> = self.request_headers.iter().cloned().collect();
        let body = self.body_string();
        let auth = self.auth_config_to_stored();

        let req = if let Some(fi) = fi {
            &mut self.stored_collections[ci].folders[fi].requests[ri]
        } else {
            &mut self.stored_collections[ci].requests[ri]
        };
        let gql_query_text = self.graphql_query_textarea.lines().join("\n");
        req.name = name.clone();
        req.method = if self.graphql_mode { "POST".to_string() } else { method };
        req.url = url;
        req.headers = headers;
        req.body = if self.graphql_mode { None } else { body };
        req.description = description;
        req.auth = auth;
        req.timeout_secs = self.request_timeout_secs;
        req.follow_redirects = self.follow_redirects;
        req.skip_tls_verify = self.skip_tls_verify;
        req.cookie_jar = self.cookie_jar;
        req.graphql = self.graphql_mode;
        req.graphql_query = if self.graphql_mode && !gql_query_text.trim().is_empty() { Some(gql_query_text) } else { None };
        req.graphql_variables = if self.graphql_mode { self.graphql_vars.iter().cloned().collect() } else { std::collections::HashMap::new() };

        crate::storage::save_collection(&self.stored_collections[ci])?;
        self.editing_request_origin = None;
        self.editing_request_name = String::new();
        self.status_message = format!("Saved: \"{}\"  —  s: send  S: save again  q: quit", name);
        Ok(())
    }

    pub(super) fn add_request(&mut self, req: StoredRequest, ci: usize, fi: Option<usize>) -> Result<()> {
        if let Some(fi) = fi {
            self.stored_collections[ci].folders[fi].requests.push(req);
        } else {
            self.stored_collections[ci].requests.push(req);
        }
        crate::storage::save_collection(&self.stored_collections[ci])?;
        Ok(())
    }

    pub(super) fn delete_node(&mut self, address: NodeAddress) -> Result<()> {
        match address {
            NodeAddress::Collection(ci) => {
                let path = self.stored_collections[ci].path.clone();
                crate::storage::delete_collection_by_path(&path)?;
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
                    let _ = crate::storage::save_active_env(None);
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
}
