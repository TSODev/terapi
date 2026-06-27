use super::BuilderApp;

pub fn move_step_up(app: &mut BuilderApp) {
    if app.cursor > 0 {
        app.campaign.steps.swap(app.cursor, app.cursor - 1);
        if app.step_comments.len() > app.cursor {
            app.step_comments.swap(app.cursor, app.cursor - 1);
        }
        app.cursor -= 1;
        app.modified = true;
    }
}

pub fn move_step_down(app: &mut BuilderApp) {
    let len = app.campaign.steps.len();
    if len > 0 && app.cursor < len - 1 {
        app.campaign.steps.swap(app.cursor, app.cursor + 1);
        if app.step_comments.len() > app.cursor + 1 {
            app.step_comments.swap(app.cursor, app.cursor + 1);
        }
        app.cursor += 1;
        app.modified = true;
    }
}

pub fn duplicate_step(app: &mut BuilderApp) {
    if app.campaign.steps.is_empty() {
        return;
    }
    let mut copy = app.campaign.steps[app.cursor].clone();
    // Append " copy" to the name, or increment an existing copy suffix
    copy.name = if copy.name.ends_with(" copy") {
        format!("{} 2", copy.name)
    } else if let Some(base) = copy.name.strip_suffix(" copy 2") {
        format!("{} copy 3", base)
    } else {
        format!("{} copy", copy.name)
    };
    let comment = app.step_comments.get(app.cursor).cloned().unwrap_or_default();
    let insert_at = app.cursor + 1;
    app.campaign.steps.insert(insert_at, copy);
    if insert_at <= app.step_comments.len() {
        app.step_comments.insert(insert_at, comment);
    } else {
        app.step_comments.push(comment);
    }
    app.cursor = insert_at;
    app.modified = true;
}

pub fn delete_step(app: &mut BuilderApp) {
    if !app.campaign.steps.is_empty() {
        app.campaign.steps.remove(app.cursor);
        if app.cursor < app.step_comments.len() {
            app.step_comments.remove(app.cursor);
        }
        if app.cursor > 0 && app.cursor >= app.campaign.steps.len() {
            app.cursor -= 1;
        }
        app.modified = true;
    }
}
