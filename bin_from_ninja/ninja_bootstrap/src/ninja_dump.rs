#![warn(clippy::nursery, clippy::pedantic)]

//! Functions to write a Ninja build file
//!
//! A lot of features are missing. Currently, only the one useful to build `ninja_bootstrap` are
//! implemented.

use thiserror::Error;

use std::collections::BTreeMap;
use std::io::{self, Write};

pub fn dump_rule(mut writer: impl Write, rule_name: &[u8], command: &[u8]) -> io::Result<()> {
    for bytes in [b"rule ", rule_name, b"\n  command = ", command, b"\n"] {
        writer.write_all(bytes)?;
    }
    Ok(())
}

#[derive(Error, Debug)]
pub enum DumpBuildError<OE, IE, IDE, OODE> {
    #[error("io error")]
    Io(#[from] io::Error),
    #[error("input error")]
    Output(OE),
    #[error("input error")]
    Input(IE),
    #[error("implicit dependency")]
    ImplicitDependency(IDE),
    #[error("order-only dependency")]
    OrderOnlyDependency(OODE),
}

pub fn dump_build<OE, IE, IDE, OODE>(
    mut writer: impl Write,
    outputs: impl Iterator<Item = Result<Vec<u8>, OE>>,
    rule_name: &[u8],
    inputs: impl Iterator<Item = Result<Vec<u8>, IE>>,
    mut implicit_dependencies: impl Iterator<Item = Result<Vec<u8>, IDE>>,
    mut order_only_dependencies: impl Iterator<Item = Result<Vec<u8>, OODE>>,
    variables: BTreeMap<Vec<u8>, Vec<u8>>,
) -> Result<(), DumpBuildError<OE, IE, IDE, OODE>> {
    writer.write_all(b"build")?;
    for output in outputs {
        let output = output.map_err(DumpBuildError::Output)?;
        writer.write_all(b" ")?;
        dump_escaped_path(&mut writer, &output)?;
    }
    writer.write_all(b": ")?;
    writer.write_all(rule_name)?;
    for input in inputs {
        let input = input.map_err(DumpBuildError::Input)?;
        writer.write_all(b" ")?;
        dump_escaped_path(&mut writer, &input)?;
    }
    if let Some(dependency) = implicit_dependencies.next() {
        let dependency = dependency.map_err(DumpBuildError::ImplicitDependency)?;
        writer.write_all(b" | ")?;
        dump_escaped_path(&mut writer, &dependency)?;
        for dependency in implicit_dependencies {
            let dependency = dependency.map_err(DumpBuildError::ImplicitDependency)?;
            writer.write_all(b" ")?;
            dump_escaped_path(&mut writer, &dependency)?;
        }
    }
    if let Some(dependency) = order_only_dependencies.next() {
        let dependency = dependency.map_err(DumpBuildError::OrderOnlyDependency)?;
        writer.write_all(b" || ")?;
        dump_escaped_path(&mut writer, &dependency)?;
        for dependency in order_only_dependencies {
            let dependency = dependency.map_err(DumpBuildError::OrderOnlyDependency)?;
            writer.write_all(b" ")?;
            dump_escaped_path(&mut writer, &dependency)?;
        }
    }
    for (variable, value) in variables {
        for bytes in [b"\n  ", &variable[..], b" = ", &value[..]] {
            writer.write_all(bytes)?;
        }
    }
    writer.write_all(b"\n")?;
    Ok(())
}

/// Dump an escaped path by adding `b'$'` before the bytes in `b"$ :|#\n"`.
///
/// In the GitHub repository of Ninja, `ninja_syntax.py` escapes `'$'`, `' '` and `':'`:
/// <https://github.com/ninja-build/ninja/blob/v1.11.1/misc/ninja_syntax.py#L27-L28>
///
/// `b'|'` must be escaped too. Otherwise, a file called `|` or  `||` could be seen as a
/// separator: <https://ninja-build.org/manual.html#ref_ninja_file>
///
/// `b'#'` must be escaped too. The Ninja documentation says "Comments begin with # and extend to
/// the end of the line.": <https://ninja-build.org/manual.html#ref_lexer>
///
/// `b'\n'` must be escaped too. The Ninja documentation says: "Newlines are significant.":
/// <https://ninja-build.org/manual.html#ref_lexer>
fn dump_escaped_path(mut writer: impl Write, rule_name: &[u8]) -> io::Result<()> {
    for &byte in rule_name {
        match byte {
            b'$' | b' ' | b':' | b'|' | b'#' | b'\n' => writer.write_all(b"$")?,
            _ => (),
        };
        writer.write_all(&[byte])?;
    }
    Ok(())
}
