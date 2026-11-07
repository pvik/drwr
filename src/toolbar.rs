use egui::{Button, Image, ImageSource, Vec2, include_image};

use crate::action::Action;
use crate::open_dialog::OpenDialog;
use crate::shortcuts;
use crate::tool::Tool;

const NEW_ICON: ImageSource<'static> = include_image!("../assets/icons/new.svg");
const OPEN_ICON: ImageSource<'static> = include_image!("../assets/icons/open.svg");
const SAVE_ICON: ImageSource<'static> = include_image!("../assets/icons/save.svg");

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

/// Shows the New, Open and Save buttons, then a button per tool with the
/// `selected` one highlighted. Returns the action clicked.
pub fn show(ui: &mut egui::Ui, open_dialog: &OpenDialog, selected: Tool) -> Option<Action> {
    let mut clicked = None;
    egui::Panel::top("toolbar").show(ui, |ui| {
        ui.horizontal(|ui| {
            // egui pads buttons wider than tall; even padding keeps them square.
            ui.spacing_mut().button_padding = Vec2::splat(4.0);

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
        });
    });
    clicked
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
