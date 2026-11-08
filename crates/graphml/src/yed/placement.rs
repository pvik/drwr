use super::codec::{get_or, qualified, set};
use super::edge_placement::SmartEdge;
use super::{Point, Rect};
use crate::{
    Error,
    xml::{Element, XmlNode},
};

/// Where yEd puts a node label: inside its node, or outside around it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LabelPosition {
    Center,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    North,
    South,
    East,
    West,
    NorthWest,
    NorthEast,
    SouthWest,
    SouthEast,
}

/// Where a label is along one axis of its node.
#[derive(Clone, Copy, PartialEq)]
enum Spot {
    /// Outside, before the node's start.
    Before,
    /// Inside, at the start.
    Start,
    Middle,
    /// Inside, at the end.
    End,
    /// Outside, past the node's end.
    After,
}

impl LabelPosition {
    const ALL: [LabelPosition; 17] = [
        LabelPosition::Center,
        LabelPosition::Top,
        LabelPosition::Bottom,
        LabelPosition::Left,
        LabelPosition::Right,
        LabelPosition::TopLeft,
        LabelPosition::TopRight,
        LabelPosition::BottomLeft,
        LabelPosition::BottomRight,
        LabelPosition::North,
        LabelPosition::South,
        LabelPosition::East,
        LabelPosition::West,
        LabelPosition::NorthWest,
        LabelPosition::NorthEast,
        LabelPosition::SouthWest,
        LabelPosition::SouthEast,
    ];

    /// The `modelPosition`, and where the label is across and down.
    fn layout(self) -> (&'static str, Spot, Spot) {
        use Spot::*;
        match self {
            LabelPosition::Center => ("c", Middle, Middle),
            LabelPosition::Top => ("t", Middle, Start),
            LabelPosition::Bottom => ("b", Middle, End),
            LabelPosition::Left => ("l", Start, Middle),
            LabelPosition::Right => ("r", End, Middle),
            LabelPosition::TopLeft => ("tl", Start, Start),
            LabelPosition::TopRight => ("tr", End, Start),
            LabelPosition::BottomLeft => ("bl", Start, End),
            LabelPosition::BottomRight => ("br", End, End),
            LabelPosition::North => ("n", Middle, Before),
            LabelPosition::South => ("s", Middle, After),
            LabelPosition::East => ("e", After, Middle),
            LabelPosition::West => ("w", Before, Middle),
            LabelPosition::NorthWest => ("nw", Before, Before),
            LabelPosition::NorthEast => ("ne", After, Before),
            LabelPosition::SouthWest => ("sw", Before, After),
            LabelPosition::SouthEast => ("se", After, After),
        }
    }

    fn from_keyword(keyword: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|position| position.layout().0 == keyword)
    }
}

/// How yEd places a label: by its model and its position in it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Placement {
    /// A position in a node label model with fixed ones.
    Fixed(Model, LabelPosition),
    Smart(Smart),
    /// By the Smart model for edge labels.
    Edge(SmartEdge),
    /// Any other placement, such as a free one, kept as written.
    Other,
}

/// yEd's node label models with fixed positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Model {
    Internal,
    Corners,
    Sandwich,
    Sides,
    EightPos,
}

/// A placement by yEd's Smart model, a `custom` one, which puts the point of
/// the label at `label_ratio` of its size from its center `offset` away from
/// the point of the node at `node_ratio` of its size from its center.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Smart {
    /// How far the model's fixed positions are from the node's sides.
    distance: f64,
    label_ratio: Point,
    node_ratio: Point,
    offset: Point,
    /// The label's up direction, which is (0, -1) unless it's rotated.
    up: Point,
}

impl Placement {
    /// Where yEd puts a new node label.
    pub(crate) const NEW: Placement = Placement::Fixed(Model::Internal, LabelPosition::Center);

    pub(crate) fn read(label: &Element) -> Result<Self, Error> {
        if label.local_name() == "EdgeLabel" {
            return Ok(SmartEdge::read(label)?.map_or(Placement::Other, Placement::Edge));
        }
        let name = label.attr("modelName");
        if name == Some("custom") {
            return Ok(Smart::read(label)?.map_or(Placement::Other, Placement::Smart));
        }
        let model = Model::ALL
            .into_iter()
            .find(|model| Some(model.keyword()) == name);
        let position = label
            .attr("modelPosition")
            .and_then(LabelPosition::from_keyword);
        Ok(match model.zip(position) {
            Some((model, position)) if model.has(position) => Placement::Fixed(model, position),
            _ => Placement::Other,
        })
    }

    pub(crate) fn write(&self, label: &mut Element) {
        match self {
            Placement::Fixed(model, position) => {
                set(label, "modelName", &model.keyword().to_owned());
                set(label, "modelPosition", &position.layout().0.to_owned());
            }
            Placement::Smart(smart) => {
                set(label, "modelName", &"custom".to_owned());
                label.attributes.remove("modelPosition");
                smart.write(label);
            }
            Placement::Edge(edge) => edge.write(label),
            Placement::Other => {}
        }
    }

    pub(crate) fn position(&self) -> Option<LabelPosition> {
        match self {
            Placement::Fixed(_, position) => Some(*position),
            Placement::Smart(smart) => LabelPosition::ALL
                .into_iter()
                .find(|&position| smart.at(position) == *smart),
            Placement::Edge(_) | Placement::Other => None,
        }
    }

    /// The same placement at `position`, in the same model if it has the
    /// position, or else in the Smart model, which has them all.
    pub(crate) fn at(self, position: LabelPosition) -> Self {
        match self {
            Placement::Fixed(model, _) if model.has(position) => Placement::Fixed(model, position),
            Placement::Smart(smart) => Placement::Smart(smart.at(position)),
            Placement::Edge(_) => self,
            _ => Placement::Smart(Smart::default().at(position)),
        }
    }

    /// The bounds of a label `width` by `height` placed so on `node`,
    /// relative to the node's top-left corner.
    pub(crate) fn place(&self, node: &Rect, width: f64, height: f64) -> Option<Rect> {
        let smart = match *self {
            // The fixed models' default distance is the Smart model's.
            Placement::Fixed(_, position) => Smart::default().at(position),
            Placement::Smart(smart) => smart,
            Placement::Edge(_) | Placement::Other => return None,
        };
        let x = node.width * (0.5 + smart.node_ratio.x) + smart.offset.x
            - width * (0.5 + smart.label_ratio.x);
        let y = node.height * (0.5 + smart.node_ratio.y) + smart.offset.y
            - height * (0.5 + smart.label_ratio.y);
        Some(Rect {
            x,
            y,
            width,
            height,
        })
    }
}

impl Model {
    const ALL: [Model; 5] = [
        Model::Internal,
        Model::Corners,
        Model::Sandwich,
        Model::Sides,
        Model::EightPos,
    ];

    /// The `modelName`.
    fn keyword(self) -> &'static str {
        match self {
            Model::Internal => "internal",
            Model::Corners => "corners",
            Model::Sandwich => "sandwich",
            Model::Sides => "sides",
            Model::EightPos => "eight_pos",
        }
    }

    fn has(self, position: LabelPosition) -> bool {
        let (_, x, y) = position.layout();
        let outside = |spot| matches!(spot, Spot::Before | Spot::After);
        match self {
            Model::Internal => !outside(x) && !outside(y),
            Model::Corners => outside(x) && outside(y),
            Model::Sandwich => x == Spot::Middle && outside(y),
            Model::Sides => outside(x) != outside(y),
            Model::EightPos => outside(x) || outside(y),
        }
    }
}

impl Default for Smart {
    /// In the middle of the node, like yEd's new labels.
    fn default() -> Self {
        Smart {
            distance: 4.0,
            label_ratio: Point::default(),
            node_ratio: Point::default(),
            offset: Point::default(),
            up: Point { x: 0.0, y: -1.0 },
        }
    }
}

impl Smart {
    /// Reads the Smart model and its parameter in `label`, if it has them.
    fn read(label: &Element) -> Result<Option<Self>, Error> {
        let inner = |outer, inner| label.child(outer).and_then(|outer| outer.child(inner));
        let model = inner("LabelModel", "SmartNodeLabelModel");
        let parameter = inner("ModelParameter", "SmartNodeLabelModelParameter");
        let (Some(model), Some(parameter)) = (model, parameter) else {
            return Ok(None);
        };
        let default = Smart::default();
        let point = |x, y, default: Point| -> Result<Point, Error> {
            Ok(Point {
                x: get_or(parameter, x, default.x)?,
                y: get_or(parameter, y, default.y)?,
            })
        };
        Ok(Some(Smart {
            distance: get_or(model, "distance", default.distance)?,
            label_ratio: point("labelRatioX", "labelRatioY", default.label_ratio)?,
            node_ratio: point("nodeRatioX", "nodeRatioY", default.node_ratio)?,
            offset: point("offsetX", "offsetY", default.offset)?,
            up: point("upX", "upY", default.up)?,
        }))
    }

    fn write(&self, label: &mut Element) {
        let model = inner_mut(label, "LabelModel", "SmartNodeLabelModel");
        set(model, "distance", &self.distance);
        let parameter = inner_mut(label, "ModelParameter", "SmartNodeLabelModelParameter");
        for (name, value) in [
            ("labelRatioX", self.label_ratio.x),
            ("labelRatioY", self.label_ratio.y),
            ("nodeRatioX", self.node_ratio.x),
            ("nodeRatioY", self.node_ratio.y),
            ("offsetX", self.offset.x),
            ("offsetY", self.offset.y),
            ("upX", self.up.x),
            ("upY", self.up.y),
        ] {
            set(parameter, name, &value);
        }
    }

    /// The same model and rotation at one of the fixed positions.
    fn at(self, position: LabelPosition) -> Self {
        let (_, x, y) = position.layout();
        let ((node_x, label_x, offset_x), (node_y, label_y, offset_y)) =
            (self.anchors(x), self.anchors(y));
        Smart {
            label_ratio: Point {
                x: label_x,
                y: label_y,
            },
            node_ratio: Point {
                x: node_x,
                y: node_y,
            },
            offset: Point {
                x: offset_x,
                y: offset_y,
            },
            ..self
        }
    }

    /// The node's and the label's ratios, and the offset, that put a label at
    /// `spot` along an axis, as yEd does.
    fn anchors(&self, spot: Spot) -> (f64, f64, f64) {
        let distance = self.distance;
        match spot {
            Spot::Before => (-0.5, 0.5, -distance),
            Spot::Start => (-0.5, -0.5, distance),
            Spot::Middle => (0.0, 0.0, 0.0),
            Spot::End => (0.5, 0.5, -distance),
            Spot::After => (0.5, -0.5, distance),
        }
    }
}

/// The element `inner` inside `label`'s child `outer`. If `outer` holds no
/// such element, it's replaced by one that does, after the other children.
pub(crate) fn inner_mut<'a>(label: &'a mut Element, outer: &str, inner: &str) -> &'a mut Element {
    if label.child(outer).and_then(|o| o.child(inner)).is_none() {
        let element =
            Element::new(qualified(label, outer)).with_child(Element::new(qualified(label, inner)));
        label
            .children
            .retain(|node| node.as_element().is_none_or(|e| e.local_name() != outer));
        label.children.push(XmlNode::Element(element));
    }
    let outer = label.child_mut(outer).expect("added if missing");
    outer.child_mut(inner).expect("added if missing")
}
