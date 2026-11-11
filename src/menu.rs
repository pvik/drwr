use std::path::{Path, PathBuf};

use directories::BaseDirs;
use egui::{Atom, Atoms, Button, Checkbox, Popup, RichText};

use crate::action::{Action, Enabled};
use crate::canvas;
use crate::shortcuts;

/// Shows the menu bar at the top of the window, its items `enabled` or not,
/// the canvas `options` ticked and the `recent_files` listed. Returns the
/// action clicked.
pub fn show(
    ui: &mut egui::Ui,
    enabled: Enabled,
    options: canvas::Options,
    recent_files: &[PathBuf],
) -> Option<Action> {
    let mut clicked = None;
    egui::Panel::top("menu_bar").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            let file = ui.menu_button("File", |ui| {
                let open = items(
                    ui,
                    [
                        ("New", Action::New, enabled.file),
                        ("Open…", Action::Open, enabled.file),
                    ],
                );
                let recent = recent_files_menu(ui, recent_files, enabled.file);
                let save = items(
                    ui,
                    [
                        ("Save", Action::Save, enabled.file),
                        ("Save As…", Action::SaveAs, enabled.file),
                    ],
                );
                ui.separator();
                let export = items(ui, [("Export…", Action::Export, enabled.file)]);
                ui.separator();
                let app = items(
                    ui,
                    [
                        ("About drwr", Action::About, true),
                        ("Quit", Action::Quit, true),
                    ],
                );
                clicked = open.or(recent).or(save).or(export).or(app);
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
                let (grid, snap) = (options.grid_lines, options.snap_lines);
                let options = check_items(
                    ui,
                    [
                        ("Canvas Grid Lines", Action::ToggleGridLines, grid),
                        ("Snap Lines", Action::ToggleSnapLines, snap),
                    ],
                );
                clicked = zoom.or(options);
            });
        });
    });
    clicked
}

/// Adds the Recent Files submenu, listing `files` by name, each with its
/// folder; disabled without any, or unless `enabled`. Returns the action
/// clicked.
fn recent_files_menu(ui: &mut egui::Ui, files: &[PathBuf], enabled: bool) -> Option<Action> {
    let menu = ui.add_enabled_ui(enabled && !files.is_empty(), |ui| {
        ui.menu_button("Recent Files", |ui| {
            let dirs = BaseDirs::new();
            let home = dirs.as_ref().map(BaseDirs::home_dir);
            let mut clicked = None;
            for path in files {
                let (name, folder) = describe(path, home);
                let response = ui.add(Button::new(name).shortcut_text(folder));
                if picked(ui, &response) {
                    clicked = Some(Action::OpenRecent(path.clone()));
                }
            }
            clicked
        })
    });
    menu.inner.inner.flatten()
}

/// The name of the file at `path`, and its folder, with `~` for `home` where
/// that's the custom.
pub fn describe(path: &Path, home: Option<&Path>) -> (String, String) {
    let name = path.file_name().unwrap_or(path.as_os_str());
    let folder = path.parent().unwrap_or(path);
    let folder = match home.and_then(|home| folder.strip_prefix(home).ok()) {
        Some(below) if cfg!(unix) && below.as_os_str().is_empty() => "~".to_owned(),
        Some(below) if cfg!(unix) => format!("~/{}", below.display()),
        _ => folder.display().to_string(),
    };
    (name.to_string_lossy().into_owned(), folder)
}

/// Adds an item per action, showing its shortcut. Returns the action clicked.
fn items<const N: usize>(ui: &mut egui::Ui, items: [(&str, Action, bool); N]) -> Option<Action> {
    let mut clicked = None;
    for (name, action, enabled) in items {
        let mut button = Button::new(name);
        if let Some(shortcut) = shortcuts::text(ui.ctx(), &action) {
            button = button.shortcut_text(shortcut);
        }
        let response = ui.add_enabled(enabled, button);
        if picked(ui, &response) {
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
        let response = ui.add(Checkbox::new(&mut ticked, text));
        if picked(ui, &response) {
            clicked = Some(action);
        }
    }
    clicked
}

/// Whether the menu item of `response` was clicked, closing the menu if so,
/// as egui does only for clicks of the pointer, not Enter.
fn picked(ui: &egui::Ui, response: &egui::Response) -> bool {
    let clicked = response.clicked();
    if clicked {
        ui.close();
    }
    clicked
}
