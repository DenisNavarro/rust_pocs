#![forbid(unsafe_code)]
#![warn(clippy::nursery, clippy::pedantic)]

use std::borrow::Cow;
use std::fs::{self, DirEntry, Metadata};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{ensure, Context};
use camino::Utf8Path;
use clap::Parser;
use regex::Regex;
use time::{format_description, OffsetDateTime};

#[derive(Parser)]
/// Synchronize a directory with a backup directory by renaming a suffix and calling rsync.
/// Tested on Linux.
///
/// For example, on 2022-12-13 14:15:16, if the directory `/my/hard/drive/foo_2022-08-09-10h11`
/// exists, then `synchronize_backup /path/to/foo /my/hard/drive` renames
/// `/my/hard/drive/foo_2022-08-09-10h11` to `/my/hard/drive/foo_2022-12-13-14h15` and then calls
/// `time rsync -aAXHv --delete --stats -- /path/to/foo/ /my/hard/drive/foo_2022-12-13-14h15`.
///
/// If there is no directory candidate to rename, `rsync` is called anyway and creates a new one.
/// If there are several candidates, no one is renamed, `rsync` is not called and an error code is
/// returned.
///
/// `synchronize_backup` follows command-line symlinks.
///
/// In the current implementation, the source directory path must be a valid UTF-8 sequence.
struct Cli {
    src_dir_path: String,
    dst_dir_path: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let now = OffsetDateTime::now_local().context("could not determine the local offset")?;
    work(cli.src_dir_path.into(), &cli.dst_dir_path, now)
}

fn work(src_dir_path: Cow<str>, dst_dir_path: &Path, now: OffsetDateTime) -> anyhow::Result<()> {
    let src_dir_name = check_src_dir_path_is_ok(src_dir_path.as_ref())?;
    let final_dst_path = get_final_dst_path(src_dir_name, dst_dir_path.to_owned(), now);
    check_is_directory_or_does_not_exist(&final_dst_path)?;
    maybe_rename_a_candidate_to_final_dst(src_dir_name, dst_dir_path, &final_dst_path)?;
    writeln!(io::stdout(), "Synchronize {src_dir_path:?} with {final_dst_path:?}.")
        .context("failed to write to stdout")?;
    synchronize(src_dir_path, &final_dst_path)
}

fn check_src_dir_path_is_ok(src_dir_path: &str) -> anyhow::Result<&str> {
    let src_dir_name = Utf8Path::new(src_dir_path)
        .file_name()
        .with_context(|| format!("{src_dir_path:?} does not have a name"))?;
    let src_dir_metadata = fs::metadata(src_dir_path)
        .with_context(|| format!("failed to read metadata from {src_dir_path:?}"))?;
    ensure!(src_dir_metadata.is_dir(), "{src_dir_path:?} is not a directory");
    Ok(src_dir_name)
}

fn get_final_dst_path(src_dir_name: &str, dst_dir_path: PathBuf, now: OffsetDateTime) -> PathBuf {
    let format = format_description::parse("_[year]-[month]-[day]-[hour]h[minute]").unwrap();
    let suffix = now.format(&format).unwrap();
    let dst_dir_name = format!("{src_dir_name}{suffix}");
    let mut result = dst_dir_path;
    result.push(dst_dir_name);
    result
}

fn check_is_directory_or_does_not_exist(path: &Path) -> anyhow::Result<()> {
    if let Ok(metadata) = path.symlink_metadata() {
        ensure!(metadata.is_dir(), "{path:?} exists but is not a directory");
    }
    Ok(())
}

fn maybe_rename_a_candidate_to_final_dst(
    src_dir_name: &str,
    dst_dir_path: &Path,
    final_dst_path: &Path,
) -> anyhow::Result<()> {
    let candidates =
        get_candidates(src_dir_name, dst_dir_path).context("failed to look for candidates")?;
    ensure!(candidates.len() < 2, "there are several candidates: {candidates:?}");
    if let Some(candidate) = candidates.get(0) {
        fs::rename(candidate, final_dst_path)
            .with_context(|| format!("failed to renamed {candidate:?} to {final_dst_path:?}"))?;
        writeln!(io::stdout(), "Renamed {candidate:?} to {final_dst_path:?}.")
            .context("failed to write to stdout")?;
    }
    Ok(())
}

fn get_candidates(src_dir_name: &str, dst_dir_path: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let regex = Regex::new(
        r"^(.*)_[[:digit:]]{4}-[[:digit:]]{2}-[[:digit:]]{2}-[[:digit:]]{2}h[[:digit:]]{2}$",
    )
    .unwrap();
    let entries_and_errors = fs::read_dir(dst_dir_path)
        .with_context(|| format!("failed to read as a directory {dst_dir_path:?}"))?;
    let mut result = Vec::<PathBuf>::new();
    for entry_or_err in entries_and_errors {
        let entry =
            entry_or_err.with_context(|| format!("failed to read an entry in {dst_dir_path:?}"))?;
        let metadata =
            entry.metadata().with_context(|| format!("failed to read metadata from {entry:?}"))?;
        if is_candidate(&entry, &metadata, src_dir_name, &regex) {
            result.push(entry.path());
        }
    }
    Ok(result)
}

fn is_candidate(entry: &DirEntry, metadata: &Metadata, src_dir_name: &str, regex: &Regex) -> bool {
    if !metadata.is_dir() {
        return false;
    };
    let dir_name = entry.file_name();
    let Some(dir_name) = dir_name.to_str() else {
        return false;
    };
    let Some(capture) = regex.captures(dir_name) else {
        return false;
    };
    &capture[1] == src_dir_name
}

fn synchronize(mut src_path: Cow<str>, dst_path: &Path) -> anyhow::Result<()> {
    if !src_path.as_ref().ends_with('/') {
        src_path.to_mut().push('/');
    }
    Command::new("time")
        .args(["rsync", "-aAXHv", "--delete", "--stats", "--", src_path.as_ref()])
        .arg(dst_path)
        .status()
        .context("failed to execute process")
        .and_then(|status| {
            status.success().then_some(()).with_context(|| format!("error status: {status}"))
        })
        .with_context(|| format!("failed to synchronize {src_path:?} with {dst_path:?}"))
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
    fn demo_without_update() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // Before:
        // .
        // ├── bar/
        // └── foo/
        //    └── colors/
        //       ├── dark/
        //       │  └── black
        //       └── red
        tmp.create_dirs(["bar", "foo", "foo/colors", "foo/colors/dark"])?;
        tmp.create_files(["foo/colors/dark/black", "foo/colors/red"])?;
        launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── bar/
        // │  └── colors_2022-12-13-14h15/
        // │     ├── dark/
        // │     │  └── black
        // │     └── red
        // └── foo/
        //    └── colors/
        //       ├── dark/
        //       │  └── black
        //       └── red
        tmp.check_dirs_exist_and_are_not_symlinks([
            "bar/colors_2022-12-13-14h15",
            "bar/colors_2022-12-13-14h15/dark",
        ])?;
        tmp.check_files_exist_and_are_not_symlinks([
            "bar/colors_2022-12-13-14h15/dark/black",
            "bar/colors_2022-12-13-14h15/red",
        ])
    }

    #[test]
    fn demo_with_update() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // Before:
        // .
        // ├── bar/
        // │  └── colors_2022-08-09-10h11/
        // │     ├── green
        // │     └── light/
        // │        └── white
        // └── foo/
        //    └── colors/
        //       ├── dark/
        //       │  └── black
        //       └── red
        tmp.create_dirs([
            "bar",
            "bar/colors_2022-08-09-10h11",
            "bar/colors_2022-08-09-10h11/light",
            "foo",
            "foo/colors",
            "foo/colors/dark",
        ])?;
        tmp.create_files([
            "bar/colors_2022-08-09-10h11/green",
            "bar/colors_2022-08-09-10h11/light/white",
            "foo/colors/dark/black",
            "foo/colors/red",
        ])?;
        launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── bar/
        // │  └── colors_2022-12-13-14h15/
        // │     ├── dark/
        // │     │  └── black
        // │     └── red
        // └── foo/
        //    └── colors/
        //       ├── dark/
        //       │  └── black
        //       └── red
        tmp.check_dirs_exist_and_are_not_symlinks([
            "bar/colors_2022-12-13-14h15",
            "bar/colors_2022-12-13-14h15/dark",
        ])?;
        tmp.check_files_exist_and_are_not_symlinks([
            "bar/colors_2022-12-13-14h15/dark/black",
            "bar/colors_2022-12-13-14h15/red",
        ])?;
        tmp.check_do_not_exist([
            "bar/colors_2022-08-09-10h11",
            "bar/colors_2022-12-13-14h15/green",
            "bar/colors_2022-12-13-14h15/light",
        ])
    }

    #[test]
    #[cfg(unix)]
    fn demo_with_symlinks() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // Before:
        // .
        // ├── bar -> baz
        // ├── baz/
        // │  └── colors_2022-08-09-10h11/
        // │     ├── green
        // │     └── light/
        // │        └── white
        // └── foo/
        //    ├── colors -> words
        //    ├── sea
        //    └── words/
        //       ├── blue -> ../sea
        //       ├── dark/
        //       │  └── black
        //       ├── not_light -> dark
        //       └── red
        tmp.create_dirs([
            "baz",
            "baz/colors_2022-08-09-10h11",
            "baz/colors_2022-08-09-10h11/light",
            "foo",
            "foo/words",
            "foo/words/dark",
        ])?;
        tmp.create_files([
            "baz/colors_2022-08-09-10h11/green",
            "baz/colors_2022-08-09-10h11/light/white",
            "foo/sea",
            "foo/words/dark/black",
            "foo/words/red",
        ])?;
        tmp.create_symlinks([
            ("bar", "baz"),
            ("foo/colors", "words"),
            ("foo/words/blue", "../sea"),
            ("foo/words/not_light", "dark"),
        ])?;
        launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── bar -> baz
        // ├── baz/
        // │  └── colors_2022-12-13-14h15/
        // │     ├── blue -> ../sea
        // │     ├── dark/
        // │     │  └── black
        // │     ├── not_light -> dark
        // │     └── red
        // └── foo/
        //    ├── colors -> words
        //    ├── sea
        //    └── words/
        //       ├── blue -> ../sea
        //       ├── dark/
        //       │  └── black
        //       ├── not_light -> dark
        //       └── red
        //
        // Remark: `synchronize_backup` follows command-line symlinks only, so
        // "colors_2022-12-13-14h15" is not a symlink, but the copies of "blue" and "not_light"
        // are symlinks. Note that "colors_2022-12-13-14h15/blue" points to an unexisting path.
        tmp.check_dirs_exist_and_are_not_symlinks([
            "baz/colors_2022-12-13-14h15",
            "baz/colors_2022-12-13-14h15/dark",
        ])?;
        tmp.check_files_exist_and_are_not_symlinks([
            "baz/colors_2022-12-13-14h15/dark/black",
            "baz/colors_2022-12-13-14h15/red",
        ])?;
        tmp.check_symlinks_exist([
            "baz/colors_2022-12-13-14h15/blue",
            "baz/colors_2022-12-13-14h15/not_light",
        ])?;
        tmp.check_do_not_exist([
            "baz/colors_2022-08-09-10h11",
            "baz/colors_2022-12-13-14h15/green",
            "baz/colors_2022-12-13-14h15/light",
        ])
    }

    #[test]
    #[cfg(unix)]
    fn symlinks_to_symlinks() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // Before:
        // .
        // ├── bar -> bay
        // ├── bay -> baz
        // ├── baz/
        // │  ├── colors_2022-08-09-10h11/
        // │  │  ├── light -> ../sun
        // │  │  └── not_dark -> light
        // │  └── sun
        // └── foo/
        //    ├── colors -> things
        //    ├── things -> words
        //    └── words/
        //       ├── dark -> non_existent_path
        //       └── not_light -> dark
        tmp.create_dirs(["baz", "baz/colors_2022-08-09-10h11", "foo", "foo/words"])?;
        tmp.create_file("baz/sun")?;
        tmp.create_symlinks([
            ("bar", "bay"),
            ("bay", "baz"),
            ("baz/colors_2022-08-09-10h11/light", "../sun"),
            ("baz/colors_2022-08-09-10h11/not_dark", "light"),
            ("foo/colors", "things"),
            ("foo/things", "words"),
            ("foo/words/dark", "non_existent_path"),
            ("foo/words/not_light", "dark"),
        ])?;
        launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── bar -> bay
        // ├── bay -> baz
        // ├── baz/
        // │  ├── colors_2022-12-13-14h15/
        // │  │  ├── dark -> non_existent_path
        // │  │  └── not_light -> dark
        // │  └── sun
        // └── foo/
        //    ├── colors -> things
        //    ├── things -> words
        //    └── words/
        //       ├── dark -> non_existent_path
        //       └── not_light -> dark
        //
        // Remark: `synchronize_backup` follows command-line symlinks only, so
        // "colors_2022-12-13-14h15" is not a symlink, but the copies of "dark" and "not_light"
        // are symlinks.
        tmp.check_dir_exists_and_is_not_a_symlink("baz/colors_2022-12-13-14h15")?;
        tmp.check_symlinks_exist([
            "baz/colors_2022-12-13-14h15/dark",
            "baz/colors_2022-12-13-14h15/not_light",
        ])?;
        tmp.check_do_not_exist([
            "baz/colors_2022-08-09-10h11",
            "baz/colors_2022-12-13-14h15/light",
            "baz/colors_2022-12-13-14h15/not_dark",
        ])
    }

    #[test]
    fn src_path_with_an_ending_slash() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // Before:
        // .
        // ├── bar/
        // └── foo/
        //    └── colors/
        tmp.create_dirs(["bar", "foo", "foo/colors"])?;
        launch_work(&tmp, "foo/colors/", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── bar/
        // |  └── colors_2022-12-13-14h15/
        // └── foo/
        //    └── colors/
        tmp.check_dir_exists_and_is_not_a_symlink("bar/colors_2022-12-13-14h15")
    }

    #[test]
    fn final_dst_path_already_exists_and_is_a_directory() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // Before:
        // .
        // ├── bar/
        // |  └── colors_2022-12-13-14h15/
        // └── foo/
        //    └── colors/
        //       └── red
        tmp.create_dirs(["bar", "bar/colors_2022-12-13-14h15", "foo", "foo/colors"])?;
        tmp.create_file("foo/colors/red")?;
        launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── bar/
        // |  └── colors_2022-12-13-14h15/
        // |     └── red
        // └── foo/
        //    └── colors/
        //       └── red
        tmp.check_file_exists_and_is_not_a_symlink("bar/colors_2022-12-13-14h15/red")
    }

    #[test]
    fn fancy_directory_names() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        let now = datetime!(2022-12-13 14:15:16 UTC);
        tmp.create_dir("foo")?;
        for (src_path, dst_path) in [
            ("foo/colors.abc.xyz", "bar.abc.xyz"),
            ("foo/ ", " "),
            ("foo/c --o l o r s", "--b a r"),
            ("foo/co -- lors", "--"),
            ("foo/-", "-"),
        ] {
            tmp.create_dirs([src_path, dst_path])?;
            launch_work(&tmp, src_path, dst_path, now)?;
        }
        tmp.check_dirs_exist_and_are_not_symlinks([
            "bar.abc.xyz/colors.abc.xyz_2022-12-13-14h15",
            " / _2022-12-13-14h15",
            "--b a r/c --o l o r s_2022-12-13-14h15",
            "--/co -- lors_2022-12-13-14h15",
            "-/-_2022-12-13-14h15",
        ])
    }

    #[test]
    fn fail_if_two_valid_candidates() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // ├── bar/
        // |  ├── colors_2022-08-09-10h11/
        // |  └── colors_2022-09-10-11h12/
        // └── foo/
        //    └── colors/
        let valid_candidates = ["bar/colors_2022-08-09-10h11", "bar/colors_2022-09-10-11h12"];
        tmp.create_dirs(["bar", "foo", "foo/colors"])?;
        tmp.create_dirs(&valid_candidates)?;
        let result = launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "there are several candidates")?;
        tmp.check_dirs_exist_and_are_not_symlinks(valid_candidates)?;
        tmp.check_does_not_exist("bar/colors_2022-12-13-14h15")
    }

    #[test]
    fn valid_and_invalid_candidates() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // Before:
        // .
        // ├── bar/
        // |  ├── colors2022-08-09-10h11/
        // |  ├── colors_222-08-09-10h11/
        // |  ├── colors_2022-08-09-10h11/
        // |  ├── colors_2022-08-09-10h11m12/
        // |  ├── colors_2022-08-bb-10h11/
        // |  ├── colors_2022-09-10-11h12
        // |  ├── colors_2022-AA-09-10h11/
        // |  └── some_colors_2022-08-09-10h11/
        // └── foo/
        //    └── colors/
        let valid_candidate = "bar/colors_2022-08-09-10h11";
        let invalid_directory_candidates = [
            "bar/colors2022-08-09-10h11",
            "bar/colors_222-08-09-10h11",
            "bar/colors_2022-08-09-10h11m12",
            "bar/colors_2022-08-bb-10h11",
            "bar/colors_2022-AA-09-10h11",
            "bar/some_colors_2022-08-09-10h11",
        ];
        let file_candidate = "bar/colors_2022-09-10-11h12"; // file, so invalid
        tmp.create_dirs(["bar", "foo", "foo/colors"])?;
        tmp.create_dir(valid_candidate)?;
        tmp.create_dirs(&invalid_directory_candidates)?;
        tmp.create_file(file_candidate)?;
        launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── bar/
        // |  ├── colors2022-08-09-10h11/
        // |  ├── colors_222-08-09-10h11/
        // |  ├── colors_2022-08-09-10h11m12/
        // |  ├── colors_2022-08-bb-10h11/
        // |  ├── colors_2022-09-10-11h12
        // |  ├── colors_2022-12-13-14h15/
        // |  ├── colors_2022-AA-09-10h11/
        // |  └── some_colors_2022-08-09-10h11/
        // └── foo/
        //    └── colors/
        tmp.check_does_not_exist(valid_candidate)?;
        tmp.check_file_exists_and_is_not_a_symlink(file_candidate)?;
        tmp.check_dirs_exist_and_are_not_symlinks(invalid_directory_candidates)?;
        tmp.check_dir_exists_and_is_not_a_symlink("bar/colors_2022-12-13-14h15")
    }

    #[test]
    #[cfg(unix)]
    fn symlink_is_invalid_candidate() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // Before:
        // .
        // ├── bar/
        // |  ├── baz/
        // |  ├── colors_2022-08-09-10h11/
        // |  └── colors_2022-09-10-11h12 -> baz
        // └── foo/
        //    └── colors/
        tmp.create_dirs(["bar", "bar/baz", "bar/colors_2022-08-09-10h11", "foo", "foo/colors"])?;
        tmp.create_symlink("bar/colors_2022-09-10-11h12", "baz")?;
        launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── bar/
        // |  ├── baz/
        // |  ├── colors_2022-09-10-11h12 -> baz
        // |  └── colors_2022-12-13-14h15/
        // └── foo/
        //    └── colors/
        tmp.check_does_not_exist("bar/colors_2022-08-09-10h11")?;
        tmp.check_symlink_exists("bar/colors_2022-09-10-11h12")?;
        tmp.check_dir_exists_and_is_not_a_symlink("bar/colors_2022-12-13-14h15")
    }

    #[test]
    fn fail_if_src_path_does_not_have_a_name() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // ├── bar/
        // └── foo/
        //    └── colors/
        //       └── dark/
        tmp.create_dirs(["bar", "foo", "foo/colors", "foo/colors/dark"])?;
        let result =
            launch_work(&tmp, "foo/colors/dark/..", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "does not have a name")?;
        tmp.check_does_not_exist("bar/colors_2022-12-13-14h15")
    }

    #[test]
    fn fail_if_src_path_does_not_exist() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // ├── bar/
        // └── foo/
        tmp.create_dirs(["bar", "foo"])?;
        let result = launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "failed to read metadata")?;
        tmp.check_does_not_exist("bar/colors_2022-12-13-14h15")
    }

    #[test]
    fn fail_if_src_path_is_a_file() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // ├── bar/
        // └── foo/
        //    └── colors
        tmp.create_dirs(["bar", "foo"])?;
        tmp.create_file("foo/colors")?;
        let result = launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "is not a directory")?;
        tmp.check_does_not_exist("bar/colors_2022-12-13-14h15")
    }

    #[test]
    #[cfg(unix)]
    fn fail_if_src_path_is_a_symlink_to_a_file() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // ├── bar/
        // └── foo/
        //    ├── colors -> words
        //    └── words
        tmp.create_dirs(["bar", "foo"])?;
        tmp.create_file("foo/words")?;
        tmp.create_symlink("foo/colors", "words")?;
        let result = launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "is not a directory")?;
        tmp.check_does_not_exist("bar/colors_2022-12-13-14h15")
    }

    #[test]
    #[cfg(unix)]
    fn fail_if_src_path_is_a_broken_symlink() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // ├── bar/
        // └── foo/
        //    ├── colors -> words
        //    └── words -> non_existent_path
        tmp.create_dirs(["bar", "foo"])?;
        tmp.create_symlinks([("foo/colors", "words"), ("foo/words", "non_existent_path")])?;
        let result = launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "failed to read metadata")?;
        tmp.check_does_not_exist("bar/colors_2022-12-13-14h15")
    }

    #[test]
    fn fail_if_dst_path_does_not_exist() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // └── foo/
        //    └── colors/
        tmp.create_dirs(["foo", "foo/colors"])?;
        let result = launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result.as_ref(), "failed to look for candidates")?;
        check_err_contains(result, "failed to read as a directory")
    }

    #[test]
    fn fail_if_dst_path_is_a_file() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // ├── bar
        // └── foo/
        //    └── colors/
        tmp.create_dirs(["foo", "foo/colors"])?;
        tmp.create_file("bar")?;
        let result = launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result.as_ref(), "failed to look for candidates")?;
        check_err_contains(result, "failed to read as a directory")?;
        tmp.check_file_exists_and_is_not_a_symlink("bar")
    }

    #[test]
    #[cfg(unix)]
    fn fail_if_dst_path_is_a_symlink_to_a_file() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // ├── bar -> baz
        // ├── baz
        // └── foo/
        //    └── colors/
        tmp.create_dirs(["foo", "foo/colors"])?;
        tmp.create_file("baz")?;
        tmp.create_symlink("bar", "baz")?;
        let result = launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result.as_ref(), "failed to look for candidates")?;
        check_err_contains(result, "failed to read as a directory")?;
        tmp.check_symlink_exists("bar")?;
        tmp.check_does_not_exist("baz/colors_2022-12-13-14h15")
    }

    #[test]
    #[cfg(unix)]
    fn fail_if_dst_path_is_a_broken_symlink() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // ├── bar -> baz
        // ├── baz -> non_existent_path
        // └── foo/
        //    └── colors/
        tmp.create_dirs(["foo", "foo/colors"])?;
        tmp.create_symlinks([("bar", "baz"), ("baz", "non_existent_path")])?;
        let result = launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result.as_ref(), "failed to look for candidates")?;
        check_err_contains(result, "failed to read as a directory")?;
        tmp.check_symlinks_exist(["bar", "baz"])
    }

    #[test]
    fn fail_if_final_dst_path_is_a_file() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // ├── bar/
        // │  └── colors_2022-12-13-14h15
        // └── foo/
        //    └── colors/
        tmp.create_dirs(["bar", "foo", "foo/colors"])?;
        tmp.create_file("bar/colors_2022-12-13-14h15")?;
        let result = launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "exists but is not a directory")?;
        tmp.check_file_exists_and_is_not_a_symlink("bar/colors_2022-12-13-14h15")
    }

    #[test]
    #[cfg(unix)]
    fn fail_if_final_dst_path_is_a_symlink() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // ├── bar/
        // │  ├── baz/
        // │  └── colors_2022-12-13-14h15 -> baz
        // └── foo/
        //    └── colors/
        tmp.create_dirs(["bar", "bar/baz", "foo", "foo/colors"])?;
        tmp.create_symlink("bar/colors_2022-12-13-14h15", "baz")?;
        let result = launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "exists but is not a directory")?;
        tmp.check_symlink_exists("bar/colors_2022-12-13-14h15")
    }

    fn launch_work(
        tmp: &TemporaryDirectory,
        src_path: &str,
        dst_path: &str,
        now: OffsetDateTime,
    ) -> anyhow::Result<()> {
        let src_dir_path = tmp.get_path(src_path);
        let src_dir_path = src_dir_path.to_str().unwrap(); // hoping the path is an UTF-8 sequence
        let dst_dir_path = tmp.get_path(dst_path);
        work(src_dir_path.into(), &dst_dir_path, now)
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
