use clap::Parser;
use time::macros::format_description;
use time::OffsetDateTime;

use common::{exists, get_now, get_size, rename};

#[derive(Parser)]
/// If the file has 42 bytes or more, move it by appending a suffix.
///
/// The suffix is `.YYYY-MM-DD.number` with `YYYY-MM-DD` the current date and
/// `number` the smallest positive integer such that the destination path does
/// not exist before the move.
struct Cli {
    /// UTF-8 file path
    file_path: String,
}

fn main() -> anyhow::Result<()> {
    let Cli { file_path } = Cli::parse();
    work(&file_path, get_now)
}

fn work(
    file_path: &str,
    get_now: impl FnOnce() -> anyhow::Result<OffsetDateTime>,
) -> anyhow::Result<()> {
    if get_size(file_path)? >= 42 {
        let dst_path = get_destination_path(file_path, get_now)?;
        rename(file_path, &dst_path)?;
    }
    Ok(())
}

fn get_destination_path(
    file_path: &str,
    get_now: impl FnOnce() -> anyhow::Result<OffsetDateTime>,
) -> anyhow::Result<String> {
    let formatted_date = {
        let now = get_now()?;
        now.format(&format_description!("[year]-[month]-[day]")).unwrap()
    };
    let mut number = 1;
    loop {
        let candidate = format!("{file_path}.{formatted_date}.{number}");
        if !exists(&candidate)? {
            break Ok(candidate);
        }
        number += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::work;

    use std::fmt;
    use std::fs::{self, Metadata};
    use std::path::Path;

    use anyhow::{ensure, Context as _};
    use assert_fs::fixture::{FileWriteStr as _, PathChild as _};
    use assert_fs::TempDir;
    use time::macros::datetime;
    use time::OffsetDateTime;

    const BIG_ENOUGH_CONTENT: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

    #[test]
    fn demo() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        temp.child("app.log").write_str(BIG_ENOUGH_CONTENT)?;
        temp.child("app.log.2011-12-13.1").write_str("one")?;
        temp.child("app.log.2011-12-13.2").write_str("two")?;
        launch_work(&temp, "app.log", datetime!(2011-12-13 14:15:16 UTC))?;
        temp.child("app.log").check_does_not_exist()?;
        temp.child("app.log.2011-12-13.1").check_is_file_with_content("one")?;
        temp.child("app.log.2011-12-13.2").check_is_file_with_content("two")?;
        temp.child("app.log.2011-12-13.3").check_is_file_with_content(BIG_ENOUGH_CONTENT)
    }

    #[test]
    fn first_backup_of_the_day() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        temp.child("app.log").write_str(BIG_ENOUGH_CONTENT)?;
        launch_work(&temp, "app.log", datetime!(2011-12-13 14:15:16 UTC))?;
        temp.child("app.log").check_does_not_exist()?;
        temp.child("app.log.2011-12-13.1").check_is_file_with_content(BIG_ENOUGH_CONTENT)
    }

    #[test]
    fn noop_because_the_file_is_small() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        temp.child("app.log").write_str("small")?;
        launch_work(&temp, "app.log", datetime!(2011-12-13 14:15:16 UTC))?;
        temp.child("app.log").check_is_file_with_content("small")?;
        temp.child("app.log.2011-12-13.1").check_does_not_exist()
    }

    #[test]
    fn fail_if_path_does_not_exist() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let result = launch_work(&temp, "app.log", datetime!(2011-12-13 14:15:16 UTC));
        check_err_contains(result, "failed to read metadata from")
    }

    fn launch_work(temp: &TempDir, file_path: &str, now: OffsetDateTime) -> anyhow::Result<()> {
        let child = temp.child(file_path);
        let file_path = child.to_str().unwrap();
        let get_now = || Ok(now);
        work(file_path, get_now)
    }

    trait Check {
        fn check_does_not_exist(&self) -> anyhow::Result<()>;
        fn check_is_file_with_content(&self, expected: impl AsRef<str>) -> anyhow::Result<()>;
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

        fn check_is_file_with_content(&self, expected: impl AsRef<str>) -> anyhow::Result<()> {
            fn inner(path: &Path, expected: &str) -> anyhow::Result<()> {
                let metadata = symlink_metadata(path)?;
                ensure!(metadata.is_file(), "{path:?} exists but is not a file");
                let cont = fs::read(path).with_context(|| format!("failed to read {path:?}"))?;
                let cont = String::from_utf8(cont)
                    .with_context(|| format!("non-UTF8 data in {path:?}"))?;
                ensure!(cont == expected, "the content of {path:?} is {cont:?}, not {expected:?}");
                Ok(())
            }
            inner(self.as_ref(), expected.as_ref())
        }
    }

    fn symlink_metadata(path: &Path) -> anyhow::Result<Metadata> {
        path.symlink_metadata().with_context(|| format!("failed to read metadata from {path:?}"))
    }

    fn check_err_contains<T, E>(result: Result<T, E>, text: impl AsRef<str>) -> anyhow::Result<()>
    where
        E: fmt::Debug,
    {
        let text = text.as_ref();
        let error = result.err().context("missing error")?;
        let msg = format!("{error:?}");
        ensure!(msg.contains(text), "the error message {msg:?} does not contain {text:?}");
        Ok(())
    }
}
