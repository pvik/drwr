use crate::document::Item;

/// What's selected: any nodes and edges, and maybe one bend of an edge
/// selected on its own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// In the order they were selected.
    items: Vec<Item>,
    /// The index of the selected bend of the only item, an edge.
    bend: Option<usize>,
}

impl Selection {
    /// Selects `items`, each once.
    pub fn of(items: impl IntoIterator<Item = Item>) -> Self {
        let mut selection = Selection::default();
        for item in items {
            selection.add(item);
        }
        selection
    }

    pub fn node(id: impl Into<String>) -> Self {
        Selection::of([Item::Node(id.into())])
    }

    pub fn edge(index: usize) -> Self {
        Selection::of([Item::Edge(index)])
    }

    /// The edge at `index`, with its bend at index `bend`.
    #[cfg(test)]
    pub fn bend_of(index: usize, bend: usize) -> Self {
        Selection {
            items: vec![Item::Edge(index)],
            bend: Some(bend),
        }
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn contains(&self, item: &Item) -> bool {
        self.items.contains(item)
    }

    pub fn contains_node(&self, id: &str) -> bool {
        self.nodes().any(|node| node == id)
    }

    pub fn contains_edge(&self, index: usize) -> bool {
        self.edges().any(|edge| edge == index)
    }

    /// The item selected, if it's the only one.
    pub fn single(&self) -> Option<&Item> {
        match &self.items[..] {
            [item] => Some(item),
            _ => None,
        }
    }

    /// The ids of the selected nodes.
    pub fn nodes(&self) -> impl Iterator<Item = &str> {
        self.items.iter().filter_map(Item::node_id)
    }

    /// The indexes of the selected edges.
    pub fn edges(&self) -> impl Iterator<Item = usize> {
        self.items.iter().filter_map(Item::edge_index)
    }

    pub fn has_nodes(&self) -> bool {
        self.nodes().next().is_some()
    }

    pub fn has_edges(&self) -> bool {
        self.edges().next().is_some()
    }

    /// The selected bend of the only item, an edge.
    pub fn bend(&self) -> Option<usize> {
        self.bend
    }

    /// Selects the bend at index `bend` of the only item, if it's an edge.
    pub fn select_bend(&mut self, bend: usize) {
        if let Some(Item::Edge(_)) = self.single() {
            self.bend = Some(bend);
        }
    }

    /// Adds `item`, unless it's selected already.
    pub fn add(&mut self, item: Item) {
        if !self.contains(&item) {
            self.items.push(item);
            self.bend = None;
        }
    }

    /// Adds `item`, or removes it if it's selected already.
    pub fn toggle(&mut self, item: Item) {
        match self.items.iter().position(|selected| *selected == item) {
            Some(index) => {
                self.items.remove(index);
                self.bend = None;
            }
            None => self.add(item),
        }
    }
}
