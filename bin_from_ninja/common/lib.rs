use std::fmt::{Debug, Display};
use std::fs::{self, Metadata};
use std::path::Path;

use anyhow::{Context as _, ensure};
use uniquote::Quote as _;

#[must_use]
pub fn quote(string: &str) -> impl Display {
    // The Rust documentation says:
    //
    // > `Debug` implementations of types provided by the standard library (`std`, `core`, `alloc`,
    // > etc.) are not stable, and may also change with future Rust versions.
    //
    // This is why I use `format!("{}", quote(string))` instead of `format!("{string:?}")`.
    string.quote()
}

#[must_use]
pub fn quote_path(path: &Path) -> impl Display {
    // The Rust documentation says:
    //
    // > `Debug` implementations of types provided by the standard library (`std`, `core`, `alloc`,
    // > etc.) are not stable, and may also change with future Rust versions.
    //
    // It also says that `std::path::Path::display` "may perform lossy conversion".
    //
    // This is why I use `format!("{}", quote_path(path))` instead of `format!("{path:?}")` or
    // `format!("{}", path.display())`.
    path.quote()
}

pub trait Check {
    fn check_does_not_exist(&self) -> anyhow::Result<()>;
    fn check_is_dir(&self) -> anyhow::Result<()>;
    fn check_is_file_with_content(&self, expected: impl AsRef<str>) -> anyhow::Result<()>;
    fn check_is_symlink_to(&self, expected: impl AsRef<Path>) -> anyhow::Result<()>;
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
            ensure!(path.symlink_metadata().is_err(), "{} exists", quote_path(path));
            Ok(())
        }
        inner(self.as_ref())
    }

    fn check_is_dir(&self) -> anyhow::Result<()> {
        fn inner(path: &Path) -> anyhow::Result<()> {
            let metadata = symlink_metadata(path)?;
            ensure!(metadata.is_dir(), "{} exists but is not a directory", quote_path(path));
            Ok(())
        }
        inner(self.as_ref())
    }

    fn check_is_file_with_content(&self, expected: impl AsRef<str>) -> anyhow::Result<()> {
        fn inner(path: &Path, expected: &str) -> anyhow::Result<()> {
            let metadata = symlink_metadata(path)?;
            ensure!(metadata.is_file(), "{path:?} exists but is not a file");
            let cont =
                fs::read(path).with_context(|| format!("failed to read {}", quote_path(path)))?;
            let cont = String::from_utf8(cont)
                .with_context(|| format!("non-UTF8 data in {}", quote_path(path)))?;
            ensure!(
                cont == expected,
                "the content of {} is {cont:?}, not {expected:?}",
                quote_path(path)
            );
            Ok(())
        }
        inner(self.as_ref(), expected.as_ref())
    }

    fn check_is_symlink_to(&self, expected: impl AsRef<Path>) -> anyhow::Result<()> {
        fn inner(path: &Path, expected: &Path) -> anyhow::Result<()> {
            let target = path
                .read_link()
                .with_context(|| format!("{} is not a symlink", quote_path(path)))?;
            ensure!(
                target == expected,
                "{} is a symlink to {}, not {}",
                quote_path(path),
                quote_path(&target),
                quote_path(expected),
            );
            Ok(())
        }
        inner(self.as_ref(), expected.as_ref())
    }
}

fn symlink_metadata(path: &Path) -> anyhow::Result<Metadata> {
    path.symlink_metadata()
        .with_context(|| format!("failed to read metadata from {}", quote_path(path)))
}

pub fn check_err_contains<T, E>(result: Result<T, E>, text: impl AsRef<str>) -> anyhow::Result<()>
where
    E: Debug,
{
    let text = text.as_ref();
    let error = result.err().context("missing error")?;
    let msg = format!("{error:?}");
    ensure!(msg.contains(text), "the error message {msg:?} does not contain {text:?}");
    Ok(())
}
