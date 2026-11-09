//! Which nodes hide which. The canvas paints the top-level graph's nodes in
//! the document's order, last on top, as yEd does.

use std::collections::HashSet;

use graphml::GraphMl;
use graphml::yed::Rect;

use crate::document;
use crate::scene::Scene;
use crate::selection::Selection;

/// A top-level node as Raise and Lower see it.
struct Layer {
    /// `None` without graphics, which nothing overlaps.
    bounds: Option<Rect>,
    selected: bool,
}

/// The order of the top-level nodes of `doc`, as their indexes, after Raise:
/// each `selected` node goes just above the nearest node above it that
/// overlaps it and isn't selected. The selected keep their order, so those
/// above one go at least as far. `None` if none would move.
pub fn raised(doc: &GraphMl, scene: &Scene, selected: &Selection) -> Option<Vec<usize>> {
    raise(&layers(doc, scene, selected))
}

/// Like [`raised`], but downwards, for Lower.
pub fn lowered(doc: &GraphMl, scene: &Scene, selected: &Selection) -> Option<Vec<usize>> {
    let mut layers = layers(doc, scene, selected);
    layers.reverse();
    let order = raise(&layers)?;
    let last = layers.len() - 1;
    Some(order.into_iter().rev().map(|index| last - index).collect())
}

fn layers(doc: &GraphMl, scene: &Scene, selected: &Selection) -> Vec<Layer> {
    if !selected.has_nodes() {
        return Vec::new();
    }
    let selected: HashSet<&str> = selected.nodes().collect();
    let layer = |node: &graphml::Node| Layer {
        bounds: (scene.node(&node.id)).map(|index| scene.nodes[index].graphics.geometry),
        selected: selected.contains(node.id.as_str()),
    };
    document::top_nodes(doc).iter().map(layer).collect()
}

/// The order of `layers`, bottom up, after Raise; see [`raised`].
fn raise(layers: &[Layer]) -> Option<Vec<usize>> {
    // Per selected layer, the unselected one it goes just above, if any.
    let mut floors = Vec::new();
    let (mut below, mut floor, mut moves) = (None, None, false);
    for (index, layer) in layers.iter().enumerate() {
        if !layer.selected {
            below = Some(index);
            continue;
        }
        let above = &layers[index + 1..];
        let over = (above.iter())
            .position(|other| !other.selected && overlap(layer, other))
            .map(|offset| index + 1 + offset);
        moves |= over.is_some();
        floor = floor.max(below).max(over);
        floors.push((index, floor));
    }
    if !moves {
        return None;
    }
    let mut floors = floors.into_iter().peekable();
    let mut order = Vec::with_capacity(layers.len());
    let mut put = |order: &mut Vec<usize>, floor| {
        while let Some((index, _)) = floors.next_if(|&(_, f)| f == floor) {
            order.push(index);
        }
    };
    put(&mut order, None);
    for index in (0..layers.len()).filter(|&index| !layers[index].selected) {
        order.push(index);
        put(&mut order, Some(index));
    }
    Some(order)
}

/// Whether the bounds of `a` and `b` share some area; touching sides don't.
fn overlap(a: &Layer, b: &Layer) -> bool {
    let (Some(a), Some(b)) = (&a.bounds, &b.bounds) else {
        return false;
    };
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

#[cfg(test)]
mod tests;
