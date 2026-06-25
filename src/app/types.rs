use std::collections::HashSet;

use crate::storage::{StoredCollection, StoredEnv};

// ── HTTP types ────────────────────────────────────────────────────────────────

pub struct HttpResult {
    pub status: u16,
    pub body: String,
    pub headers: Vec<(String, String)>,
    pub elapsed_ms: u64,
    /// Redirect hops recorded before the final response: (status, url_followed_to).
    pub redirect_chain: Vec<(u16, String)>,
}

pub type HttpOutcome = anyhow::Result<HttpResult, String>;

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum RequestFocus {
    Url,
    Body,
    Description,
    Response,
}

pub const METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE"];

pub const COMMON_HEADERS: &[(&str, &str)] = &[
    ("Authorization",    "Bearer "),
    ("Content-Type",     "application/json"),
    ("Accept",           "application/json"),
    ("Accept-Language",  "en-US,en;q=0.9"),
    ("Accept-Encoding",  "gzip, deflate, br"),
    ("Cache-Control",    "no-cache"),
    ("X-API-Key",        ""),
    ("X-Request-ID",     ""),
    ("User-Agent",       ""),
    ("Origin",           ""),
    ("Referer",          ""),
];

pub const COMMON_CONTENT_TYPES: &[&str] = &[
    "application/json",
    "application/x-www-form-urlencoded",
    "multipart/form-data",
    "text/plain; charset=utf-8",
    "text/html; charset=utf-8",
    "text/xml",
    "application/xml",
    "application/octet-stream",
    "application/graphql",
];

#[derive(Debug, Clone, PartialEq)]
pub enum BodyMode {
    Text,
    Json,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResponseView {
    Json,
    Raw,
    Http,
}

pub struct RawRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VarPickerTarget {
    Url,
    ModalValue,
    BodyText,
}

pub struct VarPickerState {
    pub target: VarPickerTarget,
    pub prefix: String,
    pub cursor: usize,
}

// ── GraphQL Schema ────────────────────────────────────────────────────────────

pub struct GqlArg {
    pub name: String,
    pub type_str: String,
}

pub struct GqlField {
    pub name: String,
    pub type_str: String,
    pub args: Vec<GqlArg>,
}

pub struct GqlTypeSummary {
    pub name: String,
    pub kind: String,
}

pub struct GqlTypeDetail {
    pub name: String,
    pub kind: String,
    pub description: Option<String>,
    pub fields: Vec<GqlField>,
    pub input_fields: Vec<GqlField>,
    pub enum_values: Vec<String>,
}

pub enum SchemaDetail {
    None,
    Loading,
    Loaded(GqlTypeDetail),
    Error(String),
}

pub enum SchemaState {
    Idle,
    LoadingList,
    Ready {
        types: Vec<GqlTypeSummary>,
        detail: SchemaDetail,
    },
    Error(String),
}

pub enum SchemaMsg {
    TypeList(Vec<GqlTypeSummary>),
    TypeDetail(GqlTypeDetail),
}

pub type SchemaOutcome = Result<SchemaMsg, String>;

// ── GQL completion ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GqlCompletionItem {
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct GqlCompletionState {
    pub items: Vec<GqlCompletionItem>,
    pub prefix: String,
    pub cursor: usize,
}

// ── GraphQL ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum GraphqlTab {
    Query,
    Variables,
    Headers,
    Schema,
    Options,
}

impl GraphqlTab {
    pub fn title(&self) -> &'static str {
        match self {
            GraphqlTab::Query     => "Query",
            GraphqlTab::Variables => "Variables",
            GraphqlTab::Headers   => "Headers",
            GraphqlTab::Schema    => "Schema",
            GraphqlTab::Options   => "Options",
        }
    }

    pub fn all() -> Vec<GraphqlTab> {
        vec![
            GraphqlTab::Query,
            GraphqlTab::Variables,
            GraphqlTab::Headers,
            GraphqlTab::Schema,
            GraphqlTab::Options,
        ]
    }

    pub fn next(&self) -> GraphqlTab {
        let all = GraphqlTab::all();
        let pos = all.iter().position(|t| t == self).unwrap_or(0);
        all.into_iter().nth((pos + 1) % 5).unwrap_or(GraphqlTab::Query)
    }

    pub fn prev(&self) -> GraphqlTab {
        let all = GraphqlTab::all();
        let pos = all.iter().position(|t| t == self).unwrap_or(0);
        all.into_iter().nth(if pos == 0 { 4 } else { pos - 1 }).unwrap_or(GraphqlTab::Options)
    }
}

// ── Navigation ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    Request,
    Collections,
    Env,
    History,
    Campaigns,
}

impl Tab {
    pub fn title(&self) -> &'static str {
        match self {
            Tab::Request     => "Request",
            Tab::Collections => "Collections",
            Tab::Env         => "Env",
            Tab::History     => "History",
            Tab::Campaigns   => "Campaigns",
        }
    }

    pub fn all() -> Vec<Tab> {
        vec![Tab::Collections, Tab::Request, Tab::Env, Tab::History, Tab::Campaigns]
    }
}

// ── Campaign tab ──────────────────────────────────────────────────────────────

pub struct CampaignEntry {
    pub name: String,
    pub path: String,
    pub campaign: crate::campaign::Campaign,
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

    pub fn next(&self) -> RequestTab {
        let all = RequestTab::all();
        let pos = all.iter().position(|t| t == self).unwrap_or(0);
        all.into_iter().nth((pos + 1) % 6).unwrap_or(RequestTab::Description)
    }

    pub fn prev(&self) -> RequestTab {
        let all = RequestTab::all();
        let pos = all.iter().position(|t| t == self).unwrap_or(0);
        all.into_iter().nth(if pos == 0 { 5 } else { pos - 1 }).unwrap_or(RequestTab::Options)
    }
}

// ── Modal field selectors ─────────────────────────────────────────────────────

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

#[derive(Debug, Clone, PartialEq)]
pub enum CampaignFocus {
    List,
    Result,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SaveField {
    Name,
    Collection,
    NewCollectionInput { input: String },
    Folder,
    NewFolderInput { input: String },
}

// ── Auth ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub enum AuthType {
    #[default]
    None,
    Bearer,
    Basic,
    ApiKey,
    OAuth2ClientCredentials,
    OAuth2AuthorizationCode,
}

impl AuthType {
    pub fn next(&self) -> AuthType {
        match self {
            AuthType::None                    => AuthType::Bearer,
            AuthType::Bearer                  => AuthType::Basic,
            AuthType::Basic                   => AuthType::ApiKey,
            AuthType::ApiKey                  => AuthType::OAuth2ClientCredentials,
            AuthType::OAuth2ClientCredentials => AuthType::OAuth2AuthorizationCode,
            AuthType::OAuth2AuthorizationCode => AuthType::None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            AuthType::None                    => "none",
            AuthType::Bearer                  => "bearer",
            AuthType::Basic                   => "basic",
            AuthType::ApiKey                  => "apikey",
            AuthType::OAuth2ClientCredentials => "oauth2_client_credentials",
            AuthType::OAuth2AuthorizationCode => "oauth2_authorization_code",
        }
    }

    pub fn from_str(s: &str) -> AuthType {
        match s {
            "bearer"                    => AuthType::Bearer,
            "basic"                     => AuthType::Basic,
            "apikey"                    => AuthType::ApiKey,
            "oauth2_client_credentials" => AuthType::OAuth2ClientCredentials,
            "oauth2_authorization_code" => AuthType::OAuth2AuthorizationCode,
            _                           => AuthType::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum ApiKeyLocation {
    #[default]
    Header,
    QueryParam,
}

impl ApiKeyLocation {
    pub fn toggle(&self) -> ApiKeyLocation {
        match self {
            ApiKeyLocation::Header    => ApiKeyLocation::QueryParam,
            ApiKeyLocation::QueryParam => ApiKeyLocation::Header,
        }
    }



    pub fn as_str(&self) -> &'static str {
        match self {
            ApiKeyLocation::Header    => "header",
            ApiKeyLocation::QueryParam => "queryparam",
        }
    }

    pub fn from_str(s: &str) -> ApiKeyLocation {
        if s == "queryparam" { ApiKeyLocation::QueryParam } else { ApiKeyLocation::Header }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AuthConfig {
    pub auth_type: AuthType,
    pub bearer_token: String,
    pub basic_username: String,
    pub basic_password: String,
    pub api_key_name: String,
    pub api_key_value: String,
    pub api_key_location: ApiKeyLocation,
    // OAuth2 shared fields
    pub oauth2_token_url: String,
    pub oauth2_client_id: String,
    pub oauth2_client_secret: String,
    pub oauth2_scope: String,
    // OAuth2 Authorization Code only
    pub oauth2_auth_url: String,
    pub oauth2_redirect_port: u16,
}

impl AuthConfig {
    pub fn field_count(&self) -> usize {
        match self.auth_type {
            AuthType::None                    => 1,
            AuthType::Bearer                  => 2,
            AuthType::Basic                   => 3,
            AuthType::ApiKey                  => 4,
            AuthType::OAuth2ClientCredentials => 5, // type + token_url + client_id + secret + scope
            AuthType::OAuth2AuthorizationCode => 7, // + auth_url + redirect_port
        }
    }

    pub fn oauth2_cache_key(&self) -> String {
        format!("{}:{}:{}", self.auth_type.as_str(), self.oauth2_token_url, self.oauth2_client_id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AuthFieldKind {
    BearerToken,
    BasicUsername,
    BasicPassword,
    ApiKeyName,
    ApiKeyValue,
    OAuth2TokenUrl,
    OAuth2ClientId,
    OAuth2ClientSecret,
    OAuth2Scope,
    OAuth2AuthUrl,
    OAuth2RedirectPort,
}

impl AuthFieldKind {
    pub fn label(&self) -> &'static str {
        match self {
            AuthFieldKind::BearerToken        => "Token",
            AuthFieldKind::BasicUsername      => "Username",
            AuthFieldKind::BasicPassword      => "Password",
            AuthFieldKind::ApiKeyName         => "Key Name",
            AuthFieldKind::ApiKeyValue        => "Key Value",
            AuthFieldKind::OAuth2TokenUrl     => "Token URL",
            AuthFieldKind::OAuth2ClientId     => "Client ID",
            AuthFieldKind::OAuth2ClientSecret => "Client Secret",
            AuthFieldKind::OAuth2Scope        => "Scope",
            AuthFieldKind::OAuth2AuthUrl      => "Auth URL",
            AuthFieldKind::OAuth2RedirectPort => "Redirect Port",
        }
    }
}

// ── OAuth2 session state ──────────────────────────────────────────────────────

pub struct CachedToken {
    pub access_token: String,
    pub expires_at: Option<std::time::Instant>,
}

impl CachedToken {
    pub fn is_valid(&self) -> bool {
        match self.expires_at {
            None      => true,
            Some(exp) => std::time::Instant::now() < exp,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum OAuth2WaitState {
    Idle,
    FetchingToken,
    WaitingForBrowser { port: u16 },
    Error(String),
}

// ── Collections tree ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum NodeAddress {
    Collection(usize),
    Folder(usize, usize),
    RootRequest(usize, usize),
    FolderRequest(usize, usize, usize),
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
    HeaderPicker {
        cursor: usize,
    },
    ContentTypePicker {
        cursor: usize,
    },
    NewHeader {
        key: String,
        value: String,
        active_field: VarField,
    },
    UrlParam {
        key: String,
        value: String,
        active_field: VarField,
        edit_idx: Option<usize>,
    },
    SaveRequest {
        name: String,
        collection_idx: usize,
        folder_display_idx: usize,
        active_field: SaveField,
    },
    BodyPair {
        key: String,
        value: String,
        active_field: VarField,
        edit_idx: Option<usize>,
    },
    ConfirmDelete {
        label: String,
        address: NodeAddress,
    },
    EditVar {
        key: String,
        value: String,
        active_field: VarField,
        env_idx: usize,
        original_key: String,
    },
    EditAuthField {
        kind: AuthFieldKind,
        value: String,
    },
    CampaignParams {
        campaign_idx: usize,
        // (name, description, current_value)
        params: Vec<(String, String, String)>,
        cursor: usize,
        editing: bool,
        input: String,
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
                            method: Some(if req.graphql { "GQL".to_string() } else { req.method.clone() }),
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
                    method: Some(if req.graphql { "GQL".to_string() } else { req.method.clone() }),
                    address: NodeAddress::RootRequest(ci, ri),
                });
            }
        }
    }
    result
}

/// Like `flatten_stored` but always includes all children regardless of expansion state.
/// Used when searching so that collapsed folders are also searched.
pub fn flatten_stored_full(cols: &[StoredCollection], expanded: &HashSet<String>) -> Vec<FlatNode> {
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
            for (ri, req) in folder.requests.iter().enumerate() {
                result.push(FlatNode {
                    depth: 2,
                    name: req.name.clone(),
                    is_folder: false,
                    expanded: false,
                    method: Some(if req.graphql { "GQL".to_string() } else { req.method.clone() }),
                    address: NodeAddress::FolderRequest(ci, fi, ri),
                });
            }
        }
        for (ri, req) in col.requests.iter().enumerate() {
            result.push(FlatNode {
                depth: 1,
                name: req.name.clone(),
                is_folder: false,
                expanded: false,
                method: Some(if req.graphql { "GQL".to_string() } else { req.method.clone() }),
                address: NodeAddress::RootRequest(ci, ri),
            });
        }
    }
    result
}

pub fn sorted_vars(env: &StoredEnv) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = env.vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    pairs
}
