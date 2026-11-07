use super::codec::{Part, get_color, get_or, keyword_enum, set, set_color};
use super::placement::{LabelPosition, Placement};
use super::{Color, Rect};
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
    /// for node labels.
    pub bounds: Rect,
    /// Read and written for node labels only.
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
        if crate::xml::local_name(name) == "NodeLabel" {
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
}

/// A new label with the attributes yEd writes that `Label` doesn't model.
fn template(name: &str) -> Element {
    let element = Element::new(name)
        .with_attr("horizontalTextPosition", "center")
        .with_attr("iconTextGap", "4")
        .with_attr("verticalTextPosition", "bottom");
    if crate::xml::local_name(name) == "EdgeLabel" {
        element
            .with_attr("distance", "2.0")
            .with_attr("modelName", "centered")
            .with_attr("modelPosition", "center")
            .with_attr("preferredPlacement", "anywhere")
            .with_attr("ratio", "0.5")
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
