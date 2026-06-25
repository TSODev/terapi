use super::BuilderApp;

pub fn move_step_up(app: &mut BuilderApp) {
    if app.cursor > 0 {
        app.campaign.steps.swap(app.cursor, app.cursor - 1);
        app.cursor -= 1;
        app.modified = true;
    }
}

pub fn move_step_down(app: &mut BuilderApp) {
    let len = app.campaign.steps.len();
    if len > 0 && app.cursor < len - 1 {
        app.campaign.steps.swap(app.cursor, app.cursor + 1);
        app.cursor += 1;
        app.modified = true;
    }
}

pub fn delete_step(app: &mut BuilderApp) {
    if !app.campaign.steps.is_empty() {
        app.campaign.steps.remove(app.cursor);
        if app.cursor > 0 && app.cursor >= app.campaign.steps.len() {
            app.cursor -= 1;
        }
        app.modified = true;
    }
}
