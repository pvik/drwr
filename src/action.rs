use graphml::yed::{Color, FontStyle, LabelPosition, LineType};

use crate::document::{ColorProperty, EdgeEnd, LabelRef};
use crate::tool::Tool;

/// Something the user asked for, e.g. from the menu or a keyboard shortcut.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    New,
    Open,
    Save,
    SaveAs,
    Quit,
    /// Undoes the last change to the graph, selecting what was selected
    /// before it.
    Undo,
    /// Redoes the last change undone, selecting what was selected after it.
    Redo,
    /// Copies, then removes what's selected.
    Cut,
    /// Puts the selected nodes, with the selected edges between them, on the
    /// clipboard.
    Copy,
    /// Asks for the clipboard's text, which comes back next frame as a
    /// [`Action::PasteText`].
    Paste,
    /// Pastes the nodes and edges of this GraphML; other text does nothing.
    PasteText(String),
    SelectTool(Tool),
    /// Zooms the canvas in to the next zoom level.
    ZoomIn,
    ZoomOut,
    /// Zooms the canvas to this level, e.g. 1.5 for 150%.
    SetZoom(f32),
    /// Moves the view right and down over the graph by these distances, in
    /// screen points.
    PanBy(f32, f32),
    /// Moves the selected bend, or else everything selected, right and down
    /// by these distances, in points; see [`crate::document::move_items`].
    MoveBy(f64, f64),
    /// Removes the selected bend, or else everything selected, nodes with
    /// their edges.
    Delete,
    /// Opens the color picker on a color of the selected nodes and edges.
    EditColor(ColorProperty),
    /// Sets a color of the selected nodes and edges; `None` removes it.
    SetColor(ColorProperty, Option<Color>),
    /// Sets the width of the selected nodes' borders and edges' lines.
    SetLineWidth(f64),
    /// Sets the line type of the selected nodes' borders and edges' lines.
    SetLineType(LineType),
    /// Rounds the corners of the selected boxes to this radius.
    SetCornerRadius(f64),
    /// Sets whether an end of the selected edges has an arrowhead.
    SetArrow(EdgeEnd, bool),
    /// Edits the first label of the selected node or edge on the canvas, if
    /// it's the only one selected.
    EditLabel,
    /// Opens the label dialog on these labels, which all get the text typed.
    EditLabelInDialog(Vec<LabelRef>),
    SetLabel(LabelRef, String),
    SetFont(LabelRef, FontPart),
    SetLabelPosition(LabelRef, LabelPosition),
}

/// A part of a label's font, which leaves the other parts as they are.
#[derive(Clone, Debug, PartialEq)]
pub enum FontPart {
    Family(String),
    Style(FontStyle),
    Size(u32),
}

/// Which commands the menu and the toolbar offer now.
#[derive(Clone, Copy, Debug)]
pub struct Enabled {
    /// New, Open, Save and Save As, unavailable while a file dialog is open.
    pub file: bool,
    /// Cut and Copy, which need nodes selected.
    pub copy: bool,
    pub undo: bool,
    pub redo: bool,
}
