use crate::document::Item;

/// What's selected: a node or an edge, and maybe one of the edge's bends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub item: Item,
    /// The index of the selected bend of the selected edge.
    pub bend: Option<usize>,
}

impl Selection {
    pub fn new(item: Item) -> Self {
        Selection { item, bend: None }
    }

    pub fn node(id: impl Into<String>) -> Self {
        Selection::new(Item::Node(id.into()))
    }

    pub fn edge(index: usize) -> Self {
        Selection::new(Item::Edge(index))
    }
}
