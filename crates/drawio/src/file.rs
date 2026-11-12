use std::io::Read;

use base64::Engine;
use flate2::read::{DeflateDecoder, ZlibDecoder};
use graphml::xml::Element;

use crate::{Error, Import, convert, model::Model};

/// Whether `text` holds a draw.io diagram rather than, say, GraphML: whether
/// its root element is `<mxfile>` or `<mxGraphModel>`.
pub fn is_drawio(text: &str) -> bool {
    matches!(root_name(text), Some("mxfile" | "mxGraphModel"))
}

/// The local name of the root element, found without parsing the document.
fn root_name(text: &str) -> Option<&str> {
    let mut rest = text.strip_prefix('\u{feff}').unwrap_or(text);
    loop {
        rest = rest.trim_start();
        if let Some(comment) = rest.strip_prefix("<!--") {
            rest = &comment[comment.find("-->")? + 3..];
        } else if rest.starts_with("<?") || rest.starts_with("<!") {
            rest = &rest[rest.find('>')? + 1..];
        } else {
            let tag = rest.strip_prefix('<')?;
            let end = tag
                .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .unwrap_or(tag.len());
            let name = &tag[..end];
            return Some(name.rsplit(':').next().unwrap_or(name));
        }
    }
}

/// The pages of a draw.io file.
#[derive(Clone, Debug)]
pub struct File {
    pages: Vec<Page>,
}

/// A page of a draw.io file.
#[derive(Clone, Debug)]
pub struct Page {
    name: String,
    model: Model,
}

impl File {
    /// Reads a draw.io file: an `<mxfile>` of pages, compressed or not, or a
    /// lone `<mxGraphModel>`.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let root = Element::parse(text)?;
        let pages = match root.local_name() {
            "mxGraphModel" => vec![Page {
                name: page_name(0),
                model: Model::read(&root),
            }],
            "mxfile" => (root.children_named("diagram").enumerate())
                .map(|(index, diagram)| Page::read(diagram, index))
                .collect::<Result<_, _>>()?,
            _ => return Err(Error::NotDrawio(root.name)),
        };
        if pages.is_empty() {
            return Err(Error::NoPages);
        }
        Ok(File { pages })
    }

    pub fn pages(&self) -> &[Page] {
        &self.pages
    }
}

impl Page {
    fn read(diagram: &Element, index: usize) -> Result<Self, Error> {
        let name = diagram
            .attr("name")
            .map_or_else(|| page_name(index), str::to_owned);
        let model = match diagram.child("mxGraphModel") {
            Some(model) => Model::read(model),
            None => {
                let text = diagram.text();
                if text.trim().is_empty() {
                    Model::default()
                } else {
                    let xml = decompress(&text).map_err(|problem| Error::Compressed {
                        page: name.clone(),
                        problem,
                    })?;
                    Model::read(&Element::parse(&xml)?)
                }
            }
        };
        Ok(Page { name, model })
    }

    /// The name on the page's tab in draw.io.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The page as GraphML, drawn with yEd's graphics.
    pub fn to_graphml(&self) -> Import {
        convert::convert(&self.model)
    }
}

/// The name draw.io gives the page at `index`.
fn page_name(index: usize) -> String {
    format!("Page-{}", index + 1)
}

/// Undoes draw.io's compression: Base64 of raw Deflate (zlib in old files)
/// of the URI-encoded XML.
fn decompress(text: &str) -> Result<String, String> {
    let text: String = text.split_whitespace().collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|err| err.to_string())?;
    let mut inflated = Vec::new();
    if DeflateDecoder::new(&bytes[..])
        .read_to_end(&mut inflated)
        .is_err()
    {
        inflated.clear();
        (ZlibDecoder::new(&bytes[..]).read_to_end(&mut inflated)).map_err(|err| err.to_string())?;
    }
    decode_uri_component(&inflated)
}

/// Decodes `%XX` escapes, as JavaScript's `decodeURIComponent` does.
fn decode_uri_component(text: &[u8]) -> Result<String, String> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut rest = text;
    while let [first, tail @ ..] = rest {
        rest = tail;
        if *first != b'%' {
            bytes.push(*first);
            continue;
        }
        let hex = tail
            .get(..2)
            .filter(|hex| hex.iter().all(u8::is_ascii_hexdigit))
            .ok_or("invalid % escape")?;
        let hex = std::str::from_utf8(hex).expect("checked to be ASCII");
        bytes.push(u8::from_str_radix(hex, 16).expect("checked to be hex"));
        rest = &tail[2..];
    }
    String::from_utf8(bytes).map_err(|_| "the text isn't UTF-8".to_owned())
}
