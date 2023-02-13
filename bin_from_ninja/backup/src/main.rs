#![forbid(unsafe_code)]
#![warn(clippy::nursery, clippy::pedantic)]

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{ensure, Context};
use clap::Parser;
use time::{format_description, OffsetDateTime};

#[derive(Parser)]
/// Copy directories and files by adding a suffix which depends on the current datetime.
/// Tested on Linux.
///
/// For example, on 2022-12-13 14:15:16, `backup /path/to/directory /path/to/file` copies
/// `/path/to/directory` to `/path/to/directory_2022-12-13-14h15` and
/// `/path/to/file` to `/path/to/file_2022-12-13-14h15`.
///
/// `backup` follows command-line symlinks.
struct Cli {
    src_paths: Vec<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let now = OffsetDateTime::now_local().context("failed to determine the local offset")?;
    work(cli.src_paths, now)
}

fn work(src_paths: Vec<PathBuf>, now: OffsetDateTime) -> anyhow::Result<()> {
    let dst_path_suffix = get_dst_path_suffix(now, "_[year]-[month]-[day]-[hour]h[minute]");
    let copy_actions: Vec<_> = check_all_copies_seem_possible(src_paths, &dst_path_suffix)?;
    for CopyAction { src_path, dst_path, src_is_dir } in copy_actions {
        copy(&src_path, &dst_path, src_is_dir)?;
        writeln!(io::stdout(), "Copied {src_path:?} to {dst_path:?}.")
            .context("failed to write to stdout")?;
    }
    Ok(())
}

fn get_dst_path_suffix(now: OffsetDateTime, format: &str) -> String {
    let format = format_description::parse(format).unwrap();
    now.format(&format).unwrap()
}

fn check_all_copies_seem_possible(
    src_paths: Vec<PathBuf>,
    dst_path_suffix: &str,
) -> anyhow::Result<Vec<CopyAction>> {
    src_paths
        .into_iter()
        .map(|src_path| {
            let src_file_name = src_path
                .file_name()
                .with_context(|| format!("{src_path:?} does not have a name"))?;
            let src_metadata = fs::metadata(&src_path)
                .with_context(|| format!("failed to read metadata from {src_path:?}"))?;
            let dst_path = {
                let mut dst_file_name = src_file_name.to_owned();
                dst_file_name.push(dst_path_suffix);
                src_path.with_file_name(&dst_file_name)
            };
            ensure!(dst_path.symlink_metadata().is_err(), "{dst_path:?} already exists");
            Ok(CopyAction { src_path, dst_path, src_is_dir: src_metadata.is_dir() })
        })
        .collect()
}

fn copy(src_path: &Path, dst_path: &Path, src_is_dir: bool) -> anyhow::Result<()> {
    (|| {
        if src_is_dir {
            // TODO: Make the code cross-plateform.
            let status = Command::new("cp")
                .args(["-rH", "--"])
                .args([src_path, dst_path])
                .status()
                .context("failed to execute process")?;
            ensure!(status.success(), "error status: {status}");
        } else {
            fs::copy(src_path, dst_path)?;
        }
        anyhow::Ok(())
    })()
    .with_context(|| format!("failed to copy {src_path:?} to {dst_path:?}"))
}

struct CopyAction {
    src_path: PathBuf,
    dst_path: PathBuf,
    src_is_dir: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    use assert_fs::fixture::{FileWriteStr, PathChild, PathCreateDir, SymlinkToDir, SymlinkToFile};
    use assert_fs::TempDir;
    use time::macros::datetime;

    use test_helper::{check_err_contains, Check};

    // TODO: make the code more readable and then remove most comments.
    // The future code will probably write and check the directory content with YAML. Example:
    // directory_name:
    //   subdirectory_name:
    //     file_name: "file content"
    //   symlink_name: [{"symlink_to": "path/to/target"}]

    #[test]
    fn simple_demo() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── colors/
        // │  ├── dark/
        // │  │  └── black
        // │  └── red
        // └── picture
        temp.child("colors").create_dir_all()?;
        temp.child("colors/dark").create_dir_all()?;
        temp.child("colors/dark/black").write_str("ink")?;
        temp.child("colors/red").write_str("blood")?;
        temp.child("picture").write_str("photo")?;
        launch_work(&temp, ["colors", "picture"], datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── colors/
        // │  ├── dark/
        // │  │  └── black
        // │  └── red
        // ├── colors_2022-12-13-14h15/
        // │  ├── dark/
        // │  │  └── black
        // │  └── red
        // ├── picture
        // └── picture_2022-12-13-14h15
        temp.child("colors_2022-12-13-14h15").check_is_dir()?;
        temp.child("colors_2022-12-13-14h15/dark").check_is_dir()?;
        temp.child("colors_2022-12-13-14h15/dark/black").check_is_file_with_content("ink")?;
        temp.child("colors_2022-12-13-14h15/red").check_is_file_with_content("blood")?;
        temp.child("picture_2022-12-13-14h15").check_is_file_with_content("photo")
    }

    #[test]
    fn demo_with_symlinks() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── colors -> words
        // ├── picture -> sea
        // ├── sea
        // └── words/
        //    ├── blue -> ../sea
        //    ├── dark/
        //    │  └── black
        //    ├── not_light -> dark
        //    └── red
        temp.child("colors").symlink_to_dir("words")?;
        temp.child("picture").symlink_to_file("sea")?;
        temp.child("sea").write_str("massive")?;
        temp.child("words").create_dir_all()?;
        temp.child("words/blue").symlink_to_file("../sea")?;
        temp.child("words/dark").create_dir_all()?;
        temp.child("words/dark/black").write_str("ink")?;
        temp.child("words/not_light").symlink_to_dir("dark")?;
        temp.child("words/red").write_str("blood")?;
        launch_work(&temp, ["colors", "picture"], datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── colors -> words
        // ├── colors_2022-12-13-14h15/
        // │  ├── blue -> ../sea
        // │  ├── dark/
        // │  │  └── black
        // │  ├── not_light -> dark
        // │  └── red
        // ├── picture -> sea
        // ├── picture_2022-12-13-14h15
        // ├── sea
        // └── words/
        //    ├── blue -> ../sea
        //    ├── dark/
        //    │  └── black
        //    ├── not_light -> dark
        //    └── red
        //
        // Remark: `backup` follows command-line symlinks only, so "colors_2022-12-13-14h15" and
        // "picture_2022-12-13-14h15" are not symlinks, but the copies of "blue" and "not_light"
        // are symlinks.
        temp.child("colors_2022-12-13-14h15").check_is_dir()?;
        temp.child("colors_2022-12-13-14h15/blue").check_is_symlink_to("../sea")?;
        temp.child("colors_2022-12-13-14h15/dark").check_is_dir()?;
        temp.child("colors_2022-12-13-14h15/dark/black").check_is_file_with_content("ink")?;
        temp.child("colors_2022-12-13-14h15/not_light").check_is_symlink_to("dark")?;
        temp.child("colors_2022-12-13-14h15/red").check_is_file_with_content("blood")?;
        temp.child("picture_2022-12-13-14h15").check_is_file_with_content("massive")
    }

    #[test]
    fn symlinks_to_symlinks() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── colors -> things
        // ├── picture -> place
        // ├── place -> sea
        // ├── sea
        // ├── things -> words
        // └── words/
        //    ├── dark -> non_existent_path
        //    └── not_light -> dark
        temp.child("colors").symlink_to_dir("things")?;
        temp.child("picture").symlink_to_file("place")?;
        temp.child("place").symlink_to_file("sea")?;
        temp.child("sea").write_str("massive")?;
        temp.child("things").symlink_to_dir("words")?;
        temp.child("words").create_dir_all()?;
        temp.child("words/dark").symlink_to_file("non_existent_path")?;
        temp.child("words/not_light").symlink_to_file("dark")?;
        launch_work(&temp, ["colors", "picture"], datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── colors -> things
        // ├── colors_2022-12-13-14h15/
        // |   ├── dark -> non_existent_path
        // |   └── not_light -> dark
        // ├── picture -> place
        // ├── picture_2022-12-13-14h15
        // ├── place -> sea
        // ├── sea
        // ├── things -> words
        // └── words/
        //    ├── dark -> non_existent_path
        //    └── not_light -> dark
        //
        // Remark: `backup` follows command-line symlinks only, so "colors_2022-12-13-14h15" and
        // "picture_2022-12-13-14h15" are not symlinks, but the copies of "dark" and "not_light"
        // are symlinks.
        temp.child("colors_2022-12-13-14h15").check_is_dir()?;
        temp.child("colors_2022-12-13-14h15/dark").check_is_symlink_to("non_existent_path")?;
        temp.child("colors_2022-12-13-14h15/not_light").check_is_symlink_to("dark")?;
        temp.child("picture_2022-12-13-14h15").check_is_file_with_content("massive")
    }

    #[test]
    fn fancy_directory_names() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let dir_names = ["foo.abc.xyz", " ", "--b a r", "--", "-"];
        dir_names.iter().try_for_each(|p| temp.child(p).create_dir_all())?;
        launch_work(&temp, dir_names, datetime!(2022-12-13 14:15:16 UTC))?;
        temp.child("foo.abc.xyz_2022-12-13-14h15").check_is_dir()?;
        temp.child(" _2022-12-13-14h15").check_is_dir()?;
        temp.child("--b a r_2022-12-13-14h15").check_is_dir()?;
        temp.child("--_2022-12-13-14h15").check_is_dir()?;
        temp.child("-_2022-12-13-14h15").check_is_dir()
    }

    #[test]
    fn fancy_file_names() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let file_names = ["foo.abc.xyz", " ", "--b a r", "--", "-"];
        file_names.iter().try_for_each(|p| temp.child(p).write_str("whatever"))?;
        launch_work(&temp, file_names, datetime!(2022-12-13 14:15:16 UTC))?;
        temp.child("foo.abc.xyz_2022-12-13-14h15").check_is_file_with_content("whatever")?;
        temp.child(" _2022-12-13-14h15").check_is_file_with_content("whatever")?;
        temp.child("--b a r_2022-12-13-14h15").check_is_file_with_content("whatever")?;
        temp.child("--_2022-12-13-14h15").check_is_file_with_content("whatever")?;
        temp.child("-_2022-12-13-14h15").check_is_file_with_content("whatever")
    }

    #[test]
    fn fail_if_src_path_does_not_have_a_name() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // │  └── baz/
        // └── foo/
        temp.child("bar").create_dir_all()?;
        temp.child("bar/baz").create_dir_all()?;
        temp.child("foo").create_dir_all()?;
        let result = launch_work(&temp, ["foo", "bar/baz/.."], datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "does not have a name")?;
        temp.child("foo_2022-12-13-14h15").check_does_not_exist()
    }

    #[test]
    fn fail_if_src_path_does_not_exist() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        temp.child("foo").create_dir_all()?;
        let result = launch_work(&temp, ["foo", "bar"], datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "failed to read metadata")?;
        temp.child("foo_2022-12-13-14h15").check_does_not_exist()
    }

    #[test]
    fn fail_if_src_path_is_a_broken_symlink() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar -> baz
        // ├── baz -> non_existent_path
        // └── foo/
        temp.child("bar").symlink_to_file("baz")?;
        temp.child("baz").symlink_to_file("non_existent_path")?;
        temp.child("foo").create_dir_all()?;
        let result = launch_work(&temp, ["foo", "bar"], datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "failed to read metadata")?;
        temp.child("foo_2022-12-13-14h15").check_does_not_exist()
    }

    #[test]
    fn fail_if_dst_path_is_a_directory() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // ├── bar_2022-12-13-14h15/
        // └── foo/
        temp.child("bar").create_dir_all()?;
        temp.child("bar_2022-12-13-14h15").create_dir_all()?;
        temp.child("foo").create_dir_all()?;
        let result = launch_work(&temp, ["foo", "bar"], datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "already exists")?;
        temp.child("foo_2022-12-13-14h15").check_does_not_exist()
    }

    #[test]
    fn fail_if_dst_path_is_a_file() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar
        // ├── bar_2022-12-13-14h15
        // └── foo
        temp.child("bar").write_str("whatever")?;
        temp.child("bar_2022-12-13-14h15").write_str("whatever")?;
        temp.child("foo").write_str("whatever")?;
        let result = launch_work(&temp, ["foo", "bar"], datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "already exists")?;
        temp.child("foo_2022-12-13-14h15").check_does_not_exist()
    }

    #[test]
    fn fail_if_dst_path_is_a_symlink() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // ├── bar_2022-12-13-14h15 -> non_existent_path
        // └── foo/
        temp.child("bar").create_dir_all()?;
        temp.child("bar_2022-12-13-14h15").symlink_to_file("non_existent_path")?;
        temp.child("foo").create_dir_all()?;
        let result = launch_work(&temp, ["foo", "bar"], datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "already exists")?;
        temp.child("foo_2022-12-13-14h15").check_does_not_exist()
    }

    fn launch_work<const N: usize>(
        temp: &TempDir,
        arg_paths: [&str; N],
        now: OffsetDateTime,
    ) -> anyhow::Result<()> {
        let src_paths = arg_paths.iter().map(|path| temp.child(path).to_path_buf()).collect();
        work(src_paths, now)
    }
}
