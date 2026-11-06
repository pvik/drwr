//! A generic XML tree, used for content the GraphML model doesn't interpret,
//! such as the inside of `<data>` elements.

use std::collections::BTreeMap;

/// Attributes by qualified name. They're written in this (alphabetical) order,
/// as yEd does.
pub type Attributes = BTreeMap<String, String>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XmlNode {
    Element(Element),
    Text(String),
    CData(String),
    Comment(String),
}

impl XmlNode {
    pub fn as_element(&self) -> Option<&Element> {
        match self {
            XmlNode::Element(element) => Some(element),
            _ => None,
        }
    }

    pub fn as_element_mut(&mut self) -> Option<&mut Element> {
        match self {
            XmlNode::Element(element) => Some(element),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Element {
    /// The qualified name as written, e.g. `y:ShapeNode`.
    pub name: String,
    pub attributes: Attributes,
    pub children: Vec<XmlNode>,
}

impl Element {
    pub fn new(name: impl Into<String>) -> Self {
        Element {
            name: name.into(),
            ..Element::default()
        }
    }

    /// The name without its namespace prefix, e.g. `ShapeNode`.
    pub fn local_name(&self) -> &str {
        local_name(&self.name)
    }

    /// The namespace prefix, e.g. `y`.
    pub fn prefix(&self) -> Option<&str> {
        prefix(&self.name)
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes.get(name).map(String::as_str)
    }

    pub fn set_attr(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.attributes.insert(name.into(), value.into());
    }

    pub fn with_attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.set_attr(name, value);
        self
    }

    pub fn with_child(mut self, child: Element) -> Self {
        self.children.push(XmlNode::Element(child));
        self
    }

    /// The child elements.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(XmlNode::as_element)
    }

    pub fn elements_mut(&mut self) -> impl Iterator<Item = &mut Element> {
        self.children.iter_mut().filter_map(XmlNode::as_element_mut)
    }

    /// The first child element with this local name.
    pub fn child(&self, local_name: &str) -> Option<&Element> {
        self.elements().find(|e| e.local_name() == local_name)
    }

    pub fn child_mut(&mut self, local_name: &str) -> Option<&mut Element> {
        self.elements_mut().find(|e| e.local_name() == local_name)
    }

    /// The child elements with this local name.
    pub fn children_named<'a>(&'a self, local_name: &'a str) -> impl Iterator<Item = &'a Element> {
        self.elements()
            .filter(move |e| e.local_name() == local_name)
    }

    /// The text and CDATA directly inside this element, concatenated.
    pub fn text(&self) -> String {
        text(&self.children)
    }
}

/// The text and CDATA in `nodes`, concatenated.
pub fn text(nodes: &[XmlNode]) -> String {
    nodes
        .iter()
        .filter_map(|node| match node {
            XmlNode::Text(text) | XmlNode::CData(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// Strips the namespace prefix from a qualified name.
pub fn local_name(name: &str) -> &str {
    name.split_once(':').map_or(name, |(_, local)| local)
}

/// The namespace prefix of a qualified name.
pub fn prefix(name: &str) -> Option<&str> {
    name.split_once(':').map(|(prefix, _)| prefix)
}
