use crate::tool::Tool;

/// Something asked for from the menu, the toolbar or a keyboard shortcut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    New,
    Open,
    Save,
    SelectTool(Tool),
}
