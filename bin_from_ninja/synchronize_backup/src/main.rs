#![forbid(unsafe_code)]
#![warn(clippy::nursery, clippy::pedantic)]

use std::borrow::Cow;
use std::fs::{self, DirEntry, Metadata};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{ensure, Context};
use camino::Utf8Path;
use clap::Parser;
use humantime::format_duration;
use regex_lite::Regex;
use time::macros::format_description;
use time::OffsetDateTime;

#[derive(Parser)]
/// Synchronize a directory with a backup directory by renaming a suffix and calling rsync.
/// Tested on Linux.
///
/// For example, on 2022-12-13 14:15:16, if the directory `/my/hard/drive/foo_2022-08-09-10h11`
/// exists, then `synchronize_backup /path/to/foo /my/hard/drive` renames
/// `/my/hard/drive/foo_2022-08-09-10h11` to `/my/hard/drive/foo_2022-12-13-14h15` and then calls
/// `rsync -aAXHv --delete --stats -- /path/to/foo/ /my/hard/drive/foo_2022-12-13-14h15`.
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
    let Cli { src_dir_path, dst_dir_path } = Cli::parse();
    let now = OffsetDateTime::now_local().context("could not determine the local offset")?;
    work(src_dir_path.into(), &dst_dir_path, now)
}

fn work(src_dir_path: Cow<str>, dst_dir_path: &Path, now: OffsetDateTime) -> anyhow::Result<()> {
    let src_dir_name = check_src_dir_path_is_ok(src_dir_path.as_ref())?;
    let final_dst_path = get_final_dst_path(src_dir_name, dst_dir_path.to_owned(), now);
    check_is_directory_or_does_not_exist(&final_dst_path)?;
    maybe_rename_a_candidate_to_final_dst(src_dir_name, dst_dir_path, &final_dst_path)?;
    writeln!(io::stdout(), "Synchronize {src_dir_path:?} with {final_dst_path:?}.")
        .context("failed to write to stdout")?;
    execute_and_print_elapsed_time(|| synchronize(src_dir_path, &final_dst_path))
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
    let format = format_description!("_[year]-[month]-[day]-[hour]h[minute]");
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
    regex.captures(dir_name).is_some_and(|capture| &capture[1] == src_dir_name)
}

fn execute_and_print_elapsed_time(f: impl FnOnce() -> anyhow::Result<()>) -> anyhow::Result<()> {
    let start = Instant::now();
    f()?;
    let duration = start.elapsed();
    writeln!(io::stdout(), "Elapsed time: {}.", format_duration(duration))
        .context("failed to write to stdout")
}

fn synchronize(mut src_path: Cow<str>, dst_path: &Path) -> anyhow::Result<()> {
    if !src_path.as_ref().ends_with('/') {
        src_path.to_mut().push('/');
    }
    Command::new("rsync")
        .args(["-aAXHv", "--delete", "--stats", "--", src_path.as_ref()])
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
    fn demo_without_update() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── bar/
        // └── foo/
        //    └── colors/
        //       ├── dark/
        //       │  └── black
        //       └── red
        temp.child("bar").create_dir_all()?;
        temp.child("foo/colors/dark/black").write_str("ink")?;
        temp.child("foo/colors/red").write_str("blood")?;
        launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
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
        temp.child("bar/colors_2022-12-13-14h15").check_is_dir()?;
        temp.child("bar/colors_2022-12-13-14h15/dark").check_is_dir()?;
        temp.child("bar/colors_2022-12-13-14h15/dark/black").check_is_file_with_content("ink")?;
        temp.child("bar/colors_2022-12-13-14h15/red").check_is_file_with_content("blood")
    }

    #[test]
    fn demo_with_update() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
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
        temp.child("bar/colors_2022-08-09-10h11/green").write_str("grass")?;
        temp.child("bar/colors_2022-08-09-10h11/light/white").write_str("milk")?;
        temp.child("foo/colors/dark/black").write_str("ink")?;
        temp.child("foo/colors/red").write_str("blood")?;
        launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
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
        temp.child("bar/colors_2022-08-09-10h11").check_does_not_exist()?;
        temp.child("bar/colors_2022-12-13-14h15").check_is_dir()?;
        temp.child("bar/colors_2022-12-13-14h15/dark").check_is_dir()?;
        temp.child("bar/colors_2022-12-13-14h15/dark/black").check_is_file_with_content("ink")?;
        temp.child("bar/colors_2022-12-13-14h15/green").check_does_not_exist()?;
        temp.child("bar/colors_2022-12-13-14h15/light").check_does_not_exist()?;
        temp.child("bar/colors_2022-12-13-14h15/red").check_is_file_with_content("blood")
    }

    #[test]
    fn demo_with_symlinks() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
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
        temp.child("bar").symlink_to_dir("baz")?;
        temp.child("baz/colors_2022-08-09-10h11/green").write_str("grass")?;
        temp.child("baz/colors_2022-08-09-10h11/light/white").write_str("milk")?;
        temp.child("foo").create_dir_all()?;
        temp.child("foo/colors").symlink_to_dir("words")?;
        temp.child("foo/sea").write_str("massive")?;
        temp.child("foo/words").create_dir_all()?;
        temp.child("foo/words/blue").symlink_to_file("../sea")?;
        temp.child("foo/words/dark/black").write_str("ink")?;
        temp.child("foo/words/not_light").symlink_to_dir("dark")?;
        temp.child("foo/words/red").write_str("blood")?;
        launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
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
        temp.child("baz/colors_2022-08-09-10h11").check_does_not_exist()?;
        temp.child("baz/colors_2022-12-13-14h15").check_is_dir()?;
        temp.child("baz/colors_2022-12-13-14h15/blue").check_is_symlink_to("../sea")?;
        temp.child("baz/colors_2022-12-13-14h15/dark").check_is_dir()?;
        temp.child("bar/colors_2022-12-13-14h15/dark/black").check_is_file_with_content("ink")?;
        temp.child("baz/colors_2022-12-13-14h15/green").check_does_not_exist()?;
        temp.child("baz/colors_2022-12-13-14h15/light").check_does_not_exist()?;
        temp.child("baz/colors_2022-12-13-14h15/not_light").check_is_symlink_to("dark")?;
        temp.child("bar/colors_2022-12-13-14h15/red").check_is_file_with_content("blood")
    }

    #[test]
    fn symlinks_to_symlinks() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
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
        temp.child("bar").symlink_to_dir("bay")?;
        temp.child("bay").symlink_to_dir("baz")?;
        temp.child("baz/colors_2022-08-09-10h11").create_dir_all()?;
        temp.child("baz/colors_2022-08-09-10h11/light").symlink_to_file("../sun")?;
        temp.child("baz/colors_2022-08-09-10h11/not_dark").symlink_to_file("light")?;
        temp.child("baz/sun").write_str("star")?;
        temp.child("foo").create_dir_all()?;
        temp.child("foo/colors").symlink_to_dir("things")?;
        temp.child("foo/things").symlink_to_dir("words")?;
        temp.child("foo/words").create_dir_all()?;
        temp.child("foo/words/dark").symlink_to_file("non_existent_path")?;
        temp.child("foo/words/not_light").symlink_to_file("dark")?;
        launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
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
        temp.child("baz/colors_2022-08-09-10h11").check_does_not_exist()?;
        temp.child("baz/colors_2022-12-13-14h15").check_is_dir()?;
        temp.child("baz/colors_2022-12-13-14h15/dark").check_is_symlink_to("non_existent_path")?;
        temp.child("baz/colors_2022-12-13-14h15/light").check_does_not_exist()?;
        temp.child("baz/colors_2022-12-13-14h15/not_dark").check_does_not_exist()?;
        temp.child("baz/colors_2022-12-13-14h15/not_light").check_is_symlink_to("dark")
    }

    #[test]
    fn src_path_with_an_ending_slash() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── bar/
        // └── foo/
        //    └── colors/
        temp.child("bar").create_dir_all()?;
        temp.child("foo/colors").create_dir_all()?;
        launch_work(&temp, "foo/colors/", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── bar/
        // |  └── colors_2022-12-13-14h15/
        // └── foo/
        //    └── colors/
        temp.child("bar/colors_2022-12-13-14h15").check_is_dir()
    }

    #[test]
    fn final_dst_path_already_exists_and_is_a_directory() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── bar/
        // |  └── colors_2022-12-13-14h15/
        // └── foo/
        //    └── colors/
        //       └── red
        temp.child("bar/colors_2022-12-13-14h15").create_dir_all()?;
        temp.child("foo/colors/red").write_str("blood")?;
        launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── bar/
        // |  └── colors_2022-12-13-14h15/
        // |     └── red
        // └── foo/
        //    └── colors/
        //       └── red
        temp.child("bar/colors_2022-12-13-14h15/red").check_is_file_with_content("blood")
    }

    #[test]
    fn fancy_directory_names() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let now = datetime!(2022-12-13 14:15:16 UTC);
        for (src_path, dst_path) in [
            ("foo/colors.abc.xyz", "bar.abc.xyz"),
            ("foo/ ", " "),
            ("foo/c --o l o r s", "--b a r"),
            ("foo/co -- lors", "--"),
            ("foo/-", "-"),
        ] {
            [src_path, dst_path].iter().try_for_each(|p| temp.child(p).create_dir_all())?;
            launch_work(&temp, src_path, dst_path, now)?;
        }
        temp.child("bar.abc.xyz/colors.abc.xyz_2022-12-13-14h15").check_is_dir()?;
        temp.child(" / _2022-12-13-14h15").check_is_dir()?;
        temp.child("--b a r/c --o l o r s_2022-12-13-14h15").check_is_dir()?;
        temp.child("--/co -- lors_2022-12-13-14h15").check_is_dir()?;
        temp.child("-/-_2022-12-13-14h15").check_is_dir()
    }

    #[test]
    fn fail_if_two_valid_candidates() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // |  ├── colors_2022-08-09-10h11/
        // |  └── colors_2022-09-10-11h12/
        // └── foo/
        //    └── colors/
        let valid_candidates = ["bar/colors_2022-08-09-10h11", "bar/colors_2022-09-10-11h12"];
        valid_candidates.iter().try_for_each(|p| temp.child(p).create_dir_all())?;
        temp.child("foo/colors").create_dir_all()?;
        let result = launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "there are several candidates")?;
        valid_candidates.iter().try_for_each(|p| temp.child(p).check_is_dir())?;
        temp.child("bar/colors_2022-12-13-14h15").check_does_not_exist()
    }

    #[test]
    fn valid_and_invalid_candidates() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
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
        //       └── red
        let valid_candidate = "bar/colors_2022-08-09-10h11";
        temp.child(valid_candidate).create_dir_all()?;
        let invalid_directory_candidates = [
            "bar/colors2022-08-09-10h11",
            "bar/colors_222-08-09-10h11",
            "bar/colors_2022-08-09-10h11m12",
            "bar/colors_2022-08-bb-10h11",
            "bar/colors_2022-AA-09-10h11",
            "bar/some_colors_2022-08-09-10h11",
        ];
        invalid_directory_candidates.iter().try_for_each(|p| temp.child(p).create_dir_all())?;
        let file_candidate = "bar/colors_2022-09-10-11h12"; // file, so invalid
        temp.child(file_candidate).write_str("whatever")?;
        temp.child("foo/colors/red").write_str("blood")?;
        launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── bar/
        // |  ├── colors2022-08-09-10h11/
        // |  ├── colors_222-08-09-10h11/
        // |  ├── colors_2022-08-09-10h11m12/
        // |  ├── colors_2022-08-bb-10h11/
        // |  ├── colors_2022-09-10-11h12
        // |  ├── colors_2022-12-13-14h15/
        // |  |  └── red
        // |  ├── colors_2022-AA-09-10h11/
        // |  └── some_colors_2022-08-09-10h11/
        // └── foo/
        //    └── colors/
        //       └── red
        temp.child(valid_candidate).check_does_not_exist()?;
        temp.child(file_candidate).check_is_file_with_content("whatever")?;
        invalid_directory_candidates.iter().try_for_each(|p| temp.child(p).check_is_dir())?;
        temp.child("bar/colors_2022-12-13-14h15").check_is_dir()?;
        temp.child("bar/colors_2022-12-13-14h15/red").check_is_file_with_content("blood")
    }

    #[test]
    fn symlink_is_invalid_candidate() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── bar/
        // |  ├── baz/
        // |  ├── colors_2022-08-09-10h11/
        // |  └── colors_2022-09-10-11h12 -> baz
        // └── foo/
        //    └── colors/
        //       └── red
        temp.child("bar/baz").create_dir_all()?;
        temp.child("bar/colors_2022-08-09-10h11").create_dir_all()?;
        temp.child("bar/colors_2022-09-10-11h12").symlink_to_dir("baz")?;
        temp.child("foo/colors/red").write_str("blood")?;
        launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── bar/
        // |  ├── baz/
        // |  ├── colors_2022-09-10-11h12 -> baz
        // |  └── colors_2022-12-13-14h15/
        // |     └── red
        // └── foo/
        //    └── colors/
        //       └── red
        temp.child("bar/colors_2022-08-09-10h11").check_does_not_exist()?;
        temp.child("bar/colors_2022-09-10-11h12").check_is_symlink_to("baz")?;
        temp.child("bar/colors_2022-12-13-14h15").check_is_dir()?;
        temp.child("bar/colors_2022-12-13-14h15/red").check_is_file_with_content("blood")
    }

    #[test]
    fn fail_if_src_path_does_not_have_a_name() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // └── foo/
        //    └── colors/
        //       └── dark/
        temp.child("bar").create_dir_all()?;
        temp.child("foo/colors/dark").create_dir_all()?;
        let result =
            launch_work(&temp, "foo/colors/dark/..", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "does not have a name")?;
        temp.child("bar/colors_2022-12-13-14h15").check_does_not_exist()
    }

    #[test]
    fn fail_if_src_path_does_not_exist() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // └── foo/
        temp.child("bar").create_dir_all()?;
        temp.child("foo").create_dir_all()?;
        let result = launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "failed to read metadata")?;
        temp.child("bar/colors_2022-12-13-14h15").check_does_not_exist()
    }

    #[test]
    fn fail_if_src_path_is_a_file() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // └── foo/
        //    └── colors
        temp.child("bar").create_dir_all()?;
        temp.child("foo/colors").write_str("whatever")?;
        let result = launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "is not a directory")?;
        temp.child("bar/colors_2022-12-13-14h15").check_does_not_exist()
    }

    #[test]
    fn fail_if_src_path_is_a_symlink_to_a_file() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // └── foo/
        //    ├── colors -> words
        //    └── words
        temp.child("bar").create_dir_all()?;
        temp.child("foo").create_dir_all()?;
        temp.child("foo/colors").symlink_to_file("words")?;
        temp.child("foo/words").write_str("whatever")?;
        let result = launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "is not a directory")?;
        temp.child("bar/colors_2022-12-13-14h15").check_does_not_exist()
    }

    #[test]
    fn fail_if_src_path_is_a_broken_symlink() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // └── foo/
        //    ├── colors -> words
        //    └── words -> non_existent_path
        temp.child("bar").create_dir_all()?;
        temp.child("foo").create_dir_all()?;
        temp.child("foo/colors").symlink_to_file("words")?;
        temp.child("foo/words").symlink_to_file("non_existent_path")?;
        let result = launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "failed to read metadata")?;
        temp.child("bar/colors_2022-12-13-14h15").check_does_not_exist()
    }

    #[test]
    fn fail_if_dst_path_does_not_exist() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // └── foo/
        //    └── colors/
        temp.child("foo/colors").create_dir_all()?;
        let result = launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result.as_ref(), "failed to look for candidates")?;
        check_err_contains(result, "failed to read as a directory")?;
        temp.child("bar").check_does_not_exist()
    }

    #[test]
    fn fail_if_dst_path_is_a_file() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar
        // └── foo/
        //    └── colors/
        temp.child("bar").write_str("whatever")?;
        temp.child("foo/colors").create_dir_all()?;
        let result = launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result.as_ref(), "failed to look for candidates")?;
        check_err_contains(result, "failed to read as a directory")?;
        temp.child("bar").check_is_file_with_content("whatever")
    }

    #[test]
    fn fail_if_dst_path_is_a_symlink_to_a_file() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar -> baz
        // ├── baz
        // └── foo/
        //    └── colors/
        temp.child("bar").symlink_to_file("baz")?;
        temp.child("baz").write_str("whatever")?;
        temp.child("foo/colors").create_dir_all()?;
        let result = launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result.as_ref(), "failed to look for candidates")?;
        check_err_contains(result, "failed to read as a directory")?;
        temp.child("bar").check_is_symlink_to("baz")?;
        temp.child("baz").check_is_file_with_content("whatever")
    }

    #[test]
    fn fail_if_dst_path_is_a_broken_symlink() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar -> baz
        // ├── baz -> non_existent_path
        // └── foo/
        //    └── colors/
        temp.child("bar").symlink_to_file("baz")?;
        temp.child("baz").symlink_to_file("non_existent_path")?;
        temp.child("foo/colors").create_dir_all()?;
        let result = launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result.as_ref(), "failed to look for candidates")?;
        check_err_contains(result, "failed to read as a directory")?;
        temp.child("bar").check_is_symlink_to("baz")?;
        temp.child("baz").check_is_symlink_to("non_existent_path")
    }

    #[test]
    fn fail_if_final_dst_path_is_a_file() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // │  └── colors_2022-12-13-14h15
        // └── foo/
        //    └── colors/
        temp.child("bar/colors_2022-12-13-14h15").write_str("whatever")?;
        temp.child("foo/colors").create_dir_all()?;
        let result = launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "exists but is not a directory")?;
        temp.child("bar/colors_2022-12-13-14h15").check_is_file_with_content("whatever")
    }

    #[test]
    fn fail_if_final_dst_path_is_a_symlink() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // │  ├── baz/
        // │  └── colors_2022-12-13-14h15 -> baz
        // └── foo/
        //    └── colors/
        //       └── red
        temp.child("bar/baz").create_dir_all()?;
        temp.child("bar/colors_2022-12-13-14h15").symlink_to_dir("baz")?;
        temp.child("foo/colors/red").write_str("blood")?;
        let result = launch_work(&temp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        check_err_contains(result, "exists but is not a directory")?;
        temp.child("bar/colors_2022-12-13-14h15").check_is_symlink_to("baz")?;
        temp.child("bar/baz").check_is_dir()?;
        temp.child("bar/baz/red").check_does_not_exist()
    }

    fn launch_work(
        temp: &TempDir,
        src_path: &str,
        dst_path: &str,
        now: OffsetDateTime,
    ) -> anyhow::Result<()> {
        let src_dir_path = temp.child(src_path);
        let src_dir_path = src_dir_path.to_str().unwrap(); // hoping the path is an UTF-8 sequence
        let dst_dir_path = temp.child(dst_path);
        work(src_dir_path.into(), &dst_dir_path, now)
    }
}
