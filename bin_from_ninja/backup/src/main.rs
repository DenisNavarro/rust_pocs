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
    if dst_path.exists() {
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
        // TODO: Make the code cross-plateform.
        let status = Command::new("cp")
            .args(["-rH", "--"])
            .args([src, dst])
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
    use std::fs::File;
    use tempfile::{tempdir, TempDir};
    use time::macros::datetime;

    #[test]
    fn simple_demo() -> anyhow::Result<()> {
        let story = Story::new();
        // Before:
        // .
        // ├── colors
        // │  ├── dark
        // │  │  └── black
        // │  └── red
        // ├── sea
        story.create_dirs(["colors", "colors/dark"])?;
        story.create_files(["colors/red", "colors/dark/black", "sea"])?;
        story.launch_work_on_paths(["colors", "sea"], datetime!(2022-12-13 14:15:16 UTC))?;
        // After:
        // .
        // ├── colors
        // │  ├── dark
        // │  │  └── black
        // │  └── red
        // ├── colors_2022-12-13-14h15
        // │  ├── dark
        // │  │  └── black
        // │  └── red
        // ├── sea
        // ├── sea_2022-12-13-14h15
        story.check_the_following_dirs_exist_and_are_not_symlinks([
            "colors_2022-12-13-14h15",
            "colors_2022-12-13-14h15/dark",
        ])?;
        story.check_the_following_files_exist_and_are_not_symlinks([
            "colors_2022-12-13-14h15/red",
            "colors_2022-12-13-14h15/dark/black",
            "sea_2022-12-13-14h15",
        ])
    }

    #[test]
    #[cfg(unix)]
    fn demo_with_symlinks() -> anyhow::Result<()> {
        let story = Story::new();
        // Before:
        // .
        // ├── colors
        // │  ├── blue -> ../sea
        // │  ├── dark
        // │  │  └── black
        // │  ├── not_light -> dark
        // │  └── red
        // ├── picture -> sea
        // ├── sea
        // └── words -> colors
        story.create_dirs(["colors", "colors/dark"])?;
        story.create_files(["colors/red", "colors/dark/black", "sea"])?;
        story.create_symlinks([
            ("words", "colors"),
            ("colors/not_light", "dark"),
            ("colors/blue", "../sea"),
            ("picture", "sea"),
        ])?;
        story.launch_work_on_paths(
            ["colors", "words", "sea", "picture"],
            datetime!(2022-12-13 14:15:16 UTC),
        )?;
        // After:
        // .
        // ├── colors
        // │  ├── blue -> ../sea
        // │  ├── dark
        // │  │  └── black
        // │  ├── not_light -> dark
        // │  └── red
        // ├── colors_2022-12-13-14h15
        // │  ├── blue -> ../sea
        // │  ├── dark
        // │  │  └── black
        // │  ├── not_light -> dark
        // │  └── red
        // ├── picture -> sea
        // ├── picture_2022-12-13-14h15
        // ├── sea
        // ├── sea_2022-12-13-14h15
        // ├── words -> colors
        // └── words_2022-12-13-14h15
        //    ├── blue -> ../sea
        //    ├── dark
        //    │  └── black
        //    ├── not_light -> dark
        //    └── red
        //
        // Remark: `backup` follows command-line symlinks only, so "words_2022-12-13-14h15" and
        // "picture_2022-12-13-14h15" are not symlinks, but the copies of "not_light" and "blue"
        // are symlinks.
        story.check_the_following_dirs_exist_and_are_not_symlinks([
            "colors_2022-12-13-14h15",
            "colors_2022-12-13-14h15/dark",
            "words_2022-12-13-14h15",
            "words_2022-12-13-14h15/dark",
        ])?;
        story.check_the_following_files_exist_and_are_not_symlinks([
            "colors_2022-12-13-14h15/red",
            "colors_2022-12-13-14h15/dark/black",
            "words_2022-12-13-14h15/red",
            "words_2022-12-13-14h15/dark/black",
            "sea_2022-12-13-14h15",
            "picture_2022-12-13-14h15",
        ])?;
        story.check_the_following_symlinks_exist([
            "colors_2022-12-13-14h15/not_light",
            "colors_2022-12-13-14h15/blue",
            "words_2022-12-13-14h15/not_light",
            "words_2022-12-13-14h15/blue",
        ])
    }

    #[test]
    fn fancy_dir_names() -> anyhow::Result<()> {
        let story = Story::new();
        story.create_dirs(["foo.abc.xyz", " ", "--b a r", "--", "-"])?;
        story.launch_work_on_paths(
            ["foo.abc.xyz", " ", "--b a r", "--", "-"],
            datetime!(2022-12-13 14:15:16 UTC),
        )?;
        story.check_the_following_dirs_exist_and_are_not_symlinks([
            "foo.abc.xyz_2022-12-13-14h15",
            " _2022-12-13-14h15",
            "--b a r_2022-12-13-14h15",
            "--_2022-12-13-14h15",
            "-_2022-12-13-14h15",
        ])
    }

    #[test]
    fn fancy_file_names() -> anyhow::Result<()> {
        let story = Story::new();
        story.create_files(["foo.abc.xyz", " ", "--b a r", "--", "-"])?;
        story.launch_work_on_paths(
            ["foo.abc.xyz", " ", "--b a r", "--", "-"],
            datetime!(2022-12-13 14:15:16 UTC),
        )?;
        story.check_the_following_files_exist_and_are_not_symlinks([
            "foo.abc.xyz_2022-12-13-14h15",
            " _2022-12-13-14h15",
            "--b a r_2022-12-13-14h15",
            "--_2022-12-13-14h15",
            "-_2022-12-13-14h15",
        ])
    }

    #[test]
    fn fail_if_src_path_does_not_have_a_name() -> anyhow::Result<()> {
        let story = Story::new();
        story.create_dirs(["foo"])?;
        let result = story.launch_work_on_paths(["foo", ".."], datetime!(2022-12-13 14:15:16 UTC));
        assert!(result.is_err());
        story.check_the_following_paths_do_not_exist(["foo_2022-12-13-14h15"])
    }

    #[test]
    fn fail_if_src_path_does_not_exist() -> anyhow::Result<()> {
        let story = Story::new();
        story.create_dirs(["foo"])?;
        let result = story.launch_work_on_paths(["foo", "bar"], datetime!(2022-12-13 14:15:16 UTC));
        assert!(result.is_err());
        story.check_the_following_paths_do_not_exist(["foo_2022-12-13-14h15"])
    }

    #[test]
    fn fail_if_dir_dst_path_already_exists() -> anyhow::Result<()> {
        let story = Story::new();
        story.create_dirs(["foo", "bar", "bar_2022-12-13-14h15"])?;
        let result = story.launch_work_on_paths(["foo", "bar"], datetime!(2022-12-13 14:15:16 UTC));
        assert!(result.is_err());
        story.check_the_following_paths_do_not_exist(["foo_2022-12-13-14h15"])
    }

    #[test]
    fn fail_if_file_dst_path_already_exists() -> anyhow::Result<()> {
        let story = Story::new();
        story.create_files(["foo", "bar", "bar_2022-12-13-14h15"])?;
        let result = story.launch_work_on_paths(["foo", "bar"], datetime!(2022-12-13 14:15:16 UTC));
        assert!(result.is_err());
        story.check_the_following_paths_do_not_exist(["foo_2022-12-13-14h15"])
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

        fn launch_work_on_paths<const N: usize>(
            &self,
            arg_paths: [&'static str; N],
            now: OffsetDateTime,
        ) -> anyhow::Result<()> {
            let tmp_dir_path = self.tmp_dir.path();
            let src_paths = arg_paths.iter().map(|path| tmp_dir_path.join(path));
            work(src_paths, now)
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
