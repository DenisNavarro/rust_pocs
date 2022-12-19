#![warn(clippy::nursery, clippy::pedantic)]

//! The `backup` binary copies files and directories by adding a suffix which depends on the
//! current datetime. It is tested on Linux.
//!
//! For example, on 2000-01-02 03:04:05, `backup /path/to/directory /path/to/file` copies:
//! - `/path/to/directory` to `/path/to/directory_2000-01-02-03h04` and
//! - `/path/to/file` to `/path/to/file_2000-01-02-03h04`.

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
        let status = Command::new("cp")
            .arg("-r")
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

    #[test]
    fn ok() {
        check_story(
            CreateDirs(vec!["empty", "colors", "colors/dark"]),
            ThenCreateFiles(vec!["colors/red", "colors/dark/black", "foo", "bar.md"]),
            ThenLaunchWorkOnPaths(vec!["empty", "colors", "foo", "bar.md"]),
            OnDatetime(datetime!(2000-01-02 03:04:05 UTC)),
            ThenCheckTheSuccessIs(true),
            AndTheFollowingDirsExist(vec![
                "empty",
                "colors",
                "colors/dark",
                "empty_2000-01-02-03h04",
                "colors_2000-01-02-03h04",
                "colors_2000-01-02-03h04/dark",
            ]),
            AndTheFollowingFilesExist(vec![
                "colors/red",
                "colors/dark/black",
                "foo",
                "bar.md",
                "colors_2000-01-02-03h04/red",
                "colors_2000-01-02-03h04/dark/black",
                "foo_2000-01-02-03h04",
                "bar.md_2000-01-02-03h04",
            ]),
            AndTheFollowingPathDoesNotExist(vec![]),
        );
    }

    #[test]
    fn fail_if_src_path_does_not_exist() {
        check_story(
            CreateDirs(vec!["empty"]),
            ThenCreateFiles(vec!["foo"]),
            ThenLaunchWorkOnPaths(vec!["empty", "foo", "bar.md"]),
            OnDatetime(datetime!(2000-01-02 03:04:05 UTC)),
            ThenCheckTheSuccessIs(false),
            AndTheFollowingDirsExist(vec!["empty"]),
            AndTheFollowingFilesExist(vec!["foo"]),
            AndTheFollowingPathDoesNotExist(vec!["empty_2000-01-02-03h04", "foo_2000-01-02-03h04"]),
        );
    }

    #[test]
    fn fail_if_src_path_does_not_have_a_name() {
        check_story(
            CreateDirs(vec!["empty"]),
            ThenCreateFiles(vec!["foo"]),
            ThenLaunchWorkOnPaths(vec!["empty", "foo", ".."]),
            OnDatetime(datetime!(2000-01-02 03:04:05 UTC)),
            ThenCheckTheSuccessIs(false),
            AndTheFollowingDirsExist(vec!["empty"]),
            AndTheFollowingFilesExist(vec!["foo"]),
            AndTheFollowingPathDoesNotExist(vec!["empty_2000-01-02-03h04", "foo_2000-01-02-03h04"]),
        );
    }

    // Workaround to emulate named arguments
    struct CreateDirs(Vec<&'static str>);
    struct ThenCreateFiles(Vec<&'static str>);
    struct ThenLaunchWorkOnPaths(Vec<&'static str>);
    struct OnDatetime(OffsetDateTime);
    struct ThenCheckTheSuccessIs(bool);
    struct AndTheFollowingDirsExist(Vec<&'static str>);
    struct AndTheFollowingFilesExist(Vec<&'static str>);
    struct AndTheFollowingPathDoesNotExist(Vec<&'static str>);

    fn check_story(
        dirs_to_create: CreateDirs,
        files_to_create: ThenCreateFiles,
        arg_paths: ThenLaunchWorkOnPaths,
        now: OnDatetime,
        should_succeed: ThenCheckTheSuccessIs,
        dirs_which_should_exist: AndTheFollowingDirsExist,
        files_which_should_exist: AndTheFollowingFilesExist,
        paths_which_should_not_exist: AndTheFollowingPathDoesNotExist,
    ) {
        let tmp_dir = tempdir().unwrap();
        let tmp_dir_path = tmp_dir.path();
        for path in &dirs_to_create.0 {
            fs::create_dir(tmp_dir_path.join(path)).unwrap();
            println!("Created dir: {:?}", tmp_dir_path.join(path));
        }
        for path in &files_to_create.0 {
            File::create(tmp_dir_path.join(path)).unwrap();
            println!("Created file: {:?}", tmp_dir_path.join(path));
        }
        let src_paths = arg_paths.0.iter().map(|path| tmp_dir_path.join(path));
        let result = work(src_paths, now.0);
        if should_succeed.0 {
            result.unwrap();
        } else {
            assert!(result.is_err());
        }
        for path in &dirs_which_should_exist.0 {
            assert!(fs::metadata(tmp_dir_path.join(path)).unwrap().is_dir());
        }
        for path in &files_which_should_exist.0 {
            assert!(fs::metadata(tmp_dir_path.join(path)).unwrap().is_file());
        }
        for path in &paths_which_should_not_exist.0 {
            assert!(fs::metadata(tmp_dir_path.join(path)).is_err());
        }
    }
}
