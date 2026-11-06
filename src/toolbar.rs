/// Shows the toolbar: a row of drawing-tool buttons below the menu bar.
///
/// egui stacks top panels in the order they are added, so call this after
/// [`crate::menu::show`], and before the [`egui::CentralPanel`].
pub fn show(ui: &mut egui::Ui) {
    egui::Panel::top("toolbar").show(ui, |ui| {
        ui.horizontal(|ui| {
            if ui.button("Box").clicked() {
                // TODO: select the box tool.
            }
            if ui.button("Circle").clicked() {
                // TODO: select the circle tool.
            }
            if ui.button("Ellipse").clicked() {
                // TODO: select the ellipse tool.
            }
            if ui.button("Text Box").clicked() {
                // TODO: select the text box tool.
            }
        });
    });
}
