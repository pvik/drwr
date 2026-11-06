use std::{fs, io::Read, mem, path::Path};

use quick_xml::{
    Reader, XmlVersion,
    escape::{EscapeError, resolve_predefined_entity},
    events::{BytesRef, BytesStart, Event, attributes::AttrError},
};

use crate::{
    Data, DefaultValue, Direction, Edge, EdgeDefault, Endpoint, Error, Graph, GraphMl, Hyperedge,
    Key, KeyDomain, Locator, Node, Port, Position,
    model::AttrType,
    xml::{self, Attributes, Element, XmlNode},
};

impl GraphMl {
    pub fn parse(text: &str) -> Result<Self, Error> {
        Parser::new(text).document()
    }

    pub fn from_reader(mut reader: impl Read) -> Result<Self, Error> {
        let mut text = String::new();
        reader.read_to_string(&mut text)?;
        Self::parse(&text)
    }

    pub fn read_file(path: impl AsRef<Path>) -> Result<Self, Error> {
        Self::parse(&fs::read_to_string(path)?)
    }
}

struct Parser<'a> {
    reader: Reader<&'a [u8]>,
    text: &'a str,
    /// Comments before the root element or directly inside it.
    comments: Vec<String>,
}

/// The start tag of the element being parsed.
struct Tag<'a> {
    start: BytesStart<'a>,
    /// Written as `<tag/>`, so there's no content or end tag to read.
    empty: bool,
    offset: u64,
    /// Whitespace-only text inside is content, not indentation. Inherited
    /// from the parent until the tag's own `xml:space` is read.
    preserve: bool,
}

impl<'a> Tag<'a> {
    fn new(start: BytesStart<'a>, empty: bool, offset: u64, preserve: bool) -> Self {
        Tag {
            start,
            empty,
            offset,
            preserve,
        }
    }

    fn name(&self) -> &str {
        self.start.name().into_inner()
    }

    fn local_name(&self) -> &str {
        xml::local_name(self.name())
    }

    fn prefix(&self) -> Option<&str> {
        xml::prefix(self.name())
    }
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        // Stripped here rather than by the reader, so offsets index `text`.
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        Parser {
            reader: Reader::from_str(text),
            text,
            comments: Vec::new(),
        }
    }

    fn event(&mut self) -> Result<(u64, Event<'a>), Error> {
        let offset = self.reader.buffer_position();
        match self.reader.read_event() {
            Ok(event) => Ok((offset, event)),
            Err(err) => Err(self.error(self.reader.error_position(), err.to_string())),
        }
    }

    fn error(&self, offset: u64, message: impl Into<String>) -> Error {
        Error::Parse {
            position: Position::locate(self.text, offset as usize),
            message: message.into(),
        }
    }

    fn unexpected(&self, child: &Tag, parent: &Tag) -> Error {
        let message = format!("unexpected <{}> in <{}>", child.name(), parent.name());
        self.error(child.offset, message)
    }

    fn document(mut self) -> Result<GraphMl, Error> {
        let root = loop {
            match self.event()? {
                (offset, Event::Start(start)) => break Tag::new(start, false, offset, false),
                (offset, Event::Empty(start)) => break Tag::new(start, true, offset, false),
                (_, Event::Comment(comment)) => self.comments.push(comment.xml10_content().into()),
                (_, Event::Decl(_) | Event::DocType(_) | Event::PI(_)) => {}
                (_, Event::Text(text)) if is_whitespace(&text) => {}
                (offset, Event::Eof) => return Err(self.error(offset, "no <graphml> element")),
                (offset, _) => return Err(self.error(offset, "unexpected text")),
            }
        };
        if root.local_name() != "graphml" {
            let message = format!("expected <graphml>, found <{}>", root.name());
            return Err(self.error(root.offset, message));
        }
        let doc = self.graphml(root)?;
        loop {
            match self.event()? {
                (_, Event::Eof) => return Ok(doc),
                (_, Event::Comment(_) | Event::PI(_)) => {}
                (_, Event::Text(text)) if is_whitespace(&text) => {}
                (offset, _) => {
                    return Err(self.error(offset, "unexpected content after </graphml>"));
                }
            }
        }
    }

    /// Reads up to `parent`'s next child element, or past its end tag. Text
    /// other than whitespace is an error, since GraphML keeps text in leaves.
    fn child(&mut self, parent: &Tag) -> Result<Option<Tag<'a>>, Error> {
        if parent.empty {
            return Ok(None);
        }
        let child = loop {
            match self.event()? {
                (offset, Event::Start(start)) => {
                    break Tag::new(start, false, offset, parent.preserve);
                }
                (offset, Event::Empty(start)) => {
                    break Tag::new(start, true, offset, parent.preserve);
                }
                (_, Event::End(_)) => return Ok(None),
                (_, Event::Comment(comment)) => {
                    if parent.local_name() == "graphml" {
                        self.comments.push(comment.xml10_content().into());
                    }
                }
                (_, Event::PI(_)) => {}
                (_, Event::Text(text)) if is_whitespace(&text) => {}
                (_, Event::Eof) => {
                    let message = format!("<{}> is never closed", parent.name());
                    return Err(self.error(parent.offset, message));
                }
                (offset, _) => {
                    let message = format!("unexpected text in <{}>", parent.name());
                    return Err(self.error(offset, message));
                }
            }
        };
        // GraphML elements all have the root's prefix (usually none), so an
        // element with another prefix is from another namespace.
        if child.prefix() != parent.prefix() {
            return Err(self.unexpected(&child, parent));
        }
        Ok(Some(child))
    }

    /// Parses `child` into `slot`, which mustn't be filled already.
    fn once<T>(
        &mut self,
        slot: &mut Option<T>,
        child: Tag<'a>,
        parent: &Tag,
        parse: fn(&mut Self, Tag<'a>) -> Result<T, Error>,
    ) -> Result<(), Error> {
        if slot.is_some() {
            return Err(self.unexpected(&child, parent));
        }
        *slot = Some(parse(self, child)?);
        Ok(())
    }

    /// Reads `tag`'s attributes, and applies its `xml:space` to `tag.preserve`.
    fn attributes(&self, tag: &mut Tag) -> Result<Attributes, Error> {
        let mut attributes = Attributes::new();
        for attribute in tag.start.attributes() {
            let attribute = attribute.map_err(|err| self.attribute_syntax_error(tag, err))?;
            let name = attribute.key.into_inner();
            let value = attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|err| {
                    let problem = match err {
                        quick_xml::Error::Escape(EscapeError::UnrecognizedEntity(_, entity)) => {
                            format!("unknown entity &{entity};")
                        }
                        quick_xml::Error::Escape(EscapeError::UnterminatedEntity(_)) => {
                            "`&` without a closing `;`".to_owned()
                        }
                        err => err.to_string(),
                    };
                    let message = format!("invalid `{name}` on <{}>: {problem}", tag.name());
                    self.error(tag.offset, message)
                })?;
            attributes.insert(name.to_owned(), value.into_owned());
        }
        tag.preserve = preserves_space(&attributes, tag.preserve);
        Ok(attributes)
    }

    fn attribute_syntax_error(&self, tag: &Tag, err: AttrError) -> Error {
        let (position, message) = match err {
            AttrError::ExpectedEq(position) => (position, "expected `=` after the attribute name"),
            AttrError::ExpectedValue(position) => (position, "expected a value after `=`"),
            AttrError::UnquotedValue(position) => (position, "attribute values must be quoted"),
            AttrError::ExpectedQuote(position, _) => {
                (position, "an attribute value is never closed")
            }
            AttrError::Duplicated(position, _) => (position, "duplicate attribute"),
        };
        // The position counts from just after the tag's `<`.
        self.error(tag.offset + 1 + position as u64, message)
    }

    fn required(
        &self,
        attributes: &mut Attributes,
        name: &str,
        tag: &Tag,
    ) -> Result<String, Error> {
        attributes.remove(name).ok_or_else(|| {
            let message = format!("<{}> is missing the `{name}` attribute", tag.name());
            self.error(tag.offset, message)
        })
    }

    fn keyword<T>(
        &self,
        attributes: &mut Attributes,
        name: &str,
        tag: &Tag,
        parse: fn(&str) -> Option<T>,
    ) -> Result<Option<T>, Error> {
        let Some(value) = attributes.remove(name) else {
            return Ok(None);
        };
        parse(&value).map(Some).ok_or_else(|| {
            let message = format!("invalid value {value:?} for `{name}` on <{}>", tag.name());
            self.error(tag.offset, message)
        })
    }

    fn graphml(&mut self, mut tag: Tag<'a>) -> Result<GraphMl, Error> {
        let mut doc = GraphMl {
            attributes: self.attributes(&mut tag)?,
            prefix: tag.prefix().map(str::to_owned),
            ..GraphMl::default()
        };
        while let Some(child) = self.child(&tag)? {
            match child.local_name() {
                "desc" => self.once(&mut doc.desc, child, &tag, Self::desc)?,
                "key" => doc.keys.push(self.key(child)?),
                "graph" => doc.graphs.push(self.graph(child)?),
                "data" => doc.data.push(self.data(child)?),
                _ => return Err(self.unexpected(&child, &tag)),
            }
        }
        doc.comments = mem::take(&mut self.comments);
        Ok(doc)
    }

    fn key(&mut self, mut tag: Tag<'a>) -> Result<Key, Error> {
        let mut attributes = self.attributes(&mut tag)?;
        let mut key = Key {
            id: self.required(&mut attributes, "id", &tag)?,
            domain: self
                .keyword(&mut attributes, "for", &tag, KeyDomain::from_keyword)?
                .unwrap_or_default(),
            attr_name: attributes.remove("attr.name"),
            attr_type: self.keyword(&mut attributes, "attr.type", &tag, AttrType::from_keyword)?,
            ..Key::default()
        };
        while let Some(child) = self.child(&tag)? {
            match child.local_name() {
                "desc" => self.once(&mut key.desc, child, &tag, Self::desc)?,
                "default" => self.once(&mut key.default, child, &tag, Self::default_value)?,
                _ => return Err(self.unexpected(&child, &tag)),
            }
        }
        key.attributes = attributes;
        Ok(key)
    }

    fn graph(&mut self, mut tag: Tag<'a>) -> Result<Graph, Error> {
        let mut attributes = self.attributes(&mut tag)?;
        let mut graph = Graph {
            id: attributes.remove("id"),
            edge_default: self
                .keyword(
                    &mut attributes,
                    "edgedefault",
                    &tag,
                    EdgeDefault::from_keyword,
                )?
                .unwrap_or_default(),
            ..Graph::default()
        };
        while let Some(child) = self.child(&tag)? {
            match child.local_name() {
                "desc" => self.once(&mut graph.desc, child, &tag, Self::desc)?,
                "data" => graph.data.push(self.data(child)?),
                "node" => graph.nodes.push(self.node(child)?),
                "edge" => graph.edges.push(self.edge(child)?),
                "hyperedge" => graph.hyperedges.push(self.hyperedge(child)?),
                "locator" => self.once(&mut graph.locator, child, &tag, Self::locator)?,
                _ => return Err(self.unexpected(&child, &tag)),
            }
        }
        graph.attributes = attributes;
        Ok(graph)
    }

    fn node(&mut self, mut tag: Tag<'a>) -> Result<Node, Error> {
        let mut attributes = self.attributes(&mut tag)?;
        let mut node = Node::new(self.required(&mut attributes, "id", &tag)?);
        while let Some(child) = self.child(&tag)? {
            match child.local_name() {
                "desc" => self.once(&mut node.desc, child, &tag, Self::desc)?,
                "data" => node.data.push(self.data(child)?),
                "port" => node.ports.push(self.port(child)?),
                "graph" => self.once(&mut node.graph, child, &tag, Self::graph)?,
                "locator" => self.once(&mut node.locator, child, &tag, Self::locator)?,
                _ => return Err(self.unexpected(&child, &tag)),
            }
        }
        node.attributes = attributes;
        Ok(node)
    }

    fn edge(&mut self, mut tag: Tag<'a>) -> Result<Edge, Error> {
        let mut attributes = self.attributes(&mut tag)?;
        let mut edge = Edge {
            id: attributes.remove("id"),
            source: self.required(&mut attributes, "source", &tag)?,
            target: self.required(&mut attributes, "target", &tag)?,
            directed: self.keyword(&mut attributes, "directed", &tag, parse_bool)?,
            source_port: attributes.remove("sourceport"),
            target_port: attributes.remove("targetport"),
            ..Edge::default()
        };
        while let Some(child) = self.child(&tag)? {
            match child.local_name() {
                "desc" => self.once(&mut edge.desc, child, &tag, Self::desc)?,
                "data" => edge.data.push(self.data(child)?),
                "graph" => self.once(&mut edge.graph, child, &tag, Self::graph)?,
                _ => return Err(self.unexpected(&child, &tag)),
            }
        }
        edge.attributes = attributes;
        Ok(edge)
    }

    fn port(&mut self, mut tag: Tag<'a>) -> Result<Port, Error> {
        let mut attributes = self.attributes(&mut tag)?;
        let mut port = Port {
            name: self.required(&mut attributes, "name", &tag)?,
            ..Port::default()
        };
        while let Some(child) = self.child(&tag)? {
            match child.local_name() {
                "desc" => self.once(&mut port.desc, child, &tag, Self::desc)?,
                "data" => port.data.push(self.data(child)?),
                "port" => port.ports.push(self.port(child)?),
                _ => return Err(self.unexpected(&child, &tag)),
            }
        }
        port.attributes = attributes;
        Ok(port)
    }

    fn hyperedge(&mut self, mut tag: Tag<'a>) -> Result<Hyperedge, Error> {
        let mut attributes = self.attributes(&mut tag)?;
        let mut hyperedge = Hyperedge {
            id: attributes.remove("id"),
            ..Hyperedge::default()
        };
        while let Some(child) = self.child(&tag)? {
            match child.local_name() {
                "desc" => self.once(&mut hyperedge.desc, child, &tag, Self::desc)?,
                "data" => hyperedge.data.push(self.data(child)?),
                "endpoint" => hyperedge.endpoints.push(self.endpoint(child)?),
                "graph" => self.once(&mut hyperedge.graph, child, &tag, Self::graph)?,
                _ => return Err(self.unexpected(&child, &tag)),
            }
        }
        hyperedge.attributes = attributes;
        Ok(hyperedge)
    }

    fn endpoint(&mut self, mut tag: Tag<'a>) -> Result<Endpoint, Error> {
        let mut attributes = self.attributes(&mut tag)?;
        let mut endpoint = Endpoint {
            id: attributes.remove("id"),
            node: self.required(&mut attributes, "node", &tag)?,
            port: attributes.remove("port"),
            direction: self.keyword(&mut attributes, "type", &tag, Direction::from_keyword)?,
            ..Endpoint::default()
        };
        while let Some(child) = self.child(&tag)? {
            match child.local_name() {
                "desc" => self.once(&mut endpoint.desc, child, &tag, Self::desc)?,
                _ => return Err(self.unexpected(&child, &tag)),
            }
        }
        endpoint.attributes = attributes;
        Ok(endpoint)
    }

    fn locator(&mut self, mut tag: Tag<'a>) -> Result<Locator, Error> {
        let attributes = self.attributes(&mut tag)?;
        if let Some(child) = self.child(&tag)? {
            return Err(self.unexpected(&child, &tag));
        }
        Ok(Locator { attributes })
    }

    fn desc(&mut self, mut tag: Tag<'a>) -> Result<String, Error> {
        // Plain text, so none of its whitespace is indentation.
        tag.preserve = true;
        let content = self.content(&tag)?;
        if let Some(element) = content.iter().find_map(XmlNode::as_element) {
            let message = format!("unexpected <{}> in <{}>", element.name, tag.name());
            return Err(self.error(tag.offset, message));
        }
        Ok(xml::text(&content))
    }

    fn default_value(&mut self, mut tag: Tag<'a>) -> Result<DefaultValue, Error> {
        let attributes = self.attributes(&mut tag)?;
        let content = self.content(&tag)?;
        Ok(DefaultValue {
            content,
            attributes,
        })
    }

    fn data(&mut self, mut tag: Tag<'a>) -> Result<Data, Error> {
        let mut attributes = self.attributes(&mut tag)?;
        let key = self.required(&mut attributes, "key", &tag)?;
        let content = self.content(&tag)?;
        Ok(Data {
            key,
            id: attributes.remove("id"),
            content,
            attributes,
        })
    }

    fn element(&mut self, mut tag: Tag<'a>) -> Result<Element, Error> {
        let attributes = self.attributes(&mut tag)?;
        let children = self.content(&tag)?;
        Ok(Element {
            name: tag.name().to_owned(),
            attributes,
            children,
        })
    }

    /// Reads `tag`'s content as generic XML. Whitespace-only text is taken for
    /// indentation and dropped, unless `tag.preserve`.
    fn content(&mut self, tag: &Tag<'a>) -> Result<Vec<XmlNode>, Error> {
        let mut nodes = Vec::new();
        if tag.empty {
            return Ok(nodes);
        }
        // Text arrives in pieces, split at each entity reference.
        let mut text = String::new();
        loop {
            let (offset, event) = self.event()?;
            match event {
                Event::Text(piece) => {
                    text.push_str(&piece.xml10_content());
                    continue;
                }
                Event::GeneralRef(reference) => {
                    self.resolve(&reference, offset, &mut text)?;
                    continue;
                }
                _ => {}
            }
            if !text.is_empty() && (tag.preserve || !is_whitespace(&text)) {
                nodes.push(XmlNode::Text(mem::take(&mut text)));
            }
            text.clear();
            match event {
                Event::Start(start) => {
                    let element = self.element(Tag::new(start, false, offset, tag.preserve))?;
                    nodes.push(XmlNode::Element(element));
                }
                Event::Empty(start) => {
                    let element = self.element(Tag::new(start, true, offset, tag.preserve))?;
                    nodes.push(XmlNode::Element(element));
                }
                Event::CData(cdata) => nodes.push(XmlNode::CData(cdata.xml10_content().into())),
                Event::Comment(comment) => {
                    nodes.push(XmlNode::Comment(comment.xml10_content().into()))
                }
                Event::End(_) => return Ok(nodes),
                Event::Eof => {
                    let message = format!("<{}> is never closed", tag.name());
                    return Err(self.error(tag.offset, message));
                }
                _ => {}
            }
        }
    }

    fn resolve(&self, reference: &BytesRef, offset: u64, text: &mut String) -> Result<(), Error> {
        let char_ref = reference
            .resolve_char_ref()
            .map_err(|err| self.error(offset, err.to_string()))?;
        if let Some(c) = char_ref {
            text.push(c);
        } else if let Some(replacement) = resolve_predefined_entity(reference) {
            text.push_str(replacement);
        } else {
            return Err(self.error(offset, format!("unknown entity &{};", &**reference)));
        }
        Ok(())
    }
}

/// Whether `xml:space` in `attributes` (or else the parent's setting)
/// says to keep whitespace-only text.
fn preserves_space(attributes: &Attributes, inherited: bool) -> bool {
    match attributes.get("xml:space").map(String::as_str) {
        Some("preserve") => true,
        Some("default") => false,
        _ => inherited,
    }
}

fn is_whitespace(text: &str) -> bool {
    text.bytes()
        .all(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
}

/// Parses an XML Schema boolean.
fn parse_bool(text: &str) -> Option<bool> {
    match text {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}
