use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::collections::HashSet;

use super::types::{BuilderFocus, StepEditorMode};
use super::BuilderApp;
use crate::storage::StoredCollection;

// ── Flat tree node ────────────────────────────────────────────────────────────

pub struct BrowserNode {
    pub depth: usize,
    pub label: String,
    pub method: String,
    pub is_folder: bool,
    pub is_expanded: bool,
    pub addr: BrowserAddr,
}

pub enum BrowserAddr {
    Collection(usize),
    Folder(usize, usize),
    RootRequest { ci: usize, ri: usize },
    FolderRequest { ci: usize, fi: usize, ri: usize },
}

pub fn flatten(collections: &[StoredCollection], expanded: &HashSet<String>) -> Vec<BrowserNode> {
    let mut nodes = Vec::new();
    for (ci, col) in collections.iter().enumerate() {
        let col_exp = expanded.contains(&format!("c{ci}"));
        nodes.push(BrowserNode {
            depth: 0,
            label: col.collection.name.clone(),
            method: String::new(),
            is_folder: true,
            is_expanded: col_exp,
            addr: BrowserAddr::Collection(ci),
        });
        if !col_exp {
            continue;
        }
        for (fi, folder) in col.folders.iter().enumerate() {
            let fold_exp = expanded.contains(&format!("c{ci}f{fi}"));
            nodes.push(BrowserNode {
                depth: 1,
                label: folder.name.clone(),
                method: String::new(),
                is_folder: true,
                is_expanded: fold_exp,
                addr: BrowserAddr::Folder(ci, fi),
            });
            if !fold_exp {
                continue;
            }
            for (ri, req) in folder.requests.iter().enumerate() {
                nodes.push(BrowserNode {
                    depth: 2,
                    label: req.name.clone(),
                    method: req.method.clone(),
                    is_folder: false,
                    is_expanded: false,
                    addr: BrowserAddr::FolderRequest { ci, fi, ri },
                });
            }
        }
        for (ri, req) in col.requests.iter().enumerate() {
            nodes.push(BrowserNode {
                depth: 1,
                label: req.name.clone(),
                method: req.method.clone(),
                is_folder: false,
                is_expanded: false,
                addr: BrowserAddr::RootRequest { ci, ri },
            });
        }
    }
    nodes
}

// ── Key handling ──────────────────────────────────────────────────────────────

pub fn handle_key(
    app: &mut BuilderApp,
    key: KeyEvent,
    for_step: usize,
    col_cursor: usize,
    mut expanded: HashSet<String>,
) -> Result<()> {
    let nodes = flatten(&app.stored_collections, &expanded);
    let n = nodes.len();

    match key.code {
        KeyCode::Esc => {
            app.focus = BuilderFocus::StepEditor {
                step_idx: for_step,
                section_cursor: 0,
                sub_cursor: 0,
                mode: StepEditorMode::Browse,
            };
        }
        KeyCode::Up => {
            let new = col_cursor.saturating_sub(1);
            app.focus = BuilderFocus::CollectionBrowser { for_step, col_cursor: new, expanded };
        }
        KeyCode::Down => {
            let new = if n > 0 { (col_cursor + 1).min(n - 1) } else { 0 };
            app.focus = BuilderFocus::CollectionBrowser { for_step, col_cursor: new, expanded };
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            if let Some(node) = nodes.get(col_cursor) {
                if node.is_folder {
                    let key_str = match &node.addr {
                        BrowserAddr::Collection(ci) => format!("c{ci}"),
                        BrowserAddr::Folder(ci, fi) => format!("c{ci}f{fi}"),
                        _ => String::new(),
                    };
                    if !key_str.is_empty() {
                        if !expanded.remove(&key_str) {
                            expanded.insert(key_str);
                        }
                    }
                    app.focus = BuilderFocus::CollectionBrowser { for_step, col_cursor, expanded };
                } else {
                    load_into_step(app, for_step, &node.addr, &node.label);
                    app.focus = BuilderFocus::StepEditor {
                        step_idx: for_step,
                        section_cursor: 0,
                        sub_cursor: 0,
                        mode: StepEditorMode::Browse,
                    };
                }
            }
        }
        _ => {}
    }

    Ok(())
}

// ── Load request fields into campaign step ────────────────────────────────────

fn load_into_step(app: &mut BuilderApp, step_idx: usize, addr: &BrowserAddr, label: &str) {
    let req = match addr {
        BrowserAddr::Collection(_) | BrowserAddr::Folder(..) => return,
        BrowserAddr::RootRequest { ci, ri } => {
            app.stored_collections[*ci].requests[*ri].clone()
        }
        BrowserAddr::FolderRequest { ci, fi, ri } => {
            app.stored_collections[*ci].folders[*fi].requests[*ri].clone()
        }
    };
    {
        let step = &mut app.campaign.steps[step_idx];
        if !req.method.is_empty() {
            step.method = req.method;
        }
        if !req.url.is_empty() {
            step.url = req.url;
        }
        if !req.headers.is_empty() {
            step.headers = req.headers;
        }
        if req.body.is_some() {
            step.body = req.body;
        }
    }
    app.modified = true;
    app.status_message = format!("Requête \"{}\" chargée dans le step", label);
}
