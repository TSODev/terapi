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
                let address = node.address.clone();
                self.load_collection_request(&address);
            }
        }
    }

    pub(super) fn load_collection_request(&mut self, address: &NodeAddress) {
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
            let (base_url, params) = split_url_params(&req.url);
            self.request_url = base_url;
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
            };
            self.auth_field_cursor = 0;
            self.editing_request_origin = None;
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
        };
        crate::storage::save_collection(&col)?;
        let ci = self.stored_collections.len();
        self.stored_collections.push(col);
        self.expanded_nodes.insert(format!("c{}", ci));
        let flat = flatten_stored(&self.stored_collections, &self.expanded_nodes);
        self.collection_cursor = flat.len().saturating_sub(1);
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
        let url = if self.request_url_params.is_empty() {
            self.request_url.clone()
        } else {
            let sep = if self.request_url.contains('?') { '&' } else { '?' };
            let query = self.request_url_params.iter()
                .filter(|(k, _)| !k.is_empty())
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            format!("{}{}{}", self.request_url, sep, query)
        };
        let desc_text = self.description_textarea.lines().join("\n");
        let description = if desc_text.trim().is_empty() { None } else { Some(desc_text) };
        let method = METHODS[self.request_method_idx].to_string();
        let headers: HMap<String, String> = self.request_headers.iter().cloned().collect();
        let body = self.body_string();
        let auth = crate::storage::StoredAuth {
            auth_type: self.auth_config.auth_type.as_str().to_string(),
            bearer_token: self.auth_config.bearer_token.clone(),
            basic_username: self.auth_config.basic_username.clone(),
            basic_password: self.auth_config.basic_password.clone(),
            api_key_name: self.auth_config.api_key_name.clone(),
            api_key_value: self.auth_config.api_key_value.clone(),
            api_key_location: self.auth_config.api_key_location.as_str().to_string(),
        };

        let req = if let Some(fi) = fi {
            &mut self.stored_collections[ci].folders[fi].requests[ri]
        } else {
            &mut self.stored_collections[ci].requests[ri]
        };
        req.name = name.clone();
        req.method = method;
        req.url = url;
        req.headers = headers;
        req.body = body;
        req.description = description;
        req.auth = auth;

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
}
