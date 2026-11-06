use egui::{Button, Image, ImageSource, Vec2, include_image};

use crate::tool::Tool;

// The icons are white line drawings on a 24×24 grid. Being white lets the
// buttons tint them to their text color, so they follow the theme and the
// hover and selected states.
const CURSOR_ICON: ImageSource<'static> = include_image!("../assets/icons/cursor.svg");
const BOX_ICON: ImageSource<'static> = include_image!("../assets/icons/box.svg");
const CIRCLE_ICON: ImageSource<'static> = include_image!("../assets/icons/circle.svg");
const ELLIPSE_ICON: ImageSource<'static> = include_image!("../assets/icons/ellipse.svg");
const TEXT_BOX_ICON: ImageSource<'static> = include_image!("../assets/icons/text_box.svg");

/// The size the icons are drawn at, in points.
const ICON_SIZE: f32 = 20.0;

/// Shows the toolbar: a row of buttons below the menu bar, one per tool.
/// Clicking a button makes its tool the `selected` one, and the selected
/// tool's button is shown highlighted.
///
/// egui stacks top panels in the order they are added, so call this after
/// [`crate::menu::show`], and before the [`egui::CentralPanel`].
///
/// The icons are SVGs, which only show up once
/// [`egui_extras::install_image_loaders`] has been called.
pub fn show(ui: &mut egui::Ui, selected: &mut Tool) {
    egui::Panel::top("toolbar").show(ui, |ui| {
        ui.horizontal(|ui| {
            // egui pads buttons more horizontally than vertically, which suits
            // text; even padding makes the icon buttons square.
            ui.spacing_mut().button_padding = Vec2::splat(4.0);

            tool_button(ui, selected, Tool::Cursor, CURSOR_ICON, "Cursor");
            tool_button(ui, selected, Tool::Box, BOX_ICON, "Box");
            tool_button(ui, selected, Tool::Circle, CIRCLE_ICON, "Circle");
            tool_button(ui, selected, Tool::Ellipse, ELLIPSE_ICON, "Ellipse");
            tool_button(ui, selected, Tool::TextBox, TEXT_BOX_ICON, "Text Box");
        });
    });
}

/// Adds an icon-only button for `tool`, highlighted while `tool` is the
/// `selected` one. Like a radio button, clicking it selects `tool`, and
/// clicking it again changes nothing.
///
/// `name` is shown as a tooltip, and is the label screen readers announce.
fn tool_button(
    ui: &mut egui::Ui,
    selected: &mut Tool,
    tool: Tool,
    icon: ImageSource<'_>,
    name: &str,
) {
    let icon = Image::new(icon)
        .fit_to_exact_size(Vec2::splat(ICON_SIZE))
        .alt_text(name);
    let button = Button::new(icon)
        .image_tint_follows_text_color(true)
        // Besides the highlight, this tells screen readers whether the button
        // is pressed.
        .selected(*selected == tool);
    if ui.add(button).on_hover_text(name).clicked() {
        *selected = tool;
    }
}
