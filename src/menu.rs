use egui::{Button, Popup};

use crate::action::{Action, Enabled};
use crate::shortcuts;

/// Shows the menu bar at the top of the window, its items `enabled` or not.
/// Returns the action clicked.
pub fn show(ui: &mut egui::Ui, enabled: Enabled) -> Option<Action> {
    let mut clicked = None;
    egui::Panel::top("menu_bar").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            let file = ui.menu_button("File", |ui| {
                let file = items(
                    ui,
                    [
                        ("New", Action::New, enabled.file),
                        ("Open…", Action::Open, enabled.file),
                        ("Save", Action::Save, enabled.file),
                        ("Save As…", Action::SaveAs, enabled.file),
                    ],
                );
                ui.separator();
                clicked = file.or(items(ui, [("Quit", Action::Quit, true)]));
            });
            if shortcuts::file_menu_pressed(ui.ctx()) {
                Popup::open_id(ui.ctx(), Popup::default_response_id(&file.response));
            }
            // Only one menu is open at a time, so only one sets `clicked`.
            ui.menu_button("Edit", |ui| {
                let history = items(
                    ui,
                    [
                        ("Undo", Action::Undo, enabled.undo),
                        ("Redo", Action::Redo, enabled.redo),
                    ],
                );
                ui.separator();
                let clipboard = items(
                    ui,
                    [
                        ("Cut", Action::Cut, enabled.copy),
                        ("Copy", Action::Copy, enabled.copy),
                        ("Paste", Action::Paste, true),
                    ],
                );
                clicked = history.or(clipboard);
            });
        });
    });
    clicked
}

/// Adds an item per action, showing its shortcut. Returns the action clicked.
fn items<const N: usize>(ui: &mut egui::Ui, items: [(&str, Action, bool); N]) -> Option<Action> {
    let mut clicked = None;
    for (name, action, enabled) in items {
        let mut button = Button::new(name);
        if let Some(shortcut) = shortcuts::text(ui.ctx(), &action) {
            button = button.shortcut_text(shortcut);
        }
        if ui.add_enabled(enabled, button).clicked() {
            clicked = Some(action);
        }
    }
    clicked
}
