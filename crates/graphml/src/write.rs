use std::{
    borrow::Cow,
    fs,
    io::{self, Write},
    mem,
    path::Path,
};

use crate::{
    Data, DefaultValue, Edge, Endpoint, Graph, GraphMl, Hyperedge, Key, Locator, Node, Port,
    model::AttrType,
    xml::{Attributes, Element, XmlNode},
};

impl GraphMl {
    /// Formats the document the way yEd does, so yEd's files round-trip
    /// byte for byte.
    pub fn to_xml(&self) -> String {
        let mut writer = XmlWriter {
            prefix: self.prefix.clone(),
            ..XmlWriter::default()
        };
        writer
            .out
            .push_str(r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>"#);
        self.write(&mut writer);
        writer.out.push('\n');
        writer.out
    }

    pub fn write_to(&self, mut writer: impl Write) -> io::Result<()> {
        writer.write_all(self.to_xml().as_bytes())
    }

    pub fn write_file(&self, path: impl AsRef<Path>) -> io::Result<()> {
        fs::write(path, self.to_xml())
    }
}

/// Writes XML indented by two spaces per level, except where that would add
/// to text: after text, and inside `xml:space="preserve"`. Java's serializer,
/// which yEd uses, follows the same rules.
#[derive(Default)]
struct XmlWriter {
    out: String,
    depth: usize,
    /// The last start tag still needs its `>`, or `/>` if nothing follows.
    tag_open: bool,
    after_text: bool,
    preserve: bool,
    /// The prefix of GraphML elements.
    prefix: Option<String>,
}

impl XmlWriter {
    /// Writes a GraphML element, with the document's prefix.
    fn element(
        &mut self,
        local_name: &str,
        attributes: Vec<(&str, &str)>,
        content: impl FnOnce(&mut Self),
    ) {
        let name = match &self.prefix {
            Some(prefix) => Cow::Owned(format!("{prefix}:{local_name}")),
            None => Cow::Borrowed(local_name),
        };
        self.xml_element(&name, attributes, content);
    }

    /// Writes an element named `name` as given, prefix and all.
    fn xml_element(
        &mut self,
        name: &str,
        mut attributes: Vec<(&str, &str)>,
        content: impl FnOnce(&mut Self),
    ) {
        attributes.sort_unstable_by_key(|&(name, _)| name);
        self.start_line();
        self.out.push('<');
        self.out.push_str(name);
        let mut preserve = self.preserve;
        for (name, value) in attributes {
            match (name, value) {
                ("xml:space", "preserve") => preserve = true,
                ("xml:space", "default") => preserve = false,
                _ => {}
            }
            self.out.push(' ');
            self.out.push_str(name);
            self.out.push_str("=\"");
            escape(value, true, &mut self.out);
            self.out.push('"');
        }

        let outer_preserve = mem::replace(&mut self.preserve, preserve);
        self.tag_open = true;
        self.after_text = false;
        self.depth += 1;
        content(self);
        self.depth -= 1;
        if self.tag_open {
            self.out.push_str("/>");
            self.tag_open = false;
        } else {
            self.indent();
            self.out.push_str("</");
            self.out.push_str(name);
            self.out.push('>');
        }
        self.preserve = outer_preserve;
        self.after_text = false;
    }

    fn text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.close_tag();
        escape(text, false, &mut self.out);
        self.after_text = true;
    }

    fn cdata(&mut self, text: &str) {
        self.close_tag();
        self.out.push_str("<![CDATA[");
        // `]]>` would end the section, so split it across two.
        self.out.push_str(&text.replace("]]>", "]]]]><![CDATA[>"));
        self.out.push_str("]]>");
        self.after_text = true;
    }

    fn comment(&mut self, text: &str) {
        self.start_line();
        self.out.push_str("<!--");
        self.out.push_str(text);
        self.out.push_str("-->");
        self.after_text = false;
    }

    fn desc(&mut self, desc: &Option<String>) {
        if let Some(desc) = desc {
            self.element("desc", Vec::new(), |w| w.text(desc));
        }
    }

    fn start_line(&mut self) {
        self.close_tag();
        self.indent();
    }

    fn close_tag(&mut self) {
        if self.tag_open {
            self.out.push('>');
            self.tag_open = false;
        }
    }

    fn indent(&mut self) {
        if !self.preserve && !self.after_text {
            self.out.push('\n');
            for _ in 0..self.depth {
                self.out.push_str("  ");
            }
        }
    }
}

/// Escapes like Java's serializer. `\r` is escaped so it isn't normalized away
/// when read back; in attributes, so are the other characters that readers
/// would turn into spaces.
fn escape(text: &str, in_attribute: bool, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#13;"),
            '"' if in_attribute => out.push_str("&quot;"),
            '\n' if in_attribute => out.push_str("&#10;"),
            '\t' if in_attribute => out.push_str("&#9;"),
            c => out.push(c),
        }
    }
}

/// An element's modelled attributes (if set) together with its other ones.
fn attributes<'a, const N: usize>(
    other: &'a Attributes,
    modelled: [(&'a str, Option<&'a str>); N],
) -> Vec<(&'a str, &'a str)> {
    let other = other
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .filter(|(name, _)| modelled.iter().all(|(modelled, _)| modelled != name));
    let modelled = modelled
        .into_iter()
        .filter_map(|(name, value)| Some((name, value?)));
    other.chain(modelled).collect()
}

trait WriteXml {
    fn write(&self, w: &mut XmlWriter);
}

impl<T: WriteXml> WriteXml for Vec<T> {
    fn write(&self, w: &mut XmlWriter) {
        for item in self {
            item.write(w);
        }
    }
}

impl<T: WriteXml> WriteXml for Option<T> {
    fn write(&self, w: &mut XmlWriter) {
        if let Some(item) = self {
            item.write(w);
        }
    }
}

impl WriteXml for GraphMl {
    fn write(&self, w: &mut XmlWriter) {
        w.element("graphml", attributes(&self.attributes, []), |w| {
            for comment in &self.comments {
                w.comment(comment);
            }
            w.desc(&self.desc);
            self.keys.write(w);
            self.graphs.write(w);
            self.data.write(w);
        });
    }
}

impl WriteXml for Key {
    fn write(&self, w: &mut XmlWriter) {
        let modelled = [
            ("id", Some(self.id.as_str())),
            ("for", Some(self.domain.as_str())),
            ("attr.name", self.attr_name.as_deref()),
            ("attr.type", self.attr_type.map(AttrType::as_str)),
        ];
        w.element("key", attributes(&self.attributes, modelled), |w| {
            w.desc(&self.desc);
            self.default.write(w);
        });
    }
}

impl WriteXml for DefaultValue {
    fn write(&self, w: &mut XmlWriter) {
        w.element("default", attributes(&self.attributes, []), |w| {
            self.content.write(w);
        });
    }
}

impl WriteXml for Graph {
    fn write(&self, w: &mut XmlWriter) {
        let modelled = [
            ("id", self.id.as_deref()),
            ("edgedefault", Some(self.edge_default.as_str())),
        ];
        w.element("graph", attributes(&self.attributes, modelled), |w| {
            w.desc(&self.desc);
            self.data.write(w);
            self.nodes.write(w);
            self.edges.write(w);
            self.hyperedges.write(w);
            self.locator.write(w);
        });
    }
}

impl WriteXml for Node {
    fn write(&self, w: &mut XmlWriter) {
        let modelled = [("id", Some(self.id.as_str()))];
        w.element("node", attributes(&self.attributes, modelled), |w| {
            w.desc(&self.desc);
            self.data.write(w);
            self.ports.write(w);
            self.graph.write(w);
            self.locator.write(w);
        });
    }
}

impl WriteXml for Edge {
    fn write(&self, w: &mut XmlWriter) {
        let modelled = [
            ("id", self.id.as_deref()),
            ("source", Some(self.source.as_str())),
            ("target", Some(self.target.as_str())),
            (
                "directed",
                self.directed.map(|d| if d { "true" } else { "false" }),
            ),
            ("sourceport", self.source_port.as_deref()),
            ("targetport", self.target_port.as_deref()),
        ];
        w.element("edge", attributes(&self.attributes, modelled), |w| {
            w.desc(&self.desc);
            self.data.write(w);
            self.graph.write(w);
        });
    }
}

impl WriteXml for Port {
    fn write(&self, w: &mut XmlWriter) {
        let modelled = [("name", Some(self.name.as_str()))];
        w.element("port", attributes(&self.attributes, modelled), |w| {
            w.desc(&self.desc);
            self.data.write(w);
            self.ports.write(w);
        });
    }
}

impl WriteXml for Hyperedge {
    fn write(&self, w: &mut XmlWriter) {
        let modelled = [("id", self.id.as_deref())];
        w.element("hyperedge", attributes(&self.attributes, modelled), |w| {
            w.desc(&self.desc);
            self.data.write(w);
            self.endpoints.write(w);
            self.graph.write(w);
        });
    }
}

impl WriteXml for Endpoint {
    fn write(&self, w: &mut XmlWriter) {
        let modelled = [
            ("id", self.id.as_deref()),
            ("node", Some(self.node.as_str())),
            ("port", self.port.as_deref()),
            ("type", self.direction.map(|d| d.as_str())),
        ];
        w.element("endpoint", attributes(&self.attributes, modelled), |w| {
            w.desc(&self.desc);
        });
    }
}

impl WriteXml for Locator {
    fn write(&self, w: &mut XmlWriter) {
        w.element("locator", attributes(&self.attributes, []), |_| {});
    }
}

impl WriteXml for Data {
    fn write(&self, w: &mut XmlWriter) {
        let modelled = [("key", Some(self.key.as_str())), ("id", self.id.as_deref())];
        w.element("data", attributes(&self.attributes, modelled), |w| {
            self.content.write(w);
        });
    }
}

impl WriteXml for XmlNode {
    fn write(&self, w: &mut XmlWriter) {
        match self {
            XmlNode::Element(element) => element.write(w),
            XmlNode::Text(text) => w.text(text),
            XmlNode::CData(text) => w.cdata(text),
            XmlNode::Comment(text) => w.comment(text),
        }
    }
}

impl WriteXml for Element {
    fn write(&self, w: &mut XmlWriter) {
        w.xml_element(&self.name, attributes(&self.attributes, []), |w| {
            self.children.write(w);
        });
    }
}
