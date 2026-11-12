use graphml::{
    xml::{Attributes, Element},
    yed::Point,
};

/// The cells of a page, in the order draw.io paints them.
#[derive(Clone, Debug, Default)]
pub(crate) struct Model {
    pub cells: Vec<Cell>,
}

/// An `<mxCell>`: a layer, a shape (vertex), or a line (edge).
#[derive(Clone, Debug, Default)]
pub(crate) struct Cell {
    pub id: String,
    pub parent: Option<String>,
    /// The label, HTML if the style says `html=1`.
    pub value: String,
    pub style: String,
    pub vertex: bool,
    pub edge: bool,
    pub source: Option<String>,
    pub target: Option<String>,
    pub visible: bool,
    pub collapsed: bool,
    pub geometry: Geometry,
    /// The attributes of a `<UserObject>` or `<object>` wrapping the cell,
    /// such as `link`, `tooltip` and custom properties.
    pub properties: Attributes,
}

/// An `<mxGeometry>`, relative to the cell's parent.
#[derive(Clone, Debug, Default)]
pub(crate) struct Geometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// For an edge's label, `x` runs from -1 to 1 along the edge and `y` is
    /// the distance from it; for a vertex, `x` and `y` are fractions of its
    /// parent's size.
    pub relative: bool,
    /// An edge's control points.
    pub points: Vec<Point>,
    /// Where an edge without a source starts.
    pub source_point: Option<Point>,
    pub target_point: Option<Point>,
    pub offset: Option<Point>,
}

impl Model {
    /// Reads the cells of an `<mxGraphModel>`.
    pub fn read(model: &Element) -> Self {
        let cells = model.child("root").into_iter().flat_map(|root| {
            root.elements()
                .filter_map(|element| match element.local_name() {
                    "mxCell" => Some(Cell::read(element, None)),
                    "UserObject" | "object" => {
                        Some(Cell::read(element.child("mxCell")?, Some(element)))
                    }
                    _ => None,
                })
        });
        Model {
            cells: cells.collect(),
        }
    }
}

impl Cell {
    fn read(cell: &Element, wrapper: Option<&Element>) -> Self {
        let attr = |name| cell.attr(name).map(str::to_owned);
        let mut properties = wrapper.map(|w| w.attributes.clone()).unwrap_or_default();
        let id = properties.remove("id").or_else(|| attr("id"));
        let value = match wrapper {
            Some(_) => properties.remove("label"),
            None => attr("value"),
        };
        Cell {
            id: id.unwrap_or_default(),
            parent: attr("parent"),
            value: value.unwrap_or_default(),
            style: attr("style").unwrap_or_default(),
            vertex: cell.attr("vertex") == Some("1"),
            edge: cell.attr("edge") == Some("1"),
            source: attr("source"),
            target: attr("target"),
            visible: cell.attr("visible") != Some("0"),
            collapsed: cell.attr("collapsed") == Some("1"),
            geometry: cell
                .children_named("mxGeometry")
                .find(|g| g.attr("as").is_none_or(|a| a == "geometry"))
                .map(Geometry::read)
                .unwrap_or_default(),
            properties,
        }
    }
}

impl Geometry {
    fn read(geometry: &Element) -> Self {
        let named = |name: &str| {
            let found = geometry.children_named("mxPoint");
            found
                .filter(|p| p.attr("as") == Some(name))
                .map(point)
                .next()
        };
        let points = geometry
            .children_named("Array")
            .find(|array| array.attr("as") == Some("points"))
            .map(|array| array.children_named("mxPoint").map(point).collect());
        Geometry {
            x: number(geometry, "x"),
            y: number(geometry, "y"),
            width: number(geometry, "width"),
            height: number(geometry, "height"),
            relative: geometry.attr("relative") == Some("1"),
            points: points.unwrap_or_default(),
            source_point: named("sourcePoint"),
            target_point: named("targetPoint"),
            offset: named("offset"),
        }
    }
}

fn point(element: &Element) -> Point {
    Point {
        x: number(element, "x"),
        y: number(element, "y"),
    }
}

/// An attribute's number, 0 if it's missing or unreadable, as in draw.io.
fn number(element: &Element, name: &str) -> f64 {
    let value = element
        .attr(name)
        .and_then(|v| v.trim().parse::<f64>().ok());
    value.filter(|v| v.is_finite()).unwrap_or(0.0)
}
