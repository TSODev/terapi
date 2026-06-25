use std::collections::HashSet;

#[derive(Debug, Clone)]
pub enum BuilderFocus {
    Pipeline,
    Catalog { insert_after: Option<usize>, cursor: usize },
    StepEditor { step_idx: usize, field_cursor: usize, editing: bool },
    CollectionBrowser { for_step: usize, col_cursor: usize, expanded: HashSet<String> },
    Variables { cursor: usize },
    Checker { results: Vec<CheckResult> },
    TomlPreview { scroll: usize },
}

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
