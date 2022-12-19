#![warn(clippy::nursery, clippy::pedantic)]

//! The `backup` binary copies files and directories by adding a suffix which depends on the
//! current datetime. It is tested on Linux.
//!
//! For example, on 2000-01-02 03:04:05, `backup /path/to/directory /path/to/file` copies:
//! - `/path/to/directory` to `/path/to/directory_2000-01-02-03h04` and
//! - `/path/to/file` to `/path/to/file_2000-01-02-03h04`.

use anyhow::Context;
use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use time::{format_description, OffsetDateTime};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    paths: Vec<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let now = OffsetDateTime::now_local().context("Cannot determine the local offset.")?;
    work(cli.paths, now)
}

fn work(src_paths: impl IntoIterator<Item = PathBuf>, now: OffsetDateTime) -> anyhow::Result<()> {
    let dst_path_suffix = get_dst_path_suffix(now, "_[year]-[month]-[day]-[hour]h[minute]");
    for copy_action in check_if_each_copy_seems_possible(src_paths, &dst_path_suffix)? {
        do_copy(&copy_action)?;
        println!("Copied {:?} to {:?}.", copy_action.src, copy_action.dst);
    }
    Ok(())
}

fn get_dst_path_suffix(now: OffsetDateTime, format: &str) -> String {
    let format = format_description::parse(format).unwrap();
    now.format(&format).unwrap()
}

fn check_if_each_copy_seems_possible(
    src_paths: impl IntoIterator<Item = PathBuf>,
    dst_path_suffix: &str,
) -> anyhow::Result<Vec<CopyAction>> {
    src_paths
        .into_iter()
        .map(|src_path| check_if_copy_seems_possible(src_path, dst_path_suffix))
        .collect()
}

fn check_if_copy_seems_possible(
    src_path: PathBuf,
    dst_path_suffix: &str,
) -> anyhow::Result<CopyAction> {
    let metadata = fs::metadata(&src_path)
        .with_context(|| format!("Failed to read metadata from {src_path:?}."))?;
    let mut file_name = src_path
        .file_name()
        .with_context(|| format!("{src_path:?} does not have a name."))?
        .to_owned();
    file_name.push(dst_path_suffix);
    let mut dst_path = src_path.clone();
    dst_path.set_file_name(&file_name);
    Ok(CopyAction {
        src: src_path,
        dst: dst_path,
        is_dir: metadata.is_dir(),
    })
}

fn do_copy(copy_action: &CopyAction) -> anyhow::Result<()> {
    let CopyAction { src, dst, is_dir } = copy_action;
    if *is_dir {
        copy_dir(src, dst).with_context(|| format!("Failed to copy {src:?} to {dst:?}."))?;
    } else {
        fs::copy(src, dst).with_context(|| format!("Failed to copy {src:?} to {dst:?}."))?;
    }
    Ok(())
}

fn copy_dir(src: &Path, dst: &Path) -> anyhow::Result<()> {
    Command::new("cp")
        .arg("-r")
        .arg("--")
        .arg(src)
        .arg(dst)
        .status()?;
    Ok(())
}

struct CopyAction {
    src: PathBuf,
    dst: PathBuf,
    is_dir: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::tempdir;
    use time::macros::datetime;

    #[test]
    fn ok() {
        check(
            ExistingDirsBefore(vec!["empty", "colors", "colors/dark"]),
            ExistingFilesBefore(vec!["colors/red", "colors/dark/black", "foo", "bar.md"]),
            ArgPaths(vec!["empty", "colors", "foo", "bar.md"]),
            Now(datetime!(2000-01-02 03:04:05 UTC)),
            ShouldSucceed(true),
            ExistingDirsAfter(vec![
                "empty",
                "colors",
                "colors/dark",
                "empty_2000-01-02-03h04",
                "colors_2000-01-02-03h04",
                "colors_2000-01-02-03h04/dark",
            ]),
            ExistingFilesAfter(vec![
                "colors/red",
                "colors/dark/black",
                "foo",
                "bar.md",
                "colors_2000-01-02-03h04/red",
                "colors_2000-01-02-03h04/dark/black",
                "foo_2000-01-02-03h04",
                "bar.md_2000-01-02-03h04",
            ]),
            NotExistingPathsAfter(vec![]),
        );
    }

    #[test]
    fn fail_if_src_path_does_not_exist() {
        check(
            ExistingDirsBefore(vec!["empty"]),
            ExistingFilesBefore(vec!["foo"]),
            ArgPaths(vec!["empty", "foo", "bar.md"]),
            Now(datetime!(2000-01-02 03:04:05 UTC)),
            ShouldSucceed(false),
            ExistingDirsAfter(vec!["empty"]),
            ExistingFilesAfter(vec!["foo"]),
            NotExistingPathsAfter(vec!["empty_2000-01-02-03h04", "foo_2000-01-02-03h04"]),
        );
    }

    #[test]
    fn fail_if_src_path_does_not_have_a_name() {
        check(
            ExistingDirsBefore(vec!["empty"]),
            ExistingFilesBefore(vec!["foo"]),
            ArgPaths(vec!["empty", "foo", ".."]),
            Now(datetime!(2000-01-02 03:04:05 UTC)),
            ShouldSucceed(false),
            ExistingDirsAfter(vec!["empty"]),
            ExistingFilesAfter(vec!["foo"]),
            NotExistingPathsAfter(vec!["empty_2000-01-02-03h04", "foo_2000-01-02-03h04"]),
        );
    }

    // Workaround to emulate named arguments
    struct ExistingDirsBefore(Vec<&'static str>);
    struct ExistingFilesBefore(Vec<&'static str>);
    struct ArgPaths(Vec<&'static str>);
    struct Now(OffsetDateTime);
    struct ShouldSucceed(bool);
    struct ExistingDirsAfter(Vec<&'static str>);
    struct ExistingFilesAfter(Vec<&'static str>);
    struct NotExistingPathsAfter(Vec<&'static str>);

    fn check(
        existing_dirs_before: ExistingDirsBefore,
        existing_files_before: ExistingFilesBefore,
        arg_paths: ArgPaths,
        now: Now,
        should_succeed: ShouldSucceed,
        existing_dirs_after: ExistingDirsAfter,
        existing_files_after: ExistingFilesAfter,
        not_existing_paths_after: NotExistingPathsAfter,
    ) {
        let tmp_dir = tempdir().unwrap();
        let tmp_dir_path = tmp_dir.path();
        for path in &existing_dirs_before.0 {
            fs::create_dir(tmp_dir_path.join(path)).unwrap();
            println!("Created dir: {:?}", tmp_dir_path.join(path));
        }
        for path in &existing_files_before.0 {
            File::create(tmp_dir_path.join(path)).unwrap();
            println!("Created file: {:?}", tmp_dir_path.join(path));
        }
        let src_paths = arg_paths
            .0
            .iter()
            .copied()
            .map(|path| tmp_dir_path.join(path));
        let result = work(src_paths, now.0);
        if should_succeed.0 {
            result.unwrap();
        } else {
            assert!(result.is_err());
        }
        for path in &existing_dirs_after.0 {
            assert!(fs::metadata(tmp_dir_path.join(path)).unwrap().is_dir());
        }
        for path in &existing_files_after.0 {
            assert!(fs::metadata(tmp_dir_path.join(path)).unwrap().is_file());
        }
        for path in &not_existing_paths_after.0 {
            assert!(fs::metadata(tmp_dir_path.join(path)).is_err());
        }
    }
}
