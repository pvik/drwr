use std::ops::RangeInclusive;

use egui::{
    Align, Align2, Atom, Button, DragValue, Grid, Id, Key, Label, Layout, Response, Stroke,
    TextStyle, Ui, Vec2, emath,
};
use graphml::GraphMl;
use graphml::yed::{Color, NodeGraphics, NodeKind};

use crate::action::Action;
use crate::color_picker;
use crate::document::{self, ColorProperty, LabelRef};
use crate::label_field::{self, End};

/// Capped, so typing "inf" can't make a border infinite.
const BORDER_WIDTHS: RangeInclusive<f64> = 0.0..=100.0;

/// A resizable panel right of the canvas with the properties of the selected
/// node.
#[derive(Default)]
pub struct SidePanel {
    edit: Option<FieldEdit>,
    /// Whether Escape just cancelled the border width typed, which egui 0.37
    /// applies anyway a frame later.
    width_cancelled: bool,
}

/// A label being typed in its field, applied once the field loses the focus.
struct FieldEdit {
    field: Id,
    label: LabelRef,
    text: String,
}

impl SidePanel {
    /// Shows the properties of the node whose id is `selected`. Returns the
    /// actions asked for.
    pub fn show(&mut self, ui: &mut Ui, graphml: &GraphMl, selected: Option<&str>) -> Vec<Action> {
        let mut actions = Vec::new();
        egui::Panel::right("side_panel").show(ui, |ui| {
            // Otherwise the panel shrinks to fit its contents, even after resizing.
            ui.take_available_width();
            let node = selected.and_then(|id| Some((id, document::graphics_of(graphml, id)?)));
            match node {
                Some((id, graphics)) => actions = self.properties(ui, id, &graphics),
                None => {
                    ui.weak("Nothing selected");
                }
            }
        });
        // A field loses the focus to Enter, a click elsewhere, or a new
        // selection hiding it.
        let left = self
            .edit
            .take_if(|edit| !ui.memory(|memory| memory.has_focus(edit.field)));
        let applied = left.map(|edit| Action::SetLabel(edit.label, edit.text));
        // First, so a dialog the same click opens shows the text applied.
        applied.into_iter().chain(actions).collect()
    }

    fn properties(&mut self, ui: &mut Ui, id: &str, graphics: &NodeGraphics) -> Vec<Action> {
        Grid::new("properties")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                let shape = match &graphics.kind {
                    NodeKind::Shape(shape) => shape.as_str(),
                    NodeKind::Generic(configuration) => configuration,
                    NodeKind::Other(name) => name,
                };
                let geometry = &graphics.geometry;
                let border = &graphics.border;
                row(ui, "Id", id);
                row(ui, "Shape", shape);
                row(ui, "X", &number(geometry.x));
                row(ui, "Y", &number(geometry.y));
                row(ui, "Width", &number(geometry.width));
                row(ui, "Height", &number(geometry.height));
                // Two, when clicking a button also applies the border width
                // typed.
                let mut actions = Vec::new();
                for property in ColorProperty::ALL {
                    ui.label(property.name());
                    if color_button(ui, property.get(graphics)).clicked() {
                        actions.push(Action::EditColor(property));
                    }
                    ui.end_row();
                }
                ui.label("Border width");
                let width = self.border_width_field(ui, border.width);
                actions.extend(width.map(Action::SetBorderWidth));
                ui.end_row();
                row(ui, "Border style", border.line_type.as_str());
                // A node without labels gets a field that adds one.
                for index in 0..graphics.labels.len().max(1) {
                    let text = graphics.labels.get(index).map_or("", |label| &label.text);
                    let label = LabelRef {
                        node: id.to_owned(),
                        index,
                    };
                    actions.extend(self.label_row(ui, label, text));
                }
                actions
            })
            .inner
    }

    /// Adds a row with a field for a label's text, and a button that opens the
    /// label dialog. Returns the action clicked.
    fn label_row(&mut self, ui: &mut Ui, label: LabelRef, text: &str) -> Option<Action> {
        ui.label("Label");
        let field = Id::unique(("label_field", &label.node, label.index));
        let mut clicked = None;
        // Right to left, so the field takes the width the button leaves.
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            if ui.button("...").on_hover_text("Edit in a dialog").clicked() {
                clicked = Some(Action::EditLabelInDialog(label.clone()));
            }
            let edit = self.edit.as_ref().filter(|edit| edit.field == field);
            let mut typed = edit.map_or(text, |edit| &edit.text).to_owned();
            let response = label_field::text_edit(&mut typed)
                .id(field)
                .desired_width(ui.available_width())
                .align(Align2::LEFT_TOP)
                .show(ui)
                .response
                .response;
            if response.has_focus() {
                self.edit = match label_field::end(ui, &response) {
                    Some(End::Cancel) => None,
                    Some(End::Apply) | None => Some(FieldEdit {
                        field,
                        label,
                        text: typed,
                    }),
                };
            }
        });
        ui.end_row();
        clicked
    }

    /// Adds a field to drag or type a border width in. Returns the width set.
    fn border_width_field(&mut self, ui: &mut Ui, mut width: f64) -> Option<f64> {
        let field = DragValue::new(&mut width)
            .range(BORDER_WIDTHS)
            // Otherwise just showing a width out of range would change it.
            .clamp_existing_to_range(false)
            .speed(0.1)
            // Unrounded, unlike the other numbers, so that entering it
            // unchanged keeps it.
            .custom_formatter(|width, _| width.to_string())
            // As in a label's field, Enter or a click elsewhere applies the
            // width typed, and Escape cancels it.
            .update_while_editing(false);
        let response = ui.add(field);
        let escaped = response.lost_focus() && ui.input(|input| input.key_pressed(Key::Escape));
        let cancelled = std::mem::replace(&mut self.width_cancelled, escaped);
        (response.changed() && !cancelled).then_some(width)
    }
}

/// Adds a row, truncating a long value, which then shows in full on hover.
fn row(ui: &mut Ui, name: &str, value: &str) {
    ui.label(name);
    ui.add(Label::new(value).truncate());
    ui.end_row();
}

/// A button showing a swatch of `color` and its hex code.
fn color_button(ui: &mut Ui, color: Option<Color>) -> Response {
    let size = Vec2::splat(ui.text_style_height(&TextStyle::Body));
    let swatch = Atom::paint(size, move |ui, args| {
        let outline = Stroke::new(1.0, args.fallback_text_color);
        color_picker::paint_swatch(ui.painter(), args.rect, color, outline);
    });
    let text = color.map_or("None".to_owned(), |color| color.to_string());
    ui.add(Button::new((swatch, text)))
}

/// Up to two decimals, so yEd's 327.33333333333337 reads as 327.33.
fn number(value: f64) -> String {
    emath::format_with_decimals_in_range(value, 0..=2)
}
