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
    },
    CollectionBrowser { for_step: usize, col_cursor: usize, expanded: HashSet<String> },
    Variables { cursor: usize },
    Checker { results: Vec<CheckResult> },
    TomlPreview { scroll: usize },
}

// ── Step editor ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum PairTarget {
    Headers,
    Extract,
}

#[derive(Debug, Clone)]
pub enum StepEditorMode {
    Browse,
    EditText { buffer: String },
    AddPairStage1 { target: PairTarget, buffer: String },
    AddPairStage2 { target: PairTarget, key: String, buffer: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum StepSection {
    Name,
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
    LoadFromCollection,
}

impl StepSection {
    pub fn label(&self) -> &'static str {
        match self {
            StepSection::Name               => "Name",
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
            StepSection::LoadFromCollection => "[L] Load from collection",
        }
    }

    pub fn is_list(&self) -> bool {
        matches!(self, StepSection::Headers | StepSection::Extract | StepSection::Assertions)
    }
}

// ── Catalog ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum BrickKind {
    Http,
    Transform,
    Pause,
    Seed,
}

impl BrickKind {
    pub fn label(&self) -> &'static str {
        match self {
            BrickKind::Http      => "HTTP step",
            BrickKind::Transform => "Transform",
            BrickKind::Pause     => "Pause",
            BrickKind::Seed      => "Seed",
        }
    }
    pub fn description(&self) -> &'static str {
        match self {
            BrickKind::Http      => "requête HTTP",
            BrickKind::Transform => "manipulation var",
            BrickKind::Pause     => "attente (ms)",
            BrickKind::Seed      => "amorce connector",
        }
    }
}

pub const BRICK_KINDS: &[BrickKind] = &[
    BrickKind::Http,
    BrickKind::Transform,
    BrickKind::Pause,
    BrickKind::Seed,
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
