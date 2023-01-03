#![warn(clippy::nursery, clippy::pedantic)]

//! Functions to write a Ninja build file
//!
//! A lot of features are missing. Currently, only the one useful to build `ninja_bootstrap` are
//! implemented.

use std::collections::BTreeMap;
use std::io::{self, Write};

pub fn dump_rule(mut writer: impl Write, rule_name: &[u8], command: &[u8]) -> io::Result<()> {
    for bytes in [b"rule ", rule_name, b"\n  command = ", command, b"\n"] {
        writer.write_all(bytes)?;
    }
    Ok(())
}

pub fn dump_build(
    mut writer: impl Write,
    outputs: impl IntoIterator<Item = impl Into<Vec<u8>>>,
    rule_name: &[u8],
    inputs: impl IntoIterator<Item = impl Into<Vec<u8>>>,
    implicit_dependencies: impl IntoIterator<Item = impl Into<Vec<u8>>>,
    order_only_dependencies: impl IntoIterator<Item = impl Into<Vec<u8>>>,
    variables: BTreeMap<Vec<u8>, Vec<u8>>,
) -> io::Result<()> {
    writer.write_all(b"build")?;
    for output in outputs {
        writer.write_all(b" ")?;
        dump_escaped_path(&mut writer, &output.into())?;
    }
    writer.write_all(b": ")?;
    writer.write_all(rule_name)?;
    for input in inputs {
        writer.write_all(b" ")?;
        dump_escaped_path(&mut writer, &input.into())?;
    }
    let mut implicit_dependencies = implicit_dependencies.into_iter();
    if let Some(dependency) = implicit_dependencies.next() {
        writer.write_all(b" | ")?;
        dump_escaped_path(&mut writer, &dependency.into())?;
        for dependency in implicit_dependencies {
            writer.write_all(b" ")?;
            dump_escaped_path(&mut writer, &dependency.into())?;
        }
    }
    let mut order_only_dependencies = order_only_dependencies.into_iter();
    if let Some(dependency) = order_only_dependencies.next() {
        writer.write_all(b" || ")?;
        dump_escaped_path(&mut writer, &dependency.into())?;
        for dependency in order_only_dependencies {
            writer.write_all(b" ")?;
            dump_escaped_path(&mut writer, &dependency.into())?;
        }
    }
    for (variable, value) in variables {
        for bytes in [b"\n  ", &variable[..], b" = ", &value[..]] {
            writer.write_all(bytes)?;
        }
    }
    writer.write_all(b"\n")
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
