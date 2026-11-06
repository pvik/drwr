use egui::{Color32, Sense, StrokeKind};

/// White like paper, even in the dark theme.
const BACKGROUND: Color32 = Color32::WHITE;

/// Shows the drawing canvas in the space left over by the other panels.
pub fn show(ui: &mut egui::Ui) {
    egui::CentralPanel::default().show(ui, |ui| {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::hover());

        // Outlined, since the light theme's panels are nearly white too.
        let border = ui.visuals().widgets.noninteractive.bg_stroke;
        painter.rect(response.rect, 0.0, BACKGROUND, border, StrokeKind::Inside);
    });
}
