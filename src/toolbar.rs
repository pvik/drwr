use egui::{Button, Image, ImageSource, Vec2, include_image};

use crate::open_dialog::OpenDialog;
use crate::tool::Tool;

const NEW_ICON: ImageSource<'static> = include_image!("../assets/icons/new.svg");
const OPEN_ICON: ImageSource<'static> = include_image!("../assets/icons/open.svg");
const SAVE_ICON: ImageSource<'static> = include_image!("../assets/icons/save.svg");

#[rustfmt::skip]
const TOOLS: [(Tool, &str, ImageSource<'static>); 5] = [
    (Tool::Cursor,  "Cursor",   include_image!("../assets/icons/cursor.svg")),
    (Tool::Box,     "Box",      include_image!("../assets/icons/box.svg")),
    (Tool::Circle,  "Circle",   include_image!("../assets/icons/circle.svg")),
    (Tool::Ellipse, "Ellipse",  include_image!("../assets/icons/ellipse.svg")),
    (Tool::TextBox, "Text Box", include_image!("../assets/icons/text_box.svg")),
];

const ICON_SIZE: Vec2 = Vec2::splat(20.0);

/// Shows the New, Open and Save buttons, then a button per tool; clicking a
/// tool's button makes it the `selected` tool.
pub fn show(
    ui: &mut egui::Ui,
    frame: &eframe::Frame,
    open_dialog: &mut OpenDialog,
    selected: &mut Tool,
) {
    egui::Panel::top("toolbar").show(ui, |ui| {
        ui.horizontal(|ui| {
            // egui pads buttons wider than tall; even padding keeps them square.
            ui.spacing_mut().button_padding = Vec2::splat(4.0);

            let new = ui.add(icon_button(NEW_ICON, "New"));
            if new.on_hover_text("New").clicked() {
                // TODO: replace the graph with an empty, unsaved one.
            }
            let open = ui.add_enabled(!open_dialog.is_open(), icon_button(OPEN_ICON, "Open"));
            if open.on_hover_text("Open").clicked() {
                open_dialog.open(ui.ctx(), frame);
            }
            let save = ui.add(icon_button(SAVE_ICON, "Save"));
            if save.on_hover_text("Save").clicked() {
                // TODO: save the graph, asking for a file if it's unsaved.
            }

            ui.separator();

            for (tool, name, icon) in TOOLS {
                let button = icon_button(icon, name).selected(*selected == tool);
                if ui.add(button).on_hover_text(name).clicked() {
                    *selected = tool;
                }
            }
        });
    });
}

fn icon_button(icon: ImageSource<'static>, name: &str) -> Button<'static> {
    let icon = Image::new(icon).fit_to_exact_size(ICON_SIZE).alt_text(name);
    // The icons are white, so tinting them makes them follow the theme.
    Button::new(icon).image_tint_follows_text_color(true)
}
