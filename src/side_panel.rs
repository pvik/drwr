use std::ops::RangeInclusive;

use egui::emath::{self, Numeric};
use egui::{
    Align, Align2, Atom, Button, ComboBox, DragValue, Grid, Id, IntoAtoms as _, Key, Label, Layout,
    Response, RichText, ScrollArea, Stroke, TextStyle, Ui, Vec2, vec2,
};
use graphml::GraphMl;
use graphml::yed::{self, Color, Font, FontStyle, LabelPosition, LineType, NodeGraphics, NodeKind};

use crate::action::Action;
use crate::color_picker;
use crate::document::{self, ColorProperty, LabelRef};
use crate::font_picker::FontPicker;
use crate::fonts::{self, Fonts};
use crate::label_field::{self, End};
use crate::label_position;
use crate::line_type;

/// Capped, so typing "inf" can't make a border infinite.
const BORDER_WIDTHS: RangeInclusive<f64> = 0.0..=100.0;
/// Capped, as egui draws no glyph over 1024 pixels.
const FONT_SIZES: RangeInclusive<u32> = 1..=500;
/// How long the line and label position samples are, in text heights.
const SAMPLE_LENGTH: f32 = 2.0;

/// A resizable panel right of the canvas with the properties of the selected
/// node.
#[derive(Default)]
pub struct SidePanel {
    edit: Option<FieldEdit>,
    /// The number field whose typing Escape just cancelled, which egui 0.37
    /// applies anyway a frame later.
    escaped: Option<Id>,
    font_picker: FontPicker,
}

/// A label being typed in its field, applied once the field loses the focus.
struct FieldEdit {
    field: Id,
    label: LabelRef,
    text: String,
}

impl SidePanel {
    /// Shows the properties of the node whose id is `selected`, with the font
    /// families `fonts` offers. Returns the actions asked for.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        graphml: &GraphMl,
        selected: Option<&str>,
        fonts: &Fonts,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        let panel = egui::Panel::right("side_panel");
        // Wide enough for the border style dropdown, which would otherwise
        // widen the panel the first time a node is selected.
        panel.default_size(245.0).show(ui, |ui| {
            // A node's rows can outgrow a small window, more so with labels.
            ScrollArea::vertical().show(ui, |ui| {
                // Otherwise the panel shrinks to fit its contents, even after
                // resizing.
                ui.take_available_width();
                let node = selected.and_then(|id| Some((id, document::graphics_of(graphml, id)?)));
                match node {
                    Some((id, graphics)) => actions = self.properties(ui, id, &graphics, fonts),
                    None => {
                        ui.weak("Nothing selected");
                    }
                }
            });
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

    fn properties(
        &mut self,
        ui: &mut Ui,
        id: &str,
        graphics: &NodeGraphics,
        fonts: &Fonts,
    ) -> Vec<Action> {
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
                // Two, when clicking a button also applies the number typed.
                let mut actions = Vec::new();
                for property in ColorProperty::NODE {
                    ui.label(property.name());
                    if color_button(ui, property.get(graphics)).clicked() {
                        actions.push(Action::EditColor(property));
                    }
                    ui.end_row();
                }
                ui.label("Border width");
                let width = self.number_field(ui, border.width, BORDER_WIDTHS, 0.1);
                actions.extend(width.map(Action::SetBorderWidth));
                ui.end_row();
                ui.label("Border style");
                let line_type = border_style_box(ui, &border.line_type);
                actions.extend(line_type.map(Action::SetBorderType));
                ui.end_row();
                // A node without labels gets a field that adds one.
                for index in 0..graphics.labels.len().max(1) {
                    let label = graphics.labels.get(index);
                    let at = LabelRef {
                        node: id.to_owned(),
                        index,
                    };
                    let text = label.map_or("", |label| &label.text);
                    actions.extend(self.label_row(ui, at.clone(), text));
                    if let Some(label) = label {
                        actions.extend(self.font_rows(ui, at.clone(), label, fonts));
                        ui.label("Position");
                        let position = position_box(ui, index, label.position());
                        actions.extend(position.map(|p| Action::SetLabelPosition(at, p)));
                        ui.end_row();
                    }
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

    /// Adds rows for the font and text color of the label `at`. Returns the
    /// actions asked for.
    fn font_rows(
        &mut self,
        ui: &mut Ui,
        at: LabelRef,
        label: &yed::Label,
        fonts: &Fonts,
    ) -> Vec<Action> {
        let font = &label.font;
        let mut actions = Vec::new();
        ui.label("Font");
        let family = self
            .font_picker
            .show(ui, ("font", at.index), &font.family, || fonts.families());
        actions.extend(family.map(|family| Action::SetFontFamily(at.clone(), family)));
        ui.end_row();
        ui.label("Font style");
        let style = font_style_box(ui, at.index, font);
        actions.extend(style.map(|style| Action::SetFontStyle(at.clone(), style)));
        ui.end_row();
        ui.label("Font size");
        let size = self.number_field(ui, font.size, FONT_SIZES, 0.2);
        actions.extend(size.map(|size| Action::SetFontSize(at.clone(), size)));
        ui.end_row();
        ui.label("Text color");
        if color_button(ui, Some(label.text_color)).clicked() {
            actions.push(Action::EditColor(ColorProperty::Text(at.index)));
        }
        ui.end_row();
        actions
    }

    /// Adds a field to drag or type a number in. Returns the number set.
    fn number_field<T: Numeric>(
        &mut self,
        ui: &mut Ui,
        mut value: T,
        range: RangeInclusive<T>,
        speed: f64,
    ) -> Option<T> {
        let field = DragValue::new(&mut value)
            .range(range)
            // Otherwise just showing a number out of range would change it.
            .clamp_existing_to_range(false)
            .speed(speed)
            // Unrounded, unlike the other numbers, so that entering it
            // unchanged keeps it.
            .custom_formatter(|value, _| value.to_string())
            // As in a label's field, Enter or a click elsewhere applies the
            // number typed, and Escape cancels it.
            .update_while_editing(false);
        let response = ui.add(field);
        let escaped = response.lost_focus() && ui.input(|input| input.key_pressed(Key::Escape));
        let cancelled = self.escaped.take_if(|id| *id == response.id).is_some();
        if escaped {
            self.escaped = Some(response.id);
        }
        (response.changed() && !cancelled).then_some(value)
    }
}

/// Adds a row, truncating a long value, which then shows in full on hover.
fn row(ui: &mut Ui, name: &str, value: &str) {
    ui.label(name);
    ui.add(Label::new(value).truncate());
    ui.end_row();
}

/// Adds a dropdown to pick a border's line type in. Returns the type picked.
fn border_style_box(ui: &mut Ui, current: &LineType) -> Option<LineType> {
    let mut picked = None;
    ComboBox::from_id_salt("border_style")
        .selected_text(line_type_atoms(ui, current))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for line_type in line_type::ALL {
                let selected = line_type == *current;
                let atoms = line_type_atoms(ui, &line_type);
                if ui.selectable_label(selected, atoms).clicked() && !selected {
                    picked = Some(line_type);
                }
            }
        });
    picked
}

/// Adds a dropdown to pick the style of the `index`th label's `font` in,
/// showing each in it. Returns the style picked.
fn font_style_box(ui: &mut Ui, index: usize, font: &Font) -> Option<FontStyle> {
    let sample = |style: &FontStyle| {
        RichText::new(fonts::style_name(style)).family(fonts::family(&font.family, style))
    };
    let mut picked = None;
    ComboBox::from_id_salt(("font_style", index))
        .selected_text(sample(&font.style))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for style in fonts::STYLES {
                let selected = style == font.style;
                if ui.selectable_label(selected, sample(&style)).clicked() && !selected {
                    picked = Some(style);
                }
            }
        });
    picked
}

/// Adds a dropdown to pick the position of the `index`th label in, which is
/// `current`, or `None` if it's elsewhere. Returns the position picked.
fn position_box(
    ui: &mut Ui,
    index: usize,
    current: Option<LabelPosition>,
) -> Option<LabelPosition> {
    let selected_text = match current {
        Some(position) => position_atoms(ui, position).into_atoms(),
        None => "Custom".into_atoms(),
    };
    let mut picked = None;
    ComboBox::from_id_salt(("label_position", index))
        .selected_text(selected_text)
        .width(ui.available_width())
        // egui's 200 pt would scroll the list. Popups stay within the window
        // anyway.
        .height(f32::INFINITY)
        .show_ui(ui, |ui| {
            let groups = [&label_position::INSIDE[..], &label_position::OUTSIDE];
            for (group, positions) in groups.into_iter().enumerate() {
                if group > 0 {
                    ui.separator();
                }
                for &position in positions {
                    let selected = current == Some(position);
                    let atoms = position_atoms(ui, position);
                    if ui.selectable_label(selected, atoms).clicked() && !selected {
                        picked = Some(position);
                    }
                }
            }
        });
    picked
}

/// A sample of `position` and its name.
fn position_atoms(ui: &Ui, position: LabelPosition) -> (Atom<'static>, &'static str) {
    let height = ui.text_style_height(&TextStyle::Body);
    let sample = Atom::paint(vec2(SAMPLE_LENGTH * height, height), move |ui, args| {
        let stroke = Stroke::new(1.0, args.fallback_text_color);
        label_position::paint_sample(ui.painter(), args.rect, stroke, position);
    });
    (sample, label_position::name(position))
}

/// A sample line of `line_type` and its name.
fn line_type_atoms<'a>(ui: &Ui, line_type: &'a LineType) -> (Atom<'a>, &'a str) {
    let height = ui.text_style_height(&TextStyle::Body);
    let sample = Atom::paint(vec2(SAMPLE_LENGTH * height, height), move |ui, args| {
        let stroke = Stroke::new(1.0, args.fallback_text_color);
        let (x, y) = (args.rect.x_range(), args.rect.center().y);
        line_type::paint_hline(ui.painter(), x, y, stroke, line_type);
    });
    (sample, line_type::name(line_type))
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
