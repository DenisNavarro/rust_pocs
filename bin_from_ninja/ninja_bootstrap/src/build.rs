//! Write a build definition in a Ninja build file with the typestate pattern
//!
//! A lot of features are missing. Currently, only the useful ones to build `ninja_bootstrap` are
//! implemented.

use std::error;
use std::ffi::OsStr;
use std::io::{self, Write};

use snafu::{ResultExt, Snafu};

// Opaque error type: https://docs.rs/snafu/0.7.4/snafu/guide/opaque/index.html
#[derive(Debug, Snafu)]
pub struct ErrorOr<E: error::Error + 'static>(InnerErrorOr<E>);

#[derive(Debug, Snafu)]
enum InnerErrorOr<E: error::Error + 'static> {
    #[snafu(display("failed to write a build definition"))]
    Build { source: Error },

    #[snafu(display("failure"))]
    Other { source: E },
}

// Opaque error type: https://docs.rs/snafu/0.7.4/snafu/guide/opaque/index.html
#[derive(Debug, Snafu)]
pub struct Error(InnerError);

#[derive(Debug, Snafu)]
enum InnerError {
    #[snafu(display("failed to write the beginning of a build definition"))]
    Beginning { source: io::Error },
    #[snafu(display("failed to write, in a build definition, the output {output:?}"))]
    Output { source: io::Error, output: String },
    #[snafu(display("failed to write, in a build definition, the rule {rule_name:?}"))]
    Rule { source: io::Error, rule_name: String },
    #[snafu(display("failed to write, in a build definition, the input {input:?}"))]
    Input { source: io::Error, input: String },
    #[snafu(display(
        "failed to write, in a build definition, the implicit dependency {dependency:?}"
    ))]
    ImplicitDependency { source: io::Error, dependency: String },
    #[snafu(display(
        "failed to write, in a build definition, the order-only dependency {dependency:?}"
    ))]
    OrderOnlyDependency { source: io::Error, dependency: String },
    #[snafu(display("failed to write, in a build definition, the variable {variable:?} with the value {value:?}"))]
    VariableAndValue { source: io::Error, variable: String, value: String },
    #[snafu(display("failed to write the end of a build definition"))]
    End { source: io::Error },
}

#[allow(clippy::module_name_repetitions)]
pub fn build<W: Write>(mut writer: W) -> Result<AfterBuild<W>, Error> {
    writer.write_all(b"build").context(BeginningSnafu)?;
    Ok(AfterBuild { writer })
}

fn write_output<W: Write>(mut writer: W, output: &[u8]) -> Result<AfterOutput<W>, Error> {
    writer
        .write_all(b" ")
        .and_then(|_| write_escaped_path(&mut writer, output))
        .with_context(|_| OutputSnafu { output: String::from_utf8_lossy(output) })?;
    Ok(AfterOutput { writer })
}

fn write_rule<W: Write>(mut writer: W, rule_name: &[u8]) -> Result<AfterRule<W>, Error> {
    writer
        .write_all(b": ")
        .and_then(|_| writer.write_all(rule_name))
        .with_context(|_| RuleSnafu { rule_name: String::from_utf8_lossy(rule_name) })?;
    Ok(AfterRule { writer })
}

fn write_input<W: Write>(mut writer: W, input: &[u8]) -> Result<AfterInput<W>, Error> {
    writer
        .write_all(b" ")
        .and_then(|_| write_escaped_path(&mut writer, input))
        .with_context(|_| InputSnafu { input: String::from_utf8_lossy(input) })?;
    Ok(AfterInput { writer })
}

fn write_first_implicit_dependency<W: Write>(
    mut writer: W,
    dependency: &[u8],
) -> Result<AfterImplicitDependency<W>, Error> {
    writer
        .write_all(b" | ")
        .and_then(|_| write_escaped_path(&mut writer, dependency))
        .with_context(|_| ImplicitDependencySnafu {
            dependency: String::from_utf8_lossy(dependency),
        })?;
    Ok(AfterImplicitDependency { writer })
}

fn write_extra_implicit_dependency<W: Write>(
    mut writer: W,
    dependency: &[u8],
) -> Result<AfterImplicitDependency<W>, Error> {
    writer.write_all(b" ").and_then(|_| write_escaped_path(&mut writer, dependency)).with_context(
        |_| ImplicitDependencySnafu { dependency: String::from_utf8_lossy(dependency) },
    )?;
    Ok(AfterImplicitDependency { writer })
}

fn write_first_order_only_dependency<W: Write>(
    mut writer: W,
    dependency: &[u8],
) -> Result<AfterOrderOnlyDependency<W>, Error> {
    writer
        .write_all(b" || ")
        .and_then(|_| write_escaped_path(&mut writer, dependency))
        .with_context(|_| OrderOnlyDependencySnafu {
            dependency: String::from_utf8_lossy(dependency),
        })?;
    Ok(AfterOrderOnlyDependency { writer })
}

fn write_variable_and_value<W: Write>(
    mut writer: W,
    variable: &[u8],
    value: &[u8],
) -> Result<AfterVariableAndValue<W>, Error> {
    for bytes in [b"\n  ", variable, b" = ", value] {
        writer.write_all(bytes).with_context(|_| VariableAndValueSnafu {
            variable: String::from_utf8_lossy(variable),
            value: String::from_utf8_lossy(value),
        })?;
    }
    Ok(AfterVariableAndValue { writer })
}

fn write_end(mut writer: impl Write) -> Result<(), Error> {
    writer.write_all(b"\n").context(EndSnafu)?;
    Ok(())
}

/// Write an escaped path by adding `b'$'` before the bytes in `b"$ :|#\n"`.
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
fn write_escaped_path(mut writer: impl Write, path: &[u8]) -> io::Result<()> {
    for &byte in path {
        match byte {
            b'$' | b' ' | b':' | b'|' | b'#' | b'\n' => writer.write_all(b"$")?,
            _ => (),
        };
        writer.write_all(&[byte])?;
    }
    Ok(())
}

#[allow(clippy::module_name_repetitions)]
#[must_use]
pub struct AfterBuild<W: Write> {
    writer: W,
}

#[must_use]
pub struct AfterOutput<W: Write> {
    writer: W,
}

#[must_use]
pub struct AfterRule<W: Write> {
    writer: W,
}

#[must_use]
pub struct AfterInput<W: Write> {
    writer: W,
}

#[must_use]
pub struct AfterImplicitDependency<W: Write> {
    writer: W,
}

#[must_use]
pub struct AfterOrderOnlyDependency<W: Write> {
    writer: W,
}

#[must_use]
pub struct AfterVariableAndValue<W: Write> {
    writer: W,
}

#[must_use]
pub enum AfterRuleOrInput<W: Write> {
    AfterRule(AfterRule<W>),
    AfterInput(AfterInput<W>),
}

#[must_use]
pub enum AfterInputOrImplicitDependency<W: Write> {
    AfterInput(AfterInput<W>),
    AfterImplicitDependency(AfterImplicitDependency<W>),
}

impl<W: Write> AfterBuild<W> {
    pub fn output(self, output: impl AsRef<[u8]>) -> Result<AfterOutput<W>, Error> {
        write_output(self.writer, output.as_ref())
    }

    #[cfg(unix)]
    pub fn unix_output(self, output: impl AsRef<OsStr>) -> Result<AfterOutput<W>, Error> {
        let output = std::os::unix::ffi::OsStrExt::as_bytes(output.as_ref());
        write_output(self.writer, output)
    }
}

impl<W: Write> AfterOutput<W> {
    pub fn rule(self, rule_name: impl AsRef<[u8]>) -> Result<AfterRule<W>, Error> {
        write_rule(self.writer, rule_name.as_ref())
    }
}

impl<W: Write> AfterRule<W> {
    pub fn input(self, input: impl AsRef<[u8]>) -> Result<AfterInput<W>, Error> {
        write_input(self.writer, input.as_ref())
    }

    pub fn inputs(
        self,
        inputs: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Result<AfterRuleOrInput<W>, Error> {
        let mut inputs = inputs.into_iter();
        if let Some(input) = inputs.next() {
            let step = self.input(input)?;
            let step = step.inputs(inputs)?;
            Ok(AfterRuleOrInput::AfterInput(step))
        } else {
            Ok(AfterRuleOrInput::AfterRule(self))
        }
    }

    fn variable_and_value(
        self,
        variable: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<AfterVariableAndValue<W>, Error> {
        write_variable_and_value(self.writer, variable.as_ref(), value.as_ref())
    }

    pub fn end(self) -> Result<(), Error> {
        write_end(self.writer)
    }
}

impl<W: Write> AfterInput<W> {
    fn input(self, input: impl AsRef<[u8]>) -> Result<Self, Error> {
        write_input(self.writer, input.as_ref())
    }

    fn inputs(mut self, inputs: impl IntoIterator<Item = impl AsRef<[u8]>>) -> Result<Self, Error> {
        for input in inputs {
            self = self.input(input)?;
        }
        Ok(self)
    }

    #[cfg(unix)]
    pub fn unix_input_results<E: error::Error + 'static>(
        mut self,
        inputs: impl IntoIterator<Item = Result<impl AsRef<OsStr>, E>>,
    ) -> Result<Self, ErrorOr<E>> {
        for input in inputs {
            let input = input.context(OtherSnafu)?;
            let input = std::os::unix::ffi::OsStrExt::as_bytes(input.as_ref());
            self = self.input(input).context(BuildSnafu)?;
        }
        Ok(self)
    }

    fn implicit_dependency(
        self,
        dependency: impl AsRef<[u8]>,
    ) -> Result<AfterImplicitDependency<W>, Error> {
        write_first_implicit_dependency(self.writer, dependency.as_ref())
    }

    pub fn implicit_dependencies(
        self,
        dependencies: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Result<AfterInputOrImplicitDependency<W>, Error> {
        let mut dependencies = dependencies.into_iter();
        if let Some(dependency) = dependencies.next() {
            let step = self.implicit_dependency(dependency)?;
            let step = step.implicit_dependencies(dependencies)?;
            Ok(AfterInputOrImplicitDependency::AfterImplicitDependency(step))
        } else {
            Ok(AfterInputOrImplicitDependency::AfterInput(self))
        }
    }

    #[cfg(unix)]
    fn unix_order_only_dependency(
        self,
        dependency: impl AsRef<OsStr>,
    ) -> Result<AfterOrderOnlyDependency<W>, Error> {
        let dependency = std::os::unix::ffi::OsStrExt::as_bytes(dependency.as_ref());
        write_first_order_only_dependency(self.writer, dependency)
    }

    pub fn variable_and_value(
        self,
        variable: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<AfterVariableAndValue<W>, Error> {
        write_variable_and_value(self.writer, variable.as_ref(), value.as_ref())
    }

    fn end(self) -> Result<(), Error> {
        write_end(self.writer)
    }
}

impl<W: Write> AfterImplicitDependency<W> {
    fn implicit_dependency(self, dependency: impl AsRef<[u8]>) -> Result<Self, Error> {
        write_extra_implicit_dependency(self.writer, dependency.as_ref())
    }

    fn implicit_dependencies(
        mut self,
        dependencies: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Result<Self, Error> {
        for dependency in dependencies {
            self = self.implicit_dependency(dependency)?;
        }
        Ok(self)
    }

    #[cfg(unix)]
    fn unix_order_only_dependency(
        self,
        dependency: impl AsRef<OsStr>,
    ) -> Result<AfterOrderOnlyDependency<W>, Error> {
        let dependency = std::os::unix::ffi::OsStrExt::as_bytes(dependency.as_ref());
        write_first_order_only_dependency(self.writer, dependency)
    }
}

impl<W: Write> AfterOrderOnlyDependency<W> {
    pub fn end(self) -> Result<(), Error> {
        write_end(self.writer)
    }
}

impl<W: Write> AfterVariableAndValue<W> {
    pub fn end(self) -> Result<(), Error> {
        write_end(self.writer)
    }
}

impl<W: Write> AfterRuleOrInput<W> {
    pub fn variable_and_value(
        self,
        variable: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<AfterVariableAndValue<W>, Error> {
        match self {
            Self::AfterRule(step) => step.variable_and_value(variable, value),
            Self::AfterInput(step) => step.variable_and_value(variable, value),
        }
    }

    pub fn end(self) -> Result<(), Error> {
        match self {
            Self::AfterRule(step) => step.end(),
            Self::AfterInput(step) => step.end(),
        }
    }
}

impl<W: Write> AfterInputOrImplicitDependency<W> {
    #[cfg(unix)]
    pub fn unix_order_only_dependency(
        self,
        dependency: impl AsRef<OsStr>,
    ) -> Result<AfterOrderOnlyDependency<W>, Error> {
        match self {
            Self::AfterInput(step) => step.unix_order_only_dependency(dependency),
            Self::AfterImplicitDependency(step) => step.unix_order_only_dependency(dependency),
        }
    }
}
