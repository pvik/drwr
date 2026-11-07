use super::*;

fn parse_args(args: &[&str]) -> Result<Command, Error> {
    parse(args.iter().copied())
}

#[test]
fn no_arguments_runs_the_app() {
    assert_eq!(parse_args(&[]), Ok(Command::Run { file: None }));
}

#[test]
fn open_command() {
    let run = |file: &str| {
        Ok(Command::Run {
            file: Some(file.into()),
        })
    };
    assert_eq!(parse_args(&["-open", "a.graphml"]), run("a.graphml"));
    assert_eq!(parse_args(&["--open", "a.graphml"]), run("a.graphml"));
    // Whatever follows `-open` is the file, even if it looks like an option.
    assert_eq!(parse_args(&["-open", "-version"]), run("-version"));
}

#[test]
fn open_without_a_file_is_rejected() {
    assert_eq!(
        parse_args(&["-open"]),
        Err(Error::MissingValue("-open".into()))
    );
    assert_eq!(
        parse_args(&["--open"]),
        Err(Error::MissingValue("--open".into()))
    );
}

#[test]
fn version_command() {
    assert_eq!(parse_args(&["-version"]), Ok(Command::Version));
    assert_eq!(parse_args(&["--version"]), Ok(Command::Version));
}

#[test]
fn unknown_command_is_rejected() {
    assert_eq!(
        parse_args(&["-bogus"]),
        Err(Error::UnknownCommand("-bogus".into()))
    );
    // Near misses are not treated as aliases.
    assert_eq!(
        parse_args(&["version"]),
        Err(Error::UnknownCommand("version".into()))
    );
    assert_eq!(parse_args(&["-v"]), Err(Error::UnknownCommand("-v".into())));
}

#[test]
fn extra_arguments_are_rejected() {
    assert_eq!(
        parse_args(&["-version", "extra"]),
        Err(Error::UnexpectedArgument("extra".into()))
    );
    assert_eq!(
        parse_args(&["-open", "a.graphml", "b.graphml"]),
        Err(Error::UnexpectedArgument("b.graphml".into()))
    );
}

#[test]
fn errors_name_the_offending_argument() {
    assert_eq!(
        Error::UnknownCommand("-bogus".into()).to_string(),
        r#"unknown command "-bogus""#
    );
    assert_eq!(
        Error::UnexpectedArgument("extra".into()).to_string(),
        r#"unexpected argument "extra""#
    );
    assert_eq!(
        Error::MissingValue("-open".into()).to_string(),
        r#"expected a file after "-open""#
    );
}

/// `std::env::args` would panic on non-Unicode arguments.
#[cfg(unix)]
#[test]
fn non_unicode_argument_is_an_error() {
    use std::os::unix::ffi::OsStringExt;

    let arg = OsString::from_vec(vec![b'-', 0xff]);
    let err = parse([arg.clone()]).unwrap_err();
    assert_eq!(err, Error::UnknownCommand(arg));
    assert_eq!(err.to_string(), r#"unknown command "-\xFF""#);
}

#[cfg(unix)]
#[test]
fn non_unicode_file_is_kept() {
    use std::os::unix::ffi::OsStringExt;

    let file = OsString::from_vec(vec![b'a', 0xff]);
    assert_eq!(
        parse(["-open".into(), file.clone()]),
        Ok(Command::Run {
            file: Some(file.into())
        })
    );
}

#[test]
fn version_line_has_name_version_and_commit() {
    let commit = env!("DRWR_GIT_SHORT_COMMIT");
    assert_eq!(
        VERSION,
        format!("drwr {} ({commit})", env!("CARGO_PKG_VERSION"))
    );
    assert!(
        commit == "unknown" || commit.bytes().all(|b| b.is_ascii_hexdigit()),
        "expected a short hash or \"unknown\", got {commit:?}"
    );
}
