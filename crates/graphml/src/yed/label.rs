use super::codec::{Part, get_color, get_or, keyword_enum, qualified, set, set_color};
use super::edge_placement::SmartEdge;
use super::placement::{LabelPosition, Placement};
use super::{Color, Point, Rect, Route};
use crate::{
    Error,
    xml::{Element, XmlNode},
};

/// A `y:NodeLabel` or `y:EdgeLabel`.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub text: String,
    pub visible: bool,
    pub font: Font,
    pub text_color: Color,
    pub background: Option<Color>,
    /// The color of a box around the label.
    pub border: Option<Color>,
    /// How the lines of multi-line text line up.
    pub alignment: Alignment,
    /// The bounds yEd last computed, relative to the node's top-left corner
    /// for node labels, and to the start of the edge's [`Route`] for edge
    /// labels.
    pub bounds: Rect,
    placement: Placement,
    source: Option<Element>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Font {
    pub family: String,
    pub size: u32,
    pub style: FontStyle,
}

impl Default for Font {
    fn default() -> Self {
        Font {
            family: "Dialog".to_owned(),
            size: 12,
            style: FontStyle::Plain,
        }
    }
}

keyword_enum! {
    pub enum FontStyle {
        Plain = "plain",
        Bold = "bold",
        Italic = "italic",
        BoldItalic = "bolditalic",
    }
}

keyword_enum! {
    pub enum Alignment {
        Left = "left",
        Center = "center",
        Right = "right",
    }
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Label {
            text: text.into(),
            visible: true,
            font: Font::default(),
            text_color: Color::BLACK,
            background: None,
            border: None,
            alignment: Alignment::Center,
            bounds: Rect::default(),
            placement: Placement::NEW,
            source: None,
        }
    }

    /// A new edge label, which yEd puts on the middle of its edge.
    pub fn new_on_edge(text: impl Into<String>) -> Self {
        Label {
            placement: Placement::Edge(SmartEdge::default()),
            ..Label::new(text)
        }
    }

    pub(crate) fn from_element(element: &Element) -> Result<Self, Error> {
        Ok(Label {
            text: element.text(),
            visible: get_or(element, "visible", true)?,
            font: Font {
                family: get_or(element, "fontFamily", "Dialog".to_owned())?,
                size: get_or(element, "fontSize", 12)?,
                style: get_or(element, "fontStyle", FontStyle::Plain)?,
            },
            text_color: get_or(element, "textColor", Color::BLACK)?,
            background: get_color(element, "hasBackgroundColor", "backgroundColor")?,
            border: get_color(element, "hasLineColor", "lineColor")?,
            alignment: get_or(element, "alignment", Alignment::Center)?,
            bounds: Rect::read(element)?,
            placement: Placement::read(element)?,
            source: Some(element.clone()),
        })
    }

    /// Writes the label as an element called `name`, e.g. `y:NodeLabel`.
    pub(crate) fn to_element(&self, name: &str) -> Element {
        let (mut element, new) = match &self.source {
            Some(source) if source.name == name => (source.clone(), false),
            _ => (template(name), true),
        };
        if new || element.text() != self.text {
            set_text(&mut element, &self.text);
        }
        set(&mut element, "visible", &self.visible);
        set(&mut element, "fontFamily", &self.font.family);
        set(&mut element, "fontSize", &self.font.size);
        set(&mut element, "fontStyle", &self.font.style);
        set(&mut element, "textColor", &self.text_color);
        set_color(
            &mut element,
            "hasBackgroundColor",
            "backgroundColor",
            self.background,
        );
        set_color(&mut element, "hasLineColor", "lineColor", self.border);
        set(&mut element, "alignment", &self.alignment);
        self.bounds.write(&mut element);
        let edge = crate::xml::local_name(name) == "EdgeLabel";
        if edge == matches!(self.placement, Placement::Edge(_)) {
            self.placement.write(&mut element);
        }
        element
    }

    /// Where yEd puts this node label, or `None` if it's anywhere else, e.g.
    /// where it was dragged to.
    pub fn position(&self) -> Option<LabelPosition> {
        self.placement.position()
    }

    /// Puts this node label at `position`, in its label model if that has
    /// the position, or else in yEd's Smart model, which has them all. Leaves
    /// `bounds` as they are; see [`Label::placed`].
    pub fn set_position(&mut self, position: LabelPosition) {
        self.placement = self.placement.at(position);
    }

    /// The bounds yEd gives this node label if it's `width` by `height`, on
    /// a node with the geometry `node`. `None` if it's placed freely or by a
    /// label model this library doesn't read.
    pub fn placed(&self, node: &Rect, width: f64, height: f64) -> Option<Rect> {
        self.placement.place(node, width, height)
    }

    /// The bounds yEd gives this edge label if it's `width` by `height`, on
    /// an edge along `route`. `None` if it's placed by a label model this
    /// library doesn't read, or turned.
    pub fn placed_on(&self, route: &Route, width: f64, height: f64) -> Option<Rect> {
        match &self.placement {
            Placement::Edge(edge) => edge.place(route, width, height),
            _ => None,
        }
    }

    /// Puts this edge label, `width` by `height`, with its center at
    /// `center` beside an edge along `route`, in yEd's Smart model, and moves
    /// its bounds there.
    pub fn place_on(&mut self, route: &Route, center: Point, width: f64, height: f64) {
        let old = match self.placement {
            Placement::Edge(edge) => Some(edge),
            _ => None,
        };
        let edge = SmartEdge::through(route, center, width, height, old);
        self.placement = Placement::Edge(edge);
        let start = route.points[0];
        self.bounds = Rect {
            x: center.x - width / 2.0 - start.x,
            y: center.y - height / 2.0 - start.y,
            width,
            height,
        };
    }

    /// Keeps this edge label on its segment, or on the one it's merged into,
    /// once the bend at index `bend` is removed from an edge whose route had
    /// `segments` segments.
    pub fn remove_bend(&mut self, bend: usize, segments: usize) {
        if let Placement::Edge(edge) = self.placement {
            self.placement = Placement::Edge(edge.without_bend(bend, segments));
        }
    }
}

/// A new label with the attributes yEd writes that `Label` doesn't model.
fn template(name: &str) -> Element {
    let element = Element::new(name)
        .with_attr("horizontalTextPosition", "center")
        .with_attr("iconTextGap", "4")
        .with_attr("verticalTextPosition", "bottom");
    if crate::xml::local_name(name) == "EdgeLabel" {
        let child = |local_name: &str, attributes: &[(&str, &str)]| {
            let child = Element::new(qualified(&element, local_name));
            attributes
                .iter()
                .fold(child, |child, &(name, value)| child.with_attr(name, value))
        };
        let model = child(
            "SmartEdgeLabelModel",
            &[
                ("autoRotationEnabled", "false"),
                ("defaultAngle", "0.0"),
                ("defaultDistance", "10.0"),
            ],
        );
        let parameter = child(
            "SmartEdgeLabelModelParameter",
            &[
                ("angle", "0.0"),
                ("distance", "30.0"),
                ("distanceToCenter", "true"),
                ("position", "center"),
                ("ratio", "0.5"),
                ("segment", "0"),
            ],
        );
        let descriptor = child(
            "PreferredPlacementDescriptor",
            &[
                ("angle", "0.0"),
                ("angleOffsetOnRightSide", "0"),
                ("angleReference", "absolute"),
                ("angleRotationOnRightSide", "co"),
                ("distance", "-1.0"),
                ("frozen", "true"),
                ("placement", "anywhere"),
                ("side", "anywhere"),
                ("sideReference", "relative_to_edge_flow"),
            ],
        );
        let model = child("LabelModel", &[]).with_child(model);
        let parameter = child("ModelParameter", &[]).with_child(parameter);
        element
            .with_attr("configuration", "AutoFlippingLabel")
            .with_attr("distance", "2.0")
            .with_attr("modelName", "custom")
            .with_attr("preferredPlacement", "anywhere")
            .with_attr("ratio", "0.5")
            .with_child(model)
            .with_child(parameter)
            .with_child(descriptor)
    } else {
        element.with_attr("autoSizePolicy", "content")
    }
}

/// Replaces the text, which yEd writes ahead of the label's child elements.
fn set_text(element: &mut Element, text: &str) {
    element
        .children
        .retain(|node| !matches!(node, XmlNode::Text(_) | XmlNode::CData(_)));
    // yEd marks empty labels, and only those, with `hasText="false"`.
    if text.is_empty() {
        element.set_attr("hasText", "false");
    } else {
        element.attributes.remove("hasText");
        element.children.insert(0, XmlNode::Text(text.to_owned()));
    }
    // Otherwise readers may take surrounding whitespace for formatting.
    if text.trim() != text {
        element.set_attr("xml:space", "preserve");
    }
}
