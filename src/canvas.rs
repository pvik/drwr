use egui::{Color32, Sense, StrokeKind};

/// The canvas stands in for a sheet of paper, so unlike the rest of the UI it
/// stays white in both the light and the dark theme.
const BACKGROUND: Color32 = Color32::WHITE;

/// Shows the drawing canvas, filling the space left over by the other panels.
///
/// The canvas lives in the [`egui::CentralPanel`], which must be added after
/// all other panels, so call this last.
pub fn show(ui: &mut egui::Ui) {
    egui::CentralPanel::default().show(ui, |ui| {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::hover());

        // The light theme's panel background is nearly white too, so outline
        // the canvas to keep its edges visible.
        let border = ui.visuals().widgets.noninteractive.bg_stroke;
        painter.rect(response.rect, 0.0, BACKGROUND, border, StrokeKind::Inside);
    });
}
