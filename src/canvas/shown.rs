//! The scene as the canvas shows it, and what's where on screen.

use std::borrow::Cow;

use egui::emath::TSTransform;
use egui::{Painter, Pos2, Rect, Vec2, pos2, vec2};
use graphml::yed::{self, EdgeGraphics, NodeGraphics, NodeKind, Route, ShapeType};

use crate::document::{EdgeEnd, Item};
use crate::scene::{Scene, SceneEdge, SceneNode};
use crate::selection::Selection;

use super::{EdgePart, GRAB, Owner, edge};

/// Where an end of an edge is: on a node of the scene, by its index, or at a
/// point in graph coordinates.
#[derive(Clone, Copy)]
pub enum Anchor {
    Node(usize),
    Point(Pos2),
}

/// An edge as it's drawn, which can differ from the document's while it's
/// being drawn or dragged.
#[derive(Clone)]
pub struct EdgeLook<'a> {
    pub graphics: Cow<'a, EdgeGraphics>,
    pub source: Anchor,
    pub target: Anchor,
}

/// The scene as it's shown this frame.
#[derive(Clone, Copy)]
pub struct Shown<'a> {
    pub scene: &'a Scene,
    pub painter: &'a Painter,
    pub to_screen: TSTransform,
    /// The zoom labels are laid out at.
    pub text_zoom: f32,
}

/// The shape the canvas draws `node` as, if it can draw it.
fn outline(node: &SceneNode) -> Option<Outline> {
    shape(&node.graphics, node.radius)
}

impl<'a> EdgeLook<'a> {
    /// `edge` as the document has it.
    pub fn of(edge: &'a SceneEdge) -> Self {
        EdgeLook {
            graphics: Cow::Borrowed(&*edge.graphics),
            source: Anchor::Node(edge.source),
            target: Anchor::Node(edge.target),
        }
    }

    pub fn into_owned(self) -> EdgeLook<'static> {
        EdgeLook {
            graphics: Cow::Owned(self.graphics.into_owned()),
            source: self.source,
            target: self.target,
        }
    }

    pub fn anchor(&self, end: EdgeEnd) -> Anchor {
        match end {
            EdgeEnd::Source => self.source,
            EdgeEnd::Target => self.target,
        }
    }
}

impl Shown<'_> {
    pub fn geometry(&self, node: usize) -> yed::Rect {
        self.scene.nodes[node].graphics.geometry
    }

    /// Where the bounds of the node at index `node` are on screen.
    pub fn node_rect(&self, node: usize) -> Rect {
        egui_rect(self.geometry(node), self.to_screen)
    }

    /// The middle of the node at index `node`, in graph coordinates.
    pub fn center(&self, node: usize) -> yed::Point {
        let node = self.geometry(node);
        yed::Point {
            x: node.x + node.width / 2.0,
            y: node.y + node.height / 2.0,
        }
    }

    /// Whether `pos` on screen is inside the node at index `node`, or its
    /// bounds if the canvas can't draw it.
    fn inside(&self, node: usize, pos: Pos2) -> bool {
        let outline = outline(&self.scene.nodes[node]);
        inside(outline, self.node_rect(node), self.to_screen.scaling, pos)
    }

    /// The topmost node or edge at `pos` on screen, of those `only` has if
    /// it's given. Edges come first, as they're painted over the nodes.
    pub fn item_at(&self, pos: Pos2, only: Option<&Selection>) -> Option<Item> {
        let edges = self.scene.edges.iter().rev();
        let mut edges = edges.filter(|edge| only.is_none_or(|only| only.contains_edge(edge.index)));
        if let Some(edge) = edges.find(|edge| self.edge_part_at(edge, pos).is_some()) {
            return Some(Item::Edge(edge.index));
        }
        let node = self.node_at(pos, only)?;
        Some(Item::Node(self.scene.nodes[node].id.clone()))
    }

    /// The part of `edge` at `pos` on screen, other than a handle: a label, or
    /// else its line.
    pub fn edge_part_at(&self, edge: &SceneEdge, pos: Pos2) -> Option<EdgePart> {
        let look = EdgeLook::of(edge);
        let owner = Owner::Edge(self.route(&look));
        let labels = edge.graphics.labels.iter().enumerate().rev();
        let label = labels
            .filter(|(_, label)| label.visible)
            .find(|(_, label)| {
                let (_, rect) = self.label_layout(&owner, label);
                rect.contains(pos)
            });
        if let Some((label, _)) = label {
            return Some(EdgePart::Label(label));
        }
        let near = edge::nearest_segment(&self.path(&look), pos).is_some_and(|(_, d)| d <= GRAB);
        // The bend goes on the segment of the whole route nearest `pos`.
        near.then(|| edge::nearest_segment(&self.unclipped(&look), pos))
            .flatten()
            .map(|(segment, _)| EdgePart::Line(segment))
    }

    /// Whether the node at index `node` is drawn, and inside `rect` on screen.
    pub fn in_box(&self, node: usize, rect: Rect) -> bool {
        let drawn = outline(&self.scene.nodes[node]).is_some();
        drawn && rect.contains_rect(self.node_rect(node))
    }

    /// Whether `edge` is inside `rect` on screen: both its nodes, by
    /// [`Self::in_box`], and all its bends.
    pub fn edge_in_box(&self, edge: &SceneEdge, rect: Rect) -> bool {
        let bends = edge.graphics.path.points.iter();
        let mut bends = bends.map(|&bend| self.to_screen * egui_pos(bend));
        self.in_box(edge.source, rect)
            && self.in_box(edge.target, rect)
            && bends.all(|bend| rect.contains(bend))
    }

    /// The index of the topmost node the canvas draws at `pos` on screen, of
    /// those `only` has if it's given.
    pub fn node_at(&self, pos: Pos2, only: Option<&Selection>) -> Option<usize> {
        let nodes = self.scene.nodes.iter().enumerate().rev();
        let drawn = nodes.filter(|(_, node)| outline(node).is_some());
        let mut wanted =
            drawn.filter(|(_, node)| only.is_none_or(|only| only.contains_node(&node.id)));
        let (index, _) = wanted.find(|&(index, _)| self.inside(index, pos))?;
        Some(index)
    }

    /// Where `look` runs on screen, from port to port through its bends.
    pub fn unclipped(&self, look: &EdgeLook) -> Vec<Pos2> {
        let port = |anchor: Anchor, offset: yed::Point| match anchor {
            Anchor::Node(node) => {
                let center = self.center(node);
                pos2((center.x + offset.x) as f32, (center.y + offset.y) as f32)
            }
            Anchor::Point(point) => point,
        };
        let path = &look.graphics.path;
        let bends = path.points.iter().copied().map(egui_pos);
        std::iter::once(port(look.source, path.source))
            .chain(bends)
            .chain(std::iter::once(port(look.target, path.target)))
            .map(|point| self.to_screen * point)
            .collect()
    }

    /// Where `look` is painted on screen: cut where it leaves its nodes.
    pub fn path(&self, look: &EdgeLook) -> Vec<Pos2> {
        let zoom = self.to_screen.scaling;
        let inside = |anchor: Anchor| {
            let node = match anchor {
                Anchor::Node(node) => {
                    Some((outline(&self.scene.nodes[node]), self.node_rect(node)))
                }
                Anchor::Point(_) => None,
            };
            move |pos| node.is_some_and(|(outline, rect)| inside(outline, rect, zoom, pos))
        };
        edge::clip(
            self.unclipped(look),
            inside(look.source),
            inside(look.target),
        )
    }

    /// The route yEd places the labels of `look` along.
    pub fn route(&self, look: &EdgeLook) -> Route {
        let bounds = |anchor: Anchor| match anchor {
            Anchor::Node(node) => self.geometry(node),
            Anchor::Point(point) => yed::Rect {
                x: point.x.into(),
                y: point.y.into(),
                width: 0.0,
                height: 0.0,
            },
        };
        Route::new(
            &look.graphics.path,
            bounds(look.source),
            bounds(look.target),
        )
    }
}

/// The shapes the canvas can draw nodes as.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outline {
    /// A box, with its corners rounded to this radius in graph points.
    Rect(f32),
    Ellipse,
}

/// The shape the canvas draws a node as, if it's a kind it can draw yet.
/// `corner_radius` rounds the corners of a box; see
/// [`crate::document::corner_radius`].
pub fn shape(graphics: &NodeGraphics, corner_radius: Option<f64>) -> Option<Outline> {
    match &graphics.kind {
        NodeKind::Shape(ShapeType::Rectangle | ShapeType::RoundRectangle) => {
            Some(Outline::Rect(corner_radius.unwrap_or_default() as f32))
        }
        NodeKind::Shape(ShapeType::Ellipse) => Some(Outline::Ellipse),
        _ => None,
    }
}

/// Whether a point is inside `outline` filling `rect`, or in `rect` for a
/// node the canvas can't draw, at `zoom` screen points a graph point.
fn inside(outline: Option<Outline>, rect: Rect, zoom: f32, pos: Pos2) -> bool {
    match outline {
        Some(Outline::Rect(radius)) => in_rounded_rect(rect, radius * zoom, pos),
        Some(Outline::Ellipse) => in_ellipse(rect, pos),
        None => rect.contains(pos),
    }
}

/// Whether a point is in `rect` with its corners rounded to `radius`, but by
/// no more than half of each side, as painted.
fn in_rounded_rect(rect: Rect, radius: f32, pos: Pos2) -> bool {
    let radius = Vec2::splat(radius).min(rect.size() / 2.0);
    if radius.min_elem() <= 0.0 {
        return rect.contains(pos);
    }
    // From the center of the corner it's in, in radii.
    let offset = (pos - rect.shrink2(radius).clamp(pos)) / radius;
    rect.contains(pos) && offset.length_sq() <= 1.0
}

/// Whether a point is in the ellipse filling `rect`. An inverted `rect` has
/// none, as none is painted.
fn in_ellipse(rect: Rect, pos: Pos2) -> bool {
    let offset = (pos - rect.center()) / (rect.size() / 2.0);
    rect.contains(pos) && offset.length_sq() <= 1.0
}

/// A yEd point as an egui one.
pub fn egui_pos(point: yed::Point) -> Pos2 {
    pos2(point.x as f32, point.y as f32)
}

/// An egui point as a yEd one.
pub fn yed_point(pos: Pos2) -> yed::Point {
    yed::Point {
        x: pos.x.into(),
        y: pos.y.into(),
    }
}

/// A yEd rect as an egui one, taken by `transform`.
pub fn egui_rect(rect: yed::Rect, transform: TSTransform) -> Rect {
    let min = transform * pos2(rect.x as f32, rect.y as f32);
    let size = vec2(rect.width as f32, rect.height as f32) * transform.scaling;
    Rect::from_min_size(min, size)
}

/// An egui rect as a yEd one.
pub fn yed_rect(rect: Rect) -> yed::Rect {
    yed::Rect {
        x: rect.min.x.into(),
        y: rect.min.y.into(),
        width: rect.width().into(),
        height: rect.height().into(),
    }
}
