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

    use std::fmt::Debug;

    use time::macros::datetime;

    use temporary_directory::TemporaryDirectory;

    // TODO: make the code more readable and then remove most comments.
    // The future code will probably write and check the directory content with YAML. Example:
    // directory_name:
    //   subdirectory_name:
    //     file_name: "file content"
    //   symlink_name: [{"symlink_to": "path/to/target"}]

    #[test]
    fn simple_demo() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // Before:
        // .
        // ├── colors/
        // │  ├── dark/
        // │  │  └── black
        // │  └── red
        // └── picture
        tmp.create_dirs(["colors", "colors/dark"])?;
        tmp.create_files(["colors/dark/black", "colors/red", "picture"])?;
        launch_work(&tmp, ["colors", "picture"], datetime!(2022-12-13 14:15:16 UTC))?;
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
        tmp.check_dirs_exist_and_are_not_symlinks([
            "colors_2022-12-13-14h15",
            "colors_2022-12-13-14h15/dark",
        ])?;
        tmp.check_files_exist_and_are_not_symlinks([
            "colors_2022-12-13-14h15/dark/black",
            "colors_2022-12-13-14h15/red",
            "picture_2022-12-13-14h15",
        ])
    }

    #[test]
    #[cfg(unix)]
    fn demo_with_symlinks() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
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
        tmp.create_dirs(["words", "words/dark"])?;
        tmp.create_files(["sea", "words/dark/black", "words/red"])?;
        tmp.create_symlinks([
            ("colors", "words"),
            ("picture", "sea"),
            ("words/blue", "../sea"),
            ("words/not_light", "dark"),
        ])?;
        launch_work(&tmp, ["colors", "picture"], datetime!(2022-12-13 14:15:16 UTC))?;
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
        tmp.check_dirs_exist_and_are_not_symlinks([
            "colors_2022-12-13-14h15",
            "colors_2022-12-13-14h15/dark",
        ])?;
        tmp.check_files_exist_and_are_not_symlinks([
            "colors_2022-12-13-14h15/dark/black",
            "colors_2022-12-13-14h15/red",
            "picture_2022-12-13-14h15",
        ])?;
        tmp.check_symlinks_exist([
            "colors_2022-12-13-14h15/blue",
            "colors_2022-12-13-14h15/not_light",
        ])
    }

    #[test]
    #[cfg(unix)]
    fn symlinks_to_symlinks() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
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
        tmp.create_dir("words")?;
        tmp.create_file("sea")?;
        tmp.create_symlinks([
            ("colors", "things"),
            ("picture", "place"),
            ("place", "sea"),
            ("things", "words"),
            ("words/dark", "non_existent_path"),
            ("words/not_light", "dark"),
        ])?;
        launch_work(&tmp, ["colors", "picture"], datetime!(2022-12-13 14:15:16 UTC))?;
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
        tmp.check_dir_exists_and_is_not_a_symlink("colors_2022-12-13-14h15")?;
        tmp.check_file_exists_and_is_not_a_symlink("picture_2022-12-13-14h15")?;
        tmp.check_symlinks_exist([
            "colors_2022-12-13-14h15/dark",
            "colors_2022-12-13-14h15/not_light",
        ])
    }

    #[test]
    fn fancy_directory_names() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        tmp.create_dirs(["foo.abc.xyz", " ", "--b a r", "--", "-"])?;
        launch_work(
            &tmp,
            ["foo.abc.xyz", " ", "--b a r", "--", "-"],
            datetime!(2022-12-13 14:15:16 UTC),
        )?;
        tmp.check_dirs_exist_and_are_not_symlinks([
            "foo.abc.xyz_2022-12-13-14h15",
            " _2022-12-13-14h15",
            "--b a r_2022-12-13-14h15",
            "--_2022-12-13-14h15",
            "-_2022-12-13-14h15",
        ])
    }

    #[test]
    fn fancy_file_names() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        tmp.create_files(["foo.abc.xyz", " ", "--b a r", "--", "-"])?;
        launch_work(
            &tmp,
            ["foo.abc.xyz", " ", "--b a r", "--", "-"],
            datetime!(2022-12-13 14:15:16 UTC),
        )?;
        tmp.check_files_exist_and_are_not_symlinks([
            "foo.abc.xyz_2022-12-13-14h15",
            " _2022-12-13-14h15",
            "--b a r_2022-12-13-14h15",
            "--_2022-12-13-14h15",
            "-_2022-12-13-14h15",
        ])
    }

    #[test]
    fn fail_if_src_path_does_not_have_a_name() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // ├── bar/
        // │  └── baz/
        // └── foo/
        tmp.create_dirs(["bar", "bar/baz", "foo"])?;
        let result = launch_work(&tmp, ["foo", "bar/baz/.."], datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "does not have a name")?;
        tmp.check_does_not_exist("foo_2022-12-13-14h15")
    }

    #[test]
    fn fail_if_src_path_does_not_exist() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        tmp.create_dir("foo")?;
        let result = launch_work(&tmp, ["foo", "bar"], datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "failed to read metadata")?;
        tmp.check_does_not_exist("foo_2022-12-13-14h15")
    }

    #[test]
    #[cfg(unix)]
    fn fail_if_src_path_is_a_broken_symlink() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // ├── bar -> baz
        // ├── baz -> non_existent_path
        // └── foo/
        tmp.create_dir("foo")?;
        tmp.create_symlinks([("bar", "baz"), ("baz", "non_existent_path")])?;
        let result = launch_work(&tmp, ["foo", "bar"], datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "failed to read metadata")?;
        tmp.check_does_not_exist("foo_2022-12-13-14h15")
    }

    #[test]
    fn fail_if_dst_path_is_a_directory() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // ├── bar/
        // ├── bar_2022-12-13-14h15/
        // └── foo/
        tmp.create_dirs(["bar", "bar_2022-12-13-14h15", "foo"])?;
        let result = launch_work(&tmp, ["foo", "bar"], datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "already exists")?;
        tmp.check_does_not_exist("foo_2022-12-13-14h15")
    }

    #[test]
    fn fail_if_dst_path_is_a_file() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // ├── bar
        // ├── bar_2022-12-13-14h15
        // └── foo
        tmp.create_files(["bar", "bar_2022-12-13-14h15", "foo"])?;
        let result = launch_work(&tmp, ["foo", "bar"], datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "already exists")?;
        tmp.check_does_not_exist("foo_2022-12-13-14h15")
    }

    #[test]
    #[cfg(unix)]
    fn fail_if_dst_path_is_a_symlink() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // ├── bar/
        // ├── bar_2022-12-13-14h15 -> non_existent_path
        // └── foo/
        tmp.create_dirs(["bar", "foo"])?;
        tmp.create_symlink("bar_2022-12-13-14h15", "non_existent_path")?;
        let result = launch_work(&tmp, ["foo", "bar"], datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "already exists")?;
        tmp.check_does_not_exist("foo_2022-12-13-14h15")
    }

    fn launch_work<const N: usize>(
        tmp: &TemporaryDirectory,
        arg_paths: [&str; N],
        now: OffsetDateTime,
    ) -> anyhow::Result<()> {
        let src_paths = arg_paths.iter().map(|path| tmp.get_path(path)).collect();
        work(src_paths, now)
    }

    fn check_err_contains<T, E>(result: Result<T, E>, text: impl AsRef<str>) -> anyhow::Result<()>
    where
        E: Debug,
    {
        let text = text.as_ref();
        let error = result.err().context("missing error")?;
        let msg = format!("{error:?}");
        ensure!(msg.contains(text), "the error message {msg:?} does not contain {text:?}");
        Ok(())
    }
}
