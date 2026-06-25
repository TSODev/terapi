use std::collections::HashSet;

use crate::campaign::Step;
use super::BuilderApp;
use super::types::{CheckLevel, CheckResult};

pub fn run(app: &BuilderApp) -> Vec<CheckResult> {
    let mut results = Vec::new();

    // Collect vars defined upstream at each step (env + extracted by previous steps)
    let mut defined: HashSet<String> = app.campaign.env.keys().cloned().collect();
    for param in &app.campaign.params {
        defined.insert(param.name.clone());
    }

    for (idx, step) in app.campaign.steps.iter().enumerate() {
        check_step_vars(idx, step, &defined, &mut results);

        // After this step, its extracted vars become available
        for var in step.extract.keys() {
            defined.insert(var.clone());
        }
    }

    if results.is_empty() {
        results.push(CheckResult {
            level: CheckLevel::Ok,
            step_idx: None,
            message: "Pipeline valide — toutes les variables sont résolues".into(),
        });
    }

    results
}

fn check_step_vars(
    idx: usize,
    step: &Step,
    defined: &HashSet<String>,
    results: &mut Vec<CheckResult>,
) {
    // Collect all {{VAR}} references in the step
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

    for var in refs {
        if !defined.contains(&var) {
            results.push(CheckResult {
                level: CheckLevel::Error,
                step_idx: Some(idx),
                message: format!("[{}] {{{{{}}}}} non définie en amont", idx + 1, var),
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
                message: format!("[{}] foreach: {{{{{}}}}} non extrait par un step précédent", idx + 1, var),
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
