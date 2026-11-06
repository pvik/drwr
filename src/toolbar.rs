use egui::{Button, Image, ImageSource, Vec2, include_image};

use crate::tool::Tool;

/// The icons are white so the buttons can tint them to follow the theme.
#[rustfmt::skip]
const TOOLS: [(Tool, &str, ImageSource<'static>); 5] = [
    (Tool::Cursor,  "Cursor",   include_image!("../assets/icons/cursor.svg")),
    (Tool::Box,     "Box",      include_image!("../assets/icons/box.svg")),
    (Tool::Circle,  "Circle",   include_image!("../assets/icons/circle.svg")),
    (Tool::Ellipse, "Ellipse",  include_image!("../assets/icons/ellipse.svg")),
    (Tool::TextBox, "Text Box", include_image!("../assets/icons/text_box.svg")),
];

const ICON_SIZE: Vec2 = Vec2::splat(20.0);

/// Shows a button per tool; clicking one makes it the `selected` tool.
pub fn show(ui: &mut egui::Ui, selected: &mut Tool) {
    egui::Panel::top("toolbar").show(ui, |ui| {
        ui.horizontal(|ui| {
            // egui pads buttons wider than tall; even padding keeps them square.
            ui.spacing_mut().button_padding = Vec2::splat(4.0);

            for (tool, name, icon) in TOOLS {
                let icon = Image::new(icon).fit_to_exact_size(ICON_SIZE).alt_text(name);
                let button = Button::new(icon)
                    .image_tint_follows_text_color(true)
                    .selected(*selected == tool);
                if ui.add(button).on_hover_text(name).clicked() {
                    *selected = tool;
                }
            }
        });
    });
}
