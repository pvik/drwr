//! Changes to a document, kept as copies of the parts they replaced, which
//! put back restore it exactly.

use std::collections::BTreeMap;

use graphml::xml::{Attributes, XmlNode};
use graphml::{Data, Edge, Graph, GraphMl, Node};

use crate::scene::{Changed, Run};

/// The parts of a document a change replaced. Swapping them back undoes it,
/// and swapping again redoes it.
#[derive(Clone, Debug, PartialEq)]
pub enum Delta {
    /// The whole document, for changes that add or remove its graphs.
    Whole(Box<GraphMl>),
    Parts {
        /// The document without the top-level graph's nodes and edges, if
        /// anything else in it changed, e.g. its keys.
        rest: Option<Box<GraphMl>>,
        nodes: Vec<Splice<Node>>,
        edges: Vec<Splice<Edge>>,
    },
}

/// Items that go in place of others in a list. A delta has one splice per
/// list, or runs that replace as many items as they have.
#[derive(Clone, Debug, PartialEq)]
pub struct Splice<T> {
    /// Where the items go.
    at: usize,
    /// How many items they replace.
    len: usize,
    items: Vec<T>,
}

impl Delta {
    /// Makes `old` the same as `new`, returning the parts that turn it back.
    pub fn update(old: &mut GraphMl, new: &GraphMl) -> Option<Delta> {
        if old.graphs.is_empty() || new.graphs.is_empty() {
            let replaced = (old != new).then(|| std::mem::replace(old, new.clone()));
            return replaced.map(|replaced| Delta::Whole(Box::new(replaced)));
        }
        let rest = (!same_rest(old, new)).then(|| {
            let mut rest = rest_of(new);
            swap_rest(old, &mut rest);
            Box::new(rest)
        });
        let (old, new) = (&mut old.graphs[0], &new.graphs[0]);
        let nodes = update_list(&mut old.nodes, &new.nodes);
        let edges = update_list(&mut old.edges, &new.edges);
        let unchanged = rest.is_none() && nodes.is_empty() && edges.is_empty();
        (!unchanged).then_some(Delta::Parts { rest, nodes, edges })
    }

    /// Puts its parts in `doc`, keeping the ones they replace instead.
    pub fn swap(&mut self, doc: &mut GraphMl) {
        match self {
            Delta::Whole(whole) => std::mem::swap(doc, whole),
            Delta::Parts { rest, nodes, edges } => {
                if let Some(rest) = rest {
                    swap_rest(doc, rest);
                }
                let top = &mut doc.graphs[0];
                for splice in nodes {
                    splice.swap(&mut top.nodes);
                }
                for splice in edges {
                    splice.swap(&mut top.edges);
                }
            }
        }
    }

    /// Copies the parts it just swapped into `from` to `to`, which was the
    /// same as `from` before.
    pub fn copy_changes(&self, from: &GraphMl, to: &mut GraphMl) {
        match self {
            Delta::Whole(_) => *to = from.clone(),
            Delta::Parts { rest, nodes, edges } => {
                if rest.is_some() {
                    swap_rest(to, &mut rest_of(from));
                }
                let (from, to) = (&from.graphs[0], &mut to.graphs[0]);
                for splice in nodes {
                    splice.copy(&from.nodes, &mut to.nodes);
                }
                for splice in edges {
                    splice.copy(&from.edges, &mut to.edges);
                }
            }
        }
    }

    /// Where it just changed the document, or was made from it.
    pub fn changed(&self) -> Changed {
        match self {
            Delta::Whole(_) | Delta::Parts { rest: Some(_), .. } => Changed::All,
            Delta::Parts { nodes, edges, .. } => Changed::Runs {
                nodes: nodes.iter().map(Splice::run).collect(),
                edges: edges.iter().map(Splice::run).collect(),
            },
        }
    }

    /// Merges `later`, made right after it, unless either changes a list's
    /// length. Returns whether it now keeps more parts.
    pub fn absorb(&mut self, later: Delta) -> Result<bool, Delta> {
        let keeps_lengths = |delta: &Delta| match delta {
            Delta::Whole(_) => false,
            Delta::Parts { nodes, edges, .. } => {
                nodes.iter().all(Splice::keeps_length) && edges.iter().all(Splice::keeps_length)
            }
        };
        if !keeps_lengths(self) || !keeps_lengths(&later) {
            return Err(later);
        }
        let (
            Delta::Parts { rest, nodes, edges },
            Delta::Parts {
                rest: later_rest,
                nodes: later_nodes,
                edges: later_edges,
            },
        ) = (self, later)
        else {
            unreachable!("only parts keep the lengths");
        };
        // Parts this change didn't touch were, before it, as `later` has them.
        let mut grew = false;
        if rest.is_none() && later_rest.is_some() {
            (*rest, grew) = (later_rest, true);
        }
        grew |= merge_runs(nodes, later_nodes);
        grew |= merge_runs(edges, later_edges);
        Ok(grew)
    }

    /// Roughly how many bytes it takes.
    pub fn size(&self) -> usize {
        match self {
            Delta::Whole(doc) => doc_size(doc),
            Delta::Parts { rest, nodes, edges } => {
                let nodes = nodes.iter().flat_map(|splice| &splice.items);
                let edges = edges.iter().flat_map(|splice| &splice.items);
                rest.as_deref().map_or(0, doc_size)
                    + nodes.map(node_size).sum::<usize>()
                    + edges.map(edge_size).sum::<usize>()
            }
        }
    }
}

impl<T: Clone> Splice<T> {
    fn swap(&mut self, list: &mut Vec<T>) {
        let added = self.items.len();
        let items = std::mem::take(&mut self.items);
        self.items = list.splice(self.at..self.at + self.len, items).collect();
        self.len = added;
    }

    /// Copies the items it just put in `from` to `to`, in place of the ones
    /// it took out.
    fn copy(&self, from: &[T], to: &mut Vec<T>) {
        let items = from[self.at..self.at + self.len].iter().cloned();
        to.splice(self.at..self.at + self.items.len(), items);
    }

    /// Where it just changed the list, or was just made from it.
    fn run(&self) -> Run {
        Run {
            at: self.at,
            removed: self.items.len(),
            added: self.len,
        }
    }

    fn keeps_length(&self) -> bool {
        self.len == self.items.len()
    }
}

/// Makes `old` the same as `new`, returning what turns it back: runs of the
/// items that differ if the lists are as long, else one splice from the first
/// difference to the last.
fn update_list<T: Clone + PartialEq>(old: &mut Vec<T>, new: &[T]) -> Vec<Splice<T>> {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    if old.len() == new.len() {
        for at in (0..old.len()).filter(|&at| old[at] != new[at]) {
            match runs.last_mut() {
                Some((start, len)) if *start + *len == at => *len += 1,
                _ => runs.push((at, 1)),
            }
        }
    } else {
        let same = |(a, b): &(&T, &T)| a == b;
        let start = old.iter().zip(new).take_while(same).count();
        let most = old.len().min(new.len()) - start;
        let end = (old.iter().rev().zip(new.iter().rev()))
            .take(most)
            .take_while(same)
            .count();
        runs.push((start, old.len() - start - end));
    }
    let (old_len, new_len) = (old.len(), new.len());
    // Taken out of `old` rather than copied.
    let splices = runs.into_iter().map(|(at, removed)| {
        let len = removed + new_len - old_len;
        let items = new[at..at + len].iter().cloned();
        let items = old.splice(at..at + removed, items).collect();
        Splice { at, len, items }
    });
    splices.collect()
}

/// Merges `later`, runs right after `earlier`, into it, none of them
/// changing the list's length. Returns whether `earlier` now has more items.
fn merge_runs<T>(earlier: &mut Vec<Splice<T>>, later: Vec<Splice<T>>) -> bool {
    let within = |run: &Splice<T>| {
        (earlier.iter())
            .any(|earlier| earlier.at <= run.at && run.at + run.len <= earlier.at + earlier.len)
    };
    // As when nudging the same items again.
    if later.iter().all(within) {
        return false;
    }
    let mut items = BTreeMap::new();
    // The earlier run's item wins, being the one from before both.
    for splice in later.into_iter().chain(std::mem::take(earlier)) {
        for (offset, item) in splice.items.into_iter().enumerate() {
            items.insert(splice.at + offset, item);
        }
    }
    for (at, item) in items {
        match earlier.last_mut() {
            Some(run) if run.at + run.len == at => {
                run.items.push(item);
                run.len += 1;
            }
            _ => earlier.push(Splice {
                at,
                len: 1,
                items: vec![item],
            }),
        }
    }
    true
}

/// Whether `a` and `b` are the same but for their top-level graphs' nodes and
/// edges. Both have graphs.
fn same_rest(a: &GraphMl, b: &GraphMl) -> bool {
    let GraphMl {
        comments,
        desc,
        keys,
        graphs,
        data,
        attributes,
        prefix,
    } = a;
    let (top, others) = graphs.split_first().expect("the document has a graph");
    let (b_top, b_others) = b.graphs.split_first().expect("the document has a graph");
    let Graph {
        id,
        edge_default,
        desc: top_desc,
        data: top_data,
        nodes: _,
        edges: _,
        hyperedges,
        locator,
        attributes: top_attributes,
    } = top;
    *comments == b.comments
        && *desc == b.desc
        && *keys == b.keys
        && *data == b.data
        && *attributes == b.attributes
        && *prefix == b.prefix
        && others == b_others
        && *id == b_top.id
        && *edge_default == b_top.edge_default
        && *top_desc == b_top.desc
        && *top_data == b_top.data
        && *hyperedges == b_top.hyperedges
        && *locator == b_top.locator
        && *top_attributes == b_top.attributes
}

/// `doc` without its top-level graph's nodes and edges.
fn rest_of(doc: &GraphMl) -> GraphMl {
    let (top, others) = doc.graphs.split_first().expect("the document has a graph");
    let top = Graph {
        nodes: Vec::new(),
        edges: Vec::new(),
        id: top.id.clone(),
        edge_default: top.edge_default,
        desc: top.desc.clone(),
        data: top.data.clone(),
        hyperedges: top.hyperedges.clone(),
        locator: top.locator.clone(),
        attributes: top.attributes.clone(),
    };
    GraphMl {
        comments: doc.comments.clone(),
        desc: doc.desc.clone(),
        keys: doc.keys.clone(),
        graphs: std::iter::once(top).chain(others.iter().cloned()).collect(),
        data: doc.data.clone(),
        attributes: doc.attributes.clone(),
        prefix: doc.prefix.clone(),
    }
}

/// Swaps all but the top-level graph's nodes and edges of `doc` with `rest`,
/// which has none.
fn swap_rest(doc: &mut GraphMl, rest: &mut GraphMl) {
    let top = &mut doc.graphs[0];
    let nodes = std::mem::take(&mut top.nodes);
    let edges = std::mem::take(&mut top.edges);
    std::mem::swap(doc, rest);
    let top = &mut doc.graphs[0];
    (top.nodes, top.edges) = (nodes, edges);
}

/// What a string or a tree node takes besides its text, roughly.
const OVERHEAD: usize = 48;

fn doc_size(doc: &GraphMl) -> usize {
    let comments = doc.comments.iter().map(|comment| comment.len() + OVERHEAD);
    let keys = doc.keys.iter().map(|key| {
        let default = key.default.as_ref();
        key.id.len()
            + attributes_size(&key.attributes)
            + default.map_or(0, |default| xml_size(&default.content))
            + OVERHEAD
    });
    comments.chain(keys).sum::<usize>()
        + doc.graphs.iter().map(graph_size).sum::<usize>()
        + data_size(&doc.data)
        + attributes_size(&doc.attributes)
}

fn graph_size(graph: &Graph) -> usize {
    graph.nodes.iter().map(node_size).sum::<usize>()
        + graph.edges.iter().map(edge_size).sum::<usize>()
        + data_size(&graph.data)
        + attributes_size(&graph.attributes)
        + OVERHEAD
}

fn node_size(node: &Node) -> usize {
    node.id.len()
        + data_size(&node.data)
        + node.graph.as_ref().map_or(0, graph_size)
        + attributes_size(&node.attributes)
        + OVERHEAD
}

fn edge_size(edge: &Edge) -> usize {
    edge.source.len()
        + edge.target.len()
        + data_size(&edge.data)
        + edge.graph.as_ref().map_or(0, graph_size)
        + attributes_size(&edge.attributes)
        + OVERHEAD
}

fn data_size(data: &[Data]) -> usize {
    let data = data.iter().map(|data| {
        data.key.len() + xml_size(&data.content) + attributes_size(&data.attributes) + OVERHEAD
    });
    data.sum()
}

fn xml_size(content: &[XmlNode]) -> usize {
    let nodes = content.iter().map(|node| match node {
        XmlNode::Element(element) => {
            element.name.len()
                + attributes_size(&element.attributes)
                + xml_size(&element.children)
                + OVERHEAD
        }
        XmlNode::Text(text) | XmlNode::CData(text) | XmlNode::Comment(text) => {
            text.len() + OVERHEAD
        }
    });
    nodes.sum()
}

fn attributes_size(attributes: &Attributes) -> usize {
    let attributes = attributes.iter();
    attributes
        .map(|(name, value)| name.len() + value.len() + 2 * OVERHEAD)
        .sum()
}
