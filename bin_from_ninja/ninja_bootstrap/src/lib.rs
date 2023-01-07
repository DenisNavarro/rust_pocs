#![warn(clippy::nursery, clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

//! Builders to call the functions from the `ninja_dump` module in a more readable way

mod ninja_dump;

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::ffi::OsString;
use std::io::{self, Write};
use std::iter;
use std::path::PathBuf;

use ninja_dump::DumpBuildError;

pub fn rule(name: &(impl AsRef<[u8]> + ?Sized)) -> Rule<'_> {
    Rule(name.as_ref())
}

#[derive(Debug, Clone, Copy)]
pub struct Rule<'r>(&'r [u8]);

#[derive(Debug, Clone, Copy)]
pub struct RuleWithCommand<'r, 'c> {
    rule_name: &'r [u8],
    command: &'c [u8],
}

pub struct Build<'r, O, OE, I, IE, ID, IDE, OOD, OODE>
where
    O: Iterator<Item = Result<Vec<u8>, OE>>,
    I: Iterator<Item = Result<Vec<u8>, IE>>,
    ID: Iterator<Item = Result<Vec<u8>, IDE>>,
    OOD: Iterator<Item = Result<Vec<u8>, OODE>>,
{
    outputs: O,
    rule_name: &'r [u8],
    inputs: I,
    implicit_dependencies: ID,
    order_only_dependencies: OOD,
    variables: BTreeMap<Vec<u8>, Vec<u8>>,
}

type Empty = iter::Empty<Result<Vec<u8>, Infallible>>;

impl<'r> Rule<'r> {
    #[must_use]
    pub fn command(self, command: &(impl AsRef<[u8]> + ?Sized)) -> RuleWithCommand<'r, '_> {
        RuleWithCommand { rule_name: self.0, command: command.as_ref() }
    }

    #[must_use]
    pub fn outputs(
        self,
        outputs: impl IntoIterator<Item = impl Into<Vec<u8>>>,
    ) -> Build<
        'r,
        impl Iterator<Item = Result<Vec<u8>, Infallible>>,
        Infallible,
        Empty,
        Infallible,
        Empty,
        Infallible,
        Empty,
        Infallible,
    > {
        Build {
            outputs: outputs.into_iter().map(|x| Ok(x.into())),
            rule_name: self.0,
            inputs: iter::empty(),
            implicit_dependencies: iter::empty(),
            order_only_dependencies: iter::empty(),
            variables: BTreeMap::new(),
        }
    }

    #[cfg(unix)]
    #[must_use]
    pub fn output_unix_paths(
        self,
        outputs: impl IntoIterator<Item = impl Into<PathBuf>>,
    ) -> Build<
        'r,
        impl Iterator<Item = Result<Vec<u8>, Infallible>>,
        Infallible,
        Empty,
        Infallible,
        Empty,
        Infallible,
        Empty,
        Infallible,
    > {
        Build {
            outputs: outputs
                .into_iter()
                .map(|x| Ok(std::os::unix::ffi::OsStringExt::into_vec(OsString::from(x.into())))),
            rule_name: self.0,
            inputs: iter::empty(),
            implicit_dependencies: iter::empty(),
            order_only_dependencies: iter::empty(),
            variables: BTreeMap::new(),
        }
    }
}

impl<'r, 'c> RuleWithCommand<'r, 'c> {
    pub fn dump_rule(self, writer: impl Write) -> io::Result<()> {
        ninja_dump::dump_rule(writer, self.rule_name, self.command)
    }
}

impl<'r, O, OE, I, IE, ID, IDE, OOD, OODE> Build<'r, O, OE, I, IE, ID, IDE, OOD, OODE>
where
    O: Iterator<Item = Result<Vec<u8>, OE>>,
    I: Iterator<Item = Result<Vec<u8>, IE>>,
    ID: Iterator<Item = Result<Vec<u8>, IDE>>,
    OOD: Iterator<Item = Result<Vec<u8>, OODE>>,
{
    #[must_use]
    pub fn inputs(
        self,
        new_value: impl IntoIterator<Item = impl Into<Vec<u8>>>,
    ) -> Build<
        'r,
        O,
        OE,
        impl Iterator<Item = Result<Vec<u8>, Infallible>>,
        Infallible,
        ID,
        IDE,
        OOD,
        OODE,
    > {
        Build {
            outputs: self.outputs,
            rule_name: self.rule_name,
            inputs: new_value.into_iter().map(|x| Ok(x.into())),
            implicit_dependencies: self.implicit_dependencies,
            order_only_dependencies: self.order_only_dependencies,
            variables: self.variables,
        }
    }

    #[cfg(unix)]
    #[must_use]
    pub fn input_path_results<E>(
        self,
        new_value: impl IntoIterator<Item = Result<PathBuf, E>>,
    ) -> Build<'r, O, OE, impl Iterator<Item = Result<Vec<u8>, E>>, E, ID, IDE, OOD, OODE> {
        Build {
            outputs: self.outputs,
            rule_name: self.rule_name,
            inputs: new_value.into_iter().map(|x| {
                x.map(|path| std::os::unix::ffi::OsStringExt::into_vec(OsString::from(path)))
            }),
            implicit_dependencies: self.implicit_dependencies,
            order_only_dependencies: self.order_only_dependencies,
            variables: self.variables,
        }
    }

    #[must_use]
    pub fn implicit_dependencies(
        self,
        new_value: impl IntoIterator<Item = impl Into<Vec<u8>>>,
    ) -> Build<
        'r,
        O,
        OE,
        I,
        IE,
        impl Iterator<Item = Result<Vec<u8>, Infallible>>,
        Infallible,
        OOD,
        OODE,
    > {
        Build {
            outputs: self.outputs,
            rule_name: self.rule_name,
            inputs: self.inputs,
            implicit_dependencies: new_value.into_iter().map(|x| Ok(x.into())),
            order_only_dependencies: self.order_only_dependencies,
            variables: self.variables,
        }
    }

    #[cfg(unix)]
    #[must_use]
    pub fn order_only_dependency_unix_paths(
        self,
        new_value: impl IntoIterator<Item = impl Into<PathBuf>>,
    ) -> Build<
        'r,
        O,
        OE,
        I,
        IE,
        ID,
        IDE,
        impl Iterator<Item = Result<Vec<u8>, Infallible>>,
        Infallible,
    > {
        Build {
            outputs: self.outputs,
            rule_name: self.rule_name,
            inputs: self.inputs,
            implicit_dependencies: self.implicit_dependencies,
            order_only_dependencies: new_value
                .into_iter()
                .map(|x| Ok(std::os::unix::ffi::OsStringExt::into_vec(OsString::from(x.into())))),
            variables: self.variables,
        }
    }

    #[must_use]
    pub fn variable(mut self, variable: impl Into<Vec<u8>>, value: impl Into<Vec<u8>>) -> Self {
        self.variables.insert(variable.into(), value.into());
        self
    }

    pub fn dump_build(self, writer: impl Write) -> Result<(), DumpBuildError<OE, IE, IDE, OODE>> {
        ninja_dump::dump_build(
            writer,
            self.outputs,
            self.rule_name,
            self.inputs,
            self.implicit_dependencies,
            self.order_only_dependencies,
            self.variables,
        )
    }
}
