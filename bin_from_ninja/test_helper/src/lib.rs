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
    // Remark: the below functions has an `inner` function, like the implementation of
    // `std::fs::read`. This trick is explained in:
    // The Rust Performance Book -> 20. Compile Times -> LLVM IR
    // <https://github.com/nnethercote/perf-book/blob/3dc0a98387ac5cd93e3edbaa691e412662bd2b43/src/compile-times.md#llvm-ir>

    fn check_does_not_exist(&self) -> anyhow::Result<()> {
        fn inner(path: &Path) -> anyhow::Result<()> {
            ensure!(path.symlink_metadata().is_err(), "{path:?} exists");
            Ok(())
        }
        inner(self.as_ref())
    }

    fn check_is_dir(&self) -> anyhow::Result<()> {
        fn inner(path: &Path) -> anyhow::Result<()> {
            let metadata = symlink_metadata(path)?;
            ensure!(metadata.is_dir(), "{path:?} exists but is not a directory");
            Ok(())
        }
        inner(self.as_ref())
    }

    fn check_is_file_with_content(&self, expected: impl AsRef<str>) -> anyhow::Result<()> {
        fn inner(path: &Path, expected: &str) -> anyhow::Result<()> {
            let metadata = symlink_metadata(path)?;
            ensure!(metadata.is_file(), "{path:?} exists but is not a file");
            let cont = fs::read(path).with_context(|| format!("failed to read {path:?}"))?;
            let cont =
                String::from_utf8(cont).with_context(|| format!("non-UTF8 data in {path:?}"))?;
            ensure!(cont == expected, "the content of {path:?} is {cont:?}, not {expected:?}");
            Ok(())
        }
        inner(self.as_ref(), expected.as_ref())
    }

    fn check_is_symlink_to(&self, expected: impl AsRef<Path>) -> anyhow::Result<()> {
        fn inner(path: &Path, expected: &Path) -> anyhow::Result<()> {
            let target = path.read_link().with_context(|| format!("{path:?} is not a symlink"))?;
            ensure!(target == expected, "{path:?} is a symlink to {target:?}, not {expected:?}");
            Ok(())
        }
        inner(self.as_ref(), expected.as_ref())
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
