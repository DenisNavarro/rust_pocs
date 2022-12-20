#![warn(clippy::nursery, clippy::pedantic)]

use anyhow::{bail, Context};
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use time::{format_description, OffsetDateTime};

#[derive(Parser)]
/// Copy directories and files by adding a suffix which depends on the current datetime.
/// Tested on Linux.
///
/// For example, on 2022-12-20 13:14:15, `backup /path/to/directory /path/to/file` copies
/// `/path/to/directory` to `/path/to/directory_2022-12-20-13h14` and
/// `/path/to/file` to `/path/to/file_2022-12-20-13h14`.
///
/// `backup` follows command-line symlinks.
struct Cli {
    src_paths: Vec<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let now = OffsetDateTime::now_local().context("could not determine the local offset")?;
    work(cli.src_paths, now)
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
    if fs::metadata(&dst_path).is_ok() {
        bail!("{dst_path:?} already exists");
    }
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
            .arg("-rH")
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
    use tempfile::{tempdir, TempDir};
    use time::macros::datetime;

    #[test]
    fn ok() {
        let story = Story::new();
        story.create_dirs(["empty", "colors", "colors/dark", "--", "-"]);
        story.create_files([
            "colors/red",
            "colors/dark/black",
            "foo",
            "bar.md",
            "--b a z",
        ]);
        let result = story.launch_work_on_paths(
            ["empty", "colors", "foo", "bar.md", "--b a z", "--", "-"],
            datetime!(2022-12-20 13:14:15 UTC),
        );
        result.unwrap();
        story.check_the_following_dirs_exist([
            "empty_2022-12-20-13h14",
            "colors_2022-12-20-13h14",
            "colors_2022-12-20-13h14/dark",
            "--_2022-12-20-13h14",
            "-_2022-12-20-13h14",
        ]);
        story.check_the_following_files_exist([
            "colors_2022-12-20-13h14/red",
            "colors_2022-12-20-13h14/dark/black",
            "foo_2022-12-20-13h14",
            "bar.md_2022-12-20-13h14",
            "--b a z_2022-12-20-13h14",
        ]);
    }

    #[test]
    fn fail_if_src_path_does_not_have_a_name() {
        let story = Story::new();
        story.create_dirs(["empty"]);
        story.create_files(["foo"]);
        let result =
            story.launch_work_on_paths(["empty", "foo", ".."], datetime!(2022-12-20 13:14:15 UTC));
        assert!(result.is_err());
        story.check_the_following_paths_do_not_exist([
            "empty_2022-12-20-13h14",
            "foo_2022-12-20-13h14",
        ]);
    }

    #[test]
    fn fail_if_src_path_does_not_exist() {
        let story = Story::new();
        story.create_dirs(["empty"]);
        story.create_files(["foo"]);
        let result = story.launch_work_on_paths(
            ["empty", "foo", "bar.md"],
            datetime!(2022-12-20 13:14:15 UTC),
        );
        assert!(result.is_err());
        story.check_the_following_paths_do_not_exist([
            "empty_2022-12-20-13h14",
            "foo_2022-12-20-13h14",
        ]);
    }

    #[test]
    fn fail_if_dir_dst_path_already_exists() {
        let story = Story::new();
        story.create_dirs(["empty", "empty_2022-12-20-13h14"]);
        story.create_files(["foo", "bar.md"]);
        let result = story.launch_work_on_paths(
            ["foo", "bar.md", "empty"],
            datetime!(2022-12-20 13:14:15 UTC),
        );
        assert!(result.is_err());
        story.check_the_following_paths_do_not_exist([
            "foo_2022-12-20-13h14",
            "bar.md_2022-12-20-13h14",
        ]);
    }

    #[test]
    fn fail_if_file_dst_path_already_exists() {
        let story = Story::new();
        story.create_dirs(["empty"]);
        story.create_files(["foo", "bar.md", "bar.md_2022-12-20-13h14"]);
        let result = story.launch_work_on_paths(
            ["empty", "foo", "bar.md"],
            datetime!(2022-12-20 13:14:15 UTC),
        );
        assert!(result.is_err());
        story.check_the_following_paths_do_not_exist([
            "empty_2022-12-20-13h14",
            "foo_2022-12-20-13h14",
        ]);
    }

    struct Story {
        tmp_dir: TempDir,
    }

    impl Story {
        fn new() -> Story {
            Story {
                tmp_dir: tempdir().unwrap(),
            }
        }

        fn create_dirs<const N: usize>(&self, dirs_to_create: [&'static str; N]) {
            let tmp_dir_path = self.tmp_dir.path();
            for path in dirs_to_create {
                fs::create_dir(tmp_dir_path.join(path)).unwrap();
                println!("Created dir: {:?}", tmp_dir_path.join(path));
            }
        }

        fn create_files<const N: usize>(&self, files_to_create: [&'static str; N]) {
            let tmp_dir_path = self.tmp_dir.path();
            for path in files_to_create {
                File::create(tmp_dir_path.join(path)).unwrap();
                println!("Created file: {:?}", tmp_dir_path.join(path));
            }
        }

        fn launch_work_on_paths<const N: usize>(
            &self,
            arg_paths: [&'static str; N],
            now: OffsetDateTime,
        ) -> anyhow::Result<()> {
            let tmp_dir_path = self.tmp_dir.path();
            let src_paths = arg_paths.iter().map(|path| tmp_dir_path.join(path));
            work(src_paths, now)
        }

        fn check_the_following_dirs_exist<const N: usize>(
            &self,
            dirs_which_should_exist: [&'static str; N],
        ) {
            let tmp_dir_path = self.tmp_dir.path();
            for path in dirs_which_should_exist {
                assert!(fs::metadata(tmp_dir_path.join(path)).unwrap().is_dir());
            }
        }

        fn check_the_following_files_exist<const N: usize>(
            &self,
            files_which_should_exist: [&'static str; N],
        ) {
            let tmp_dir_path = self.tmp_dir.path();
            for path in files_which_should_exist {
                assert!(fs::metadata(tmp_dir_path.join(path)).unwrap().is_file());
            }
        }

        fn check_the_following_paths_do_not_exist<const N: usize>(
            &self,
            paths_which_should_not_exist: [&'static str; N],
        ) {
            let tmp_dir_path = self.tmp_dir.path();
            for path in paths_which_should_not_exist {
                assert!(fs::metadata(tmp_dir_path.join(path)).is_err());
            }
        }
    }

    // TODO: add unit tests with symlinks.
}
