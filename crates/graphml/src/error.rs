use std::{fmt, io};

/// An error from reading GraphML or interpreting its yEd graphics.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    Io(io::Error),
    /// The input isn't well-formed XML, or isn't valid GraphML.
    Parse {
        position: Position,
        message: String,
    },
    /// A yEd attribute holds a value that can't be parsed.
    InvalidValue {
        element: String,
        attribute: String,
        value: String,
    },
}

/// A 1-based line and column (in characters) in the input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    /// Finds the position of byte `offset` in `text`.
    pub(crate) fn locate(text: &str, offset: usize) -> Self {
        let mut offset = offset.min(text.len());
        while !text.is_char_boundary(offset) {
            offset -= 1;
        }
        let before = &text[..offset];
        let line_start = before.rfind('\n').map_or(0, |i| i + 1);
        Position {
            line: before.matches('\n').count() + 1,
            column: before[line_start..].chars().count() + 1,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(err) => err.fmt(f),
            Error::Parse { position, message } => {
                write!(
                    f,
                    "line {}, column {}: {message}",
                    position.line, position.column
                )
            }
            Error::InvalidValue {
                element,
                attribute,
                value,
            } => write!(
                f,
                "invalid value {value:?} for `{attribute}` on <{element}>"
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}
