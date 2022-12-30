#![warn(clippy::nursery, clippy::pedantic)]

use anyhow::{bail, Context};
use clap::Parser;
use regex::Regex;
use std::borrow::Cow;
use std::fs::{self, DirEntry, Metadata};
use std::path::{Path, PathBuf};
use std::process::Command;
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
    let final_dst_path = get_final_dst_path(src_dir_name, dst_dir_path, now);
    maybe_rename_a_candidate_to_final_dst(src_dir_name, dst_dir_path, &final_dst_path)?;
    println!("Synchronize {src_dir_path:?} with {final_dst_path:?}.");
    synchronize(src_dir_path, &final_dst_path)
}

fn check_src_dir_is_ok(src_dir_path: &str) -> anyhow::Result<&str> {
    let src_dir_path: &Path = src_dir_path.as_ref();
    let src_dir_name = src_dir_path
        .file_name()
        .with_context(|| format!("{src_dir_path:?} does not have a name"))?
        .to_str()
        .unwrap(); // src_dir_path is a valid UTF-8 sequence.
    let src_dir_metadata = fs::metadata(src_dir_path)
        .with_context(|| format!("failed to read metadata from {src_dir_path:?}"))?;
    if !src_dir_metadata.is_dir() {
        bail!("{src_dir_path:?} is not a directory");
    }
    Ok(src_dir_name)
}

fn get_final_dst_path(src_dir_name: &str, dst_dir_path: &Path, now: OffsetDateTime) -> PathBuf {
    let format = format_description::parse("_[year]-[month]-[day]-[hour]h[minute]").unwrap();
    let suffix = now.format(&format).unwrap();
    let dst_dir_name = format!("{src_dir_name}{suffix}");
    dst_dir_path.join(dst_dir_name)
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
        println!("Renamed {candidate:?} to {final_dst_path:?}.");
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
        let metadata = entry
            .metadata()
            .with_context(|| format!("failed to read metadata from {entry:?}"))?;
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
    let status = Command::new("time")
        .args(["rsync", "-aAXHv", "--delete", "--stats", "--", src.as_ref()])
        .arg(dst)
        .status()
        .with_context(|| {
            format!("failed to synchronize {src:?} with {dst:?}: failed to execute process")
        })?;
    if !status.success() {
        bail!("failed to synchronize {src:?} with {dst:?}: {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::{tempdir, TempDir};
    use time::macros::datetime;

    #[test]
    fn demo_without_update() -> anyhow::Result<()> {
        let story = Story::new();
        // Before:
        // .
        // ├── bar
        // └── foo
        //    └── colors
        //       ├── dark
        //       │  └── black
        //       └── red
        story.create_dirs(["foo", "foo/colors", "foo/colors/dark", "bar"])?;
        story.create_files(["foo/colors/red", "foo/colors/dark/black"])?;
        story.launch_work("foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── bar
        // │  └── colors_2022-12-13-14h15
        // │     ├── dark
        // │     │  └── black
        // │     └── red
        // └── foo
        //    └── colors
        //       ├── dark
        //       │  └── black
        //       └── red
        story.check_the_following_dirs_exist_and_are_not_symlinks([
            "bar/colors_2022-12-13-14h15",
            "bar/colors_2022-12-13-14h15/dark",
        ])?;
        story.check_the_following_files_exist_and_are_not_symlinks([
            "bar/colors_2022-12-13-14h15/red",
            "bar/colors_2022-12-13-14h15/dark/black",
        ])
    }

    #[test]
    fn demo_with_update() -> anyhow::Result<()> {
        let story = Story::new();
        // Before:
        // .
        // ├── bar
        // │  └── colors_2022-08-09-10h11
        // │     ├── green
        // │     └── light
        // │        └── white
        // └── foo
        //    └── colors
        //       ├── dark
        //       │  └── black
        //       └── red
        story.create_dirs([
            "foo",
            "foo/colors",
            "foo/colors/dark",
            "bar",
            "bar/colors_2022-08-09-10h11",
            "bar/colors_2022-08-09-10h11/light",
        ])?;
        story.create_files([
            "foo/colors/red",
            "foo/colors/dark/black",
            "bar/colors_2022-08-09-10h11/green",
            "bar/colors_2022-08-09-10h11/light/white",
        ])?;
        story.launch_work("foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── bar
        // │  └── colors_2022-12-13-14h15
        // │     ├── dark
        // │     │  └── black
        // │     └── red
        // └── foo
        //    └── colors
        //       ├── dark
        //       │  └── black
        //       └── red
        story.check_the_following_dirs_exist_and_are_not_symlinks([
            "bar/colors_2022-12-13-14h15",
            "bar/colors_2022-12-13-14h15/dark",
        ])?;
        story.check_the_following_files_exist_and_are_not_symlinks([
            "bar/colors_2022-12-13-14h15/red",
            "bar/colors_2022-12-13-14h15/dark/black",
        ])?;
        story.check_the_following_paths_do_not_exist([
            "bar/colors_2022-08-09-10h11",
            "bar/colors_2022-12-13-14h15/light",
            "bar/colors_2022-12-13-14h15/green",
        ])
    }

    #[test]
    #[cfg(unix)]
    fn demo_with_symlinks() -> anyhow::Result<()> {
        let story = Story::new();
        // Before:
        // .
        // ├── bar
        // │  └── words_2022-08-09-10h11
        // │     ├── green
        // │     └── light
        // │        └── white
        // ├── dest -> bar
        // └── foo
        //    ├── colors
        //    │  ├── blue -> ../sea
        //    │  ├── dark
        //    │  │  └── black
        //    │  ├── not_light -> dark
        //    │  └── red
        //    ├── sea
        //    └── words -> colors
        story.create_dirs([
            "foo",
            "foo/colors",
            "foo/colors/dark",
            "bar",
            "bar/words_2022-08-09-10h11",
            "bar/words_2022-08-09-10h11/light",
        ])?;
        story.create_files([
            "foo/colors/red",
            "foo/colors/dark/black",
            "bar/words_2022-08-09-10h11/green",
            "bar/words_2022-08-09-10h11/light/white",
        ])?;
        story.create_symlinks([
            ("dest", "bar"),
            ("foo/words", "colors"),
            ("foo/colors/not_light", "dark"),
            ("foo/colors/blue", "../sea"),
        ])?;
        story.launch_work("foo/words", "dest", datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── bar
        // │  └── words_2022-12-13-14h15
        // │     ├── blue -> ../sea
        // │     ├── dark
        // │     │  └── black
        // │     ├── not_light -> dark
        // │     └── red
        // ├── dest -> bar
        // └── foo
        //    ├── colors
        //    │  ├── blue -> ../sea
        //    │  ├── dark
        //    │  │  └── black
        //    │  ├── not_light -> dark
        //    │  └── red
        //    ├── sea
        //    └── words -> colors
        //
        // Remark: `synchronize_backup` follows command-line symlinks only, so
        // "words_2022-12-13-14h15" is not a symlink, but the copies of "not_light" and "blue" are
        // symlinks. Note that "words_2022-12-13-14h15/blue" points to an unexisting path.
        story.check_the_following_dirs_exist_and_are_not_symlinks([
            "bar/words_2022-12-13-14h15",
            "bar/words_2022-12-13-14h15/dark",
        ])?;
        story.check_the_following_files_exist_and_are_not_symlinks([
            "bar/words_2022-12-13-14h15/red",
            "bar/words_2022-12-13-14h15/dark/black",
        ])?;
        story.check_the_following_symlinks_exist([
            "bar/words_2022-12-13-14h15/not_light",
            "bar/words_2022-12-13-14h15/blue",
        ])?;
        story.check_the_following_paths_do_not_exist([
            "bar/colors_2022-08-09-10h11",
            "bar/words_2022-12-13-14h15/light",
            "bar/words_2022-12-13-14h15/green",
        ])
    }

    #[test]
    fn src_path_with_an_ending_slash() -> anyhow::Result<()> {
        let story = Story::new();
        story.create_dirs(["foo", "foo/colors", "bar"])?;
        story.launch_work("foo/colors/", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        story.check_the_following_dirs_exist_and_are_not_symlinks(["bar/colors_2022-12-13-14h15"])
    }

    #[test]
    fn fancy_dir_names() -> anyhow::Result<()> {
        let story = Story::new();
        story.create_dirs([
            "foo",
            "foo/colors.abc.xyz",
            "bar.abc.xyz",
            "foo/ ",
            " ",
            "foo/c --o l o r s",
            "--b a r",
            "foo/co -- lors",
            "--",
            "foo/-",
            "-",
        ])?;
        let now = datetime!(2022-12-13 14:15:16 UTC);
        story.launch_work("foo/colors.abc.xyz", "bar.abc.xyz", now)?;
        story.launch_work("foo/ ", " ", now)?;
        story.launch_work("foo/c --o l o r s", "--b a r", now)?;
        story.launch_work("foo/co -- lors", "--", now)?;
        story.launch_work("foo/-", "-", now)?;
        story.check_the_following_dirs_exist_and_are_not_symlinks([
            "bar.abc.xyz/colors.abc.xyz_2022-12-13-14h15",
            " / _2022-12-13-14h15",
            "--b a r/c --o l o r s_2022-12-13-14h15",
            "--/co -- lors_2022-12-13-14h15",
            "-/-_2022-12-13-14h15",
        ])
    }

    #[test]
    fn fail_if_two_valid_candidates() -> anyhow::Result<()> {
        let story = Story::new();
        let valid_candidates = ["bar/colors_2022-08-09-10h11", "bar/colors_2022-09-10-11h12"];
        story.create_dirs(["foo", "foo/colors", "bar"])?;
        story.create_dirs(valid_candidates)?;
        let result = story.launch_work("foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        assert!(result.is_err());
        story.check_the_following_dirs_exist_and_are_not_symlinks(valid_candidates)?;
        story.check_the_following_paths_do_not_exist(["bar/colors_2022-12-13-14h15"])
    }

    #[test]
    fn valid_and_invalid_candidates() -> anyhow::Result<()> {
        let story = Story::new();
        let valid_candidate = ["bar/colors_2022-08-09-10h11"];
        let invalid_candidates = [
            "bar/some_colors_2022-08-09-10h11",
            "bar/colors2022-08-09-10h11",
            "bar/colors_222-08-09-10h11",
            "bar/colors_2022-AA-09-10h11",
            "bar/colors_2022-08-bb-10h11",
            "bar/colors_2022-08-09-10h11m12",
        ];
        let file_candidate = ["bar/colors_2022-09-10-11h12"]; // file, so invalid
        story.create_dirs(["foo", "foo/colors", "bar"])?;
        story.create_dirs(valid_candidate)?;
        story.create_dirs(invalid_candidates)?;
        story.create_files(file_candidate)?;
        story.launch_work("foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC))?;
        story.check_the_following_paths_do_not_exist(valid_candidate)?;
        story
            .check_the_following_dirs_exist_and_are_not_symlinks(["bar/colors_2022-12-13-14h15"])?;
        story.check_the_following_dirs_exist_and_are_not_symlinks(invalid_candidates)?;
        story.check_the_following_files_exist_and_are_not_symlinks(file_candidate)
    }

    #[test]
    fn fail_if_src_path_does_not_have_a_name() -> anyhow::Result<()> {
        let story = Story::new();
        story.create_dirs(["foo", "foo/colors", "foo/colors/dark", "bar"])?;
        let result = story.launch_work(
            "foo/colors/dark/..",
            "bar",
            datetime!(2022-12-13 14:15:16 UTC),
        );
        assert!(result.is_err());
        story.check_the_following_paths_do_not_exist(["bar/colors_2022-12-13-14h15"])
    }

    #[test]
    fn fail_if_src_path_does_not_exist() -> anyhow::Result<()> {
        let story = Story::new();
        story.create_dirs(["foo", "bar"])?;
        let result = story.launch_work("foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        assert!(result.is_err());
        story.check_the_following_paths_do_not_exist(["bar/colors_2022-12-13-14h15"])
    }

    #[test]
    fn fail_if_src_path_is_a_file() -> anyhow::Result<()> {
        let story = Story::new();
        story.create_dirs(["foo", "bar"])?;
        story.create_files(["foo/colors"])?;
        let result = story.launch_work("foo/colors", "bar", datetime!(2022-12-13 14:15:16 UTC));
        assert!(result.is_err());
        story.check_the_following_paths_do_not_exist(["bar/colors_2022-12-13-14h15"])
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

        fn create_dirs<const N: usize>(&self, paths: [&'static str; N]) -> anyhow::Result<()> {
            let tmp_dir_path = self.tmp_dir.path();
            for path in paths {
                let path = tmp_dir_path.join(path);
                fs::create_dir(&path)
                    .with_context(|| format!("failed to create directory {path:?}"))?;
                println!("Created directory {path:?}.");
            }
            Ok(())
        }

        fn create_files<const N: usize>(&self, paths: [&'static str; N]) -> anyhow::Result<()> {
            let tmp_dir_path = self.tmp_dir.path();
            for path in paths {
                let path = tmp_dir_path.join(path);
                File::create(&path).with_context(|| format!("failed to create file {path:?}"))?;
                println!("Created file {path:?}.");
            }
            Ok(())
        }

        #[cfg(unix)]
        fn create_symlinks<const N: usize>(
            &self,
            symlinks: [(&'static str, &'static str); N],
        ) -> anyhow::Result<()> {
            let tmp_dir_path = self.tmp_dir.path();
            for (from, to) in symlinks {
                let path = tmp_dir_path.join(from);
                std::os::unix::fs::symlink(to, &path)
                    .with_context(|| format!("failed to create symlink from {path:?} to {to:?}"))?;
                println!("Created symlink from {path:?} to {to:?}.");
            }
            Ok(())
        }

        fn launch_work(
            &self,
            src: &'static str,
            dst: &'static str,
            now: OffsetDateTime,
        ) -> anyhow::Result<()> {
            let tmp_dir_path = self.tmp_dir.path();
            let src_dir_path = tmp_dir_path.join(src);
            let dst_dir_path = tmp_dir_path.join(dst);
            work(src_dir_path.to_str().unwrap().into(), &dst_dir_path, now)
        }

        fn check_the_following_dirs_exist_and_are_not_symlinks<const N: usize>(
            &self,
            paths: [&'static str; N],
        ) -> anyhow::Result<()> {
            let tmp_dir_path = self.tmp_dir.path();
            for path in paths {
                let path = tmp_dir_path.join(path);
                let metadata = fs::symlink_metadata(&path)
                    .with_context(|| format!("failed to read metadata from {path:?}"))?;
                if !metadata.is_dir() {
                    bail!("{path:?} is not a directory")
                }
            }
            Ok(())
        }

        fn check_the_following_files_exist_and_are_not_symlinks<const N: usize>(
            &self,
            paths: [&'static str; N],
        ) -> anyhow::Result<()> {
            let tmp_dir_path = self.tmp_dir.path();
            for path in paths {
                let path = tmp_dir_path.join(path);
                let metadata = fs::symlink_metadata(&path)
                    .with_context(|| format!("failed to read metadata from {path:?}"))?;
                if !metadata.is_file() {
                    bail!("{path:?} is not a file")
                }
            }
            Ok(())
        }

        fn check_the_following_symlinks_exist<const N: usize>(
            &self,
            paths: [&'static str; N],
        ) -> anyhow::Result<()> {
            let tmp_dir_path = self.tmp_dir.path();
            for path in paths {
                let path = tmp_dir_path.join(path);
                let metadata = fs::symlink_metadata(&path)
                    .with_context(|| format!("failed to read metadata from {path:?}"))?;
                if !metadata.is_symlink() {
                    bail!("{path:?} is not a symlink")
                }
            }
            Ok(())
        }

        fn check_the_following_paths_do_not_exist<const N: usize>(
            &self,
            paths: [&'static str; N],
        ) -> anyhow::Result<()> {
            let tmp_dir_path = self.tmp_dir.path();
            for path in paths {
                let path = tmp_dir_path.join(path);
                if path.exists() {
                    bail!("{path:?} exists")
                }
            }
            Ok(())
        }
    }
}
