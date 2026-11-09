mod stored;

use graphml::yed::{Color, EdgeGraphics, FontStyle, Label, LineType, NodeGraphics, Stroke};
use serde::{Deserialize, Serialize};

use crate::action::FontPart;
use crate::document::ColorProperty;
use crate::selection::Selection;

/// The properties the user last set on nodes, lines and labels, which new
/// ones get. Each is `None` until set, keeping its default, and a color set
/// to none is `Some(None)`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Style {
    #[serde(with = "stored::color_or_none")]
    fill: Option<Option<Color>>,
    #[serde(with = "stored::color_or_none")]
    gradient: Option<Option<Color>>,
    #[serde(skip_serializing_if = "StrokeStyle::is_unset")]
    border: StrokeStyle,
    corner_radius: Option<f64>,
    /// Of edges, apart from nodes' borders.
    #[serde(skip_serializing_if = "StrokeStyle::is_unset")]
    line: StrokeStyle,
    font_family: Option<String>,
    #[serde(with = "stored::keyword")]
    font_style: Option<FontStyle>,
    font_size: Option<u32>,
    #[serde(with = "stored::color")]
    text_color: Option<Color>,
}

/// The parts of a node's border or an edge's line last set.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct StrokeStyle {
    #[serde(with = "stored::color_or_none")]
    color: Option<Option<Color>>,
    width: Option<f64>,
    #[serde(with = "stored::keyword")]
    line_type: Option<LineType>,
}

impl Style {
    /// Remembers `color` as `property` of the kinds of items `selected` has.
    pub fn set_color(
        &mut self,
        selected: &Selection,
        property: ColorProperty,
        color: Option<Color>,
    ) {
        let nodes = selected.has_nodes();
        match property {
            ColorProperty::Fill if nodes => self.fill = Some(color),
            ColorProperty::Gradient if nodes => self.gradient = Some(color),
            ColorProperty::Border | ColorProperty::Line => {
                for stroke in self.strokes(selected) {
                    stroke.color = Some(color);
                }
            }
            // Text can't have no color.
            ColorProperty::Text(_) if color.is_some() => self.text_color = color,
            ColorProperty::Fill | ColorProperty::Gradient | ColorProperty::Text(_) => {}
        }
    }

    /// Remembers `width` for the borders or lines of the kinds of items
    /// `selected` has.
    pub fn set_line_width(&mut self, selected: &Selection, width: f64) {
        for stroke in self.strokes(selected) {
            stroke.width = Some(width);
        }
    }

    /// Remembers `line_type` for the borders or lines of the kinds of items
    /// `selected` has.
    pub fn set_line_type(&mut self, selected: &Selection, line_type: &LineType) {
        for stroke in self.strokes(selected) {
            stroke.line_type = Some(line_type.clone());
        }
    }

    /// Remembers `radius` for new boxes, if `selected` has nodes.
    pub fn set_corner_radius(&mut self, selected: &Selection, radius: f64) {
        if selected.has_nodes() {
            self.corner_radius = Some(radius);
        }
    }

    /// Remembers `part` of the font for new labels.
    pub fn set_font(&mut self, part: &FontPart) {
        match part {
            FontPart::Family(family) => self.font_family = Some(family.clone()),
            FontPart::Style(style) => self.font_style = Some(style.clone()),
            FontPart::Size(size) => self.font_size = Some(*size),
        }
    }

    /// The strokes of the kinds of items `selected` has: nodes' borders and
    /// edges' lines.
    fn strokes(&mut self, selected: &Selection) -> impl Iterator<Item = &mut StrokeStyle> {
        let (nodes, edges) = (selected.has_nodes(), selected.has_edges());
        let strokes = [(nodes, &mut self.border), (edges, &mut self.line)];
        strokes
            .into_iter()
            .filter_map(|(has, stroke)| has.then_some(stroke))
    }

    /// Gives a new node the fill and border last set.
    pub fn apply_to_node(&self, graphics: &mut NodeGraphics) {
        apply(&self.fill, &mut graphics.fill.color);
        apply(&self.gradient, &mut graphics.fill.gradient);
        self.border.apply_to(&mut graphics.border);
    }

    /// The corner radius last set, which new boxes get.
    pub fn corner_radius(&self) -> Option<f64> {
        self.corner_radius
    }

    /// Gives a new edge the line last set.
    pub fn apply_to_edge(&self, graphics: &mut EdgeGraphics) {
        self.line.apply_to(&mut graphics.line);
    }

    /// Gives a new label the font and text color last set.
    pub fn apply_to_label(&self, label: &mut Label) {
        apply(&self.font_family, &mut label.font.family);
        apply(&self.font_style, &mut label.font.style);
        apply(&self.font_size, &mut label.font.size);
        apply(&self.text_color, &mut label.text_color);
    }
}

impl StrokeStyle {
    fn is_unset(&self) -> bool {
        *self == StrokeStyle::default()
    }

    fn apply_to(&self, stroke: &mut Stroke) {
        apply(&self.color, &mut stroke.color);
        apply(&self.width, &mut stroke.width);
        apply(&self.line_type, &mut stroke.line_type);
    }
}

/// Sets `property` to `value`, if that was ever set.
fn apply<T: Clone>(value: &Option<T>, property: &mut T) {
    if let Some(value) = value {
        property.clone_from(value);
    }
}

#[cfg(test)]
mod tests;
