//! Typed access to the graphics yEd stores in `<data>` elements.
//!
//! Graphics keep the XML they were read from, and writing them back changes
//! only what was edited, so untouched files stay byte-for-byte identical.

mod codec;
mod edge;
mod edge_placement;
mod label;
mod node;
mod placement;
mod route;
mod style;

pub use edge::{Arrow, Arrows, EdgeGraphics, EdgeKind, Path};
pub use label::{Alignment, Font, FontStyle, Label};
pub use node::{Fill, NodeGraphics, NodeKind, ShapeType};
pub use placement::LabelPosition;
pub use route::Route;
pub use style::{Color, LineType, Point, Rect, Stroke};

use crate::{
    Data, Edge, Error, GraphMl, Key, KeyDomain, Node,
    xml::{Element, XmlNode},
};

/// The namespace of yEd's GraphML extensions, conventionally prefixed `y`.
pub const NAMESPACE: &str = "http://www.yworks.com/xml/graphml";

/// The ids of the keys a document stores yEd graphics under.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    node_graphics: Option<String>,
    edge_graphics: Option<String>,
}

impl Keys {
    /// Finds the keys `doc` declares for graphics.
    pub fn find(doc: &GraphMl) -> Self {
        Keys {
            node_graphics: find_key(doc, "nodegraphics"),
            edge_graphics: find_key(doc, "edgegraphics"),
        }
    }

    /// Like [`Keys::find`], but declares the keys first if they're missing,
    /// along with the GraphML and `y` namespaces yEd needs.
    pub fn ensure(doc: &mut GraphMl) -> Self {
        ensure_namespaces(doc);
        Keys {
            node_graphics: Some(ensure_key(doc, "nodegraphics", KeyDomain::Node)),
            edge_graphics: Some(ensure_key(doc, "edgegraphics", KeyDomain::Edge)),
        }
    }

    /// Reads `node`'s graphics, if it has any.
    pub fn node_graphics(&self, node: &Node) -> Result<Option<NodeGraphics>, Error> {
        graphics(&node.data, &self.node_graphics)
            .map(NodeGraphics::from_element)
            .transpose()
    }

    /// # Panics
    /// If the document has no key for node graphics; see [`Keys::ensure`].
    pub fn set_node_graphics(&self, node: &mut Node, graphics: &NodeGraphics) {
        set_graphics(&mut node.data, &self.node_graphics, graphics.to_element());
    }

    /// Reads `edge`'s graphics, if it has any.
    pub fn edge_graphics(&self, edge: &Edge) -> Result<Option<EdgeGraphics>, Error> {
        graphics(&edge.data, &self.edge_graphics)
            .map(EdgeGraphics::from_element)
            .transpose()
    }

    /// # Panics
    /// If the document has no key for edge graphics; see [`Keys::ensure`].
    pub fn set_edge_graphics(&self, edge: &mut Edge, graphics: &EdgeGraphics) {
        set_graphics(&mut edge.data, &self.edge_graphics, graphics.to_element());
    }
}

/// The `y:Resources` element holding the images and SVG drawings that nodes
/// refer to by `refid`.
pub fn resources(doc: &GraphMl) -> Option<&Element> {
    let key = find_key(doc, "resources")?;
    doc.data.iter().find(|data| data.key == key)?.element()
}

/// Like [`resources`], but adds the element, its data and its key if missing.
pub fn resources_mut(doc: &mut GraphMl) -> &mut Element {
    ensure_namespaces(doc);
    let key = ensure_key(doc, "resources", KeyDomain::GraphMl);
    let index = match doc.data.iter().position(|data| data.key == key) {
        Some(index) => index,
        None => {
            doc.data.push(Data {
                key,
                ..Data::default()
            });
            doc.data.len() - 1
        }
    };
    let data = &mut doc.data[index];
    if data.element().is_none() {
        (data.content).push(XmlNode::Element(Element::new("y:Resources")));
    }
    data.element_mut().expect("added if missing")
}

/// Declares the GraphML and `y` namespaces yEd needs, if missing.
fn ensure_namespaces(doc: &mut GraphMl) {
    // A prefixed document has GraphML's namespace on its prefix.
    if doc.prefix.is_none() {
        doc.attributes
            .entry("xmlns".to_owned())
            .or_insert_with(|| crate::NAMESPACE.to_owned());
    }
    doc.attributes
        .entry("xmlns:y".to_owned())
        .or_insert_with(|| NAMESPACE.to_owned());
}

fn find_key(doc: &GraphMl, yfiles_type: &str) -> Option<String> {
    doc.keys
        .iter()
        .find(|key| {
            key.attributes
                .get("yfiles.type")
                .is_some_and(|t| t == yfiles_type)
        })
        .map(|key| key.id.clone())
}

fn ensure_key(doc: &mut GraphMl, yfiles_type: &str, domain: KeyDomain) -> String {
    if let Some(id) = find_key(doc, yfiles_type) {
        return id;
    }
    // yEd numbers its keys d0, d1, …
    let id = (0..)
        .map(|n| format!("d{n}"))
        .find(|id| doc.keys.iter().all(|key| key.id != *id))
        .expect("the ids are endless");
    doc.keys.push(Key {
        id: id.clone(),
        domain,
        attributes: [("yfiles.type".to_owned(), yfiles_type.to_owned())].into(),
        ..Key::default()
    });
    id
}

fn graphics<'a>(data: &'a [Data], key: &Option<String>) -> Option<&'a Element> {
    let key = key.as_deref()?;
    data.iter().find(|data| data.key == key)?.element()
}

fn set_graphics(data: &mut Vec<Data>, key: &Option<String>, element: Element) {
    let key = key
        .as_deref()
        .expect("the document declares no key for these graphics; see `Keys::ensure`");
    match data.iter_mut().find(|data| data.key == key) {
        Some(data) => match data.element_mut() {
            Some(old) => *old = element,
            None => data.content.push(XmlNode::Element(element)),
        },
        None => data.push(Data {
            key: key.to_owned(),
            content: vec![XmlNode::Element(element)],
            ..Data::default()
        }),
    }
}

/// An empty document with the namespaces, keys and resources of a new yEd file.
pub fn new_document() -> GraphMl {
    GraphMl::parse(EMPTY_DOCUMENT).expect("the template is valid GraphML")
}

const EMPTY_DOCUMENT: &str = r#"<graphml xmlns="http://graphml.graphdrawing.org/xmlns" xmlns:java="http://www.yworks.com/xml/yfiles-common/1.0/java" xmlns:sys="http://www.yworks.com/xml/yfiles-common/markup/primitives/2.0" xmlns:x="http://www.yworks.com/xml/yfiles-common/markup/2.0" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:y="http://www.yworks.com/xml/graphml" xmlns:yed="http://www.yworks.com/xml/yed/3" xsi:schemaLocation="http://graphml.graphdrawing.org/xmlns http://www.yworks.com/xml/schema/graphml/1.1/ygraphml.xsd">
  <key attr.name="Description" attr.type="string" for="graph" id="d0"/>
  <key for="port" id="d1" yfiles.type="portgraphics"/>
  <key for="port" id="d2" yfiles.type="portgeometry"/>
  <key for="port" id="d3" yfiles.type="portuserdata"/>
  <key attr.name="url" attr.type="string" for="node" id="d4"/>
  <key attr.name="description" attr.type="string" for="node" id="d5"/>
  <key for="node" id="d6" yfiles.type="nodegraphics"/>
  <key for="graphml" id="d7" yfiles.type="resources"/>
  <key attr.name="url" attr.type="string" for="edge" id="d8"/>
  <key attr.name="description" attr.type="string" for="edge" id="d9"/>
  <key for="edge" id="d10" yfiles.type="edgegraphics"/>
  <graph edgedefault="directed" id="G"/>
  <data key="d7">
    <y:Resources/>
  </data>
</graphml>"#;
