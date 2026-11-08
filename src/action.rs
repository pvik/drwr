use graphml::yed::{Color, Font, LabelPosition, LineType};

use crate::document::{ColorProperty, EdgeEnd, LabelRef};
use crate::tool::Tool;

/// Something the user asked for, e.g. from the menu or a keyboard shortcut.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    New,
    Open,
    Save,
    SelectTool(Tool),
    /// Zooms the canvas in to the next zoom level.
    ZoomIn,
    ZoomOut,
    /// Zooms the canvas to this level, e.g. 1.5 for 150%.
    SetZoom(f32),
    /// Moves the view right and down over the graph by these distances, in
    /// screen points.
    PanBy(f32, f32),
    /// Moves the selected node, or the selected edge's selected bend or else
    /// all its bends, right and down by these distances, in points.
    MoveBy(f64, f64),
    /// Removes the selected bend, or else the selected edge or node, a node
    /// with its edges.
    Delete,
    /// Opens the color picker on a color of the selected node or edge.
    EditColor(ColorProperty),
    /// Sets a color of the selected node or edge; `None` removes it.
    SetColor(ColorProperty, Option<Color>),
    /// Sets the width of the selected node's border or edge's line.
    SetLineWidth(f64),
    /// Sets the line type of the selected node's border or edge's line.
    SetLineType(LineType),
    /// Rounds the corners of the selected box to this radius.
    SetCornerRadius(f64),
    /// Sets whether an end of the selected edge has an arrowhead.
    SetArrow(EdgeEnd, bool),
    /// Edits the first label of the selected node or edge on the canvas.
    EditLabel,
    EditLabelInDialog(LabelRef),
    SetLabel(LabelRef, String),
    SetFont(LabelRef, Font),
    SetLabelPosition(LabelRef, LabelPosition),
}
