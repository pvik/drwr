use std::ops::RangeInclusive;

use egui::emath::{self, Numeric};
use egui::{
    Align, Align2, AsIdSalt, Atom, Button, Checkbox, ComboBox, DragValue, Grid, Id, IntoAtoms, Key,
    Label, Layout, Painter, Rect, Response, RichText, ScrollArea, Stroke, StrokeKind, TextStyle,
    Ui, vec2,
};
use graphml::GraphMl;
use graphml::yed::{
    self, Arrow, Color, EdgeGraphics, FontStyle, LabelPosition, LineType, NodeGraphics, NodeKind,
};

use crate::action::{Action, FontPart};
use crate::color_picker;
use crate::document::{self, ColorProperty, EdgeEnd, Graphics, Item, LabelRef};
use crate::font_picker::FontPicker;
use crate::fonts::{self, Fonts};
use crate::label_position;
use crate::line_type;
use crate::selection::Selection;
use crate::text_field::{self, End};
use crate::toolbar;

/// Of borders and edges. Capped, so typing "inf" can't make one infinite.
const LINE_WIDTHS: RangeInclusive<f64> = 0.0..=100.0;
/// Capped too, though corners are rounded by no more than half a side.
const CORNER_RADII: RangeInclusive<f64> = 0.0..=1000.0;
/// Capped, as egui draws no glyph over 1024 pixels.
const FONT_SIZES: RangeInclusive<u32> = 1..=500;
/// How long the line and label position samples are, in text heights.
const SAMPLE_LENGTH: f32 = 2.0;
/// The rows of a node's bounds, and how each reads them.
const GEOMETRY: [(&str, Side); 4] = [
    ("X", |rect| rect.x),
    ("Y", |rect| rect.y),
    ("Width", |rect| rect.width),
    ("Height", |rect| rect.height),
];

/// Reads one side or corner coordinate of a node's bounds.
type Side = fn(&yed::Rect) -> f64;

/// A resizable panel right of the canvas with a toolbar and the properties of
/// the selected nodes and edges.
#[derive(Default)]
pub struct SidePanel {
    edit: Option<FieldEdit>,
    /// The number field whose typing Escape just cancelled, which egui 0.37
    /// applies anyway a frame later.
    escaped: Option<Id>,
    font_picker: FontPicker,
}

/// Text being typed in a label field, which goes to `labels` once the field
/// loses the focus.
struct FieldEdit {
    field: Id,
    labels: Vec<LabelRef>,
    text: String,
    /// Whether anything was typed. Otherwise a field left blank, as the
    /// labels differ, would blank them all just by losing the focus.
    typed: bool,
}

/// The selected nodes and edges, with their graphics.
type Items<'a> = [(&'a Item, Graphics)];

impl SidePanel {
    /// Shows the toolbar and the properties of the `selected` nodes and
    /// edges, with the font families `fonts` offers. Returns the actions asked
    /// for.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        graphml: &GraphMl,
        selected: &Selection,
        fonts: &Fonts,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        let mut clicked = None;
        let panel = egui::Panel::right("side_panel");
        // Wide enough for the border style dropdown, which would otherwise
        // widen the panel the first time a node is selected.
        panel.default_size(245.0).show(ui, |ui| {
            clicked = toolbar::show_for_selection(ui, !selected.is_empty());
            ui.separator();
            // A node's rows can outgrow a small window, more so with labels.
            ScrollArea::vertical().show(ui, |ui| {
                // Otherwise the panel shrinks to fit its contents, even after
                // resizing.
                ui.take_available_width();
                let items: Vec<_> = (selected.items().iter())
                    .filter_map(|item| Some((item, document::graphics(graphml, item)?)))
                    .collect();
                if items.is_empty() {
                    ui.weak("Nothing selected");
                } else {
                    actions = self.properties(ui, graphml, &items, fonts);
                }
            });
        });
        // A field loses the focus to Enter, a click elsewhere, or a new
        // selection hiding it.
        let left = self
            .edit
            .take_if(|edit| !ui.memory(|memory| memory.has_focus(edit.field)));
        let applied = left.filter(|edit| edit.typed).into_iter().flat_map(|edit| {
            let text = edit.text;
            (edit.labels.into_iter()).map(move |label| Action::SetLabel(label, text.clone()))
        });
        // First, so a dialog the same click opens shows the text applied.
        // Delete last, as it hands a deleted edge's index to the next edge.
        applied.chain(actions).chain(clicked).collect()
    }

    /// The properties that the selected nodes and edges, `items`, all have,
    /// each blank where their values differ. Returns the actions asked for.
    fn properties(
        &mut self,
        ui: &mut Ui,
        graphml: &GraphMl,
        items: &Items,
        fonts: &Fonts,
    ) -> Vec<Action> {
        let nodes: Vec<(&str, &NodeGraphics)> = (items.iter())
            .filter_map(|(item, graphics)| match (item, graphics) {
                (Item::Node(id), Graphics::Node(node)) => Some((id.as_str(), node)),
                _ => None,
            })
            .collect();
        let edges: Vec<(&graphml::Edge, &EdgeGraphics)> = (items.iter())
            .filter_map(|(item, graphics)| match (item, graphics) {
                (&&Item::Edge(index), Graphics::Edge(edge)) => {
                    Some((&graphml.graphs[0].edges[index], edge))
                }
                _ => None,
            })
            .collect();
        Grid::new("properties")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                match (&nodes[..], &edges[..]) {
                    ([(id, _)], []) => row(ui, "Id", Some(id)),
                    ([], [(edge, _)]) => row(ui, "Id", edge.id.as_deref().or(Some(""))),
                    _ => row(ui, "Selected", Some(&count(nodes.len(), edges.len()))),
                }
                if edges.is_empty() {
                    let shapes: Vec<_> = nodes.iter().map(|(_, node)| shape(node)).collect();
                    row(ui, "Shape", same(&shapes).copied());
                    for (name, side) in GEOMETRY {
                        let sides: Vec<_> = nodes.iter().map(|(_, n)| side(&n.geometry)).collect();
                        row(ui, name, same(&sides).map(|side| number(*side)).as_deref());
                    }
                }
                if nodes.is_empty() {
                    let sources: Vec<_> = edges.iter().map(|(e, _)| e.source.as_str()).collect();
                    row(ui, "Source", same(&sources).copied());
                    let targets: Vec<_> = edges.iter().map(|(e, _)| e.target.as_str()).collect();
                    row(ui, "Target", same(&targets).copied());
                }
                // Two, when clicking a button also applies the number typed.
                let mut actions = Vec::new();
                // A node's border counts as a line.
                let (colors, stroke): (&[_], _) = if edges.is_empty() {
                    (&ColorProperty::NODE, "Border")
                } else {
                    (&[ColorProperty::Line], "Line")
                };
                for &property in colors {
                    let colors: Vec<_> = items.iter().map(|(_, g)| property.get(g)).collect();
                    ui.label(property.name());
                    if color_button(ui, same(&colors).copied()).clicked() {
                        actions.push(Action::EditColor(property));
                    }
                    ui.end_row();
                }
                let strokes: Vec<_> = items.iter().map(|(_, g)| g.stroke()).collect();
                ui.label(format!("{stroke} width"));
                let widths: Vec<_> = strokes.iter().map(|stroke| stroke.width).collect();
                let width = self.number_field(ui, &widths, LINE_WIDTHS, 0.1);
                actions.extend(width.map(Action::SetLineWidth));
                ui.end_row();
                ui.label(format!("{stroke} style"));
                let types: Vec<_> = strokes.iter().map(|stroke| &stroke.line_type).collect();
                let line_type = line_style_box(ui, same(&types).copied());
                actions.extend(line_type.map(Action::SetLineType));
                ui.end_row();
                let radii = nodes
                    .iter()
                    .map(|(id, node)| document::corner_radius_of(graphml, id, node));
                // Only boxes have one.
                if edges.is_empty()
                    && let Some(radii) = radii.collect::<Option<Vec<_>>>()
                {
                    ui.label("Corner radius");
                    let radius = self.number_field(ui, &radii, CORNER_RADII, 0.2);
                    actions.extend(radius.map(Action::SetCornerRadius));
                    ui.end_row();
                }
                if nodes.is_empty() {
                    for (name, end) in [
                        ("Start arrow", EdgeEnd::Source),
                        ("End arrow", EdgeEnd::Target),
                    ] {
                        let arrows: Vec<_> = (edges.iter())
                            .map(|(_, edge)| match end {
                                EdgeEnd::Source => edge.arrows.source != Arrow::None,
                                EdgeEnd::Target => edge.arrows.target != Arrow::None,
                            })
                            .collect();
                        ui.label(name);
                        let shown = same(&arrows).copied();
                        let mut on = shown.unwrap_or_default();
                        let checkbox = Checkbox::new(&mut on, "").indeterminate(shown.is_none());
                        if ui.add(checkbox).changed() {
                            actions.push(Action::SetArrow(end, on));
                        }
                        ui.end_row();
                    }
                }
                actions.extend(self.label_rows(ui, items, fonts));
                actions
            })
            .inner
    }

    /// Adds the rows of the labels of the selected nodes and edges, `items`:
    /// one per label they all have, or one that adds a label to each if some
    /// have none. Returns the actions asked for.
    fn label_rows(&mut self, ui: &mut Ui, items: &Items, fonts: &Fonts) -> Vec<Action> {
        let count = items.iter().map(|(_, g)| g.labels().len()).min();
        let count = count.unwrap_or_default();
        let mut actions = Vec::new();
        for index in 0..count.max(1) {
            let label = |item: &Item| LabelRef {
                item: item.clone(),
                index,
            };
            let all: Vec<_> = items.iter().map(|(item, _)| label(item)).collect();
            let texts: Vec<_> = (items.iter())
                .map(|(_, g)| g.labels().get(index).map_or("", |label| &label.text))
                .collect();
            actions.extend(self.label_row(ui, &all, same(&texts).copied()));
            // Of the labels there are, as items without one have no font.
            let (at, labels): (Vec<_>, Vec<_>) = (items.iter())
                .filter_map(|(item, g)| Some((label(item), g.labels().get(index)?)))
                .unzip();
            if labels.is_empty() {
                continue;
            }
            actions.extend(self.font_rows(ui, &at, &labels, fonts));
            // Edge labels are placed by dragging them.
            if items.iter().all(|(item, _)| matches!(item, Item::Node(_))) {
                ui.label("Position");
                let positions: Vec<_> = labels.iter().map(|label| label.position()).collect();
                let position = position_box(ui, index, same(&positions).copied());
                actions.extend(position.into_iter().flat_map(|position| {
                    (at.iter()).map(move |at| Action::SetLabelPosition(at.clone(), position))
                }));
                ui.end_row();
            }
        }
        actions
    }

    /// Adds a row with a field for the text of the labels `labels`, which is
    /// `text`, or blank if theirs differ, and a button that opens the label
    /// dialog. Returns the action clicked.
    fn label_row(
        &mut self,
        ui: &mut Ui,
        labels: &[LabelRef],
        text: Option<&str>,
    ) -> Option<Action> {
        ui.label("Label");
        let field = Id::unique(("label_field", labels));
        let mut clicked = None;
        // Right to left, so the field takes the width the button leaves.
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            if ui.button("...").on_hover_text("Edit in a dialog").clicked() {
                clicked = Some(Action::EditLabelInDialog(labels.to_vec()));
            }
            let edit = self.edit.as_ref().filter(|edit| edit.field == field);
            let typed_before = edit.is_some_and(|edit| edit.typed);
            let mut typed = edit
                .map_or(text.unwrap_or_default(), |edit| &edit.text)
                .to_owned();
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
                        labels: labels.to_vec(),
                        text: typed,
                        typed: typed_before || response.changed(),
                    }),
                };
            }
        });
        ui.end_row();
        clicked
    }

    /// Adds rows for the font and text color of the labels `at`, which are
    /// `labels`, each blank where theirs differ. Returns the actions asked
    /// for.
    fn font_rows(
        &mut self,
        ui: &mut Ui,
        at: &[LabelRef],
        labels: &[&yed::Label],
        fonts: &Fonts,
    ) -> Vec<Action> {
        let index = at.first().map_or(0, |at| at.index);
        let set = |part: FontPart| -> Vec<Action> {
            (at.iter())
                .map(|at| Action::SetFont(at.clone(), part.clone()))
                .collect()
        };
        let mut actions = Vec::new();
        ui.label("Font");
        let families: Vec<_> = labels
            .iter()
            .map(|label| label.font.family.as_str())
            .collect();
        let shown = same(&families).copied().unwrap_or_default();
        let family = self
            .font_picker
            .show(ui, ("font", index), shown, || fonts.families());
        actions.extend(family.map(FontPart::Family).into_iter().flat_map(set));
        ui.end_row();
        ui.label("Font style");
        let styles: Vec<_> = labels.iter().map(|label| &label.font.style).collect();
        // Each shown in the first label's family.
        let style = font_style_box(ui, index, families[0], same(&styles).copied());
        actions.extend(style.map(FontPart::Style).into_iter().flat_map(set));
        ui.end_row();
        ui.label("Font size");
        let sizes: Vec<_> = labels.iter().map(|label| label.font.size).collect();
        let size = self.number_field(ui, &sizes, FONT_SIZES, 0.2);
        actions.extend(size.map(FontPart::Size).into_iter().flat_map(set));
        ui.end_row();
        ui.label("Text color");
        let colors: Vec<_> = labels.iter().map(|label| Some(label.text_color)).collect();
        if color_button(ui, same(&colors).copied()).clicked() {
            actions.push(Action::EditColor(ColorProperty::Text(index)));
        }
        ui.end_row();
        actions
    }

    /// Adds a field to drag or type a number in, which shows the number all
    /// `values` are, or nothing if they differ. Returns the number set, which
    /// they all get.
    fn number_field<T: Numeric>(
        &mut self,
        ui: &mut Ui,
        values: &[T],
        range: RangeInclusive<T>,
        speed: f64,
    ) -> Option<T> {
        let first = values.first()?.to_f64();
        let differ = same(values).is_none();
        let (mut value, mut set) = (first, false);
        // Told apart from the value changing, as a blank field set to the
        // first value must still set the others.
        let field = DragValue::from_get_set(|new| {
            if let Some(new) = new {
                (value, set) = (new, true);
            }
            value
        })
        .range(range)
        // Otherwise just showing a number out of range would change it.
        .clamp_existing_to_range(false)
        .speed(speed)
        // Unrounded, unlike the other numbers, so that entering it unchanged
        // keeps it.
        .custom_formatter(move |value, _| {
            if differ {
                String::new()
            } else {
                value.to_string()
            }
        })
        // As in a label's field, Enter or a click elsewhere applies the
        // number typed, and Escape cancels it.
        .update_while_editing(false);
        let response = ui.add(field);
        let escaped = response.lost_focus() && ui.input(|input| input.key_pressed(Key::Escape));
        let cancelled = self.escaped.take_if(|id| *id == response.id).is_some();
        if escaped {
            self.escaped = Some(response.id);
        }
        (set && !cancelled && (differ || value != first)).then(|| T::from_f64(value))
    }
}

/// The value all `values` have, if they're the same.
fn same<T: PartialEq>(values: &[T]) -> Option<&T> {
    let (first, rest) = values.split_first()?;
    rest.iter().all(|value| value == first).then_some(first)
}

/// How many nodes and edges there are, e.g. "2 nodes, 1 line".
fn count(nodes: usize, edges: usize) -> String {
    let count = |n: usize, what: &str| match n {
        1 => format!("1 {what}"),
        n => format!("{n} {what}s"),
    };
    match (nodes, edges) {
        (nodes, 0) => count(nodes, "node"),
        (0, edges) => count(edges, "line"),
        (nodes, edges) => format!("{}, {}", count(nodes, "node"), count(edges, "line")),
    }
}

/// The name of a node's shape, or of its kind if it has none.
fn shape(node: &NodeGraphics) -> &str {
    match &node.kind {
        NodeKind::Shape(shape) => shape.as_str(),
        NodeKind::Generic(configuration) => configuration,
        NodeKind::Other(name) => name,
    }
}

/// Adds a row, truncating a long value, which then shows in full on hover.
/// The value is blank if it's `None`, as the selected items' values differ.
fn row(ui: &mut Ui, name: &str, value: Option<&str>) {
    ui.label(name);
    ui.add(Label::new(value.unwrap_or_default()).truncate());
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

/// Adds a dropdown to pick a border's or an edge's line type in, which reads
/// `current`, or nothing if the selected items' types differ. Returns the
/// type picked.
fn line_style_box(ui: &mut Ui, current: Option<&LineType>) -> Option<LineType> {
    let shown = match current {
        Some(current) => line_type_atoms(ui, current).into_atoms(),
        None => "".into_atoms(),
    };
    dropdown(
        ui,
        "border_style",
        shown,
        &[&line_type::ALL],
        current,
        |ui, line_type, selected| {
            let atoms = line_type_atoms(ui, line_type);
            ui.selectable_label(selected, atoms)
        },
    )
}

/// Adds a dropdown to pick the font style of the `index`th labels in, which
/// is `current`, or blank if theirs differ, showing each style in `family`.
/// Returns the style picked.
fn font_style_box(
    ui: &mut Ui,
    index: usize,
    family: &str,
    current: Option<&FontStyle>,
) -> Option<FontStyle> {
    let styled = |style: &FontStyle| {
        RichText::new(fonts::style_name(style)).family(fonts::family(family, style))
    };
    dropdown(
        ui,
        ("font_style", index),
        current.map_or_else(|| RichText::new(""), styled),
        &[&fonts::STYLES],
        current,
        |ui, style, selected| ui.selectable_label(selected, styled(style)),
    )
}

/// Adds a dropdown to pick the position of the `index`th labels in, which is
/// `current`: `Some(None)` if it's elsewhere, and `None` if theirs differ,
/// which leaves it blank. Returns the position picked.
fn position_box(
    ui: &mut Ui,
    index: usize,
    current: Option<Option<LabelPosition>>,
) -> Option<LabelPosition> {
    let shown = match current {
        Some(Some(position)) => position_atoms(ui, position).into_atoms(),
        Some(None) => "Custom".into_atoms(),
        None => "".into_atoms(),
    };
    dropdown(
        ui,
        ("label_position", index),
        shown,
        &[&label_position::INSIDE, &label_position::OUTSIDE],
        current.flatten().as_ref(),
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

/// A button showing a swatch of `color` and its hex code, or an empty
/// swatch if `color` is `None`, as the selected items' colors differ.
fn color_button(ui: &mut Ui, color: Option<Option<Color>>) -> Response {
    let Some(color) = color else {
        let swatch = sample(ui, 1.0, |painter, rect, outline| {
            painter.rect_stroke(rect, 0.0, outline, StrokeKind::Inside);
        });
        return ui.add(Button::new(swatch));
    };
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
