use std::io::Read;

use base64::{
    Engine, alphabet,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};
use flate2::read::{DeflateDecoder, ZlibDecoder};
use graphml::xml::Element;

use crate::{Error, Import, convert, model::Model, text::zap_gremlins};

/// Whether `text` holds a draw.io diagram rather than, say, GraphML: whether
/// its root element is `<mxfile>` or `<mxGraphModel>`.
pub fn is_drawio(text: &str) -> bool {
    matches!(
        Element::root_local_name(&zap_gremlins(text)).as_deref(),
        Ok("mxfile" | "mxGraphModel")
    )
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
        let root = Element::parse(&zap_gremlins(text))?;
        let pages = match root.local_name() {
            "mxGraphModel" => vec![Page::from_model(&root, page_name(0))?],
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
        let text = diagram.text();
        let mut children = diagram.elements();
        match (children.next(), text.trim().is_empty()) {
            (Some(model), true) if children.next().is_none() => Self::from_model(model, name),
            (None, true) => Ok(Page {
                name,
                model: Model::default(),
            }),
            (None, false) => {
                let xml = decompress(&text).map_err(|problem| Error::Compressed {
                    page: name.clone(),
                    problem,
                })?;
                Self::from_model(&Element::parse(&xml)?, name)
            }
            _ => Err(Error::InvalidPage {
                page: name,
                problem: "expected a single <mxGraphModel> or compressed text".to_owned(),
            }),
        }
    }

    fn from_model(element: &Element, name: String) -> Result<Self, Error> {
        match Model::read(element) {
            Ok(model) => Ok(Page { name, model }),
            Err(problem) => Err(Error::InvalidPage {
                page: name,
                problem,
            }),
        }
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
    // As lenient as browsers' atob, which draw.io decodes with.
    const ATOB: GeneralPurpose = GeneralPurpose::new(
        &alphabet::STANDARD,
        GeneralPurposeConfig::new()
            .with_decode_padding_mode(DecodePaddingMode::Indifferent)
            .with_decode_allow_trailing_bits(true),
    );
    let text: String = text.split_whitespace().collect();
    let bytes = ATOB.decode(text).map_err(|err| err.to_string())?;
    let mut inflated = Vec::new();
    if DeflateDecoder::new(&bytes[..])
        .read_to_end(&mut inflated)
        .is_err()
    {
        inflated.clear();
        (ZlibDecoder::new(&bytes[..]).read_to_end(&mut inflated)).map_err(|err| err.to_string())?;
    }
    let xml = decode_uri_component(&inflated)?;
    Ok(zap_gremlins(&xml).into_owned())
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
