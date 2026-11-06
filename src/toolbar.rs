use egui::{Button, Image, ImageSource, Response, Vec2, include_image};

// The icons are white line drawings on a 24×24 grid. Being white lets the
// buttons tint them to their text color, so they follow the theme and the
// hover state.
const BOX_ICON: ImageSource<'static> = include_image!("../assets/icons/box.svg");
const CIRCLE_ICON: ImageSource<'static> = include_image!("../assets/icons/circle.svg");
const ELLIPSE_ICON: ImageSource<'static> = include_image!("../assets/icons/ellipse.svg");
const TEXT_BOX_ICON: ImageSource<'static> = include_image!("../assets/icons/text_box.svg");

/// The size the icons are drawn at, in points.
const ICON_SIZE: f32 = 20.0;

/// Shows the toolbar: a row of drawing-tool buttons below the menu bar.
///
/// egui stacks top panels in the order they are added, so call this after
/// [`crate::menu::show`], and before the [`egui::CentralPanel`].
///
/// The icons are SVGs, which only show up once
/// [`egui_extras::install_image_loaders`] has been called.
pub fn show(ui: &mut egui::Ui) {
    egui::Panel::top("toolbar").show(ui, |ui| {
        ui.horizontal(|ui| {
            // egui pads buttons more horizontally than vertically, which suits
            // text; even padding makes the icon buttons square.
            ui.spacing_mut().button_padding = Vec2::splat(4.0);

            if tool_button(ui, BOX_ICON, "Box").clicked() {
                // TODO: select the box tool.
            }
            if tool_button(ui, CIRCLE_ICON, "Circle").clicked() {
                // TODO: select the circle tool.
            }
            if tool_button(ui, ELLIPSE_ICON, "Ellipse").clicked() {
                // TODO: select the ellipse tool.
            }
            if tool_button(ui, TEXT_BOX_ICON, "Text Box").clicked() {
                // TODO: select the text box tool.
            }
        });
    });
}

/// Adds an icon-only button for a tool. `name` is shown as a tooltip, and is
/// the label screen readers announce.
fn tool_button(ui: &mut egui::Ui, icon: ImageSource<'_>, name: &str) -> Response {
    let icon = Image::new(icon)
        .fit_to_exact_size(Vec2::splat(ICON_SIZE))
        .alt_text(name);
    ui.add(Button::new(icon).image_tint_follows_text_color(true))
        .on_hover_text(name)
}
