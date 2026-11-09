use std::ops::RangeInclusive;

use egui::{Align, Button, Context, DragValue, Layout, Rect, Response, Ui, vec2};

use crate::dialog;
use crate::export::{self, Format, Settings};

/// The PNG scales offered, in percent.
const SCALES: RangeInclusive<u32> = 10..=1000;
/// The margins offered, in points.
const MARGINS: RangeInclusive<u32> = 0..=1000;
/// So that the dialog keeps its width as the numbers change.
const MIN_WIDTH: f32 = 280.0;
/// Of the labels left of the choices.
const LABEL_WIDTH: f32 = 60.0;

/// A modal dialog asking how to export the drawing: its format, all of it or
/// what's selected, and the format's options.
#[derive(Default)]
pub struct ExportDialog {
    dialog: Option<Dialog>,
}

/// What the dialog was accepted with.
#[derive(Clone, Debug, PartialEq)]
pub struct Accepted {
    pub settings: Settings,
    /// Whether only what's selected is exported.
    pub selected_only: bool,
}

struct Dialog {
    accepted: Accepted,
    /// What the drawing covers, in graph points.
    everything: Rect,
    /// What the selected items cover, if any are selected.
    selected: Option<Rect>,
    /// Whether Export has yet to take the focus.
    focus: bool,
}

impl ExportDialog {
    pub fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    /// Opens the dialog on `settings`, with all of the drawing chosen, for a
    /// drawing covering `everything`, of which the selected items cover
    /// `selected`, unless none are. Unless it's open already.
    pub fn open(&mut self, settings: Settings, everything: Rect, selected: Option<Rect>) {
        if self.is_open() {
            return;
        }
        self.dialog = Some(Dialog {
            accepted: Accepted {
                settings,
                selected_only: false,
            },
            everything,
            selected,
            focus: true,
        });
    }

    pub fn close(&mut self) {
        self.dialog = None;
    }

    /// Shows the dialog while it's open. Returns what it's accepted with.
    pub fn show(&mut self, ctx: &Context) -> Option<Accepted> {
        let accepted = dialog::show(ctx, "export_dialog", &mut self.dialog, Dialog::ui)?;
        Some(accepted.accepted)
    }
}

impl Dialog {
    /// Returns whether Export was clicked.
    fn ui(&mut self, ui: &mut Ui) -> bool {
        ui.set_min_width(MIN_WIDTH);
        ui.strong("Export");
        let Accepted {
            settings,
            selected_only,
        } = &mut self.accepted;
        row(ui, "Format", |ui| {
            for format in [Format::Png, Format::Svg] {
                ui.radio_value(&mut settings.format, format, format.name());
            }
        });
        row(ui, "Area", |ui| {
            ui.vertical(|ui| {
                ui.radio_value(selected_only, false, "Entire document");
                ui.add_enabled_ui(self.selected.is_some(), |ui| {
                    ui.radio_value(selected_only, true, "Selected area only");
                });
            });
        });

        ui.separator();
        let png = settings.format == Format::Png;
        ui.strong(format!("{} options", settings.format.name()));
        let bounds = match self.selected {
            Some(selected) if *selected_only => selected,
            _ => self.everything,
        };
        let pixels =
            export::page(bounds, settings.margin).map(|page| export::pixels(page, settings.scale));
        if png {
            let (label, field) = row(ui, "Scale", |ui| {
                let field = ui.add(
                    DragValue::new(&mut settings.scale)
                        .range(SCALES)
                        .suffix("%"),
                );
                if let Some([width, height]) = pixels {
                    ui.weak(format!("{width} × {height} px"));
                }
                field
            });
            field.labelled_by(label.id);
        }
        let (label, field) = row(ui, "Margin", |ui| {
            ui.add(
                DragValue::new(&mut settings.margin)
                    .range(MARGINS)
                    .suffix(" pt"),
            )
        });
        field.labelled_by(label.id);
        row(ui, "", |ui| {
            ui.checkbox(&mut settings.transparent, "Transparent background");
        });
        if !png {
            row(ui, "", |ui| {
                ui.checkbox(&mut settings.outlines, "Text as outlines")
                    .on_hover_text(
                        "Draws text as shapes, which look the same without its fonts, \
                         but can't be edited as text",
                    );
            });
        }
        let problem = match pixels {
            None => Some("Nothing to export".to_owned()),
            Some(pixels) if png && export::too_large(pixels) => Some(format!(
                "Too large: the most is {} megapixels",
                export::MAX_PIXELS / 1_000_000
            )),
            Some(_) => None,
        };
        if let Some(problem) = &problem {
            ui.colored_label(ui.visuals().error_fg_color, problem);
        }

        ui.separator();
        ui.horizontal(|ui| {
            let export = ui.add_enabled(problem.is_none(), Button::new("Export…"));
            // So Enter exports.
            if std::mem::take(&mut self.focus) && problem.is_none() {
                export.request_focus();
            }
            if ui.button("Cancel").clicked() {
                ui.close();
            }
            export.clicked()
        })
        .inner
    }
}

/// Adds `label` in a column of its own, then what `add_contents` adds. Unlike
/// a grid, it's sized from the first frame, so the dialog doesn't move.
fn row<R>(ui: &mut Ui, label: &str, add_contents: impl FnOnce(&mut Ui) -> R) -> (Response, R) {
    ui.horizontal_top(|ui| {
        let size = vec2(LABEL_WIDTH, ui.spacing().interact_size.y);
        let layout = Layout::left_to_right(Align::Center);
        let label = ui.allocate_ui_with_layout(size, layout, |ui| {
            ui.set_min_width(LABEL_WIDTH);
            ui.label(label)
        });
        (label.inner, add_contents(ui))
    })
    .inner
}

#[cfg(test)]
mod tests;
