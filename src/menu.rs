use egui::{Atom, Atoms, Button, Checkbox, Popup, RichText};

use crate::action::{Action, Enabled};
use crate::canvas;
use crate::shortcuts;

/// Shows the menu bar at the top of the window, its items `enabled` or not,
/// and the canvas `options` ticked. Returns the action clicked.
pub fn show(ui: &mut egui::Ui, enabled: Enabled, options: canvas::Options) -> Option<Action> {
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
                let export = items(ui, [("Export…", Action::Export, enabled.file)]);
                ui.separator();
                let quit = items(ui, [("Quit", Action::Quit, true)]);
                clicked = file.or(export).or(quit);
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
            ui.menu_button("View", |ui| {
                let zoom = items(
                    ui,
                    [
                        ("Zoom In", Action::ZoomIn, enabled.zoom_in),
                        ("Zoom Out", Action::ZoomOut, enabled.zoom_out),
                        ("Zoom to 100%", Action::SetZoom(1.0), true),
                    ],
                );
                ui.separator();
                let options = check_items(
                    ui,
                    [(
                        "Canvas Grid Lines",
                        Action::ToggleGridLines,
                        options.grid_lines,
                    )],
                );
                clicked = zoom.or(options);
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

/// Adds a check box item per action, ticked or not, showing its shortcut.
/// Returns the action clicked.
fn check_items<const N: usize>(
    ui: &mut egui::Ui,
    items: [(&str, Action, bool); N],
) -> Option<Action> {
    let mut clicked = None;
    for (name, action, mut ticked) in items {
        let mut text = Atoms::new(name);
        if let Some(shortcut) = shortcuts::text(ui.ctx(), &action) {
            // Right-aligned and weak, as a button's shortcut text is.
            text.push_right(Atom::grow());
            text.push_right(RichText::new(shortcut).weak());
        }
        if ui.add(Checkbox::new(&mut ticked, text)).clicked() {
            clicked = Some(action);
        }
    }
    clicked
}
