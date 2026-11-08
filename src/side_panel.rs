use std::ops::RangeInclusive;

use egui::emath::{self, Numeric};
use egui::{
    Align, Align2, AsIdSalt, Atom, Button, ComboBox, DragValue, Grid, Id, IntoAtoms, Key, Label,
    Layout, Painter, Rect, Response, RichText, ScrollArea, Stroke, TextStyle, Ui, vec2,
};
use graphml::GraphMl;
use graphml::yed::{
    self, Arrow, Color, EdgeGraphics, Font, FontStyle, LabelPosition, LineType, NodeKind,
};

use crate::action::Action;
use crate::color_picker;
use crate::document::{self, ColorProperty, EdgeEnd, Graphics, Item, LabelRef};
use crate::font_picker::FontPicker;
use crate::fonts::{self, Fonts};
use crate::label_position;
use crate::line_type;
use crate::text_field::{self, End};

/// Of borders and edges. Capped, so typing "inf" can't make one infinite.
const LINE_WIDTHS: RangeInclusive<f64> = 0.0..=100.0;
/// Capped too, though corners are rounded by no more than half a side.
const CORNER_RADII: RangeInclusive<f64> = 0.0..=1000.0;
/// Capped, as egui draws no glyph over 1024 pixels.
const FONT_SIZES: RangeInclusive<u32> = 1..=500;
/// How long the line and label position samples are, in text heights.
const SAMPLE_LENGTH: f32 = 2.0;

/// A resizable panel right of the canvas with the properties of the selected
/// node or edge.
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
    /// Shows the properties of the `selected` node or edge, with the font
    /// families `fonts` offers. Returns the actions asked for.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        graphml: &GraphMl,
        selected: Option<&Item>,
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
                let graphics = selected.and_then(|item| document::graphics(graphml, item));
                match (selected, graphics) {
                    (Some(Item::Node(id)), Some(graphics @ Graphics::Node(_))) => {
                        let corner_radius = document::corner_radius(graphml, id);
                        actions = self.properties(ui, id, &graphics, corner_radius, fonts);
                    }
                    (Some(&Item::Edge(index)), Some(Graphics::Edge(graphics))) => {
                        let edge = &graphml.graphs[0].edges[index];
                        actions = self.edge_properties(ui, edge, index, &graphics, fonts);
                    }
                    _ => {
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

    /// The properties of the node with id `id`, whose graphics are `node`.
    /// `corner_radius` is the node's if it's a box.
    fn properties(
        &mut self,
        ui: &mut Ui,
        id: &str,
        node: &Graphics,
        corner_radius: Option<f64>,
        fonts: &Fonts,
    ) -> Vec<Action> {
        let Graphics::Node(graphics) = node else {
            return Vec::new();
        };
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
                    if color_button(ui, property.get(node)).clicked() {
                        actions.push(Action::EditColor(property));
                    }
                    ui.end_row();
                }
                ui.label("Border width");
                let width = self.number_field(ui, border.width, LINE_WIDTHS, 0.1);
                actions.extend(width.map(Action::SetLineWidth));
                ui.end_row();
                ui.label("Border style");
                let line_type = line_style_box(ui, &border.line_type);
                actions.extend(line_type.map(Action::SetLineType));
                ui.end_row();
                if let Some(radius) = corner_radius {
                    ui.label("Corner radius");
                    let radius = self.number_field(ui, radius, CORNER_RADII, 0.2);
                    actions.extend(radius.map(Action::SetCornerRadius));
                    ui.end_row();
                }
                let item = Item::Node(id.to_owned());
                actions.extend(self.label_rows(ui, &item, &graphics.labels, fonts));
                actions
            })
            .inner
    }

    /// The properties of `edge`, the one at `index`, whose graphics are
    /// `graphics`.
    fn edge_properties(
        &mut self,
        ui: &mut Ui,
        edge: &graphml::Edge,
        index: usize,
        graphics: &EdgeGraphics,
        fonts: &Fonts,
    ) -> Vec<Action> {
        Grid::new("properties")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                let line = &graphics.line;
                row(ui, "Id", edge.id.as_deref().unwrap_or_default());
                row(ui, "Source", &edge.source);
                row(ui, "Target", &edge.target);
                let mut actions = Vec::new();
                ui.label(ColorProperty::Line.name());
                if color_button(ui, line.color).clicked() {
                    actions.push(Action::EditColor(ColorProperty::Line));
                }
                ui.end_row();
                ui.label("Line width");
                let width = self.number_field(ui, line.width, LINE_WIDTHS, 0.1);
                actions.extend(width.map(Action::SetLineWidth));
                ui.end_row();
                ui.label("Line style");
                let line_type = line_style_box(ui, &line.line_type);
                actions.extend(line_type.map(Action::SetLineType));
                ui.end_row();
                let arrows = &graphics.arrows;
                for (name, end, arrow) in [
                    ("Start arrow", EdgeEnd::Source, &arrows.source),
                    ("End arrow", EdgeEnd::Target, &arrows.target),
                ] {
                    ui.label(name);
                    let mut on = *arrow != Arrow::None;
                    if ui.checkbox(&mut on, "").changed() {
                        actions.push(Action::SetArrow(end, on));
                    }
                    ui.end_row();
                }
                let item = Item::Edge(index);
                actions.extend(self.label_rows(ui, &item, &graphics.labels, fonts));
                actions
            })
            .inner
    }

    /// Adds the rows of the labels of `item`, which are `labels`, and a row
    /// that adds one if there are none. Returns the actions asked for.
    fn label_rows(
        &mut self,
        ui: &mut Ui,
        item: &Item,
        labels: &[yed::Label],
        fonts: &Fonts,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        for index in 0..labels.len().max(1) {
            let label = labels.get(index);
            let at = LabelRef {
                item: item.clone(),
                index,
            };
            let text = label.map_or("", |label| &label.text);
            actions.extend(self.label_row(ui, at.clone(), text));
            if let Some(label) = label {
                actions.extend(self.font_rows(ui, at.clone(), label, fonts));
                // Edge labels are placed by dragging them.
                if let Item::Node(_) = item {
                    ui.label("Position");
                    let position = position_box(ui, index, label.position());
                    actions.extend(position.map(|p| Action::SetLabelPosition(at, p)));
                    ui.end_row();
                }
            }
        }
        actions
    }

    /// Adds a row with a field for a label's text, and a button that opens the
    /// label dialog. Returns the action clicked.
    fn label_row(&mut self, ui: &mut Ui, label: LabelRef, text: &str) -> Option<Action> {
        ui.label("Label");
        let field = Id::unique(("label_field", &label.item, label.index));
        let mut clicked = None;
        // Right to left, so the field takes the width the button leaves.
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            if ui.button("...").on_hover_text("Edit in a dialog").clicked() {
                clicked = Some(Action::EditLabelInDialog(label.clone()));
            }
            let edit = self.edit.as_ref().filter(|edit| edit.field == field);
            let mut typed = edit.map_or(text, |edit| &edit.text).to_owned();
            let response = text_field::label_edit(&mut typed)
                .id(field)
                .desired_width(ui.available_width())
                .align(Align2::LEFT_TOP)
                .show(ui)
                .response
                .response;
            if response.has_focus() {
                self.edit = match text_field::end(ui, &response) {
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
        let set = |font: Font| Action::SetFont(at.clone(), font);
        let mut actions = Vec::new();
        ui.label("Font");
        let family = self
            .font_picker
            .show(ui, ("font", at.index), &font.family, || fonts.families());
        actions.extend(family.map(|family| {
            set(Font {
                family,
                ..font.clone()
            })
        }));
        ui.end_row();
        ui.label("Font style");
        let style = font_style_box(ui, at.index, font);
        actions.extend(style.map(|style| {
            set(Font {
                style,
                ..font.clone()
            })
        }));
        ui.end_row();
        ui.label("Font size");
        let size = self.number_field(ui, font.size, FONT_SIZES, 0.2);
        actions.extend(size.map(|size| {
            set(Font {
                size,
                ..font.clone()
            })
        }));
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

/// Adds a dropdown listing `groups` of items, a separator between the
/// groups, which reads `shown` while closed. `row` adds each item's row, told
/// whether the item is `current`. Returns the item clicked, unless it's
/// `current`.
fn dropdown<'a, T: PartialEq + Clone>(
    ui: &mut Ui,
    id_salt: impl AsIdSalt,
    shown: impl IntoAtoms<'a>,
    groups: &[&[T]],
    current: Option<&T>,
    row: impl Fn(&mut Ui, &T, bool) -> Response,
) -> Option<T> {
    let mut picked = None;
    ComboBox::from_id_salt(id_salt)
        .selected_text(shown)
        .width(ui.available_width())
        // egui's 200 pt would scroll the position list. Popups stay within
        // the window anyway.
        .height(f32::INFINITY)
        .show_ui(ui, |ui| {
            for (group, items) in groups.iter().enumerate() {
                if group > 0 {
                    ui.separator();
                }
                for item in *items {
                    let selected = current == Some(item);
                    if row(ui, item, selected).clicked() && !selected {
                        picked = Some(item.clone());
                    }
                }
            }
        });
    picked
}

/// Adds a dropdown to pick a border's or an edge's line type in. Returns the
/// type picked.
fn line_style_box(ui: &mut Ui, current: &LineType) -> Option<LineType> {
    let shown = line_type_atoms(ui, current);
    dropdown(
        ui,
        "border_style",
        shown,
        &[&line_type::ALL],
        Some(current),
        |ui, line_type, selected| {
            let atoms = line_type_atoms(ui, line_type);
            ui.selectable_label(selected, atoms)
        },
    )
}

/// Adds a dropdown to pick the style of the `index`th label's `font` in,
/// showing each in it. Returns the style picked.
fn font_style_box(ui: &mut Ui, index: usize, font: &Font) -> Option<FontStyle> {
    let styled = |style: &FontStyle| {
        RichText::new(fonts::style_name(style)).family(fonts::family(&font.family, style))
    };
    dropdown(
        ui,
        ("font_style", index),
        styled(&font.style),
        &[&fonts::STYLES],
        Some(&font.style),
        |ui, style, selected| ui.selectable_label(selected, styled(style)),
    )
}

/// Adds a dropdown to pick the position of the `index`th label in, which is
/// `current`, or `None` if it's elsewhere. Returns the position picked.
fn position_box(
    ui: &mut Ui,
    index: usize,
    current: Option<LabelPosition>,
) -> Option<LabelPosition> {
    let shown = match current {
        Some(position) => position_atoms(ui, position).into_atoms(),
        None => "Custom".into_atoms(),
    };
    dropdown(
        ui,
        ("label_position", index),
        shown,
        &[&label_position::INSIDE, &label_position::OUTSIDE],
        current.as_ref(),
        |ui, position, selected| {
            let atoms = position_atoms(ui, *position);
            ui.selectable_label(selected, atoms)
        },
    )
}

/// A sample of `position` and its name.
fn position_atoms(ui: &Ui, position: LabelPosition) -> (Atom<'static>, &'static str) {
    let sample = sample(ui, SAMPLE_LENGTH, move |painter, rect, stroke| {
        label_position::paint_sample(painter, rect, stroke, position);
    });
    (sample, label_position::name(position))
}

/// A sample line of `line_type` and its name.
fn line_type_atoms<'a>(ui: &Ui, line_type: &'a LineType) -> (Atom<'a>, &'a str) {
    let sample = sample(ui, SAMPLE_LENGTH, move |painter, rect, stroke| {
        let (x, y) = (rect.x_range(), rect.center().y);
        line_type::paint_hline(painter, x, y, stroke, line_type);
    });
    (sample, line_type::name(line_type))
}

/// A button showing a swatch of `color` and its hex code.
fn color_button(ui: &mut Ui, color: Option<Color>) -> Response {
    let swatch = sample(ui, 1.0, move |painter, rect, outline| {
        color_picker::paint_swatch(painter, rect, color, outline);
    });
    let text = color.map_or("None".to_owned(), |color| color.to_string());
    ui.add(Button::new((swatch, text)))
}

/// An atom `length` text heights long and one high, which `paint` paints in
/// the text color, e.g. as a sample of a line type.
fn sample<'a>(ui: &Ui, length: f32, paint: impl Fn(&Painter, Rect, Stroke) + 'a) -> Atom<'a> {
    let height = ui.text_style_height(&TextStyle::Body);
    Atom::paint(vec2(length * height, height), move |ui, args| {
        let stroke = Stroke::new(1.0, args.fallback_text_color);
        paint(ui.painter(), args.rect, stroke);
    })
}

/// Up to two decimals, so yEd's 327.33333333333337 reads as 327.33.
fn number(value: f64) -> String {
    emath::format_with_decimals_in_range(value, 0..=2)
}
