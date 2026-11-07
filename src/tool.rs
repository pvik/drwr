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
