use egui::{Button, ComboBox, Image, ImageSource, Vec2, include_image};

use crate::action::{Action, Enabled};
use crate::shortcuts;
use crate::tool::Tool;
use crate::zoom;

const NEW_ICON: ImageSource<'static> = include_image!("../assets/icons/new.svg");
const OPEN_ICON: ImageSource<'static> = include_image!("../assets/icons/open.svg");
const SAVE_ICON: ImageSource<'static> = include_image!("../assets/icons/save.svg");
const CUT_ICON: ImageSource<'static> = include_image!("../assets/icons/cut.svg");
const COPY_ICON: ImageSource<'static> = include_image!("../assets/icons/copy.svg");
const PASTE_ICON: ImageSource<'static> = include_image!("../assets/icons/paste.svg");
const UNDO_ICON: ImageSource<'static> = include_image!("../assets/icons/undo.svg");
const REDO_ICON: ImageSource<'static> = include_image!("../assets/icons/redo.svg");
const ZOOM_IN_ICON: ImageSource<'static> = include_image!("../assets/icons/zoom_in.svg");
const ZOOM_OUT_ICON: ImageSource<'static> = include_image!("../assets/icons/zoom_out.svg");
const DELETE_ICON: ImageSource<'static> = include_image!("../assets/icons/delete.svg");

#[rustfmt::skip]
const TOOLS: [(Tool, &str, ImageSource<'static>); 7] = [
    (Tool::Cursor,  "Cursor",   include_image!("../assets/icons/cursor.svg")),
    (Tool::Box,     "Box",      include_image!("../assets/icons/box.svg")),
    (Tool::Circle,  "Circle",   include_image!("../assets/icons/circle.svg")),
    (Tool::Ellipse, "Ellipse",  include_image!("../assets/icons/ellipse.svg")),
    (Tool::Line,    "Line",     include_image!("../assets/icons/line.svg")),
    (Tool::Arrow,   "Arrow",    include_image!("../assets/icons/arrow.svg")),
    (Tool::TextBox, "Text Box", include_image!("../assets/icons/text_box.svg")),
];

const ICON_SIZE: Vec2 = Vec2::splat(20.0);
/// egui pads buttons wider than tall; even padding keeps them square.
const BUTTON_PADDING: Vec2 = Vec2::splat(4.0);
/// Wide enough for "400%", so the zoom dropdown keeps its width.
const ZOOM_BOX_WIDTH: f32 = 64.0;

/// Shows the New, Open and Save buttons, the Cut, Copy and Paste buttons, the
/// Undo and Redo buttons, each as `enabled` says, a button per tool with the
/// `selected` one highlighted, then the zoom buttons and a dropdown showing
/// `zoom`. Returns the action clicked.
pub fn show(ui: &mut egui::Ui, enabled: Enabled, selected: Tool, zoom: f32) -> Option<Action> {
    let toolbar = egui::Panel::top("toolbar").show(ui, |ui| {
        button_row(ui, |ui| {
            // So the zoom dropdown is as tall as the icon buttons.
            ui.spacing_mut().interact_size.y = ICON_SIZE.y + 2.0 * BUTTON_PADDING.y;

            let file = action_buttons(
                ui,
                [
                    (Action::New, "New", NEW_ICON, enabled.file),
                    (Action::Open, "Open", OPEN_ICON, enabled.file),
                    (Action::Save, "Save", SAVE_ICON, enabled.file),
                ],
            );

            ui.separator();

            let clipboard = action_buttons(
                ui,
                [
                    (Action::Cut, "Cut", CUT_ICON, enabled.copy),
                    (Action::Copy, "Copy", COPY_ICON, enabled.copy),
                    (Action::Paste, "Paste", PASTE_ICON, true),
                ],
            );

            ui.separator();

            let history = action_buttons(
                ui,
                [
                    (Action::Undo, "Undo", UNDO_ICON, enabled.undo),
                    (Action::Redo, "Redo", REDO_ICON, enabled.redo),
                ],
            );

            ui.separator();

            let mut picked = None;
            for (tool, name, icon) in TOOLS {
                let action = Action::SelectTool(tool);
                let button = icon_button(icon, name).selected(selected == tool);
                if add(ui, button, true, name, &action) {
                    picked = Some(action);
                }
            }

            ui.separator();

            let can_zoom_in = zoom::step_in(zoom).is_some();
            let can_zoom_out = zoom::step_out(zoom).is_some();
            let zoom_step = action_buttons(
                ui,
                [
                    (Action::ZoomIn, "Zoom In", ZOOM_IN_ICON, can_zoom_in),
                    (Action::ZoomOut, "Zoom Out", ZOOM_OUT_ICON, can_zoom_out),
                ],
            );
            let preset = zoom_box(ui, zoom).map(Action::SetZoom);
            (file.or(clipboard).or(history))
                .or(picked)
                .or(zoom_step)
                .or(preset)
        })
    });
    toolbar.inner
}

/// Adds an icon button per action, enabled or not. Returns the action clicked.
fn action_buttons<const N: usize>(
    ui: &mut egui::Ui,
    buttons: [(Action, &str, ImageSource<'static>, bool); N],
) -> Option<Action> {
    let mut clicked = None;
    for (action, name, icon, enabled) in buttons {
        if add(ui, icon_button(icon, name), enabled, name, &action) {
            clicked = Some(action);
        }
    }
    clicked
}

/// Adds the side panel's toolbar, whose Delete button is enabled if a node or
/// edge is `selected`. Returns the action clicked.
pub fn show_for_selection(ui: &mut egui::Ui, selected: bool) -> Option<Action> {
    button_row(ui, |ui| {
        let action = Action::Delete;
        let button = icon_button(DELETE_ICON, "Delete");
        add(ui, button, selected, "Delete", &action).then_some(action)
    })
}

/// Lays out icon buttons, added by `add_buttons`, side by side.
fn button_row<R>(ui: &mut egui::Ui, add_buttons: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.spacing_mut().button_padding = BUTTON_PADDING;
        add_buttons(ui)
    })
    .inner
}

/// Adds a dropdown that shows `zoom` and offers the presets. Returns the
/// preset picked.
fn zoom_box(ui: &mut egui::Ui, zoom: f32) -> Option<f32> {
    let shown = zoom::text(zoom);
    let mut picked = None;
    ComboBox::from_id_salt("zoom")
        .selected_text(shown.clone())
        .width(ZOOM_BOX_WIDTH)
        .show_ui(ui, |ui| {
            for preset in zoom::PRESETS {
                let text = zoom::text(preset);
                if ui.selectable_label(text == shown, text).clicked() {
                    picked = Some(preset);
                }
            }
        })
        .response
        .on_hover_text("Zoom");
    picked
}

fn icon_button(icon: ImageSource<'static>, name: &str) -> Button<'static> {
    let icon = Image::new(icon).fit_to_exact_size(ICON_SIZE).alt_text(name);
    // The icons are white, so tinting them makes them follow the theme.
    Button::new(icon).image_tint_follows_text_color(true)
}

/// Adds `button` with a tooltip giving its name and `action`'s shortcut,
/// returning whether it was clicked.
fn add(ui: &mut egui::Ui, button: Button, enabled: bool, name: &str, action: &Action) -> bool {
    let tooltip = match shortcuts::text(ui.ctx(), action) {
        Some(shortcut) => format!("{name} ({shortcut})"),
        None => name.to_owned(),
    };
    ui.add_enabled(enabled, button)
        .on_hover_text(tooltip)
        .clicked()
}
