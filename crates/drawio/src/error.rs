use std::fmt;

/// Why a draw.io file can't be read.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The file, or a page in it, isn't well-formed XML.
    Xml(graphml::Error),
    /// The root element, by name, isn't `<mxfile>` or `<mxGraphModel>`.
    NotDrawio(String),
    /// A compressed page can't be decompressed.
    Compressed {
        page: String,
        problem: String,
    },
    NoPages,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Xml(err) => err.fmt(f),
            Error::NotDrawio(root) => {
                write!(f, "expected <mxfile> or <mxGraphModel>, found <{root}>")
            }
            Error::Compressed { page, problem } => {
                write!(f, "page {page:?} can't be decompressed: {problem}")
            }
            Error::NoPages => f.write_str("the file has no pages"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Xml(err) => Some(err),
            _ => None,
        }
    }
}

impl From<graphml::Error> for Error {
    fn from(err: graphml::Error) -> Self {
        Error::Xml(err)
    }
}
