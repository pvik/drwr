//! The GraphML document model. Attributes without a field of their own are
//! kept in `attributes`, and `<data>` contents as generic XML, so writing a
//! document back keeps all of its content. Only formatting may change, along
//! with the order of nodes and edges and any comments outside `<data>`.

use crate::xml::{self, Attributes, Element, XmlNode};

/// Defines an enum written in XML as one of a fixed set of keywords.
macro_rules! keyword_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident { $($(#[$variant_meta:meta])* $variant:ident = $keyword:literal,)+ }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name { $($(#[$variant_meta])* $variant,)+ }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $keyword,)+ }
            }

            pub(crate) fn from_keyword(keyword: &str) -> Option<Self> {
                match keyword { $($keyword => Some(Self::$variant),)+ _ => None }
            }
        }
    };
}

/// A GraphML document: the `<graphml>` root and everything in it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphMl {
    /// Comments at the top of the document, such as yEd's "Created by yEd".
    pub comments: Vec<String>,
    pub desc: Option<String>,
    pub keys: Vec<Key>,
    pub graphs: Vec<Graph>,
    pub data: Vec<Data>,
    /// Other attributes, such as namespace declarations.
    pub attributes: Attributes,
    /// The prefix of the GraphML elements, as in `<g:graphml xmlns:g="…">`.
    /// Usually `None`, with GraphML as the default namespace.
    pub prefix: Option<String>,
}

/// A `<key>`, declaring a data attribute that elements can have.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Key {
    pub id: String,
    pub domain: KeyDomain,
    pub attr_name: Option<String>,
    pub attr_type: Option<AttrType>,
    pub desc: Option<String>,
    pub default: Option<DefaultValue>,
    /// Other attributes, such as yEd's `yfiles.type`.
    pub attributes: Attributes,
}

/// A key's `<default>`: the value of elements without `<data>` for the key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DefaultValue {
    pub content: Vec<XmlNode>,
    pub attributes: Attributes,
}

impl DefaultValue {
    pub fn text(&self) -> String {
        xml::text(&self.content)
    }
}

keyword_enum! {
    /// The elements a key applies to (its `for` attribute).
    #[derive(Default)]
    pub enum KeyDomain {
        #[default]
        All = "all",
        GraphMl = "graphml",
        Graph = "graph",
        Node = "node",
        Edge = "edge",
        Hyperedge = "hyperedge",
        Port = "port",
        Endpoint = "endpoint",
    }
}

keyword_enum! {
    /// The value type of a key (its `attr.type` attribute).
    pub enum AttrType {
        Boolean = "boolean",
        Int = "int",
        Long = "long",
        Float = "float",
        Double = "double",
        String = "string",
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Graph {
    pub id: Option<String>,
    pub edge_default: EdgeDefault,
    pub desc: Option<String>,
    pub data: Vec<Data>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub hyperedges: Vec<Hyperedge>,
    pub locator: Option<Locator>,
    pub attributes: Attributes,
}

keyword_enum! {
    /// Whether a graph's edges are directed unless they say otherwise.
    #[derive(Default)]
    pub enum EdgeDefault {
        #[default]
        Directed = "directed",
        Undirected = "undirected",
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub desc: Option<String>,
    pub data: Vec<Data>,
    pub ports: Vec<Port>,
    /// A nested graph, as used by yEd's group nodes.
    pub graph: Option<Graph>,
    pub locator: Option<Locator>,
    pub attributes: Attributes,
}

impl Node {
    pub fn new(id: impl Into<String>) -> Self {
        Node {
            id: id.into(),
            ..Node::default()
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Edge {
    pub id: Option<String>,
    pub source: String,
    pub target: String,
    /// Overrides the graph's [`EdgeDefault`].
    pub directed: Option<bool>,
    pub source_port: Option<String>,
    pub target_port: Option<String>,
    pub desc: Option<String>,
    pub data: Vec<Data>,
    pub graph: Option<Graph>,
    pub attributes: Attributes,
}

impl Edge {
    pub fn new(source: impl Into<String>, target: impl Into<String>) -> Self {
        Edge {
            source: source.into(),
            target: target.into(),
            ..Edge::default()
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Port {
    pub name: String,
    pub desc: Option<String>,
    pub data: Vec<Data>,
    pub ports: Vec<Port>,
    pub attributes: Attributes,
}

/// An edge joining any number of nodes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hyperedge {
    pub id: Option<String>,
    pub desc: Option<String>,
    pub data: Vec<Data>,
    pub endpoints: Vec<Endpoint>,
    pub graph: Option<Graph>,
    pub attributes: Attributes,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Endpoint {
    pub id: Option<String>,
    pub node: String,
    pub port: Option<String>,
    /// The `type` attribute.
    pub direction: Option<Direction>,
    pub desc: Option<String>,
    pub attributes: Attributes,
}

keyword_enum! {
    /// How a hyperedge meets one of its endpoints.
    pub enum Direction {
        In = "in",
        Out = "out",
        Undirected = "undir",
    }
}

/// A `<locator>`, pointing at content stored elsewhere.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Locator {
    pub attributes: Attributes,
}

impl Locator {
    /// The `xlink:href` URI.
    pub fn href(&self) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(name, _)| xml::local_name(name) == "href")
            .map(|(_, value)| value.as_str())
    }
}

/// A `<data>` value: text, XML (such as yEd graphics), or both.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Data {
    pub key: String,
    pub id: Option<String>,
    pub content: Vec<XmlNode>,
    pub attributes: Attributes,
}

impl Data {
    pub fn from_text(key: impl Into<String>, text: impl Into<String>) -> Self {
        Data {
            key: key.into(),
            content: vec![XmlNode::Text(text.into())],
            ..Data::default()
        }
    }

    pub fn text(&self) -> String {
        xml::text(&self.content)
    }

    /// The first element in the content.
    pub fn element(&self) -> Option<&Element> {
        self.content.iter().find_map(XmlNode::as_element)
    }

    pub fn element_mut(&mut self) -> Option<&mut Element> {
        self.content.iter_mut().find_map(XmlNode::as_element_mut)
    }
}
