//! Write a Ninja build file with the typestate pattern
//!
//! A lot of features are missing. Currently, only the useful ones to build `ninja_bootstrap` are
//! implemented.

use std::error;
use std::ffi::OsStr;
use std::io::{self, Write};

use snafu::{ResultExt, Snafu};

#[must_use]
#[derive(Clone, Copy)]
pub struct Config {
    width: usize,
}

impl Config {
    pub const fn with_width(width: usize) -> Self {
        Self { width }
    }
}

// Opaque error type: https://docs.rs/snafu/0.7.5/snafu/guide/opaque/index.html
#[derive(Debug, Snafu)]
pub struct ErrorOr<E: error::Error + 'static>(InnerErrorOr<E>);

#[derive(Debug, Snafu)]
enum InnerErrorOr<E: error::Error + 'static> {
    #[snafu(display("failed to write a definition"))]
    Definition { source: Error },

    #[snafu(display("failure"))]
    Other { source: E },
}

// Opaque error type: https://docs.rs/snafu/0.7.5/snafu/guide/opaque/index.html
#[derive(Debug, Snafu)]
pub struct Error(InnerError);

#[derive(Debug, Snafu)]
enum InnerError {
    #[snafu(display("failed to write the beginning of the definition of the rule {rule_name:?}"))]
    Rule { source: io::Error, rule_name: String },
    #[snafu(display("failed to write, in a rule definition, the command {command:?}"))]
    Command { source: io::Error, command: String },
    #[snafu(display("failed to write the end of a rule definition"))]
    RuleEnd { source: io::Error },
    #[snafu(display("failed to write the beginning of a build definition"))]
    Beginning { source: io::Error },
    #[snafu(display("failed to write, in a build definition, the output {output:?}"))]
    Output { source: io::Error, output: String },
    #[snafu(display("failed to write, in a build definition, the rule {rule_name:?}"))]
    BuildRule { source: io::Error, rule_name: String },
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
    BuildEnd { source: io::Error },
}

#[allow(dead_code)] // TODO: remove this after using the config
pub struct NinjaWriter<W: Write> {
    config: Config,
    writer: W,
    current_line_size: usize,
}

impl<W: Write> NinjaWriter<W> {
    pub const fn new(config: Config, writer: W) -> Self {
        Self { config, writer, current_line_size: 0 }
    }

    pub fn rule(&mut self, rule_name: impl AsRef<[u8]>) -> Result<AfterRule<W>, Error> {
        assert!(self.current_line_size == 0);
        let rule_name = rule_name.as_ref();
        self.writer
            .write_all(b"rule ")
            .and_then(|_| self.writer.write_all(rule_name))
            .with_context(|_| RuleSnafu { rule_name: String::from_utf8_lossy(rule_name) })?;
        self.current_line_size = 5 + rule_name.len();
        Ok(AfterRule(self))
    }

    fn write_command(&mut self, command: &[u8]) -> Result<AfterCommand<W>, Error> {
        self.writer
            .write_all(b"\n  command = ")
            .and_then(|_| self.writer.write_all(command))
            .with_context(|_| CommandSnafu { command: String::from_utf8_lossy(command) })?;
        self.current_line_size = 12 + command.len();
        Ok(AfterCommand(self))
    }

    fn write_rule_end(&mut self) -> Result<(), Error> {
        self.writer.write_all(b"\n").context(RuleEndSnafu)?;
        self.current_line_size = 0;
        Ok(())
    }

    pub fn build(&mut self) -> Result<AfterBuild<W>, Error> {
        assert!(self.current_line_size == 0);
        self.writer.write_all(b"build").context(BeginningSnafu)?;
        self.current_line_size = 5;
        Ok(AfterBuild(self))
    }

    fn write_output(&mut self, output: &[u8]) -> Result<AfterOutput<W>, Error> {
        self.writer
            .write_all(b" ")
            .with_context(|_| OutputSnafu { output: String::from_utf8_lossy(output) })?;
        self.current_line_size += 1;
        self.write_escaped_path(output)
            .with_context(|_| OutputSnafu { output: String::from_utf8_lossy(output) })?;
        Ok(AfterOutput(self))
    }

    fn write_rule(&mut self, rule_name: &[u8]) -> Result<AfterBuildRule<W>, Error> {
        self.writer
            .write_all(b": ")
            .with_context(|_| BuildRuleSnafu { rule_name: String::from_utf8_lossy(rule_name) })?;
        self.current_line_size += 2;
        self.write_unescaped_text(rule_name)
            .with_context(|_| BuildRuleSnafu { rule_name: String::from_utf8_lossy(rule_name) })?;
        Ok(AfterBuildRule(self))
    }

    fn write_input(&mut self, input: &[u8]) -> Result<AfterInput<W>, Error> {
        self.writer
            .write_all(b" ")
            .with_context(|_| InputSnafu { input: String::from_utf8_lossy(input) })?;
        self.current_line_size += 1;
        self.write_escaped_path(input)
            .with_context(|_| InputSnafu { input: String::from_utf8_lossy(input) })?;
        Ok(AfterInput(self))
    }

    fn write_first_implicit_dependency(
        &mut self,
        dependency: &[u8],
    ) -> Result<AfterImplicitDependency<W>, Error> {
        self.writer.write_all(b" | ").with_context(|_| ImplicitDependencySnafu {
            dependency: String::from_utf8_lossy(dependency),
        })?;
        self.current_line_size += 3;
        self.write_escaped_path(dependency).with_context(|_| ImplicitDependencySnafu {
            dependency: String::from_utf8_lossy(dependency),
        })?;
        Ok(AfterImplicitDependency(self))
    }

    fn write_extra_implicit_dependency(
        &mut self,
        dependency: &[u8],
    ) -> Result<AfterImplicitDependency<W>, Error> {
        self.writer.write_all(b" ").with_context(|_| ImplicitDependencySnafu {
            dependency: String::from_utf8_lossy(dependency),
        })?;
        self.current_line_size += 1;
        self.write_escaped_path(dependency).with_context(|_| ImplicitDependencySnafu {
            dependency: String::from_utf8_lossy(dependency),
        })?;
        Ok(AfterImplicitDependency(self))
    }

    fn write_first_order_only_dependency(
        &mut self,
        dependency: &[u8],
    ) -> Result<AfterOrderOnlyDependency<W>, Error> {
        self.writer.write_all(b" || ").with_context(|_| OrderOnlyDependencySnafu {
            dependency: String::from_utf8_lossy(dependency),
        })?;
        self.current_line_size += 3;
        self.write_escaped_path(dependency).with_context(|_| OrderOnlyDependencySnafu {
            dependency: String::from_utf8_lossy(dependency),
        })?;
        Ok(AfterOrderOnlyDependency(self))
    }

    fn write_variable_and_value(
        &mut self,
        variable: &[u8],
        value: &[u8],
    ) -> Result<AfterVariableAndValue<W>, Error> {
        for bytes in [b"\n  ", variable, b" = ", value] {
            self.writer.write_all(bytes).with_context(|_| VariableAndValueSnafu {
                variable: String::from_utf8_lossy(variable),
                value: String::from_utf8_lossy(value),
            })?;
        }
        self.current_line_size = 5 + variable.len() + value.len();
        Ok(AfterVariableAndValue(self))
    }

    fn write_build_end(&mut self) -> Result<(), Error> {
        self.writer.write_all(b"\n").context(BuildEndSnafu)?;
        self.current_line_size = 0;
        Ok(())
    }

    fn write_unescaped_text(&mut self, text: &[u8]) -> io::Result<()> {
        let text_size = text.len();
        // "+ 2" because, in the worst case, the text could be followed by " $".
        if self.current_line_size + text_size + 2 > self.config.width {
            self.writer.write_all(b"$\n  ")?;
            self.current_line_size = 2;
        }
        self.writer.write_all(text)?;
        self.current_line_size += text_size;
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
    fn write_escaped_path(&mut self, path: &[u8]) -> io::Result<()> {
        let escaped_path_size = path.len()
            + path
                .iter()
                .filter(|byte| matches!(byte, b'$' | b' ' | b':' | b'|' | b'#' | b'\n'))
                .count();
        // "+ 5" because, in the worst case, the path could be followed by " || $".
        if self.current_line_size + escaped_path_size + 5 > self.config.width {
            self.writer.write_all(b"$\n  ")?;
            self.current_line_size = 2;
        }
        for &byte in path {
            match byte {
                b'$' | b' ' | b':' | b'|' | b'#' | b'\n' => self.writer.write_all(b"$")?,
                _ => (),
            };
            self.writer.write_all(&[byte])?;
        }
        self.current_line_size += escaped_path_size;
        Ok(())
    }
}

#[must_use]
pub struct AfterRule<'a, W: Write>(&'a mut NinjaWriter<W>);

#[must_use]
pub struct AfterCommand<'a, W: Write>(&'a mut NinjaWriter<W>);

#[must_use]
pub struct AfterBuild<'a, W: Write>(&'a mut NinjaWriter<W>);

#[must_use]
pub struct AfterOutput<'a, W: Write>(&'a mut NinjaWriter<W>);

#[must_use]
pub struct AfterBuildRule<'a, W: Write>(&'a mut NinjaWriter<W>);

#[must_use]
pub struct AfterInput<'a, W: Write>(&'a mut NinjaWriter<W>);

#[must_use]
pub struct AfterImplicitDependency<'a, W: Write>(&'a mut NinjaWriter<W>);

#[must_use]
pub struct AfterOrderOnlyDependency<'a, W: Write>(&'a mut NinjaWriter<W>);

#[must_use]
pub struct AfterVariableAndValue<'a, W: Write>(&'a mut NinjaWriter<W>);

#[must_use]
pub enum AfterBuildRuleOrInput<'a, W: Write> {
    AfterBuildRule(AfterBuildRule<'a, W>),
    AfterInput(AfterInput<'a, W>),
}

#[must_use]
pub enum AfterInputOrImplicitDependency<'a, W: Write> {
    AfterInput(AfterInput<'a, W>),
    AfterImplicitDependency(AfterImplicitDependency<'a, W>),
}

impl<'a, W: Write> AfterRule<'a, W> {
    pub fn command(self, command: impl AsRef<[u8]>) -> Result<AfterCommand<'a, W>, Error> {
        self.0.write_command(command.as_ref())
    }
}

impl<'a, W: Write> AfterCommand<'a, W> {
    pub fn end(self) -> Result<(), Error> {
        self.0.write_rule_end()
    }
}

impl<'a, W: Write> AfterBuild<'a, W> {
    pub fn output(self, output: impl AsRef<[u8]>) -> Result<AfterOutput<'a, W>, Error> {
        self.0.write_output(output.as_ref())
    }

    #[cfg(unix)]
    pub fn unix_output(self, output: impl AsRef<OsStr>) -> Result<AfterOutput<'a, W>, Error> {
        let output = std::os::unix::ffi::OsStrExt::as_bytes(output.as_ref());
        self.0.write_output(output)
    }
}

impl<'a, W: Write> AfterOutput<'a, W> {
    pub fn rule(self, rule_name: impl AsRef<[u8]>) -> Result<AfterBuildRule<'a, W>, Error> {
        self.0.write_rule(rule_name.as_ref())
    }
}

impl<'a, W: Write> AfterBuildRule<'a, W> {
    pub fn input(self, input: impl AsRef<[u8]>) -> Result<AfterInput<'a, W>, Error> {
        self.0.write_input(input.as_ref())
    }

    pub fn inputs(
        self,
        inputs: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Result<AfterBuildRuleOrInput<'a, W>, Error> {
        let mut inputs = inputs.into_iter();
        if let Some(input) = inputs.next() {
            let step = self.input(input)?;
            let step = step.inputs(inputs)?;
            Ok(AfterBuildRuleOrInput::AfterInput(step))
        } else {
            Ok(AfterBuildRuleOrInput::AfterBuildRule(self))
        }
    }

    pub fn end(self) -> Result<(), Error> {
        self.0.write_build_end()
    }
}

impl<'a, W: Write> AfterInput<'a, W> {
    fn input(self, input: impl AsRef<[u8]>) -> Result<Self, Error> {
        self.0.write_input(input.as_ref())
    }

    pub fn inputs(
        mut self,
        inputs: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Result<Self, Error> {
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
            self = self.input(input).context(DefinitionSnafu)?;
        }
        Ok(self)
    }

    fn implicit_dependency(
        self,
        dependency: impl AsRef<[u8]>,
    ) -> Result<AfterImplicitDependency<'a, W>, Error> {
        self.0.write_first_implicit_dependency(dependency.as_ref())
    }

    pub fn implicit_dependencies(
        self,
        dependencies: impl IntoIterator<Item = impl AsRef<[u8]>>,
    ) -> Result<AfterInputOrImplicitDependency<'a, W>, Error> {
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
    ) -> Result<AfterOrderOnlyDependency<'a, W>, Error> {
        let dependency = std::os::unix::ffi::OsStrExt::as_bytes(dependency.as_ref());
        self.0.write_first_order_only_dependency(dependency)
    }

    pub fn variable_and_value(
        self,
        variable: impl AsRef<[u8]>,
        value: impl AsRef<[u8]>,
    ) -> Result<AfterVariableAndValue<'a, W>, Error> {
        self.0.write_variable_and_value(variable.as_ref(), value.as_ref())
    }

    pub fn end(self) -> Result<(), Error> {
        self.0.write_build_end()
    }
}

impl<'a, W: Write> AfterImplicitDependency<'a, W> {
    fn implicit_dependency(self, dependency: impl AsRef<[u8]>) -> Result<Self, Error> {
        self.0.write_extra_implicit_dependency(dependency.as_ref())
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
    ) -> Result<AfterOrderOnlyDependency<'a, W>, Error> {
        let dependency = std::os::unix::ffi::OsStrExt::as_bytes(dependency.as_ref());
        self.0.write_first_order_only_dependency(dependency)
    }
}

impl<'a, W: Write> AfterOrderOnlyDependency<'a, W> {
    pub fn end(self) -> Result<(), Error> {
        self.0.write_build_end()
    }
}

impl<'a, W: Write> AfterVariableAndValue<'a, W> {
    pub fn end(self) -> Result<(), Error> {
        self.0.write_build_end()
    }
}

impl<'a, W: Write> AfterBuildRuleOrInput<'a, W> {
    pub fn end(self) -> Result<(), Error> {
        match self {
            Self::AfterBuildRule(step) => step.end(),
            Self::AfterInput(step) => step.end(),
        }
    }
}

impl<'a, W: Write> AfterInputOrImplicitDependency<'a, W> {
    #[cfg(unix)]
    pub fn unix_order_only_dependency(
        self,
        dependency: impl AsRef<OsStr>,
    ) -> Result<AfterOrderOnlyDependency<'a, W>, Error> {
        match self {
            Self::AfterInput(step) => step.unix_order_only_dependency(dependency),
            Self::AfterImplicitDependency(step) => step.unix_order_only_dependency(dependency),
        }
    }
}
