use std::collections::HashSet;

use crate::campaign::Step;
use super::BuilderApp;
use super::types::{CheckLevel, CheckResult};

pub fn run(app: &BuilderApp) -> Vec<CheckResult> {
    let mut results = Vec::new();

    // Build a set of all step names for from_step reference checks
    let step_names: HashSet<&str> = app.campaign.steps.iter()
        .map(|s| s.name.as_str())
        .collect();

    // ── Step name uniqueness ───────────────────────────────────────────────────
    let mut seen_names: HashSet<&str> = HashSet::new();
    for (idx, step) in app.campaign.steps.iter().enumerate() {
        if step.name.is_empty() {
            results.push(CheckResult {
                level: CheckLevel::Warning,
                step_idx: Some(idx),
                message: format!("[{}] step name is empty", idx + 1),
            });
        } else if !seen_names.insert(step.name.as_str()) {
            results.push(CheckResult {
                level: CheckLevel::Warning,
                step_idx: Some(idx),
                message: format!("[{}] duplicate step name \"{}\"", idx + 1, step.name),
            });
        }
    }

    // ── Per-step variable resolution + field checks ────────────────────────────
    let mut defined: HashSet<String> = app.campaign.env.keys().cloned().collect();
    for param in &app.campaign.params {
        defined.insert(param.name.clone());
    }

    for (idx, step) in app.campaign.steps.iter().enumerate() {
        check_step_vars(idx, step, &defined, &mut results);
        check_step_fields(idx, step, &mut results);

        // Vars extracted by this step are available to subsequent steps
        for var in step.extract.keys() {
            defined.insert(var.clone());
        }
    }

    // ── Output `from_step` validation ─────────────────────────────────────────
    for (i, output) in app.campaign.outputs.iter().enumerate() {
        if output.from_step.is_empty() {
            results.push(CheckResult {
                level: CheckLevel::Error,
                step_idx: None,
                message: format!("Output [{}]: from_step is empty", i + 1),
            });
        } else if !step_names.contains(output.from_step.as_str()) {
            results.push(CheckResult {
                level: CheckLevel::Error,
                step_idx: None,
                message: format!(
                    "Output [{}]: from_step \"{}\" does not match any step name",
                    i + 1, output.from_step
                ),
            });
        }
        if output.path.is_empty() {
            results.push(CheckResult {
                level: CheckLevel::Warning,
                step_idx: None,
                message: format!("Output [{}]: path is empty", i + 1),
            });
        }
    }

    // ── Connector `from_step` / path validation ────────────────────────────────
    for (i, connector) in app.campaign.connectors.iter().enumerate() {
        if let Some(ref from_step) = connector.from_step {
            if !from_step.is_empty() && !step_names.contains(from_step.as_str()) {
                results.push(CheckResult {
                    level: CheckLevel::Error,
                    step_idx: None,
                    message: format!(
                        "Connector [{}]: from_step \"{}\" does not match any step name",
                        i + 1, from_step
                    ),
                });
            }
        } else if connector.path.is_empty() {
            results.push(CheckResult {
                level: CheckLevel::Warning,
                step_idx: None,
                message: format!("Connector [{}]: path is empty (and no from_step set)", i + 1),
            });
        }
    }

    if results.is_empty() {
        results.push(CheckResult {
            level: CheckLevel::Ok,
            step_idx: None,
            message: "Pipeline OK — all variables resolved, all references valid".into(),
        });
    }

    results
}

// ── Per-step field checks ─────────────────────────────────────────────────────

fn check_step_fields(idx: usize, step: &Step, results: &mut Vec<CheckResult>) {
    match step.kind.as_str() {
        "file" => {
            if step.file_path.as_deref().unwrap_or("").trim().is_empty() {
                results.push(CheckResult {
                    level: CheckLevel::Error,
                    step_idx: Some(idx),
                    message: format!("[{}] File Loader: file_path is empty", idx + 1),
                });
            }
        }
        "transform" => {
            if step.transforms.is_empty() {
                results.push(CheckResult {
                    level: CheckLevel::Warning,
                    step_idx: Some(idx),
                    message: format!("[{}] Transform: no transforms defined", idx + 1),
                });
            }
        }
        "comment" | "pause" | "seed" => {}
        _ => {
            // HTTP step
            if step.url.trim().is_empty() {
                results.push(CheckResult {
                    level: CheckLevel::Warning,
                    step_idx: Some(idx),
                    message: format!("[{}] HTTP step: URL is empty", idx + 1),
                });
            }
            // Multipart parts with empty name
            for (pi, part) in step.multipart_parts.iter().enumerate() {
                if part.name.trim().is_empty() {
                    results.push(CheckResult {
                        level: CheckLevel::Warning,
                        step_idx: Some(idx),
                        message: format!("[{}] multipart part {}: name is empty", idx + 1, pi + 1),
                    });
                }
            }
        }
    }
}

// ── Variable reference extraction ─────────────────────────────────────────────

fn check_step_vars(
    idx: usize,
    step: &Step,
    defined: &HashSet<String>,
    results: &mut Vec<CheckResult>,
) {
    let mut refs: Vec<String> = Vec::new();
    collect_vars(&step.url, &mut refs);
    if let Some(body) = &step.body {
        collect_vars(body, &mut refs);
    }
    for v in step.headers.values() {
        collect_vars(v, &mut refs);
    }
    if let Some(foreach) = &step.foreach {
        collect_vars(foreach, &mut refs);
    }
    if let Some(when) = &step.when {
        if let Some(eq) = &when.eq { collect_vars(eq, &mut refs); }
        if let Some(ne) = &when.ne { collect_vars(ne, &mut refs); }
        refs.push(when.var.clone());
    }
    for part in &step.multipart_parts {
        collect_vars(&part.value, &mut refs);
    }

    for var in refs {
        if !defined.contains(&var) {
            results.push(CheckResult {
                level: CheckLevel::Error,
                step_idx: Some(idx),
                message: format!("[{}] {{{{{}}}}} not defined by any upstream step", idx + 1, var),
            });
        }
    }

    // Warn on foreach pointing to an undefined var
    if let Some(foreach) = &step.foreach {
        let var = foreach.trim_start_matches("{{").trim_end_matches("}}");
        if !defined.contains(var) {
            results.push(CheckResult {
                level: CheckLevel::Warning,
                step_idx: Some(idx),
                message: format!("[{}] foreach: {{{{{}}}}} not extracted by a preceding step", idx + 1, var),
            });
        }
    }
}

fn collect_vars(text: &str, out: &mut Vec<String>) {
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        rest = &rest[start + 2..];
        if let Some(end) = rest.find("}}") {
            let var = &rest[..end];
            if !var.is_empty() {
                out.push(var.to_string());
            }
            rest = &rest[end + 2..];
        } else {
            break;
        }
    }
}
