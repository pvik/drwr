use super::codec::{
    Part, child_or_insert, get, get_color, get_or, keyword_enum, qualified, read_part,
    replace_children, set, set_color, write_part,
};
use super::{Color, Label, Rect, Stroke};
use crate::{Error, xml::Element};

/// The graphics of a node: the element inside its `nodegraphics` data.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeGraphics {
    pub kind: NodeKind,
    /// The top-left corner and size.
    pub geometry: Rect,
    pub fill: Fill,
    pub border: Stroke,
    pub labels: Vec<Label>,
    /// The element this was read from, which keeps what isn't modelled here.
    source: Option<Element>,
}

/// The kind of yEd node, which decides how it's drawn.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// A `y:ShapeNode`.
    Shape(ShapeType),
    /// A `y:GenericNode`, by its `configuration`, e.g.
    /// `com.yworks.flowchart.process`.
    Generic(String),
    /// A `y:SVGNode`, drawing the SVG in the resource with id `resource`; see
    /// [`super::resource_text`].
    Svg { resource: Option<String> },
    /// Any other type, by element name, e.g. `UMLClassNode`. New nodes of
    /// these kinds get none of their type-specific content.
    Other(String),
}

impl NodeKind {
    fn element_name(&self) -> &str {
        match self {
            NodeKind::Shape(_) => "ShapeNode",
            NodeKind::Generic(_) => "GenericNode",
            NodeKind::Svg { .. } => "SVGNode",
            NodeKind::Other(name) => name,
        }
    }
}

keyword_enum! {
    /// The outline of a [`NodeKind::Shape`] node.
    pub enum ShapeType {
        Rectangle = "rectangle",
        RoundRectangle = "roundrectangle",
        Ellipse = "ellipse",
        Parallelogram = "parallelogram",
        Hexagon = "hexagon",
        Triangle = "triangle",
        Rectangle3d = "rectangle3d",
        Octagon = "octagon",
        Diamond = "diamond",
        Trapezoid = "trapezoid",
        Trapezoid2 = "trapezoid2",
    }
}

impl Part for ShapeType {
    fn read(element: &Element) -> Result<Self, Error> {
        get_or(element, "type", ShapeType::Rectangle)
    }

    fn write(&self, element: &mut Element) {
        set(element, "type", self);
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fill {
    /// `None` leaves the node transparent.
    pub color: Option<Color>,
    /// The color a gradient fades to.
    pub gradient: Option<Color>,
}

impl Part for Fill {
    fn read(element: &Element) -> Result<Self, Error> {
        Ok(Fill {
            color: get_color(element, "hasColor", "color")?,
            gradient: get(element, "color2")?,
        })
    }

    fn write(&self, element: &mut Element) {
        set_color(element, "hasColor", "color", self.color);
        match &self.gradient {
            Some(gradient) => set(element, "color2", gradient),
            None => {
                element.attributes.remove("color2");
            }
        }
    }
}

const BEFORE_LABELS: &[&str] = &["Geometry", "Fill", "BorderStyle"];
const BEFORE_SHAPE: &[&str] = &["Geometry", "Fill", "BorderStyle", "NodeLabel"];
const BEFORE_SVG_MODEL: &[&str] = &[
    "Geometry",
    "Fill",
    "BorderStyle",
    "NodeLabel",
    "SVGNodeProperties",
];

impl NodeGraphics {
    /// A node like the ones yEd creates.
    pub fn new(kind: NodeKind) -> Self {
        // yEd's, though it draws no fill on SVG nodes.
        let color = match kind {
            NodeKind::Svg { .. } => Color::rgb(0xCC, 0xCC, 0xFF),
            _ => Color::rgb(0xFF, 0xCC, 0x00),
        };
        NodeGraphics {
            kind,
            geometry: Rect {
                width: 30.0,
                height: 30.0,
                ..Rect::default()
            },
            fill: Fill {
                color: Some(color),
                gradient: None,
            },
            border: Stroke::default(),
            labels: Vec::new(),
            source: None,
        }
    }

    pub fn from_element(element: &Element) -> Result<Self, Error> {
        let realizer = realizer(element);
        let kind = match realizer.local_name() {
            "ShapeNode" => NodeKind::Shape(read_part(realizer, "Shape")?),
            "GenericNode" => NodeKind::Generic(get_or(realizer, "configuration", String::new())?),
            "SVGNode" => NodeKind::Svg {
                resource: svg_content(realizer)
                    .and_then(|content| content.attr("refid"))
                    .map(str::to_owned),
            },
            name => NodeKind::Other(name.to_owned()),
        };
        Ok(NodeGraphics {
            kind,
            geometry: read_part(realizer, "Geometry")?,
            fill: read_part(realizer, "Fill")?,
            border: read_part(realizer, "BorderStyle")?,
            labels: realizer
                .children_named("NodeLabel")
                .map(Label::from_element)
                .collect::<Result<_, _>>()?,
            source: Some(element.clone()),
        })
    }

    /// The element for these graphics: the one they were read from, changed
    /// only where they were, or a new one if the kind changed.
    pub fn to_element(&self) -> Element {
        let name = self.kind.element_name();
        let mut element = match &self.source {
            Some(source) if realizer(source).local_name() == name => source.clone(),
            _ => template(name),
        };
        let realizer = realizer_mut(&mut element);

        if let NodeKind::Generic(configuration) = &self.kind {
            set(realizer, "configuration", configuration);
        }
        write_part(realizer, "Geometry", &[], &self.geometry);
        write_part(realizer, "Fill", &["Geometry"], &self.fill);
        write_part(realizer, "BorderStyle", &["Geometry", "Fill"], &self.border);
        let label_name = qualified(realizer, "NodeLabel");
        let labels = self
            .labels
            .iter()
            .map(|label| label.to_element(&label_name));
        replace_children(realizer, "NodeLabel", BEFORE_LABELS, labels.collect());
        if let NodeKind::Shape(shape) = &self.kind {
            write_part(realizer, "Shape", BEFORE_SHAPE, shape);
        }
        if let NodeKind::Svg { resource } = &self.kind {
            let model = child_or_insert(realizer, "SVGModel", BEFORE_SVG_MODEL);
            let content = child_or_insert(model, "SVGContent", &[]);
            match resource {
                Some(id) => set(content, "refid", id),
                None => {
                    content.attributes.remove("refid");
                }
            }
        }
        element
    }
}

/// The `y:SVGContent` of an SVG node's realizer.
fn svg_content(realizer: &Element) -> Option<&Element> {
    realizer.child("SVGModel")?.child("SVGContent")
}

/// A new node element with the parts yEd always writes.
fn template(name: &str) -> Element {
    let border = Element::new("y:BorderStyle");
    // yEd leaves it out on SVG and generic nodes.
    let border = match name {
        "SVGNode" | "GenericNode" => border,
        _ => border.with_attr("raised", "false"),
    };
    let element = Element::new(format!("y:{name}"))
        .with_child(Element::new("y:Geometry"))
        .with_child(Element::new("y:Fill").with_attr("transparent", "false"))
        .with_child(border);
    match name {
        "ShapeNode" => element.with_child(Element::new("y:Shape")),
        "SVGNode" => element
            .with_child(Element::new("y:SVGNodeProperties").with_attr("usingVisualBounds", "true"))
            .with_child(
                Element::new("y:SVGModel")
                    .with_attr("svgBoundsPolicy", "0")
                    .with_child(Element::new("y:SVGContent")),
            ),
        _ => element,
    }
}

/// Group nodes wrap their graphics in a `y:ProxyAutoBoundsNode`, with one
/// realizer per state (open, closed) and an index to the one in use.
fn active_realizer(element: &Element) -> Option<usize> {
    let realizers = element.child("Realizers")?;
    let active = get_or(realizers, "active", 0u32).ok()? as usize;
    (active < realizers.elements().count()).then_some(active)
}

/// The element holding the graphics: `element` itself, or the realizer a
/// group node is using.
fn realizer(element: &Element) -> &Element {
    active_realizer(element)
        .and_then(|active| element.child("Realizers")?.elements().nth(active))
        .unwrap_or(element)
}

fn realizer_mut(element: &mut Element) -> &mut Element {
    let Some(active) = active_realizer(element) else {
        return element;
    };
    element
        .child_mut("Realizers")
        .and_then(|r| r.elements_mut().nth(active))
        .expect("checked by `active_realizer`")
}
