/// A tool from the toolbar. Exactly one is selected at any time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    /// The plain pointer, which doesn't draw anything. Selected at startup.
    #[default]
    Cursor,
    Box,
    Circle,
    Ellipse,
    TextBox,
}
