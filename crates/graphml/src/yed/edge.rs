use super::codec::{
    Part, get_or, keyword_enum, qualified, read_part, replace_children, set, write_part,
};
use super::{Label, Point, Stroke};
use crate::{Error, xml::Element};

/// The graphics of an edge: the element inside its `edgegraphics` data.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeGraphics {
    pub kind: EdgeKind,
    pub path: Path,
    pub line: Stroke,
    pub arrows: Arrows,
    pub labels: Vec<Label>,
    /// The element this was read from, which keeps what isn't modelled here.
    source: Option<Element>,
}

/// The kind of yEd edge, which decides how its path is drawn.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    /// A `y:PolyLineEdge`: straight segments between the bends.
    PolyLine,
    /// A `y:GenericEdge`, by its `configuration`.
    Generic(String),
    /// Any other type, by element name, e.g. `ArcEdge` or `BezierEdge`.
    Other(String),
}

impl EdgeKind {
    fn element_name(&self) -> &str {
        match self {
            EdgeKind::PolyLine => "PolyLineEdge",
            EdgeKind::Generic(_) => "GenericEdge",
            EdgeKind::Other(name) => name,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    /// Where the edge leaves the source node, relative to its center.
    pub source: Point,
    /// Where the edge meets the target node, relative to its center.
    pub target: Point,
    /// The bends in between.
    pub points: Vec<Point>,
}

impl Part for Path {
    fn read(element: &Element) -> Result<Self, Error> {
        Ok(Path {
            source: Point {
                x: get_or(element, "sx", 0.0)?,
                y: get_or(element, "sy", 0.0)?,
            },
            target: Point {
                x: get_or(element, "tx", 0.0)?,
                y: get_or(element, "ty", 0.0)?,
            },
            points: element
                .children_named("Point")
                .map(Point::read)
                .collect::<Result<_, _>>()?,
        })
    }

    fn write(&self, element: &mut Element) {
        set(element, "sx", &self.source.x);
        set(element, "sy", &self.source.y);
        set(element, "tx", &self.target.x);
        set(element, "ty", &self.target.y);
        if Path::read(element).is_ok_and(|path| path.points == self.points) {
            return;
        }
        let name = qualified(element, "Point");
        let points = self.points.iter().map(|point| {
            let mut element = Element::new(name.clone());
            point.write(&mut element);
            element
        });
        replace_children(element, "Point", &[], points.collect());
    }
}

impl Part for Point {
    fn read(element: &Element) -> Result<Self, Error> {
        Ok(Point {
            x: get_or(element, "x", 0.0)?,
            y: get_or(element, "y", 0.0)?,
        })
    }

    fn write(&self, element: &mut Element) {
        set(element, "x", &self.x);
        set(element, "y", &self.y);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arrows {
    pub source: Arrow,
    pub target: Arrow,
}

impl Part for Arrows {
    fn read(element: &Element) -> Result<Self, Error> {
        Ok(Arrows {
            source: get_or(element, "source", Arrow::None)?,
            target: get_or(element, "target", Arrow::None)?,
        })
    }

    fn write(&self, element: &mut Element) {
        set(element, "source", &self.source);
        set(element, "target", &self.target);
    }
}

keyword_enum! {
    /// An arrowhead.
    pub enum Arrow {
        None = "none",
        Standard = "standard",
        Delta = "delta",
        WhiteDelta = "white_delta",
        Diamond = "diamond",
        WhiteDiamond = "white_diamond",
        Short = "short",
        Plain = "plain",
        Concave = "concave",
        Convex = "convex",
        Circle = "circle",
        TransparentCircle = "transparent_circle",
        Dash = "dash",
        SkewedDash = "skewed_dash",
        TShape = "t_shape",
    }
}

const BEFORE_LABELS: &[&str] = &["Path", "LineStyle", "Arrows"];

impl EdgeGraphics {
    /// An edge like the ones yEd creates.
    pub fn new(kind: EdgeKind) -> Self {
        EdgeGraphics {
            kind,
            path: Path::default(),
            line: Stroke::default(),
            arrows: Arrows {
                source: Arrow::None,
                target: Arrow::Standard,
            },
            labels: Vec::new(),
            source: None,
        }
    }

    pub fn from_element(element: &Element) -> Result<Self, Error> {
        let kind = match element.local_name() {
            "PolyLineEdge" => EdgeKind::PolyLine,
            "GenericEdge" => EdgeKind::Generic(get_or(element, "configuration", String::new())?),
            name => EdgeKind::Other(name.to_owned()),
        };
        Ok(EdgeGraphics {
            kind,
            path: read_part(element, "Path")?,
            line: read_part(element, "LineStyle")?,
            arrows: read_part(element, "Arrows")?,
            labels: element
                .children_named("EdgeLabel")
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
            Some(source) if source.local_name() == name => source.clone(),
            _ => template(name),
        };

        if let EdgeKind::Generic(configuration) = &self.kind {
            set(&mut element, "configuration", configuration);
        }
        write_part(&mut element, "Path", &[], &self.path);
        write_part(&mut element, "LineStyle", &["Path"], &self.line);
        write_part(&mut element, "Arrows", &["Path", "LineStyle"], &self.arrows);
        let label_name = qualified(&element, "EdgeLabel");
        let labels = self
            .labels
            .iter()
            .map(|label| label.to_element(&label_name));
        replace_children(&mut element, "EdgeLabel", BEFORE_LABELS, labels.collect());
        element
    }
}

/// A new edge element with the parts yEd always writes.
fn template(name: &str) -> Element {
    let element = Element::new(format!("y:{name}"))
        .with_child(Element::new("y:Path"))
        .with_child(Element::new("y:LineStyle"))
        .with_child(Element::new("y:Arrows"));
    if name == "PolyLineEdge" {
        element.with_child(Element::new("y:BendStyle").with_attr("smoothed", "false"))
    } else {
        element
    }
}
