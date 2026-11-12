use graphml::yed::ShapeType;

use crate::style::Style;

/// How a draw.io vertex is drawn in yEd's terms.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Shape {
    /// A rectangle, or with a radius, a round rectangle.
    Box { radius: f64 },
    /// Another of yEd's shape nodes.
    Yed(ShapeType),
    /// A yEd generic node, by configuration, e.g. a flowchart symbol.
    Generic(&'static str),
    /// An image, by its `image` style value.
    Image(String),
    /// A line through the middle of its bounds, across or (`vertical`) down.
    Line { vertical: bool },
    /// A container with a title bar `start` deep, at the top or (`!horizontal`) on the left.
    Swimlane { start: f64, horizontal: bool },
    /// A sequence diagram's lifeline: a box `head` tall above a line.
    Lifeline { head: f64 },
    /// An invisible container, drawn only if lines end on it.
    Group,
    /// A box standing in for a shape yEd has nothing like, by name.
    Unknown(String),
}

/// The shape of a vertex with this style and size.
pub(crate) fn shape(style: &Style, width: f64, height: f64) -> Shape {
    let name = style.get("shape").unwrap_or("label");
    let icon = style.names.iter().any(|n| n == "icon");
    match (name, style.get("image")) {
        ("image", Some(image)) => return Shape::Image(image.to_owned()),
        ("label", Some(image)) if icon => return Shape::Image(image.to_owned()),
        ("image", None) => return Shape::Unknown("image".to_owned()),
        _ => {}
    }
    if style.names.iter().any(|n| n == "group") && !visible(style) {
        return Shape::Group;
    }
    let generic = |configuration: &'static str| Shape::Generic(configuration);
    match name {
        "label"
        | "rect"
        | "rectangle"
        | "text"
        | "ext"
        | "partialRectangle"
        | "tableRow"
        | "mxgraph.flowchart.process"
        | "mxgraph.basic.rect" => Shape::Box {
            radius: corner_radius(style, width, height),
        },
        "ellipse"
        | "doubleEllipse"
        | "mxgraph.flowchart.or"
        | "mxgraph.flowchart.summing_function"
        | "mxgraph.basic.ellipse" => Shape::Yed(ShapeType::Ellipse),
        "rhombus" | "mxgraph.flowchart.decision" => Shape::Yed(ShapeType::Diamond),
        "triangle" | "mxgraph.flowchart.extract_or_measurement" => Shape::Yed(ShapeType::Triangle),
        "hexagon" | "mxgraph.flowchart.preparation" => Shape::Yed(ShapeType::Hexagon),
        "parallelogram" | "mxgraph.flowchart.data" => Shape::Yed(ShapeType::Parallelogram),
        // yEd's trapezoid is narrow at the top, like draw.io's unturned one.
        "trapezoid" => match turned_upside_down(style) {
            true => Shape::Yed(ShapeType::Trapezoid2),
            false => Shape::Yed(ShapeType::Trapezoid),
        },
        "mxgraph.basic.octagon2" => Shape::Yed(ShapeType::Octagon),
        "cylinder" | "cylinder2" | "cylinder3" | "datastore" | "mxgraph.flowchart.database" => {
            generic("com.yworks.flowchart.dataBase")
        }
        "document" | "mxgraph.flowchart.document" => generic("com.yworks.flowchart.document"),
        "cloud" => generic("com.yworks.flowchart.cloud"),
        "process" | "mxgraph.flowchart.predefined_process" => {
            generic("com.yworks.flowchart.predefinedProcess")
        }
        "card" | "mxgraph.flowchart.card" => generic("com.yworks.flowchart.card"),
        "tape" | "mxgraph.flowchart.paper_tape" => generic("com.yworks.flowchart.paperType"),
        "internalStorage" | "mxgraph.flowchart.internal_storage" => {
            generic("com.yworks.flowchart.internalStorage")
        }
        "manualInput" | "mxgraph.flowchart.manual_input" => {
            generic("com.yworks.flowchart.manualInput")
        }
        "delay" | "mxgraph.flowchart.delay" => generic("com.yworks.flowchart.delay"),
        "display" | "mxgraph.flowchart.display" => generic("com.yworks.flowchart.display"),
        "mxgraph.flowchart.terminator" => generic("com.yworks.flowchart.terminator"),
        "mxgraph.flowchart.manual_operation" => generic("com.yworks.flowchart.manualOperation"),
        "mxgraph.flowchart.loop_limit" => generic("com.yworks.flowchart.loopLimit"),
        "dataStorage" | "mxgraph.flowchart.stored_data" => {
            generic("com.yworks.flowchart.storedData")
        }
        "mxgraph.flowchart.sequential_data" => generic("com.yworks.flowchart.sequentialData"),
        "mxgraph.flowchart.direct_data" => generic("com.yworks.flowchart.directData"),
        "offPageConnector" | "mxgraph.flowchart.off-page_reference" => {
            generic("com.yworks.flowchart.offPageReference")
        }
        "mxgraph.flowchart.on-page_reference" => generic("com.yworks.flowchart.onPageReference"),
        "mxgraph.flowchart.annotation_1" | "mxgraph.flowchart.annotation_2" => {
            generic("com.yworks.flowchart.annotation")
        }
        "mxgraph.flowchart.start_1" => generic("com.yworks.flowchart.start1"),
        "mxgraph.flowchart.start_2" => generic("com.yworks.flowchart.start2"),
        "line" => Shape::Line {
            vertical: matches!(style.get("direction"), Some("north" | "south")),
        },
        // draw.io's tables are swimlanes too.
        "swimlane" | "table" => Shape::Swimlane {
            start: style.number("startSize", 40.0).max(0.0),
            horizontal: style.get("horizontal") != Some("0"),
        },
        "umlLifeline" => Shape::Lifeline {
            head: style.number("size", 40.0).clamp(0.0, height),
        },
        other => Shape::Unknown(other.to_owned()),
    }
}

/// Whether the style draws anything of the vertex itself.
pub(crate) fn visible(style: &Style) -> bool {
    style.get("fillColor").is_some() || style.get("strokeColor").is_some()
}

/// Whether a shape is drawn upside down: flipped, or turned half round.
fn turned_upside_down(style: &Style) -> bool {
    style.flag("flipV") != style.is("direction", "west")
}

/// The radius draw.io rounds a box's corners to: `arcSize` percent of its
/// shorter side, or with `absoluteArcSize`, half of `arcSize`.
fn corner_radius(style: &Style, width: f64, height: f64) -> f64 {
    if !style.flag("rounded") {
        return 0.0;
    }
    let radius = if style.flag("absoluteArcSize") {
        style.number("arcSize", 20.0) / 2.0
    } else {
        style.number("arcSize", 15.0) / 100.0 * width.min(height)
    };
    radius.clamp(0.0, width.min(height) / 2.0)
}
