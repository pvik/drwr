//! The top-level graph's nodes and edges with their graphics read, kept up to
//! date by reading again only what changed.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use egui::Vec2;
use graphml::GraphMl;
use graphml::yed::{self, EdgeGraphics, NodeGraphics, NodeKind};

use crate::document::{self, GraphicsRef, Item, Reader};
use crate::selection::Selection;
use crate::svg_image::SvgImage;

/// The top-level graph's nodes and edges, as the canvas shows them. Clones
/// share the graphics until they're changed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// Those with graphics, in the document's order.
    pub nodes: Vec<Arc<SceneNode>>,
    /// Those with graphics whose nodes are both in `nodes`, in the document's
    /// order.
    pub edges: Vec<SceneEdge>,
    /// The index in `nodes` of each id.
    ids: Arc<HashMap<String, usize>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneNode {
    pub id: String,
    pub graphics: NodeGraphics,
    /// The corner radius of a box.
    pub radius: Option<f64>,
    /// The drawing of an SVG node, unless it has none or it isn't SVG.
    pub image: Option<Arc<SvgImage>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneEdge {
    /// Its index in the document.
    pub index: usize,
    /// The indexes of its nodes in the scene.
    pub source: usize,
    pub target: usize,
    pub graphics: Arc<EdgeGraphics>,
}

impl Scene {
    #[cfg(test)]
    pub fn read(graphml: &GraphMl) -> Self {
        Cache::read(graphml).scene
    }

    /// The index of the node with id `id`.
    pub fn node(&self, id: &str) -> Option<usize> {
        self.ids.get(id).copied()
    }

    /// The edge at `index` in the document.
    pub fn edge(&self, index: usize) -> Option<&SceneEdge> {
        let found = self.edges.binary_search_by_key(&index, |edge| edge.index);
        found.ok().map(|at| &self.edges[at])
    }

    /// The graphics of `item`, if it's in the scene.
    pub fn graphics(&self, item: &Item) -> Option<GraphicsRef<'_>> {
        match item {
            Item::Node(id) => {
                let node = &self.nodes[self.node(id)?];
                Some(GraphicsRef::Node(&node.graphics))
            }
            Item::Edge(index) => Some(GraphicsRef::Edge(&self.edge(*index)?.graphics)),
        }
    }

    /// Moves the `selected` nodes, and the bends of the edges at `edges`, by
    /// `delta` in graph points, as a drag shows them before it's let go.
    pub fn translate(&mut self, selected: &Selection, edges: &HashSet<usize>, delta: Vec2) {
        let (dx, dy) = (f64::from(delta.x), f64::from(delta.y));
        for node in &mut self.nodes {
            if selected.contains_node(&node.id) {
                let geometry = &mut Arc::make_mut(node).graphics.geometry;
                geometry.x += dx;
                geometry.y += dy;
            }
        }
        let moved = self
            .edges
            .iter_mut()
            .filter(|edge| edges.contains(&edge.index));
        for edge in moved {
            for point in &mut Arc::make_mut(&mut edge.graphics).path.points {
                point.x += dx;
                point.y += dy;
            }
        }
    }
}

/// A [`Scene`], with what it read from each top-level node and edge.
#[derive(Default)]
pub struct Cache {
    scene: Scene,
    /// Per node of the top-level graph, in its order: what the scene shows of
    /// it, if it has graphics.
    nodes: Vec<Option<Arc<SceneNode>>>,
    /// Per edge of the top-level graph: its graphics, if it has any.
    edges: Vec<Option<Arc<EdgeGraphics>>>,
    /// The drawing parsed from each SVG text, if it could be.
    images: HashMap<String, Option<Arc<SvgImage>>>,
    /// How many nodes and edges it has read.
    #[cfg(test)]
    pub reads: usize,
}

/// Reads the drawings of a document's SVG nodes, each resource once.
struct Images<'a> {
    doc: &'a GraphMl,
    /// By their text, as parsed in any update.
    parsed: &'a mut HashMap<String, Option<Arc<SvgImage>>>,
    /// By resource id, as found in this update.
    resources: HashMap<String, Option<Arc<SvgImage>>>,
}

impl Images<'_> {
    /// The drawing in the resource with id `resource`, if it can be drawn.
    fn get(&mut self, resource: &str) -> Option<Arc<SvgImage>> {
        if let Some(image) = self.resources.get(resource) {
            return image.clone();
        }
        let image = yed::resource_text(self.doc, resource).and_then(|text| {
            let parse = |text: &String| SvgImage::parse(text).ok().map(Arc::new);
            self.parsed.entry(text).or_insert_with_key(parse).clone()
        });
        self.resources.insert(resource.to_owned(), image.clone());
        image
    }
}

/// What changed in a document, for the [`Cache`] to read again.
#[derive(Clone, Debug, PartialEq)]
pub enum Changed {
    All,
    /// Runs of the top-level graph's nodes and edges.
    Runs {
        nodes: Vec<Run>,
        edges: Vec<Run>,
    },
}

/// Items that took the place of others in a list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    /// The index of the first.
    pub at: usize,
    /// How many there were.
    pub removed: usize,
    /// How many there are.
    pub added: usize,
}

impl Cache {
    pub fn read(graphml: &GraphMl) -> Self {
        let mut cache = Cache::default();
        cache.update(graphml, &Changed::All);
        cache
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Reads again what `changed` in `graphml` since the last update. Runs
    /// that change a list's length come one per list, so their indexes hold.
    pub fn update(&mut self, graphml: &GraphMl, changed: &Changed) {
        let reader = Reader::new(graphml);
        let (nodes, edges) = (document::top_nodes(graphml), document::top_edges(graphml));
        let mut images = Images {
            doc: graphml,
            parsed: &mut self.images,
            resources: HashMap::new(),
        };
        let mut read_node = |node: &graphml::Node| {
            let (graphics, radius) = reader.node(node)?;
            let image = match &graphics.kind {
                NodeKind::Svg {
                    resource: Some(resource),
                } => images.get(resource),
                _ => None,
            };
            let id = node.id.clone();
            Some(Arc::new(SceneNode {
                id,
                graphics,
                radius,
                image,
            }))
        };
        let read_edge = |edge| reader.edge(edge).map(Arc::new);
        match changed {
            Changed::All => {
                self.nodes = nodes.iter().map(&mut read_node).collect();
                self.edges = edges.iter().map(read_edge).collect();
                #[cfg(test)]
                {
                    self.reads += nodes.len() + edges.len();
                }
            }
            Changed::Runs {
                nodes: node_runs,
                edges: edge_runs,
            } => {
                for run in node_runs {
                    let read = nodes[run.at..run.at + run.added].iter().map(&mut read_node);
                    self.nodes.splice(run.at..run.at + run.removed, read);
                    #[cfg(test)]
                    {
                        self.reads += run.added;
                    }
                }
                for run in edge_runs {
                    let read = edges[run.at..run.at + run.added].iter().map(read_edge);
                    self.edges.splice(run.at..run.at + run.removed, read);
                    #[cfg(test)]
                    {
                        self.reads += run.added;
                    }
                }
            }
        }
        self.index(edges);
    }

    /// Makes the scene of what's been read, `edges` being the document's.
    fn index(&mut self, edges: &[graphml::Edge]) {
        let nodes: Vec<_> = self.nodes.iter().flatten().cloned().collect();
        let mut ids = HashMap::with_capacity(nodes.len());
        for (index, node) in nodes.iter().enumerate() {
            // As the first node with an id is the one found.
            ids.entry(node.id.clone()).or_insert(index);
        }
        let edges = (edges.iter().zip(&self.edges).enumerate())
            .filter_map(|(index, (edge, graphics))| {
                Some(SceneEdge {
                    index,
                    source: *ids.get(edge.source.as_str())?,
                    target: *ids.get(edge.target.as_str())?,
                    graphics: graphics.clone()?,
                })
            })
            .collect();
        self.scene = Scene {
            nodes,
            edges,
            ids: Arc::new(ids),
        };
    }
}

#[cfg(test)]
mod tests;
