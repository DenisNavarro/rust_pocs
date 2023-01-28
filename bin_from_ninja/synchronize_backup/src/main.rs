#![warn(clippy::nursery, clippy::pedantic)]

use std::borrow::Cow;
use std::fs::{self, DirEntry, Metadata};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context};
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
    let src_dir_name = check_src_dir_is_ok(src_dir_path.as_ref())?;
    let final_dst_path = get_final_dst_path(src_dir_name, dst_dir_path.to_owned(), now);
    maybe_rename_a_candidate_to_final_dst(src_dir_name, dst_dir_path, &final_dst_path)?;
    writeln!(io::stdout(), "Synchronize {src_dir_path:?} with {final_dst_path:?}.")
        .context("failed to write to stdout")?;
    synchronize(src_dir_path, &final_dst_path)
}

fn check_src_dir_is_ok(src_dir_path: &str) -> anyhow::Result<&str> {
    let src_dir_name = Utf8Path::new(src_dir_path)
        .file_name()
        .ok_or_else(|| anyhow!("{src_dir_path:?} does not have a name"))?;
    let src_dir_metadata = fs::metadata(src_dir_path)
        .with_context(|| format!("failed to read metadata from {src_dir_path:?}"))?;
    if !src_dir_metadata.is_dir() {
        bail!("{src_dir_path:?} is not a directory");
    }
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

fn maybe_rename_a_candidate_to_final_dst(
    src_dir_name: &str,
    dst_dir_path: &Path,
    final_dst_path: &Path,
) -> anyhow::Result<()> {
    let candidates =
        get_candidates(src_dir_name, dst_dir_path).context("failed to look for candidates")?;
    if candidates.len() >= 2 {
        bail!("there are several candidates: {candidates:?}");
    }
    if let Some(candidate) = candidates.get(0) {
        fs::rename(candidate, final_dst_path)
            .with_context(|| format!("failed to renamed {candidate:?} to {final_dst_path:?}"))?;
        writeln!(io::stdout(), "Renamed {candidate:?} to {final_dst_path:?}.")
            .context("failed to write to stdout")?;
    }
    Ok(())
}

fn get_candidates(src_dir_name: &str, dst_dir_path: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let re = Regex::new(
        r"^(.*)_[[:digit:]]{4}-[[:digit:]]{2}-[[:digit:]]{2}-[[:digit:]]{2}h[[:digit:]]{2}$",
    )
    .unwrap();
    let entries_and_errors = fs::read_dir(dst_dir_path)
        .with_context(|| format!("failed to read {dst_dir_path:?} as a directory"))?;
    let mut result = Vec::<PathBuf>::new();
    for entry_or_err in entries_and_errors {
        let entry =
            entry_or_err.with_context(|| format!("failed to read an entry in {dst_dir_path:?}"))?;
        let metadata =
            entry.metadata().with_context(|| format!("failed to read metadata from {entry:?}"))?;
        if is_candidate(&entry, &metadata, src_dir_name, &re) {
            result.push(entry.path());
        }
    }
    Ok(result)
}

fn is_candidate(entry: &DirEntry, metadata: &Metadata, src_dir_name: &str, re: &Regex) -> bool {
    if !metadata.is_dir() {
        return false;
    };
    let dir_name = entry.file_name();
    let Some(dir_name) = dir_name.to_str() else {
        return false;
    };
    let Some(capture) = re.captures(dir_name) else {
        return false;
    };
    &capture[1] == src_dir_name
}

fn synchronize(mut src: Cow<str>, dst: &Path) -> anyhow::Result<()> {
    if !src.as_ref().ends_with('/') {
        src.to_mut().push('/');
    }
    Command::new("time")
        .args(["rsync", "-aAXHv", "--delete", "--stats", "--", src.as_ref()])
        .arg(dst)
        .status()
        .context("failed to execute process")
        .and_then(|status| {
            status.success().then_some(()).ok_or_else(|| anyhow!("error status: {status}"))
        })
        .with_context(|| format!("failed to synchronize {src:?} with {dst:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    use time::macros::datetime;

    use temporary_directory::TemporaryDirectory;

    // TODO: remove duplication between code and comments.
    // The future code will probably write and check the directory content with YAML. Example:
    // directory_name:
    //   subdirectory_name:
    //     file_name: "file content"
    //   symlink_name: ["path/to/target"]

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
        tmp.check_the_following_dirs_exist_and_are_not_symlinks([
            "bar/colors_2022-12-13-14h15",
            "bar/colors_2022-12-13-14h15/dark",
        ])?;
        tmp.check_the_following_files_exist_and_are_not_symlinks([
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
        tmp.check_the_following_dirs_exist_and_are_not_symlinks([
            "bar/colors_2022-12-13-14h15",
            "bar/colors_2022-12-13-14h15/dark",
        ])?;
        tmp.check_the_following_files_exist_and_are_not_symlinks([
            "bar/colors_2022-12-13-14h15/dark/black",
            "bar/colors_2022-12-13-14h15/red",
        ])?;
        tmp.check_the_following_paths_do_not_exist([
            "bar/colors_2022-08-09-10h11",
            "bar/colors_2022-12-13-14h15/light",
            "bar/colors_2022-12-13-14h15/green",
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
        // "colors_2022-12-13-14h15" is not a symlink, but the copies of "not_light" and "blue"
        // are symlinks. Note that "colors_2022-12-13-14h15/blue" points to an unexisting path.
        tmp.check_the_following_dirs_exist_and_are_not_symlinks([
            "baz/colors_2022-12-13-14h15",
            "baz/colors_2022-12-13-14h15/dark",
        ])?;
        tmp.check_the_following_files_exist_and_are_not_symlinks([
            "baz/colors_2022-12-13-14h15/dark/black",
            "baz/colors_2022-12-13-14h15/red",
        ])?;
        tmp.check_the_following_symlinks_exist([
            "baz/colors_2022-12-13-14h15/blue",
            "baz/colors_2022-12-13-14h15/not_light",
        ])?;
        tmp.check_the_following_paths_do_not_exist([
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
        // │  └── colors_2022-08-09-10h11/
        // │     └── green
        // └── foo/
        //    ├── colors -> things
        //    ├── things -> words
        //    └── words/
        //       └── not_light -> non_existent_path
        tmp.create_dirs(["baz", "baz/colors_2022-08-09-10h11", "foo", "foo/words"])?;
        tmp.create_files(["baz/colors_2022-08-09-10h11/green"])?;
        tmp.create_symlinks([
            ("bar", "bay"),
            ("bay", "baz"),
            ("foo/colors", "things"),
            ("foo/things", "words"),
            ("foo/words/not_light", "non_existent_path"),
        ])?;
        launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── bar -> bay
        // ├── bay -> baz
        // ├── baz/
        // │  └── colors_2022-12-13-14h15/
        // │     └── not_light -> non_existent_path
        // └── foo/
        //    ├── colors -> things
        //    ├── things -> words
        //    └── words/
        //       └── not_light -> non_existent_path
        //
        // Remark: `synchronize_backup` follows command-line symlinks only, so
        // "colors_2022-12-13-14h15" is not a symlink, but the copy of "not_light" is a symlink.
        tmp.check_the_following_dirs_exist_and_are_not_symlinks(["baz/colors_2022-12-13-14h15"])?;
        tmp.check_the_following_symlinks_exist(["baz/colors_2022-12-13-14h15/not_light"])?;
        tmp.check_the_following_paths_do_not_exist([
            "baz/colors_2022-08-09-10h11",
            "baz/colors_2022-12-13-14h15/green",
        ])
    }

    #[test]
    fn src_path_with_an_ending_slash() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        tmp.create_dirs(["foo", "foo/colors", "bar"])?;
        launch_work(&tmp, "foo/colors/", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        tmp.check_the_following_dirs_exist_and_are_not_symlinks(["bar/colors_2022-12-13-14h15"])
    }

    #[test]
    fn fancy_dir_names() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        let now = datetime!(2022-12-13 14:15:16 UTC);
        tmp.create_dirs(["foo"])?;
        for (src, dst) in [
            ("foo/colors.abc.xyz", "bar.abc.xyz"),
            ("foo/ ", " "),
            ("foo/c --o l o r s", "--b a r"),
            ("foo/co -- lors", "--"),
            ("foo/-", "-"),
        ] {
            tmp.create_dirs([src, dst])?;
            launch_work(&tmp, src, dst, now)?;
        }
        tmp.check_the_following_dirs_exist_and_are_not_symlinks([
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
        let valid_candidates = ["bar/colors_2022-08-09-10h11", "bar/colors_2022-09-10-11h12"];
        tmp.create_dirs(["foo", "foo/colors", "bar"])?;
        tmp.create_dirs(&valid_candidates)?;
        let result = launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        assert!(result.is_err());
        tmp.check_the_following_dirs_exist_and_are_not_symlinks(&valid_candidates)?;
        tmp.check_the_following_paths_do_not_exist(["bar/colors_2022-12-13-14h15"])
    }

    #[test]
    fn valid_and_invalid_candidates() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        let valid_candidate = ["bar/colors_2022-08-09-10h11"];
        let invalid_dir_candidates = [
            "bar/some_colors_2022-08-09-10h11",
            "bar/colors2022-08-09-10h11",
            "bar/colors_222-08-09-10h11",
            "bar/colors_2022-AA-09-10h11",
            "bar/colors_2022-08-bb-10h11",
            "bar/colors_2022-08-09-10h11m12",
        ];
        let file_candidate = ["bar/colors_2022-09-10-11h12"]; // file, so invalid
        tmp.create_dirs(["foo", "foo/colors", "bar"])?;
        tmp.create_dirs(&valid_candidate)?;
        tmp.create_dirs(&invalid_dir_candidates)?;
        tmp.create_files(&file_candidate)?;
        launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        tmp.check_the_following_paths_do_not_exist(&valid_candidate)?;
        tmp.check_the_following_files_exist_and_are_not_symlinks(&file_candidate)?;
        tmp.check_the_following_dirs_exist_and_are_not_symlinks(&invalid_dir_candidates)?;
        tmp.check_the_following_dirs_exist_and_are_not_symlinks(["bar/colors_2022-12-13-14h15"])
    }

    #[test]
    #[cfg(unix)]
    fn symlink_is_invalid_candidate() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        tmp.create_dirs(["foo", "foo/colors", "bar", "bar/colors_2022-08-09-10h11", "bar/baz"])?;
        tmp.create_symlinks([("bar/colors_2022-09-10-11h12", "baz")])?;
        launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        tmp.check_the_following_paths_do_not_exist(["bar/colors_2022-08-09-10h11"])?;
        tmp.check_the_following_symlinks_exist(["bar/colors_2022-09-10-11h12"])?;
        tmp.check_the_following_dirs_exist_and_are_not_symlinks(["bar/colors_2022-12-13-14h15"])
    }

    #[test]
    fn fail_if_src_path_does_not_have_a_name() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        tmp.create_dirs(["foo", "foo/colors", "foo/colors/dark", "bar"])?;
        let result =
            launch_work(&tmp, "foo/colors/dark/..", "bar", datetime!(2022-12-13 14:15:16 UTC));
        assert!(result.is_err());
        tmp.check_the_following_paths_do_not_exist(["bar/colors_2022-12-13-14h15"])
    }

    #[test]
    fn fail_if_src_path_does_not_exist() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        tmp.create_dirs(["foo", "bar"])?;
        let result = launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        assert!(result.is_err());
        tmp.check_the_following_paths_do_not_exist(["bar/colors_2022-12-13-14h15"])
    }

    #[test]
    fn fail_if_src_path_is_a_file() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        tmp.create_dirs(["foo", "bar"])?;
        tmp.create_files(["foo/colors"])?;
        let result = launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        assert!(result.is_err());
        tmp.check_the_following_paths_do_not_exist(["bar/colors_2022-12-13-14h15"])
    }

    #[test]
    #[cfg(unix)]
    fn fail_if_src_path_is_a_broken_symlink() -> anyhow::Result<()> {
        let tmp = TemporaryDirectory::new();
        // .
        // ├── bar/
        // │  └── colors_2022-08-09-10h11/
        // └── foo/
        //    ├── colors -> words
        //    └── words -> non_existent_path
        tmp.create_dirs(["bar", "bar/colors_2022-08-09-10h11", "foo"])?;
        tmp.create_symlinks([("foo/colors", "words"), ("foo/words", "non_existent_path")])?;
        let result = launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        assert!(result.is_err());
        tmp.check_the_following_dirs_exist_and_are_not_symlinks(["bar/colors_2022-08-09-10h11"])?;
        tmp.check_the_following_paths_do_not_exist(["bar/colors_2022-12-13-14h15"])
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
        //       └── red
        tmp.create_dirs(["foo", "foo/colors"])?;
        tmp.create_files(["foo/colors/red"])?;
        tmp.create_symlinks([("bar", "baz"), ("baz", "non_existent_path")])?;
        let result = launch_work(&tmp, "foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        assert!(result.is_err());
        tmp.check_the_following_symlinks_exist(["bar", "baz"])
    }

    fn launch_work(
        tmp: &TemporaryDirectory,
        src: &str,
        dst: &str,
        now: OffsetDateTime,
    ) -> anyhow::Result<()> {
        let src_dir_path = tmp.get_path(src);
        let src_dir_path = src_dir_path.to_str().unwrap(); // hoping the path is an UTF-8 sequence
        let dst_dir_path = tmp.get_path(dst);
        work(src_dir_path.into(), &dst_dir_path, now)
    }
}
