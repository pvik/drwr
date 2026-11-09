use egui::{Button, Popup};

use crate::action::Action;
use crate::shortcuts;

/// Shows the menu bar at the top of the window, with New, Open, Save and Save
/// As disabled while `file_dialog_open`, and Cut and Copy enabled if
/// `can_copy`. Returns the action clicked.
pub fn show(ui: &mut egui::Ui, file_dialog_open: bool, can_copy: bool) -> Option<Action> {
    let mut clicked = None;
    egui::Panel::top("menu_bar").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            let file = ui.menu_button("File", |ui| {
                let can_pick = !file_dialog_open;
                clicked = items(
                    ui,
                    [
                        ("New", Action::New, can_pick),
                        ("Open", Action::Open, can_pick),
                        ("Save", Action::Save, can_pick),
                        ("Save As…", Action::SaveAs, can_pick),
                    ],
                );
                ui.separator();
                if ui.button("Quit").clicked() {
                    ui.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            if shortcuts::file_menu_pressed(ui.ctx()) {
                Popup::open_id(ui.ctx(), Popup::default_response_id(&file.response));
            }
            // Only one menu is open at a time, so only one sets `clicked`.
            ui.menu_button("Edit", |ui| {
                clicked = items(
                    ui,
                    [
                        ("Cut", Action::Cut, can_copy),
                        ("Copy", Action::Copy, can_copy),
                        ("Paste", Action::Paste, true),
                    ],
                );
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
