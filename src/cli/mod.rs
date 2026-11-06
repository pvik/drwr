//! Command-line parsing.
//!
//! [`parse`] has no side effects: it turns the arguments into a [`Command`]
//! (or an [`Error`]) and leaves printing and exiting to `main`.

use std::ffi::OsString;
use std::fmt;

/// The line printed by `-version`, e.g. `drwr 0.1.0 (5a37b68)`.
///
/// `DRWR_GIT_SHORT_COMMIT` is set by `build.rs`.
pub const VERSION: &str = concat!(
    env!("CARGO_PKG_NAME"),
    " ",
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("DRWR_GIT_SHORT_COMMIT"),
    ")"
);

/// One-line usage summary, shown alongside parse errors.
pub const USAGE: &str = concat!("usage: ", env!("CARGO_PKG_NAME"), " [-version]");

/// What the program was asked to do.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// No arguments were given: open the window.
    Run,
    /// `-version`: print the build version and exit.
    Version,
}

/// A command line that could not be understood.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// The first argument is not a known command.
    UnknownCommand(OsString),
    /// The command was followed by more arguments than it takes.
    UnexpectedArgument(OsString),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `Debug` quotes the argument and escapes anything that isn't valid
        // Unicode, so it can be echoed back safely.
        match self {
            Error::UnknownCommand(arg) => write!(f, "unknown command {arg:?}"),
            Error::UnexpectedArgument(arg) => write!(f, "unexpected argument {arg:?}"),
        }
    }
}

impl std::error::Error for Error {}

/// Parses the command-line arguments, not including the program name.
///
/// At most one command is accepted; `-version` may also be spelled
/// `--version`.
pub fn parse<I, T>(args: I) -> Result<Command, Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into);

    let command = match args.next() {
        None => return Ok(Command::Run),
        Some(arg) if arg == "-version" || arg == "--version" => Command::Version,
        Some(arg) => return Err(Error::UnknownCommand(arg)),
    };

    match args.next() {
        None => Ok(command),
        Some(extra) => Err(Error::UnexpectedArgument(extra)),
    }
}

#[cfg(test)]
mod tests;
