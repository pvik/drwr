use graphml::yed::{Color, FontStyle, LineType};

use crate::document::{ColorProperty, LabelRef};
use crate::tool::Tool;

/// Something the user asked for, e.g. from the menu or a keyboard shortcut.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    New,
    Open,
    Save,
    SelectTool(Tool),
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
    SetFontFamily(LabelRef, String),
    SetFontStyle(LabelRef, FontStyle),
    /// Sets a label's font size, in points.
    SetFontSize(LabelRef, u32),
}
