use graphml::yed::{Color, Font, LabelPosition, LineType};

use crate::document::{ColorProperty, LabelRef};
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
    /// Moves the selected node right and down by these distances, in points.
    MoveBy(f64, f64),
    /// Opens the color picker on a color of the selected node.
    EditColor(ColorProperty),
    /// Sets a color of the selected node; `None` removes it.
    SetColor(ColorProperty, Option<Color>),
    /// Sets the border width of the selected node.
    SetBorderWidth(f64),
    /// Sets the line type of the selected node's border.
    SetBorderType(LineType),
    /// Edits the selected node's first label on the canvas.
    EditLabel,
    EditLabelInDialog(LabelRef),
    SetLabel(LabelRef, String),
    SetFont(LabelRef, Font),
    SetLabelPosition(LabelRef, LabelPosition),
}
