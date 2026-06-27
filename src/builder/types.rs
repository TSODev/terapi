use std::collections::HashSet;

// ── Builder focus ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum BuilderFocus {
    Pipeline,
    Catalog { insert_after: Option<usize>, cursor: usize },
    StepEditor {
        step_idx: usize,
        section_cursor: usize,
        sub_cursor: usize,
        mode: StepEditorMode,
        desc_active: bool,
    },
    CollectionBrowser { for_step: usize, col_cursor: usize, expanded: HashSet<String> },
    CampaignSettings { cursor: usize, mode: CampaignSettingsMode },
    Variables { cursor: usize, mode: VariablesMode },
    Checker { results: Vec<CheckResult> },
    TomlPreview { scroll: usize },
    Run { scroll: usize },
    ParamsEditor      { cursor: usize, mode: ParamEditorMode },
    ConnectorsEditor    { cursor: usize, mode: IoEditorMode },
    OutputsEditor       { cursor: usize, mode: IoEditorMode },
    PipelineConnectors  { cursor: usize },
    PipelineOutputs     { cursor: usize },
    OutputStepPicker  {
        output_idx:    Option<usize>, // None = adding new
        step_cursor:   usize,
        f1: String, f2: String, f3: String, // path, select, include_vars
        output_cursor: usize,         // return cursor in OutputsEditor
    },
}

// ── I/O editors (connectors + outputs share the same mode shape) ──────────────

/// field indices for ConnectorsEditor: 0=kind, 1=path, 2=select, 3=from_step
/// field indices for OutputsEditor:    0=from_step, 1=path, 2=select, 3=include_vars
#[derive(Debug, Clone)]
pub enum IoEditorMode {
    Browse,
    Edit {
        idx:    Option<usize>, // None = add new
        f0:     String,
        f1:     String,
        f2:     String,
        f3:     String,
        field:  u8,
    },
}

// ── Params editor ─────────────────────────────────────────────────────────────

/// field: 0 = name, 1 = description, 2 = default value
#[derive(Debug, Clone)]
pub enum ParamEditorMode {
    Browse,
    AddParam  { name: String, desc: String, default_val: String, field: u8 },
    EditParam { idx: usize, name: String, desc: String, default_val: String, field: u8 },
}

// ── Variables mode ────────────────────────────────────────────────────────────

/// field: 0 = key, 1 = value
#[derive(Debug, Clone)]
pub enum VariablesMode {
    Browse,
    Edit { original_key: Option<String>, key: String, value: String, field: u8 },
}

// ── Campaign settings mode ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum CampaignSettingsMode {
    Browse,
    EditText { buffer: String },
}

// ── Step editor ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum PairTarget {
    Headers,
    Extract,
    GraphqlVariables,
    Vars,
    ParallelSteps,  // single-stage: step name only (no value)
}

/// (label, needs_value)
pub const ASSERT_OPS: &[(&str, bool)] = &[
    ("eq",         true),
    ("ne",         true),
    ("lt",         true),
    ("lte",        true),
    ("gt",         true),
    ("gte",        true),
    ("contains",   true),
    ("matches",    true),
    ("exists",     false),
    ("not exists", false),
];

/// (label, needs_value)
pub const WHEN_OPS: &[(&str, bool)] = &[
    ("eq",         true),
    ("ne",         true),
    ("exists",     false),
    ("not exists", false),
];

#[derive(Debug, Clone)]
pub enum StepEditorMode {
    Browse,
    EditText { buffer: String, cursor: usize },
    EditBody,
    AddPairStage1 { target: PairTarget, buffer: String },
    AddPairStage2 { target: PairTarget, key: String, buffer: String, cursor: usize },
    // Assertion creation flow
    AddAssertPath { buffer: String },
    AddAssertOp   { path: String, op: usize },
    AddAssertValue { path: String, op: usize, buffer: String },
    // When-condition edit flow
    EditWhenVar   { buffer: String },
    EditWhenOp    { var: String, op: usize },
    EditWhenValue { var: String, op: usize, buffer: String },
    // Multipart part add/edit flow: stage 0=name 1=value 2=content_type
    AddMultipart  { idx: Option<usize>, name: String, value: String, content_type: String, stage: u8 },
    // JSON dot-path picker for Extract value fields (opened with Tab from AddPairStage2 Extract)
    ExtractPicker { key: String, paths: Vec<String>, filter: String, cursor: usize },
    // GraphQL query editor (multi-line textarea)
    EditGraphqlQuery,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StepSection {
    Name,
    Description,
    Method,
    Url,
    Body,
    Headers,
    Extract,
    Assertions,
    Foreach,
    When,
    ContinueOnError,
    WaitMs,
    TransformKind,
    TransformInput,
    TransformOutput,
    FilePath,
    FileOutput,
    FileEncoding,
    MultipartParts,
    GraphqlQuery,
    GraphqlVariables,
    LoadFromCollection,
    // Loop step sections
    LoopUrl,
    LoopMethod,
    LoopUntilVar,
    LoopUntilCond,
    LoopAccumulateVar,
    LoopAccumulateFrom,
    LoopExtract,
    LoopHeaders,
    LoopContinueOnError,
    // Search step sections
    SearchInput,
    SearchPath,
    SearchMatch,
    SearchOutput,
    SearchFirstOnly,
    // Set step sections
    SetVars,
    // JQ step sections
    JqInput,
    JqExpression,
    JqOutput,
    JqRaw,
    // Poll step sections
    PollUrl,
    PollMethod,
    PollHeaders,
    PollExtract,
    PollUntilVar,
    PollUntilCond,
    PollIntervalMs,
    PollTimeoutSecs,
    PollContinueOnError,
    // Parallel step sections
    ParallelSteps,
    // Notify step sections
    NotifyUrl,
    NotifyMethod,
    NotifyMessage,
}

impl StepSection {
    pub fn label(&self) -> &'static str {
        match self {
            StepSection::Name               => "Name",
            StepSection::Description        => "Description",
            StepSection::Method             => "Method",
            StepSection::Url                => "URL",
            StepSection::Body               => "Body",
            StepSection::Headers            => "Headers",
            StepSection::Extract            => "Extract",
            StepSection::Assertions         => "Assertions",
            StepSection::Foreach            => "Foreach",
            StepSection::When               => "When",
            StepSection::ContinueOnError    => "Continue on error",
            StepSection::WaitMs             => "Wait (ms)",
            StepSection::TransformKind      => "Kind",
            StepSection::TransformInput     => "Input",
            StepSection::TransformOutput    => "Output var",
            StepSection::FilePath           => "File path",
            StepSection::FileOutput         => "Output var",
            StepSection::FileEncoding       => "Encoding",
            StepSection::MultipartParts     => "Multipart parts",
            StepSection::GraphqlQuery       => "GQL Query",
            StepSection::GraphqlVariables   => "GQL Variables",
            StepSection::LoadFromCollection => "[L] Load from collection",
            StepSection::LoopUrl            => "URL",
            StepSection::LoopMethod         => "Method",
            StepSection::LoopUntilVar       => "Until — var",
            StepSection::LoopUntilCond      => "Until — condition",
            StepSection::LoopAccumulateVar  => "Accumulate — var",
            StepSection::LoopAccumulateFrom => "Accumulate — from",
            StepSection::LoopExtract        => "Extract (per-iter)",
            StepSection::LoopHeaders        => "Headers",
            StepSection::LoopContinueOnError => "Continue on error",
            StepSection::SearchInput    => "Input (JSON array var)",
            StepSection::SearchPath     => "Match on field (dot-path)",
            StepSection::SearchMatch    => "Pattern (regex)",
            StepSection::SearchOutput   => "Output var",
            StepSection::SearchFirstOnly => "First match only",
            StepSection::SetVars            => "Variables",
            StepSection::JqInput            => "Input (JSON var)",
            StepSection::JqExpression       => "Expression (jq filter)",
            StepSection::JqOutput           => "Output var",
            StepSection::JqRaw              => "Raw output (-r)",
            StepSection::PollUrl            => "URL",
            StepSection::PollMethod         => "Method",
            StepSection::PollHeaders        => "Headers",
            StepSection::PollExtract        => "Extract (per-poll)",
            StepSection::PollUntilVar       => "Until — var",
            StepSection::PollUntilCond      => "Until — condition",
            StepSection::PollIntervalMs     => "Interval (ms)",
            StepSection::PollTimeoutSecs    => "Timeout (s)",
            StepSection::PollContinueOnError => "Continue on error",
            StepSection::ParallelSteps => "Steps (run in parallel)",
            StepSection::NotifyUrl     => "Webhook URL",
            StepSection::NotifyMethod  => "Method",
            StepSection::NotifyMessage => "Message (body)",
        }
    }

    pub fn is_list(&self) -> bool {
        matches!(self,
            StepSection::Headers | StepSection::Extract | StepSection::Assertions |
            StepSection::MultipartParts | StepSection::GraphqlVariables |
            StepSection::LoopExtract | StepSection::LoopHeaders |
            StepSection::PollHeaders | StepSection::PollExtract |
            StepSection::SetVars | StepSection::ParallelSteps
        )
    }
}

// ── Catalog ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum BrickKind {
    Http,
    GraphQL,
    Search,
    Loop,
    Poll,
    Set,
    Jq,
    Parallel,
    Notify,
    Transform,
    Pause,
    Seed,
    Comment,
    FileLoader,
    Connector,
    Output,
}

impl BrickKind {
    pub fn label(&self) -> &'static str {
        match self {
            BrickKind::Http       => "HTTP step",
            BrickKind::GraphQL    => "GraphQL step",
            BrickKind::Search     => "Search / Filter",
            BrickKind::Loop       => "Loop (pagination)",
            BrickKind::Poll       => "Poll (wait for condition)",
            BrickKind::Set        => "Set variables",
            BrickKind::Jq         => "JQ transform",
            BrickKind::Parallel   => "Parallel",
            BrickKind::Notify     => "Notify (webhook)",
            BrickKind::Transform  => "Transform",
            BrickKind::Pause      => "Pause",
            BrickKind::Seed       => "Seed",
            BrickKind::Comment    => "Comment",
            BrickKind::FileLoader => "File Loader",
            BrickKind::Connector  => "Connector [IN]",
            BrickKind::Output     => "Output [OUT]",
        }
    }
    pub fn description(&self) -> &'static str {
        match self {
            BrickKind::Http       => "HTTP request",
            BrickKind::GraphQL    => "GraphQL query (POST, body built from query + variables)",
            BrickKind::Search     => "filter a JSON array by regex on a field, store matches",
            BrickKind::Loop       => "repeat HTTP until condition, accumulate results (pagination)",
            BrickKind::Poll       => "poll an endpoint until a condition is true (async job polling)",
            BrickKind::Set        => "assign literal values to variables ({{VAR}} supported in values)",
            BrickKind::Jq         => "apply a jq expression to a JSON variable (requires jq installed)",
            BrickKind::Parallel   => "run multiple named steps concurrently, wait for all to complete",
            BrickKind::Notify     => "POST a message to a webhook URL (Slack, Discord, Teams, custom)",
            BrickKind::Transform  => "variable transform",
            BrickKind::Pause      => "wait (ms)",
            BrickKind::Seed       => "seed connector (inline)",
            BrickKind::Comment    => "text note / separator",
            BrickKind::FileLoader => "read file → variable (base64/text/hex)",
            BrickKind::Connector  => "CSV / JSON input data source [[connectors]]",
            BrickKind::Output     => "collect step responses to JSON [[outputs]]",
        }
    }
}

pub const BRICK_KINDS: &[BrickKind] = &[
    BrickKind::Http,
    BrickKind::GraphQL,
    BrickKind::Loop,
    BrickKind::Poll,
    BrickKind::Parallel,
    BrickKind::Search,
    BrickKind::Set,
    BrickKind::Jq,
    BrickKind::Notify,
    BrickKind::Transform,
    BrickKind::Pause,
    BrickKind::Seed,
    BrickKind::Comment,
    BrickKind::FileLoader,
    BrickKind::Connector,
    BrickKind::Output,
];

// ── Checker ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum CheckLevel {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub level: CheckLevel,
    pub step_idx: Option<usize>,
    pub message: String,
}
