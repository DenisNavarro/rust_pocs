//! Write a rule definition in a Ninja build file with the typestate pattern
//!
//! Some features are missing. Currently, only the useful ones to build `ninja_bootstrap` are
//! implemented.

use std::io::{self, Write};

use snafu::{ResultExt, Snafu};

// Opaque error type: https://docs.rs/snafu/0.7.4/snafu/guide/opaque/index.html
#[derive(Debug, Snafu)]
pub struct Error(InnerError);

#[derive(Debug, Snafu)]
enum InnerError {
    #[snafu(display("failed to write the beginning of the definition of the rule {rule_name:?}"))]
    Rule { source: io::Error, rule_name: String },
    #[snafu(display("failed to write, in a rule definition, the command {command:?}"))]
    Command { source: io::Error, command: String },
    #[snafu(display("failed to write the end of a rule definition"))]
    End { source: io::Error },
}

#[allow(clippy::module_name_repetitions)]
pub fn rule<W: Write>(
    mut writer: W,
    rule_name: &(impl AsRef<[u8]> + ?Sized),
) -> Result<AfterRule<W>, Error> {
    let rule_name = rule_name.as_ref();
    writer
        .write_all(b"rule ")
        .and_then(|_| writer.write_all(rule_name))
        .with_context(|_| RuleSnafu { rule_name: String::from_utf8_lossy(rule_name) })?;
    Ok(AfterRule { writer })
}

fn write_command<W: Write>(mut writer: W, command: &[u8]) -> Result<AfterCommand<W>, Error> {
    writer
        .write_all(b"\n  command = ")
        .and_then(|_| writer.write_all(command))
        .with_context(|_| CommandSnafu { command: String::from_utf8_lossy(command) })?;
    Ok(AfterCommand { writer })
}

fn write_end(mut writer: impl Write) -> Result<(), Error> {
    writer.write_all(b"\n").context(EndSnafu)?;
    Ok(())
}

#[allow(clippy::module_name_repetitions)]
#[must_use]
pub struct AfterRule<W: Write> {
    writer: W,
}

#[must_use]
pub struct AfterCommand<W: Write> {
    writer: W,
}

impl<W: Write> AfterRule<W> {
    pub fn command(self, command: impl AsRef<[u8]>) -> Result<AfterCommand<W>, Error> {
        write_command(self.writer, command.as_ref())
    }
}

impl<W: Write> AfterCommand<W> {
    pub fn end(self) -> Result<(), Error> {
        write_end(self.writer)
    }
}
