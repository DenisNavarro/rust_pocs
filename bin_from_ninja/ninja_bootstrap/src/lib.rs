#![warn(clippy::nursery, clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

//! The current module contains builders to call the functions from the `ninja_dump` module in a
//! more readable way.

mod ninja_dump;

use ninja_dump::{dump_build, dump_rule};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{self, Write};
use std::iter;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub struct RuleName<'r>(&'r [u8]);

#[derive(Debug, Clone, Copy)]
pub struct Rule<'r, 'c> {
    name: &'r [u8],
    command: &'c [u8],
}

pub struct Build<'r, O, I, ID, OOD>
where
    O: IntoIterator,
    I: IntoIterator,
    ID: IntoIterator,
    OOD: IntoIterator,
    O::Item: Into<Vec<u8>>,
    I::Item: Into<Vec<u8>>,
    ID::Item: Into<Vec<u8>>,
    OOD::Item: Into<Vec<u8>>,
{
    outputs: O,
    rule_name: &'r [u8],
    inputs: I,
    implicit_dependencies: ID,
    order_only_dependencies: OOD,
    variables: BTreeMap<Vec<u8>, Vec<u8>>,
}

pub fn rule_name(name: &(impl AsRef<[u8]> + ?Sized)) -> RuleName<'_> {
    RuleName(name.as_ref())
}

type Empty = iter::Empty<Vec<u8>>;

impl<'r> RuleName<'r> {
    #[must_use]
    pub fn command(self, command: &(impl AsRef<[u8]> + ?Sized)) -> Rule<'r, '_> {
        Rule {
            name: self.0,
            command: command.as_ref(),
        }
    }

    #[must_use]
    pub const fn build_outputs<O>(self, outputs: O) -> Build<'r, O, Empty, Empty, Empty>
    where
        O: IntoIterator,
        O::Item: Into<Vec<u8>>,
    {
        Build {
            outputs,
            rule_name: self.0,
            inputs: iter::empty(),
            implicit_dependencies: iter::empty(),
            order_only_dependencies: iter::empty(),
            variables: BTreeMap::new(),
        }
    }

    #[cfg(unix)]
    #[must_use]
    pub fn build_output_paths(
        self,
        outputs: impl IntoIterator<Item = impl Into<PathBuf>>,
    ) -> Build<'r, impl IntoIterator<Item = impl Into<Vec<u8>>>, Empty, Empty, Empty> {
        self.build_outputs(
            outputs
                .into_iter()
                .map(|path| std::os::unix::ffi::OsStringExt::into_vec(OsString::from(path.into()))),
        )
    }
}

impl<'r, 'c> Rule<'r, 'c> {
    pub fn dump(self, writer: impl Write) -> io::Result<()> {
        dump_rule(writer, self.name, self.command)
    }
}

impl<'r, O, I, ID, OOD> Build<'r, O, I, ID, OOD>
where
    O: IntoIterator,
    I: IntoIterator,
    ID: IntoIterator,
    OOD: IntoIterator,
    O::Item: Into<Vec<u8>>,
    I::Item: Into<Vec<u8>>,
    ID::Item: Into<Vec<u8>>,
    OOD::Item: Into<Vec<u8>>,
{
    #[allow(clippy::missing_const_for_fn)] // false positive from Clippy 0.1.66
    #[must_use]
    pub fn inputs<T>(self, new_value: T) -> Build<'r, O, T, ID, OOD>
    where
        T: IntoIterator,
        T::Item: Into<Vec<u8>>,
    {
        Build {
            outputs: self.outputs,
            rule_name: self.rule_name,
            inputs: new_value,
            implicit_dependencies: self.implicit_dependencies,
            order_only_dependencies: self.order_only_dependencies,
            variables: self.variables,
        }
    }

    #[cfg(unix)]
    #[must_use]
    pub fn input_paths(
        self,
        new_value: impl IntoIterator<Item = impl Into<PathBuf>>,
    ) -> Build<'r, O, impl IntoIterator<Item = impl Into<Vec<u8>>>, ID, OOD> {
        self.inputs(
            new_value
                .into_iter()
                .map(|path| std::os::unix::ffi::OsStringExt::into_vec(OsString::from(path.into()))),
        )
    }

    #[allow(clippy::missing_const_for_fn)] // false positive from Clippy 0.1.66
    #[must_use]
    pub fn implicit_dependencies<T>(self, new_value: T) -> Build<'r, O, I, T, OOD>
    where
        T: IntoIterator,
        T::Item: Into<Vec<u8>>,
    {
        Build {
            outputs: self.outputs,
            rule_name: self.rule_name,
            inputs: self.inputs,
            implicit_dependencies: new_value,
            order_only_dependencies: self.order_only_dependencies,
            variables: self.variables,
        }
    }

    #[allow(clippy::missing_const_for_fn)] // false positive from Clippy 0.1.66
    #[must_use]
    pub fn order_only_dependencies<T>(self, new_value: T) -> Build<'r, O, I, ID, T>
    where
        T: IntoIterator,
        T::Item: Into<Vec<u8>>,
    {
        Build {
            outputs: self.outputs,
            rule_name: self.rule_name,
            inputs: self.inputs,
            implicit_dependencies: self.implicit_dependencies,
            order_only_dependencies: new_value,
            variables: self.variables,
        }
    }

    #[cfg(unix)]
    #[must_use]
    pub fn order_only_dependency_paths(
        self,
        new_value: impl IntoIterator<Item = impl Into<PathBuf>>,
    ) -> Build<'r, O, I, ID, impl IntoIterator<Item = impl Into<Vec<u8>>>> {
        self.order_only_dependencies(
            new_value
                .into_iter()
                .map(|path| std::os::unix::ffi::OsStringExt::into_vec(OsString::from(path.into()))),
        )
    }

    #[must_use]
    pub fn variable(mut self, variable: impl Into<Vec<u8>>, value: impl Into<Vec<u8>>) -> Self {
        self.variables.insert(variable.into(), value.into());
        self
    }

    pub fn dump(self, writer: impl Write) -> io::Result<()> {
        dump_build(
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
