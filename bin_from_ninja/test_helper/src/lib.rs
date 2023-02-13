#![forbid(unsafe_code)]
#![warn(clippy::nursery, clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

//! Utility to write unit tests

use std::fmt;
use std::fs::{self, Metadata};
use std::path::Path;

use anyhow::{ensure, Context};

pub trait Check {
    fn check_does_not_exist(&self) -> anyhow::Result<()>;
    fn check_is_dir(&self) -> anyhow::Result<()>;
    fn check_is_file_with_content(&self, content: impl AsRef<str>) -> anyhow::Result<()>;
    fn check_is_symlink_to(&self, path: impl AsRef<Path>) -> anyhow::Result<()>;
}

impl<T> Check for T
where
    T: AsRef<Path>,
{
    fn check_does_not_exist(&self) -> anyhow::Result<()> {
        let path = self.as_ref();
        path.symlink_metadata().is_err().then_some(()).with_context(|| format!("{path:?} exists"))
    }

    fn check_is_dir(&self) -> anyhow::Result<()> {
        let path = self.as_ref();
        let metadata = symlink_metadata(path)?;
        ensure!(metadata.is_dir(), "{path:?} exists but is not a directory");
        Ok(())
    }

    fn check_is_file_with_content(&self, expected: impl AsRef<str>) -> anyhow::Result<()> {
        let path = self.as_ref();
        let expected = expected.as_ref();
        let metadata = symlink_metadata(path)?;
        ensure!(metadata.is_file(), "{path:?} exists but is not a file");
        let cont = fs::read(path).with_context(|| format!("failed to read {path:?}"))?;
        let cont = String::from_utf8(cont).with_context(|| format!("non-UTF8 data in {path:?}"))?;
        ensure!(cont == expected, "the content of {path:?} is {cont:?}, not {expected:?}");
        Ok(())
    }

    fn check_is_symlink_to(&self, expected: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = self.as_ref();
        let expected = expected.as_ref();
        let target = path.read_link().with_context(|| format!("{path:?} is not a symlink"))?;
        ensure!(target == expected, "{path:?} is a symlink to {target:?}, not {expected:?}");
        Ok(())
    }
}

fn symlink_metadata(path: &Path) -> anyhow::Result<Metadata> {
    path.symlink_metadata().with_context(|| format!("failed to read metadata from {path:?}"))
}

pub fn check_err_contains<T, E>(result: Result<T, E>, text: impl AsRef<str>) -> anyhow::Result<()>
where
    E: fmt::Debug,
{
    let text = text.as_ref();
    let error = result.err().context("missing error")?;
    let msg = format!("{error:?}");
    ensure!(msg.contains(text), "the error message {msg:?} does not contain {text:?}");
    Ok(())
}
