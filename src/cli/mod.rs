//! Command-line parsing.

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

/// The `-version` line, e.g. `drwr 0.1.0 (5a37b68)`; `build.rs` sets the hash.
pub const VERSION: &str = concat!(
    env!("CARGO_PKG_NAME"),
    " ",
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("DRWR_GIT_SHORT_COMMIT"),
    ")"
);

/// One-line usage summary, shown alongside parse errors.
pub const USAGE: &str = concat!(
    "usage: ",
    env!("CARGO_PKG_NAME"),
    " [-version | -open <file>]"
);

/// What the program was asked to do.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// Open the window, first loading `file` if `-open` gave one.
    Run { file: Option<PathBuf> },
    /// `-version`: print the build version and exit.
    Version,
}

/// A command line that could not be understood.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    UnknownCommand(OsString),
    UnexpectedArgument(OsString),
    MissingValue(OsString),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `Debug` quotes the argument and escapes any non-Unicode bytes.
        match self {
            Error::UnknownCommand(arg) => write!(f, "unknown command {arg:?}"),
            Error::UnexpectedArgument(arg) => write!(f, "unexpected argument {arg:?}"),
            Error::MissingValue(arg) => write!(f, "expected a file after {arg:?}"),
        }
    }
}

impl std::error::Error for Error {}

/// Parses the command-line arguments, not including the program name.
pub fn parse<I, T>(args: I) -> Result<Command, Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into);

    let command = match args.next() {
        None => return Ok(Command::Run { file: None }),
        Some(arg) if arg == "-version" || arg == "--version" => Command::Version,
        Some(arg) if arg == "-open" || arg == "--open" => match args.next() {
            Some(file) => Command::Run {
                file: Some(file.into()),
            },
            None => return Err(Error::MissingValue(arg)),
        },
        Some(arg) => return Err(Error::UnknownCommand(arg)),
    };

    match args.next() {
        None => Ok(command),
        Some(extra) => Err(Error::UnexpectedArgument(extra)),
    }
}

#[cfg(test)]
mod tests;
