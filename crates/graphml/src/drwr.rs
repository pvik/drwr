//! Data attributes drwr uses in otherwise standard yEd GraphML.

use crate::{
    AttrType, Data, GraphMl, Key, KeyDomain, Node,
    xml::XmlNode,
    yed::{self, NodeKind, ShapeType},
};

/// The corner radius yEd draws every round rectangle with.
pub const YED_CORNER_RADIUS: f64 = 4.0;
const CORNER_RADIUS: &str = "drwr.cornerRadius";

/// Finds the key id used to store custom corner radii.
pub fn corner_radius_key(doc: &GraphMl) -> Option<&str> {
    doc.keys
        .iter()
        .find(|key| {
            key.domain == KeyDomain::Node && key.attr_name.as_deref() == Some(CORNER_RADIUS)
        })
        .map(|key| key.id.as_str())
}

/// Returns the corner-radius key id, declaring it if needed.
pub fn ensure_corner_radius_key(doc: &mut GraphMl) -> String {
    if let Some(id) = corner_radius_key(doc) {
        return id.to_owned();
    }
    let id = yed::unused_key_id(doc);
    doc.keys.push(Key {
        id: id.clone(),
        domain: KeyDomain::Node,
        attr_name: Some(CORNER_RADIUS.to_owned()),
        attr_type: Some(AttrType::Double),
        ..Key::default()
    });
    id
}

/// The radius of a rectangle or round rectangle, using yEd's default when
/// there is no valid custom radius under `key`.
pub fn corner_radius(kind: &NodeKind, node: &Node, key: Option<&str>) -> Option<f64> {
    match kind {
        NodeKind::Shape(ShapeType::Rectangle) => Some(0.0),
        NodeKind::Shape(ShapeType::RoundRectangle) => {
            let radius = node
                .data
                .iter()
                .find(|data| Some(data.key.as_str()) == key)
                .and_then(|data| data.text().trim().parse::<f64>().ok())
                .filter(|radius| radius.is_finite() && *radius >= 0.0);
            Some(radius.unwrap_or(YED_CORNER_RADIUS))
        }
        _ => None,
    }
}

/// Stores a custom radius on a round rectangle under its document's
/// corner-radius key.
pub fn store_corner_radius(node: &mut Node, key: &str, radius: f64) {
    let text = format!("{radius:?}");
    match node.data.iter_mut().find(|data| data.key == key) {
        Some(data) => data.content = vec![XmlNode::Text(text)],
        None => node.data.push(Data::from_text(key, text)),
    }
}
