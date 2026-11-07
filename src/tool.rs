use graphml::yed::ShapeType;

/// A drawing tool. Exactly one is selected at a time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Cursor,
    Box,
    Circle,
    Ellipse,
    Line,
    Arrow,
    TextBox,
}

/// The shape of the nodes a tool draws by dragging them out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeShape {
    pub shape: ShapeType,
    /// Whether the node is kept as wide as it's high, like a circle.
    pub square: bool,
}

impl Tool {
    /// The shape of the nodes this tool draws, if it draws nodes.
    pub fn node_shape(self) -> Option<NodeShape> {
        let (shape, square) = match self {
            Tool::Box => (ShapeType::Rectangle, false),
            Tool::Circle => (ShapeType::Ellipse, true),
            Tool::Ellipse => (ShapeType::Ellipse, false),
            Tool::Cursor | Tool::Line | Tool::Arrow | Tool::TextBox => return None,
        };
        Some(NodeShape { shape, square })
    }
}
