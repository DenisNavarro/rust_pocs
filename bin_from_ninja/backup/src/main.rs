#![warn(clippy::nursery, clippy::pedantic)]

//! The `backup` binary copies files and directories by adding a suffix which depends on the
//! current datetime. It is tested on Linux.
//!
//! For example, on 2000-01-02 03:04:05, `backup /path/to/directory /path/to/file` copies:
//! - `/path/to/directory` to `/path/to/directory_2000-01-02-03h04` and
//! - `/path/to/file` to `/path/to/file_2000-01-02-03h04`.
//!
//! `backup` follows symbolic links.

use anyhow::{bail, Context};
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use time::{format_description, OffsetDateTime};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    paths: Vec<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let now = OffsetDateTime::now_local().context("could not determine the local offset")?;
    work(cli.paths, now)
}

fn work(src_paths: impl IntoIterator<Item = PathBuf>, now: OffsetDateTime) -> anyhow::Result<()> {
    let dst_path_suffix = get_dst_path_suffix(now, "_[year]-[month]-[day]-[hour]h[minute]");
    let copy_actions: Vec<_> = check_if_each_copy_seems_possible(src_paths, &dst_path_suffix)?;
    for copy_action in copy_actions {
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
    let mut file_name = src_path
        .file_name()
        .with_context(|| format!("{src_path:?} does not have a name"))?
        .to_owned();
    let metadata = fs::metadata(&src_path)
        .with_context(|| format!("failed to read metadata from {src_path:?}"))?;
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
        // TODO: Find an easy cross-plateform way to copy recursively a directory.
        let status = Command::new("cp")
            .arg("-rL")
            .arg("--")
            .arg(src)
            .arg(dst)
            .status()
            .with_context(|| {
                format!("failed to copy {src:?} to {dst:?}: failed to execute process")
            })?;
        if !status.success() {
            bail!("failed to copy {src:?} to {dst:?}: {status}");
        }
    } else {
        fs::copy(src, dst).with_context(|| format!("failed to copy {src:?} to {dst:?}"))?;
    }
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

    macro_rules! check_story {
        (
            Create dirs ($dirs_to_create:expr).
            Then create files ($files_to_create:expr).
            Then launch work on paths ($arg_paths:expr)
            on ($now:expr).
            Then check the success is ($should_succeed:expr)
            and the following dirs exist: ($dirs_which_should_exist:expr)
            and the following files exist: ($files_which_should_exist:expr)
            and the following paths do not exist: ($paths_which_should_not_exist:expr).
        ) => {
            let tmp_dir = tempdir().unwrap();
            let tmp_dir_path = tmp_dir.path();
            for path in $dirs_to_create {
                fs::create_dir(tmp_dir_path.join(path)).unwrap();
                println!("Created dir: {:?}", tmp_dir_path.join(path));
            }
            for path in $files_to_create {
                File::create(tmp_dir_path.join(path)).unwrap();
                println!("Created file: {:?}", tmp_dir_path.join(path));
            }
            let src_paths = $arg_paths.iter().map(|path| tmp_dir_path.join(path));
            let result = work(src_paths, $now);
            if $should_succeed {
                result.unwrap();
            } else {
                assert!(result.is_err());
            }
            for path in $dirs_which_should_exist {
                let path: &'static str = path; // help the compiler to infer type
                assert!(fs::metadata(tmp_dir_path.join(path)).unwrap().is_dir());
            }
            for path in $files_which_should_exist {
                let path: &'static str = path; // help the compiler to infer type
                assert!(fs::metadata(tmp_dir_path.join(path)).unwrap().is_file());
            }
            for path in $paths_which_should_not_exist {
                let path: &'static str = path; // help the compiler to infer type
                assert!(fs::metadata(tmp_dir_path.join(path)).is_err());
            }
        };
    }

    #[test]
    fn ok() {
        check_story!(
            Create dirs (["empty", "colors", "colors/dark", "--", "-"]).
            Then create files (["colors/red", "colors/dark/black", "foo", "bar.md", "--b a z"]).
            Then launch work on paths (["empty", "colors", "foo", "bar.md", "--b a z", "--", "-"])
            on (datetime!(2000-01-02 03:04:05 UTC)).
            Then check the success is (true)
            and the following dirs exist: ([
                "empty_2000-01-02-03h04",
                "colors_2000-01-02-03h04",
                "colors_2000-01-02-03h04/dark",
                "--_2000-01-02-03h04",
                "-_2000-01-02-03h04",
            ])
            and the following files exist: ([
                "colors_2000-01-02-03h04/red",
                "colors_2000-01-02-03h04/dark/black",
                "foo_2000-01-02-03h04",
                "bar.md_2000-01-02-03h04",
                "--b a z_2000-01-02-03h04",
            ])
            and the following paths do not exist: ([]).
        );
    }

    #[test]
    fn fail_if_src_path_does_not_have_a_name() {
        check_story!(
            Create dirs (["empty"]).
            Then create files (["foo"]).
            Then launch work on paths (["empty", "foo", ".."])
            on (datetime!(2000-01-02 03:04:05 UTC)).
            Then check the success is (false)
            and the following dirs exist: ([])
            and the following files exist: ([])
            and the following paths do not exist: ([
                "empty_2000-01-02-03h04",
                "foo_2000-01-02-03h04",
            ]).
        );
    }

    #[test]
    fn fail_if_src_path_does_not_exist() {
        check_story!(
            Create dirs (["empty"]).
            Then create files (["foo"]).
            Then launch work on paths (["empty", "foo", "bar.md"])
            on (datetime!(2000-01-02 03:04:05 UTC)).
            Then check the success is (false)
            and the following dirs exist: ([])
            and the following files exist: ([])
            and the following paths do not exist: ([
                "empty_2000-01-02-03h04",
                "foo_2000-01-02-03h04",
            ]).
        );
    }
}
