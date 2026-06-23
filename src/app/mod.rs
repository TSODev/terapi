use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::collections::HashSet;
use tokio::sync::mpsc;
use tui_textarea::TextArea;

use crate::storage::{HistoryEntry, StoredCollection, StoredEnv, StoredRequest};

mod campaigns_tab;
mod collections;
mod envs;
mod gql_completion;
mod http;
mod request;
mod response;
mod schema;
mod types;
mod var_picker;

pub use types::*;

// ── App state ─────────────────────────────────────────────────────────────────

pub struct App {
    pub running: bool,
    pub confirm_quit: bool,
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
    pub gql_completion: Option<GqlCompletionState>,
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
    pub description_textarea: TextArea<'static>,
    pub request_focus: RequestFocus,
    pub request_loading: bool,
    pub editing_request_origin: Option<(usize, Option<usize>, usize)>,
    pub editing_request_name: String,
    // Auth
    pub auth_config: AuthConfig,
    pub auth_field_cursor: usize,
    // Options
    pub skip_tls_verify: bool,
    pub follow_redirects: bool,
    pub request_timeout_secs: u64,
    pub cookie_jar: bool,
    pub options_cursor: usize,
    // HTTP client — persistent across requests (shares cookie jar)
    pub(super) http_client: reqwest::Client,
    pub(super) cookie_jar_store: std::sync::Arc<reqwest::cookie::Jar>,
    // Response
    pub last_request_raw: Option<RawRequest>,
    pub response_body: Option<String>,
    pub response_status: Option<u16>,
    pub response_elapsed_ms: Option<u64>,
    pub response_headers: Vec<(String, String)>,
    pub response_view: ResponseView,
    pub response_cursor: usize,
    pub response_scroll: u16,
    pub response_folds: HashSet<String>,
    pub json_search: Option<String>,
    pub key_col_width: u16,
    pub status_message: String,
    // History
    pub history: Vec<HistoryEntry>,
    pub history_cursor: usize,
    // GraphQL
    pub graphql_mode: bool,
    pub graphql_query_textarea: TextArea<'static>,
    pub graphql_vars: Vec<(String, String)>,
    pub graphql_vars_cursor: usize,
    pub active_graphql_tab: GraphqlTab,
    // GraphQL schema introspection
    pub schema_state: SchemaState,
    pub schema_type_cursor: usize,
    pub schema_field_scroll: u16,
    // Campaigns tab
    pub campaigns: Vec<CampaignEntry>,
    pub campaign_cursor: usize,
    pub campaign_run_state: crate::campaign::CampaignRunState,
    // Async channels — receive results from spawned tasks
    pub(super) response_rx: mpsc::UnboundedReceiver<HttpOutcome>,
    pub(super) response_tx: mpsc::UnboundedSender<HttpOutcome>,
    pub(super) schema_rx: mpsc::UnboundedReceiver<SchemaOutcome>,
    pub(super) schema_tx: mpsc::UnboundedSender<SchemaOutcome>,
    pub(super) campaign_rx: mpsc::UnboundedReceiver<crate::campaign::CampaignEvent>,
    pub(super) campaign_tx: mpsc::UnboundedSender<crate::campaign::CampaignEvent>,
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
        let campaigns = crate::storage::load_campaigns()
            .into_iter()
            .map(|(name, path, campaign)| CampaignEntry { name, path, campaign })
            .collect::<Vec<_>>();
        let (response_tx, response_rx) = mpsc::unbounded_channel();
        let (schema_tx, schema_rx) = mpsc::unbounded_channel();
        let (campaign_tx, campaign_rx) = mpsc::unbounded_channel();
        Self {
            running: true,
            confirm_quit: false,
            active_tab: Tab::Collections,
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
            gql_completion: None,
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
            description_textarea: TextArea::default(),
            request_focus: RequestFocus::Response,
            request_loading: false,
            editing_request_origin: None,
            editing_request_name: String::new(),
            auth_config: AuthConfig::default(),
            auth_field_cursor: 0,
            skip_tls_verify: false,
            follow_redirects: true,
            request_timeout_secs: 30,
            cookie_jar: false,
            options_cursor: 0,
            http_client: reqwest::Client::builder()
                .user_agent(concat!("terapi/", env!("CARGO_PKG_VERSION")))
                .timeout(std::time::Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::limited(10))
                .build()
                .expect("HTTP client init failed"),
            cookie_jar_store: std::sync::Arc::new(reqwest::cookie::Jar::default()),
            last_request_raw: None,
            response_body,
            response_status: None,
            response_elapsed_ms: None,
            response_headers: Vec::new(),
            response_view: ResponseView::Json,
            response_cursor: 0,
            response_scroll: 0,
            response_folds: HashSet::new(),
            json_search: None,
            key_col_width: 22,
            status_message: "Tab: panels  e: edit URL  s: send  S: save  n: new  m: method  ←/→: section  ↑/↓: cursor  r: raw  q: quit".into(),
            history,
            history_cursor: 0,
            graphql_mode: false,
            graphql_query_textarea: TextArea::default(),
            graphql_vars: Vec::new(),
            graphql_vars_cursor: 0,
            active_graphql_tab: GraphqlTab::Query,
            schema_state: SchemaState::Idle,
            schema_type_cursor: 0,
            schema_field_scroll: 0,
            campaigns,
            campaign_cursor: 0,
            campaign_run_state: crate::campaign::CampaignRunState::Idle,
            response_rx,
            response_tx,
            schema_rx,
            schema_tx,
            campaign_rx,
            campaign_tx,
        }
    }

    pub(super) fn rebuild_http_client(&mut self) {
        let mut builder = reqwest::Client::builder()
            .user_agent(concat!("terapi/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(self.request_timeout_secs))
            .danger_accept_invalid_certs(self.skip_tls_verify)
            .redirect(if self.follow_redirects {
                reqwest::redirect::Policy::limited(10)
            } else {
                reqwest::redirect::Policy::none()
            });
        if self.cookie_jar {
            builder = builder.cookie_provider(self.cookie_jar_store.clone());
        }
        self.http_client = builder.build().expect("HTTP client build failed");
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Reset quit confirmation on any key except q itself
        let was_confirming_quit = self.confirm_quit;
        if key.code != KeyCode::Char('q') {
            self.confirm_quit = false;
        }

        if self.gql_completion.is_some() {
            return self.handle_gql_completion_key(key);
        }

        if self.var_picker.is_some() {
            return self.handle_var_picker_key(key);
        }

        if self.modal.is_some() {
            return self.handle_modal_key(key);
        }

        // JSON search bar intercepts all keys when open
        if self.json_search.is_some()
            && self.active_tab == Tab::Request
            && self.response_view == ResponseView::Json
        {
            return self.handle_json_search_key(key);
        }

        // Body editor intercepts all keys when focused
        if self.active_tab == Tab::Request && self.request_focus == RequestFocus::Body {
            return self.handle_body_key(key);
        }

        // Description editor intercepts all keys when focused
        if self.active_tab == Tab::Request && self.request_focus == RequestFocus::Description {
            if key.code == KeyCode::Esc {
                self.request_focus = RequestFocus::Response;
                self.update_request_status_hint();
            } else {
                self.description_textarea.input(tui_textarea::Input::from(key));
            }
            return Ok(());
        }

        match key.code {
            // URL edit mode Esc must come before the global quit handler
            KeyCode::Esc
                if self.active_tab == Tab::Request
                    && self.request_focus == RequestFocus::Url =>
            {
                self.parse_url_into_params();
                self.request_focus = RequestFocus::Response;
                self.status_message = "Tab: panels  e: edit URL  s: send  m: method  ←/→: section  ↑/↓: cursor  r: raw  q: quit".into();
            }

            KeyCode::Char('q') => {
                if was_confirming_quit {
                    self.running = false;
                } else {
                    self.confirm_quit = true;
                    self.status_message = "Press q again to quit".into();
                }
            }
            KeyCode::Tab => {
                self.active_tab = match self.active_tab {
                    Tab::Collections => Tab::Request,
                    Tab::Request     => Tab::Env,
                    Tab::Env         => Tab::History,
                    Tab::History     => Tab::Campaigns,
                    Tab::Campaigns   => Tab::Collections,
                };
                match self.active_tab {
                    Tab::Request     => self.update_request_status_hint(),
                    Tab::Collections => self.status_message = "Tab: switch panel  ↑/↓: navigate  Enter: expand/load  n: new  f: folder  a: add  e: edit  d: delete  q: quit".into(),
                    Tab::Env         => self.status_message = "Tab: switch panel  ←/→: switch focus  ↑/↓: navigate  Enter: activate  n: new env  a: add var  d: delete  q: quit".into(),
                    Tab::History     => self.status_message = "Tab: switch panel  ↑/↓: navigate  Enter: load  d: delete  q: quit".into(),
                    Tab::Campaigns   => self.status_message = "Tab: switch panel  ↑/↓: navigate  r: run  Esc: clear  q: quit".into(),
                };
            }

            // ── Request panel — URL edit mode ──────────────────────────────
            KeyCode::Enter
                if self.active_tab == Tab::Request
                    && self.request_focus == RequestFocus::Url =>
            {
                self.parse_url_into_params();
                self.send_request();
            }
            KeyCode::Up
                if self.active_tab == Tab::Request
                    && self.request_focus == RequestFocus::Url
                    && !self.graphql_mode =>
            {
                self.request_method_idx = if self.request_method_idx == 0 {
                    METHODS.len() - 1
                } else {
                    self.request_method_idx - 1
                };
            }
            KeyCode::Down
                if self.active_tab == Tab::Request
                    && self.request_focus == RequestFocus::Url
                    && !self.graphql_mode =>
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

            // ── Request panel — GraphQL mode toggle ────────────────────────
            KeyCode::Char('g') if self.active_tab == Tab::Request => {
                self.graphql_mode = !self.graphql_mode;
                self.request_focus = RequestFocus::Response;
                if self.graphql_mode {
                    self.active_graphql_tab = GraphqlTab::Query;
                    self.status_message = "GraphQL — i: edit query  ←/→: section  s: send  S: save  g: REST mode  q: quit".into();
                } else {
                    self.update_request_status_hint();
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
                    let (name, collection_idx, folder_display_idx) =
                        if let Some((ci, fi, _)) = self.editing_request_origin {
                            (self.editing_request_name.clone(), ci, fi.map_or(0, |f| f + 1))
                        } else {
                            (String::new(), 0, 0)
                        };
                    self.modal = Some(ModalState::SaveRequest {
                        name,
                        collection_idx,
                        folder_display_idx,
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
                    && self.graphql_mode
                    && self.active_graphql_tab == GraphqlTab::Query =>
            {
                self.request_focus = RequestFocus::Body;
                self.status_message = "Query: editing  Esc: done".into();
            }
            KeyCode::Char('i')
                if self.active_tab == Tab::Request
                    && !self.graphql_mode
                    && self.active_request_tab == RequestTab::Body =>
            {
                self.request_focus = RequestFocus::Body;
                self.status_message = match self.body_mode {
                    BodyMode::Text => "Body [Text]: editing  Esc: exit editor".into(),
                    BodyMode::Json => "Body [JSON]: ↑↓: navigate  a: add  d: delete  Enter: edit  Esc: exit".into(),
                };
            }
            KeyCode::Char('i')
                if self.active_tab == Tab::Request
                    && !self.graphql_mode
                    && self.active_request_tab == RequestTab::Description =>
            {
                self.request_focus = RequestFocus::Description;
                self.status_message = "Description: editing  Esc: exit editor".into();
            }
            KeyCode::Char('t')
                if self.active_tab == Tab::Request
                    && !self.graphql_mode
                    && self.active_request_tab == RequestTab::Body
                    && self.request_focus != RequestFocus::Body =>
            {
                self.toggle_body_mode();
            }
            KeyCode::Char('m')
                if self.active_tab == Tab::Request && !self.graphql_mode =>
            {
                self.request_method_idx = (self.request_method_idx + 1) % METHODS.len();
            }
            // GraphQL Variables tab — add/delete/edit
            KeyCode::Char('a')
                if self.active_tab == Tab::Request
                    && self.graphql_mode
                    && self.active_graphql_tab == GraphqlTab::Variables =>
            {
                self.modal = Some(ModalState::BodyPair {
                    key: String::new(),
                    value: String::new(),
                    active_field: VarField::Key,
                    edit_idx: None,
                });
            }
            KeyCode::Char('d')
                if self.active_tab == Tab::Request
                    && self.graphql_mode
                    && self.active_graphql_tab == GraphqlTab::Variables
                    && !self.graphql_vars.is_empty() =>
            {
                self.graphql_vars.remove(self.graphql_vars_cursor);
                if self.graphql_vars_cursor > 0 && self.graphql_vars_cursor >= self.graphql_vars.len() {
                    self.graphql_vars_cursor -= 1;
                }
            }
            KeyCode::Enter
                if self.active_tab == Tab::Request
                    && self.graphql_mode
                    && self.active_graphql_tab == GraphqlTab::Variables
                    && !self.graphql_vars.is_empty() =>
            {
                let (k, v) = self.graphql_vars[self.graphql_vars_cursor].clone();
                self.modal = Some(ModalState::BodyPair {
                    key: k,
                    value: v,
                    active_field: VarField::Key,
                    edit_idx: Some(self.graphql_vars_cursor),
                });
            }
            KeyCode::Up
                if self.active_tab == Tab::Request
                    && self.graphql_mode
                    && self.active_graphql_tab == GraphqlTab::Variables =>
            {
                if self.graphql_vars_cursor > 0 {
                    self.graphql_vars_cursor -= 1;
                }
            }
            KeyCode::Down
                if self.active_tab == Tab::Request
                    && self.graphql_mode
                    && self.active_graphql_tab == GraphqlTab::Variables =>
            {
                if self.graphql_vars_cursor + 1 < self.graphql_vars.len() {
                    self.graphql_vars_cursor += 1;
                }
            }
            // ── Request panel — GraphQL Schema tab ────────────────────────
            KeyCode::Char('f')
                if self.active_tab == Tab::Request
                    && self.graphql_mode
                    && self.active_graphql_tab == GraphqlTab::Schema =>
            {
                self.fetch_schema();
            }
            KeyCode::Enter
                if self.active_tab == Tab::Request
                    && self.graphql_mode
                    && self.active_graphql_tab == GraphqlTab::Schema =>
            {
                if let SchemaState::Ready { ref types, .. } = self.schema_state {
                    if let Some(t) = types.get(self.schema_type_cursor) {
                        let name = t.name.clone();
                        self.fetch_type_detail(name);
                    }
                }
            }
            KeyCode::Up
                if self.active_tab == Tab::Request
                    && self.graphql_mode
                    && self.active_graphql_tab == GraphqlTab::Schema =>
            {
                if self.schema_type_cursor > 0 {
                    self.schema_type_cursor -= 1;
                    self.schema_field_scroll = 0;
                    if let SchemaState::Ready { ref mut detail, .. } = self.schema_state {
                        *detail = SchemaDetail::None;
                    }
                }
            }
            KeyCode::Down
                if self.active_tab == Tab::Request
                    && self.graphql_mode
                    && self.active_graphql_tab == GraphqlTab::Schema =>
            {
                let len = if let SchemaState::Ready { ref types, .. } = self.schema_state {
                    types.len()
                } else {
                    0
                };
                if self.schema_type_cursor + 1 < len {
                    self.schema_type_cursor += 1;
                    self.schema_field_scroll = 0;
                    if let SchemaState::Ready { ref mut detail, .. } = self.schema_state {
                        *detail = SchemaDetail::None;
                    }
                }
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
            KeyCode::Up
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::Options =>
            {
                if self.options_cursor > 0 { self.options_cursor -= 1; }
            }
            KeyCode::Down
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::Options =>
            {
                if self.options_cursor < 3 { self.options_cursor += 1; }
            }
            KeyCode::Char(' ') | KeyCode::Enter
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::Options =>
            {
                const TIMEOUT_STEPS: &[u64] = &[5, 10, 15, 20, 30, 45, 60, 90, 120, 300];
                match self.options_cursor {
                    0 => { self.skip_tls_verify = !self.skip_tls_verify; }
                    1 => { self.follow_redirects = !self.follow_redirects; }
                    2 => {
                        let next = TIMEOUT_STEPS.iter()
                            .find(|&&v| v > self.request_timeout_secs)
                            .copied()
                            .unwrap_or(TIMEOUT_STEPS[0]);
                        self.request_timeout_secs = next;
                    }
                    3 => {
                        self.cookie_jar = !self.cookie_jar;
                        if !self.cookie_jar {
                            // clear the jar when disabled
                            self.cookie_jar_store = std::sync::Arc::new(reqwest::cookie::Jar::default());
                        }
                    }
                    _ => {}
                }
                self.rebuild_http_client();
            }
            KeyCode::Up
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::Auth =>
            {
                if self.auth_field_cursor > 0 {
                    self.auth_field_cursor -= 1;
                }
            }
            KeyCode::Down
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::Auth =>
            {
                if self.auth_field_cursor + 1 < self.auth_config.field_count() {
                    self.auth_field_cursor += 1;
                }
            }
            KeyCode::Char(' ') | KeyCode::Enter
                if self.active_tab == Tab::Request
                    && self.active_request_tab == RequestTab::Auth =>
            {
                if self.auth_field_cursor == 0 {
                    self.auth_config.auth_type = self.auth_config.auth_type.next();
                    self.auth_field_cursor = 0;
                } else {
                    match (&self.auth_config.auth_type, self.auth_field_cursor) {
                        (AuthType::Bearer, 1) => {
                            self.modal = Some(ModalState::EditAuthField {
                                kind: AuthFieldKind::BearerToken,
                                value: self.auth_config.bearer_token.clone(),
                            });
                        }
                        (AuthType::Basic, 1) => {
                            self.modal = Some(ModalState::EditAuthField {
                                kind: AuthFieldKind::BasicUsername,
                                value: self.auth_config.basic_username.clone(),
                            });
                        }
                        (AuthType::Basic, 2) => {
                            self.modal = Some(ModalState::EditAuthField {
                                kind: AuthFieldKind::BasicPassword,
                                value: self.auth_config.basic_password.clone(),
                            });
                        }
                        (AuthType::ApiKey, 1) => {
                            self.modal = Some(ModalState::EditAuthField {
                                kind: AuthFieldKind::ApiKeyName,
                                value: self.auth_config.api_key_name.clone(),
                            });
                        }
                        (AuthType::ApiKey, 2) => {
                            self.modal = Some(ModalState::EditAuthField {
                                kind: AuthFieldKind::ApiKeyValue,
                                value: self.auth_config.api_key_value.clone(),
                            });
                        }
                        (AuthType::ApiKey, 3) => {
                            self.auth_config.api_key_location = self.auth_config.api_key_location.toggle();
                        }
                        _ => {}
                    }
                }
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
                if self.graphql_mode {
                    self.active_graphql_tab = self.active_graphql_tab.next();
                    self.update_graphql_status_hint();
                } else {
                    self.active_request_tab = self.active_request_tab.next();
                    self.update_request_status_hint();
                }
            }
            KeyCode::Left if self.active_tab == Tab::Request => {
                if self.graphql_mode {
                    self.active_graphql_tab = self.active_graphql_tab.prev();
                    self.update_graphql_status_hint();
                } else {
                    self.active_request_tab = self.active_request_tab.prev();
                    self.update_request_status_hint();
                }
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
            KeyCode::Char('/') if self.active_tab == Tab::Request && self.response_view == ResponseView::Json => {
                self.json_search = Some(String::new());
                self.status_message = "Search: type to filter  >: next  <: prev  Esc: close".into();
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
                        let req_name = if let Some(fi) = fi {
                            self.stored_collections[ci].folders[fi].requests[ri].name.clone()
                        } else {
                            self.stored_collections[ci].requests[ri].name.clone()
                        };
                        let address = node.address.clone();
                        self.load_collection_request(&address);
                        self.editing_request_origin = Some((ci, fi, ri));
                        self.editing_request_name = req_name;
                        self.active_request_tab = RequestTab::Description;
                        self.status_message = "Editing — i: description  ←/→: section  S: save  s: send  n: new request  q: quit".into();
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

            // ── Campaigns panel ────────────────────────────────────────────
            KeyCode::Up if self.active_tab == Tab::Campaigns => {
                if self.campaign_cursor > 0 { self.campaign_cursor -= 1; }
            }
            KeyCode::Down if self.active_tab == Tab::Campaigns => {
                if self.campaign_cursor + 1 < self.campaigns.len() {
                    self.campaign_cursor += 1;
                }
            }
            KeyCode::Char('r') if self.active_tab == Tab::Campaigns => {
                self.open_campaign_params_or_run();
            }
            KeyCode::Esc if self.active_tab == Tab::Campaigns => {
                self.campaign_run_state = crate::campaign::CampaignRunState::Idle;
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
                    if self.graphql_mode {
                        if let Some(idx) = edit_idx {
                            self.graphql_vars[idx] = (bp_key.trim().to_string(), bp_val.trim().to_string());
                        } else {
                            self.graphql_vars.push((bp_key.trim().to_string(), bp_val.trim().to_string()));
                            self.graphql_vars_cursor = self.graphql_vars.len() - 1;
                        }
                    } else if let Some(idx) = edit_idx {
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
                        let overwrite_origin = self.editing_request_origin
                            .filter(|(oci, ofi, _)| *oci == collection_idx && *ofi == fi);
                        if let Some((ci, ofi, ri)) = overwrite_origin {
                            self.overwrite_request(name.trim().to_string(), ci, ofi, ri)?;
                        } else {
                            self.save_request_to_collection(name.trim().to_string(), collection_idx, fi)?;
                        }
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

            Some(ModalState::EditAuthField { kind, mut value }) => match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter => {
                    match kind {
                        AuthFieldKind::BearerToken   => self.auth_config.bearer_token   = value,
                        AuthFieldKind::BasicUsername => self.auth_config.basic_username = value,
                        AuthFieldKind::BasicPassword => self.auth_config.basic_password = value,
                        AuthFieldKind::ApiKeyName    => self.auth_config.api_key_name   = value,
                        AuthFieldKind::ApiKeyValue   => self.auth_config.api_key_value  = value,
                    }
                }
                KeyCode::Char(c) => {
                    value.push(c);
                    self.modal = Some(ModalState::EditAuthField { kind, value });
                }
                KeyCode::Backspace => {
                    value.pop();
                    self.modal = Some(ModalState::EditAuthField { kind, value });
                }
                _ => { self.modal = Some(ModalState::EditAuthField { kind, value }); }
            },

            Some(ModalState::CampaignParams { campaign_idx, mut params, mut cursor, mut editing, mut input }) => {
                if editing {
                    match key.code {
                        KeyCode::Enter => {
                            params[cursor].2 = input.clone();
                            editing = false;
                            input = String::new();
                            self.modal = Some(ModalState::CampaignParams { campaign_idx, params, cursor, editing, input });
                        }
                        KeyCode::Esc => {
                            editing = false;
                            input = String::new();
                            self.modal = Some(ModalState::CampaignParams { campaign_idx, params, cursor, editing, input });
                        }
                        KeyCode::Char(c) => {
                            input.push(c);
                            self.modal = Some(ModalState::CampaignParams { campaign_idx, params, cursor, editing, input });
                        }
                        KeyCode::Backspace => {
                            input.pop();
                            self.modal = Some(ModalState::CampaignParams { campaign_idx, params, cursor, editing, input });
                        }
                        _ => { self.modal = Some(ModalState::CampaignParams { campaign_idx, params, cursor, editing, input }); }
                    }
                } else {
                    match key.code {
                        KeyCode::Esc => {}
                        KeyCode::Up => {
                            if cursor > 0 { cursor -= 1; }
                            self.modal = Some(ModalState::CampaignParams { campaign_idx, params, cursor, editing, input });
                        }
                        KeyCode::Down => {
                            if cursor + 1 < params.len() { cursor += 1; }
                            self.modal = Some(ModalState::CampaignParams { campaign_idx, params, cursor, editing, input });
                        }
                        KeyCode::Enter => {
                            input = params[cursor].2.clone();
                            editing = true;
                            self.modal = Some(ModalState::CampaignParams { campaign_idx, params, cursor, editing, input });
                        }
                        KeyCode::Char('r') => {
                            let overrides: std::collections::HashMap<String, String> = params.iter()
                                .map(|(name, _, value)| (name.clone(), value.clone()))
                                .collect();
                            self.campaign_cursor = campaign_idx;
                            self.run_selected_campaign(overrides);
                        }
                        _ => { self.modal = Some(ModalState::CampaignParams { campaign_idx, params, cursor, editing, input }); }
                    }
                }
            }

            None => {}
        }
        Ok(())
    }
}
