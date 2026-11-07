use graphml::yed::Color;

use crate::document::{ColorProperty, LabelRef};
use crate::tool::Tool;

/// Something the user asked for, e.g. from the menu or a keyboard shortcut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    New,
    Open,
    Save,
    SelectTool(Tool),
    /// Opens the color picker on a color of the selected node.
    EditColor(ColorProperty),
    /// Sets a color of the selected node; `None` removes it.
    SetColor(ColorProperty, Option<Color>),
    /// Edits the selected node's first label on the canvas.
    EditLabel,
    EditLabelInDialog(LabelRef),
    SetLabel(LabelRef, String),
}
