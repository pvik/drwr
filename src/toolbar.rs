use egui::{Button, ComboBox, Image, ImageSource, Vec2, include_image};

use crate::action::Action;
use crate::open_dialog::OpenDialog;
use crate::shortcuts;
use crate::tool::Tool;
use crate::zoom;

const NEW_ICON: ImageSource<'static> = include_image!("../assets/icons/new.svg");
const OPEN_ICON: ImageSource<'static> = include_image!("../assets/icons/open.svg");
const SAVE_ICON: ImageSource<'static> = include_image!("../assets/icons/save.svg");
const ZOOM_IN_ICON: ImageSource<'static> = include_image!("../assets/icons/zoom_in.svg");
const ZOOM_OUT_ICON: ImageSource<'static> = include_image!("../assets/icons/zoom_out.svg");

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
/// Wide enough for "400%", so the zoom dropdown keeps its width.
const ZOOM_BOX_WIDTH: f32 = 64.0;

/// Shows the New, Open and Save buttons, a button per tool with the
/// `selected` one highlighted, then the zoom buttons and a dropdown showing
/// `zoom`. Returns the action clicked.
pub fn show(
    ui: &mut egui::Ui,
    open_dialog: &OpenDialog,
    selected: Tool,
    zoom: f32,
) -> Option<Action> {
    let mut clicked = None;
    egui::Panel::top("toolbar").show(ui, |ui| {
        ui.horizontal(|ui| {
            // egui pads buttons wider than tall; even padding keeps them square.
            let padding = Vec2::splat(4.0);
            ui.spacing_mut().button_padding = padding;
            // So the zoom dropdown is as tall as the icon buttons.
            ui.spacing_mut().interact_size.y = ICON_SIZE.y + 2.0 * padding.y;

            for (action, name, icon, enabled) in [
                (Action::New, "New", NEW_ICON, true),
                (Action::Open, "Open", OPEN_ICON, !open_dialog.is_open()),
                (Action::Save, "Save", SAVE_ICON, true),
            ] {
                if add(ui, icon_button(icon, name), enabled, name, &action) {
                    clicked = Some(action);
                }
            }

            ui.separator();

            for (tool, name, icon) in TOOLS {
                let action = Action::SelectTool(tool);
                let button = icon_button(icon, name).selected(selected == tool);
                if add(ui, button, true, name, &action) {
                    clicked = Some(action);
                }
            }

            ui.separator();

            let can_zoom_in = zoom::step_in(zoom).is_some();
            let can_zoom_out = zoom::step_out(zoom).is_some();
            for (action, name, icon, enabled) in [
                (Action::ZoomIn, "Zoom In", ZOOM_IN_ICON, can_zoom_in),
                (Action::ZoomOut, "Zoom Out", ZOOM_OUT_ICON, can_zoom_out),
            ] {
                if add(ui, icon_button(icon, name), enabled, name, &action) {
                    clicked = Some(action);
                }
            }
            if let Some(zoom) = zoom_box(ui, zoom) {
                clicked = Some(Action::SetZoom(zoom));
            }
        });
    });
    clicked
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
