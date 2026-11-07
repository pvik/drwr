use egui::{Button, Popup};

use crate::action::Action;
use crate::open_dialog::OpenDialog;
use crate::shortcuts;

/// Shows the menu bar at the top of the window, returning the action clicked.
pub fn show(ui: &mut egui::Ui, open_dialog: &OpenDialog) -> Option<Action> {
    let mut clicked = None;
    egui::Panel::top("menu_bar").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            let file = ui.menu_button("File", |ui| {
                for (name, action, enabled) in [
                    ("New", Action::New, true),
                    ("Open", Action::Open, !open_dialog.is_open()),
                    ("Save", Action::Save, true),
                ] {
                    if item(ui, name, &action, enabled) {
                        clicked = Some(action);
                    }
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ui.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            if shortcuts::file_menu_pressed(ui.ctx()) {
                Popup::open_id(ui.ctx(), Popup::default_response_id(&file.response));
            }
        });
    });
    clicked
}

/// Adds an item showing `action`'s shortcut, returning whether it was clicked.
fn item(ui: &mut egui::Ui, name: &str, action: &Action, enabled: bool) -> bool {
    let mut button = Button::new(name);
    if let Some(shortcut) = shortcuts::text(ui.ctx(), action) {
        button = button.shortcut_text(shortcut);
    }
    ui.add_enabled(enabled, button).clicked()
}
