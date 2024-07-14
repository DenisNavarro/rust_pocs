use std::borrow::Cow;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{bail, Context};
use camino::Utf8Path;
use clap::Parser;
use humantime::format_duration;

#[allow(clippy::doc_markdown)]
#[derive(Parser)]
/// Synchronize parts of two directories. rsync is used to synchronize directory parts.
/// Tested on Linux.
///
/// For example, if `/aaa/bbb/foo` is a file and `/aaa/bbb/bar/baz` a directory, then
/// `synchronize_partially /aaa/bbb /xxx/yyy foo bar/baz` copies `/aaa/bbb/foo` to `/xxx/yyy/foo`
/// and calls `rsync -aHUXv --delete --stats -- /aaa/bbb/bar/baz/ /xxx/yyy/bar/baz`.
///
/// In this example, you can see that `synchronize_partially` works on joined command-line paths.
/// When a joined command-line path is a symlink, `synchronize_partially` follows it.
///
/// In the current implementation, only the second command-line argument (<DST_PREFIX_PATH>) can
/// be a non-UTF-8 sequence.
struct Cli {
    src_prefix_path: String,
    dst_prefix_path: PathBuf,
    subpaths: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let Cli { src_prefix_path, dst_prefix_path, subpaths } = Cli::parse();
    work(&src_prefix_path, &dst_prefix_path, &subpaths)
}

fn work(src_prefix_path: &str, dst_prefix_path: &Path, subpaths: &[String]) -> anyhow::Result<()> {
    subpaths.iter().try_for_each(|subpath| check_is_relative(Path::new(subpath)))?;
    [Path::new(src_prefix_path), dst_prefix_path].into_iter().try_for_each(check_is_directory)?;
    let actions: Vec<_> =
        check_all_synchronizations_seem_possible(src_prefix_path, dst_prefix_path, subpaths)?;
    actions.into_iter().try_for_each(|Action { src_path, dst_path, operation }| match operation {
        Operation::SynchronizeDir | Operation::RemoveDestFileAndCopyDir => {
            if operation == Operation::RemoveDestFileAndCopyDir {
                writeln!(io::stdout(), "---> Remove the file {dst_path:?}.")
                    .context("failed to write to stdout")?;
                remove_file(&dst_path)?;
            }
            writeln!(io::stdout(), "---> Synchronize {src_path:?} with {dst_path:?}.")
                .context("failed to write to stdout")?;
            execute_and_print_elapsed_time(|| synchronize_directory(src_path.into(), &dst_path))
        }
        Operation::CopyFile | Operation::RemoveDestDirAndCopyFile => {
            if operation == Operation::RemoveDestDirAndCopyFile {
                writeln!(io::stdout(), "---> Remove the diretory {dst_path:?}.")
                    .context("failed to write to stdout")?;
                remove_directory(&dst_path)?;
            }
            writeln!(io::stdout(), "---> Copy the file {src_path:?} to {dst_path:?}.")
                .context("failed to write to stdout")?;
            execute_and_print_elapsed_time(|| copy_file(&src_path, &dst_path))
        }
    })
}

fn check_is_relative(path: &Path) -> anyhow::Result<()> {
    path.is_relative().then_some(()).with_context(|| format!("{path:?} is absolute"))
}

fn check_is_directory(path: &Path) -> anyhow::Result<()> {
    let metadata =
        path.metadata().with_context(|| format!("failed to read metadata from {path:?}"))?;
    metadata.is_dir().then_some(()).with_context(|| format!("{path:?} is not a directory"))
}

fn check_all_synchronizations_seem_possible(
    src_prefix_path: &str,
    dst_prefix_path: &Path,
    subpaths: &[String],
) -> anyhow::Result<Vec<Action>> {
    subpaths
        .iter()
        .map(|subpath| {
            let src_path = Utf8Path::new(src_prefix_path).join(subpath).to_string();
            let src_metadata = fs::metadata(&src_path)
                .with_context(|| format!("failed to read metadata from {src_path:?}"))?;
            let dst_path = dst_prefix_path.join(subpath);
            let operation = check_dst_path_is_ok(src_metadata.is_dir(), &dst_path)?;
            Ok(Action { src_path, dst_path, operation })
        })
        .collect()
}

fn check_dst_path_is_ok(src_is_dir: bool, dst_path: &Path) -> anyhow::Result<Operation> {
    if src_is_dir {
        if let Ok(dst_metadata) = dst_path.symlink_metadata() {
            if dst_metadata.is_file() {
                return Ok(Operation::RemoveDestFileAndCopyDir);
            }
            if dst_metadata.is_symlink() {
                let metadata = fs::metadata(dst_path)
                    .with_context(|| format!("{dst_path:?} is a broken symlink"))?;
                if metadata.is_file() {
                    bail!("{dst_path:?} is a symlink whose final target is a file");
                }
            }
        }
        return Ok(Operation::SynchronizeDir);
    }
    if let Ok(dst_metadata) = dst_path.symlink_metadata() {
        if dst_metadata.is_dir() {
            return Ok(Operation::RemoveDestDirAndCopyFile);
        }
        if dst_metadata.is_symlink() {
            let metadata = fs::metadata(dst_path)
                .with_context(|| format!("{dst_path:?} is a broken symlink"))?;
            if metadata.is_dir() {
                bail!("{dst_path:?} is a symlink whose final target is a directory");
            }
        }
    }
    Ok(Operation::CopyFile)
}

fn execute_and_print_elapsed_time(f: impl FnOnce() -> anyhow::Result<()>) -> anyhow::Result<()> {
    let start = Instant::now();
    f()?;
    let duration = start.elapsed();
    writeln!(io::stdout(), "Elapsed time: {}.", format_duration(duration))
        .context("failed to write to stdout")
}

fn synchronize_directory(mut src_path: Cow<str>, dst_path: &Path) -> anyhow::Result<()> {
    if !src_path.as_ref().ends_with('/') {
        src_path.to_mut().push('/');
    }
    Command::new("rsync")
        .args(["-aHUXv", "--delete", "--stats", "--", src_path.as_ref()])
        .arg(dst_path)
        .status()
        .context("failed to execute process")
        .and_then(|status| {
            status.success().then_some(()).with_context(|| format!("error status: {status}"))
        })
        .with_context(|| format!("failed to synchronize {src_path:?} with {dst_path:?}"))
}

fn copy_file(src_path: &str, dst_path: &Path) -> anyhow::Result<()> {
    fs::copy(src_path, dst_path)
        .with_context(|| format!("failed to copy the file {src_path:?} to {dst_path:?}"))?;
    Ok(())
}

fn remove_directory(path: &Path) -> anyhow::Result<()> {
    fs::remove_dir_all(path).with_context(|| format!("failed to remove the diretory {path:?}"))
}

fn remove_file(path: &Path) -> anyhow::Result<()> {
    fs::remove_file(path).with_context(|| format!("failed to remove the file {path:?}"))
}

struct Action {
    src_path: String,
    dst_path: PathBuf,
    operation: Operation,
}

#[derive(PartialEq, Eq)]
enum Operation {
    SynchronizeDir,
    RemoveDestFileAndCopyDir,
    CopyFile,
    RemoveDestDirAndCopyFile,
}

#[cfg(test)]
mod tests {
    use super::*;

    use assert_fs::fixture::{FileWriteStr, PathChild, PathCreateDir, SymlinkToDir, SymlinkToFile};
    use assert_fs::TempDir;

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
        // │  └── sun
        // └── foo/
        //    ├── colors/
        //    │  ├── dark/
        //    │  │  └── black
        //    │  └── red
        //    ├── picture
        //    └── sea
        temp.child("bar/sun").write_str("star")?;
        temp.child("foo/colors/dark/black").write_str("ink")?;
        temp.child("foo/colors/red").write_str("blood")?;
        temp.child("foo/picture").write_str("photo")?;
        temp.child("foo/sea").write_str("massive")?;
        launch_work(&temp, "foo", "bar", ["colors", "picture"])?;
        // After:
        // .
        // ├── bar/
        // │  ├── colors/
        // │  │  ├── dark/
        // │  │  │  └── black
        // │  │  └── red
        // │  ├── picture
        // │  └── sun
        // └── foo/
        //    ├── colors/
        //    │  ├── dark/
        //    │  │  └── black
        //    │  └── red
        //    ├── picture
        //    └── sea
        temp.child("bar/colors").check_is_dir()?;
        temp.child("bar/colors/dark").check_is_dir()?;
        temp.child("bar/colors/dark/black").check_is_file_with_content("ink")?;
        temp.child("bar/colors/red").check_is_file_with_content("blood")?;
        temp.child("bar/picture").check_is_file_with_content("photo")?;
        temp.child("bar/sea").check_does_not_exist()?;
        temp.child("bar/sun").check_is_file_with_content("star")
    }

    #[test]
    fn demo_with_update() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── bar/
        // │  ├── colors/
        // │  │  ├── green
        // │  │  └── light/
        // │  │     └── white
        // │  ├── picture
        // │  └── sun
        // └── foo/
        //    ├── colors/
        //    │  ├── dark/
        //    │  │  └── black
        //    │  └── red
        //    ├── picture
        //    └── sea
        temp.child("bar/colors/green").write_str("grass")?;
        temp.child("bar/colors/light/white").write_str("milk")?;
        temp.child("bar/picture").write_str("old photo")?;
        temp.child("bar/sun").write_str("star")?;
        temp.child("foo/colors/dark/black").write_str("ink")?;
        temp.child("foo/colors/red").write_str("blood")?;
        temp.child("foo/picture").write_str("new photo")?;
        temp.child("foo/sea").write_str("massive")?;
        launch_work(&temp, "foo", "bar", ["colors", "picture"])?;
        // After:
        // .
        // ├── bar/
        // │  ├── colors/
        // │  │  ├── dark/
        // │  │  │  └── black
        // │  │  └── red
        // │  ├── picture
        // │  └── sun
        // └── foo/
        //    ├── colors/
        //    │  ├── dark/
        //    │  │  └── black
        //    │  └── red
        //    ├── picture
        //    └── sea
        temp.child("bar/colors").check_is_dir()?;
        temp.child("bar/colors/dark").check_is_dir()?;
        temp.child("bar/colors/dark/black").check_is_file_with_content("ink")?;
        temp.child("bar/colors/green").check_does_not_exist()?;
        temp.child("bar/colors/light").check_does_not_exist()?;
        temp.child("bar/colors/red").check_is_file_with_content("blood")?;
        temp.child("bar/picture").check_is_file_with_content("new photo")?;
        temp.child("bar/sea").check_does_not_exist()?;
        temp.child("bar/sun").check_is_file_with_content("star")
    }

    #[test]
    fn demo_with_symlinks() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── bar -> baz
        // ├── baz/
        // │  ├── colors -> images
        // │  ├── images/
        // │  │  ├── green
        // │  │  └── light/
        // │  │     └── white
        // │  ├── picture -> sun
        // │  └── sun
        // ├── foo -> fox
        // └── fox/
        //    ├── colors -> words
        //    ├── picture -> sea
        //    ├── sea
        //    └── words/
        //       ├── blue -> ../sea
        //       ├── dark/
        //       │  └── black
        //       ├── not_light -> dark
        //       └── red
        temp.child("bar").symlink_to_dir("baz")?;
        temp.child("baz").create_dir_all()?;
        temp.child("baz/colors").symlink_to_dir("images")?;
        temp.child("baz/images/green").write_str("grass")?;
        temp.child("baz/images/light/white").write_str("milk")?;
        temp.child("baz/picture").symlink_to_file("sun")?;
        temp.child("baz/sun").write_str("star")?;
        temp.child("foo").symlink_to_dir("fox")?;
        temp.child("fox").create_dir_all()?;
        temp.child("fox/colors").symlink_to_dir("words")?;
        temp.child("fox/picture").symlink_to_file("sea")?;
        temp.child("fox/sea").write_str("massive")?;
        temp.child("fox/words").create_dir_all()?;
        temp.child("fox/words/blue").symlink_to_file("../sea")?;
        temp.child("fox/words/dark/black").write_str("ink")?;
        temp.child("fox/words/not_light").symlink_to_dir("dark")?;
        temp.child("fox/words/red").write_str("blood")?;
        launch_work(&temp, "foo", "bar", ["colors", "picture"])?;
        // After:
        // .
        // ├── bar -> baz
        // ├── baz/
        // │  ├── colors -> images
        // │  ├── images/
        // │  │  ├── blue -> ../sea
        // │  │  ├── dark/
        // │  │  │  └── black
        // │  │  ├── not_light -> dark
        // │  │  └── red
        // │  ├── picture -> sun
        // │  └── sun
        // ├── foo -> fox
        // └── fox/
        //    ├── colors -> words
        //    ├── picture -> sea
        //    ├── sea
        //    └── words/
        //       ├── blue -> ../sea
        //       ├── dark/
        //       │  └── black
        //       ├── not_light -> dark
        //       └── red
        //
        // Note that "images/blue" points to an unexisting path.
        temp.child("baz/colors").check_is_symlink_to("images")?;
        temp.child("baz/images").check_is_dir()?;
        temp.child("baz/images/blue").check_is_symlink_to("../sea")?;
        temp.child("baz/images/dark").check_is_dir()?;
        temp.child("baz/images/dark/black").check_is_file_with_content("ink")?;
        temp.child("baz/images/green").check_does_not_exist()?;
        temp.child("baz/images/light").check_does_not_exist()?;
        temp.child("baz/images/not_light").check_is_symlink_to("dark")?;
        temp.child("baz/images/red").check_is_file_with_content("blood")?;
        temp.child("baz/picture").check_is_symlink_to("sun")?;
        temp.child("baz/sea").check_does_not_exist()?;
        temp.child("baz/sun").check_is_file_with_content("massive")?;
        temp.child("baz/words").check_does_not_exist()
    }

    #[test]
    fn symlinks_to_symlinks() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── bar -> bay
        // ├── bay -> baz
        // ├── baz/
        // │  ├── colors -> examples
        // │  ├── examples -> images
        // │  ├── images/
        // │  │  ├── light -> ../sun
        // │  │  └── not_dark -> light
        // │  ├── picture -> sky
        // │  ├── sky -> sun
        // │  └── sun
        // ├── foo -> for
        // ├── for -> fox
        // └── fox/
        //    ├── colors -> things
        //    ├── picture -> place
        //    ├── place -> sea
        //    ├── sea
        //    ├── things -> words
        //    └── words/
        //       ├── dark -> non_existent_path
        //       └── not_light -> dark
        temp.child("bar").symlink_to_dir("bay")?;
        temp.child("bay").symlink_to_dir("baz")?;
        temp.child("baz").create_dir_all()?;
        temp.child("baz/colors").symlink_to_dir("examples")?;
        temp.child("baz/examples").symlink_to_dir("images")?;
        temp.child("baz/images").create_dir_all()?;
        temp.child("baz/images/light").symlink_to_file("../sun")?;
        temp.child("baz/images/not_dark").symlink_to_file("light")?;
        temp.child("baz/picture").symlink_to_file("sky")?;
        temp.child("baz/sky").symlink_to_file("sun")?;
        temp.child("baz/sun").write_str("star")?;
        temp.child("foo").symlink_to_dir("for")?;
        temp.child("for").symlink_to_dir("fox")?;
        temp.child("fox").create_dir_all()?;
        temp.child("fox/colors").symlink_to_dir("things")?;
        temp.child("fox/picture").symlink_to_file("place")?;
        temp.child("fox/place").symlink_to_file("sea")?;
        temp.child("fox/sea").write_str("massive")?;
        temp.child("fox/things").symlink_to_dir("words")?;
        temp.child("fox/words").create_dir_all()?;
        temp.child("fox/words/dark").symlink_to_file("non_existent_path")?;
        temp.child("fox/words/not_light").symlink_to_file("dark")?;
        launch_work(&temp, "foo", "bar", ["colors", "picture"])?;
        // After:
        // .
        // ├── bar -> bay
        // ├── bay -> baz
        // ├── baz/
        // │  ├── colors -> examples
        // │  ├── examples -> images
        // │  ├── images/
        // │  │  ├── dark -> non_existent_path
        // │  │  └── not_light -> dark
        // │  ├── picture -> sky
        // │  ├── sky -> sun
        // │  └── sun
        // ├── foo -> for
        // ├── for -> fox
        // └── fox/
        //    ├── colors -> things
        //    ├── picture -> place
        //    ├── place -> sea
        //    ├── sea
        //    ├── things -> words
        //    └── words/
        //       ├── dark -> non_existent_path
        //       └── not_light -> dark
        temp.child("baz/colors").check_is_symlink_to("examples")?;
        temp.child("baz/examples").check_is_symlink_to("images")?;
        temp.child("baz/images").check_is_dir()?;
        temp.child("baz/images/dark").check_is_symlink_to("non_existent_path")?;
        temp.child("baz/images/light").check_does_not_exist()?;
        temp.child("baz/images/not_dark").check_does_not_exist()?;
        temp.child("baz/images/not_light").check_is_symlink_to("dark")?;
        temp.child("baz/picture").check_is_symlink_to("sky")?;
        temp.child("baz/place").check_does_not_exist()?;
        temp.child("baz/sea").check_does_not_exist()?;
        temp.child("baz/sky").check_is_symlink_to("sun")?;
        temp.child("baz/sun").check_is_file_with_content("massive")?;
        temp.child("baz/things").check_does_not_exist()?;
        temp.child("baz/words").check_does_not_exist()
    }

    #[test]
    fn replace_a_file_with_a_directory() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── bar/
        // |  └── colors
        // └── foo/
        //    └── colors/
        //       ├── dark/
        //       │  └── black
        //       └── red
        temp.child("bar/colors").write_str("whatever")?;
        temp.child("foo/colors/dark/black").write_str("ink")?;
        temp.child("foo/colors/red").write_str("blood")?;
        launch_work(&temp, "foo", "bar", ["colors"])?;
        // After:
        // .
        // ├── bar/
        // |  └── colors/
        // │     ├── dark/
        // │     │  └── black
        // │     └── red
        // └── foo/
        //    └── colors/
        //       ├── dark/
        //       │  └── black
        //       └── red
        temp.child("bar/colors").check_is_dir()?;
        temp.child("bar/colors/dark").check_is_dir()?;
        temp.child("bar/colors/dark/black").check_is_file_with_content("ink")?;
        temp.child("bar/colors/red").check_is_file_with_content("blood")
    }

    #[test]
    fn replace_a_directory_with_a_file() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── bar/
        // |  └── colors/
        // │     ├── dark/
        // │     │  └── black
        // │     └── red
        // └── foo/
        //    └── colors
        temp.child("bar/colors/dark/black").write_str("ink")?;
        temp.child("bar/colors/red").write_str("blood")?;
        temp.child("foo/colors").write_str("whatever")?;
        launch_work(&temp, "foo", "bar", ["colors"])?;
        // After:
        // .
        // ├── bar/
        // |  └── colors
        // └── foo/
        //    └── colors
        temp.child("bar/colors").check_is_file_with_content("whatever")
    }

    #[test]
    fn replace_a_file_with_a_directory_and_there_are_symlinks_to_symlinks() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── bar -> bay
        // ├── bay -> baz
        // ├── baz/
        // |  └── colors
        // ├── foo -> for
        // ├── for -> fox
        // └── fox/
        //    ├── colors -> things
        //    ├── things -> words
        //    └── words/
        //       ├── dark -> non_existent_path
        //       └── not_light -> dark
        temp.child("bar").symlink_to_dir("bay")?;
        temp.child("bay").symlink_to_dir("baz")?;
        temp.child("baz/colors").write_str("whatever")?;
        temp.child("foo").symlink_to_dir("for")?;
        temp.child("for").symlink_to_dir("fox")?;
        temp.child("fox").create_dir_all()?;
        temp.child("fox/colors").symlink_to_dir("things")?;
        temp.child("fox/things").symlink_to_dir("words")?;
        temp.child("fox/words").create_dir_all()?;
        temp.child("fox/words/dark").symlink_to_file("non_existent_path")?;
        temp.child("fox/words/not_light").symlink_to_file("dark")?;
        launch_work(&temp, "foo", "bar", ["colors"])?;
        // After:
        // .
        // ├── bar -> bay
        // ├── bay -> baz
        // ├── baz/
        // |  └── colors/
        // |     ├── dark -> non_existent_path
        // |     └── not_light -> dark
        // ├── foo -> for
        // ├── for -> fox
        // └── fox/
        //    ├── colors -> things
        //    ├── things -> words
        //    └── words/
        //       ├── dark -> non_existent_path
        //       └── not_light -> dark
        temp.child("baz/colors").check_is_dir()?;
        temp.child("baz/colors/dark").check_is_symlink_to("non_existent_path")?;
        temp.child("baz/colors/not_light").check_is_symlink_to("dark")
    }

    #[test]
    fn replace_a_directory_with_a_file_and_there_are_symlinks_to_symlinks() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── bar -> bay
        // ├── bay -> baz
        // ├── baz/
        // |  └── colors/
        // |     ├── dark -> non_existent_path
        // |     └── not_light -> dark
        // ├── foo -> for
        // ├── for -> fox
        // └── fox/
        //    ├── colors -> things
        //    ├── things -> words
        //    └── words
        temp.child("bar").symlink_to_dir("bay")?;
        temp.child("bay").symlink_to_dir("baz")?;
        temp.child("baz/colors").create_dir_all()?;
        temp.child("baz/colors/dark").symlink_to_file("non_existent_path")?;
        temp.child("baz/colors/not_light").symlink_to_file("dark")?;
        temp.child("foo").symlink_to_dir("for")?;
        temp.child("for").symlink_to_dir("fox")?;
        temp.child("fox").create_dir_all()?;
        temp.child("fox/colors").symlink_to_file("things")?;
        temp.child("fox/things").symlink_to_file("words")?;
        temp.child("fox/words").write_str("whatever")?;
        launch_work(&temp, "foo", "bar", ["colors"])?;
        // After:
        // .
        // ├── bar -> bay
        // ├── bay -> baz
        // ├── baz/
        // |  └── colors
        // ├── foo -> for
        // ├── for -> fox
        // └── fox/
        //    ├── colors -> things
        //    ├── things -> words
        //    └── words
        temp.child("baz/colors").check_is_file_with_content("whatever")
    }

    #[test]
    fn subpath_with_an_ending_slash() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── bar/
        // └── foo/
        //    └── colors/
        temp.child("bar").create_dir_all()?;
        temp.child("foo/colors").create_dir_all()?;
        launch_work(&temp, "foo", "bar", ["colors/"])?;
        // After:
        // .
        // ├── bar/
        // |  └── colors/
        // └── foo/
        //    └── colors/
        temp.child("bar/colors").check_is_dir()
    }

    #[test]
    fn no_subpath() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // |  └── images/
        // └── foo/
        //    └── colors/
        temp.child("bar/images").create_dir_all()?;
        temp.child("foo/colors").create_dir_all()?;
        launch_work(&temp, "foo", "bar", [])?;
        temp.child("bar/colors").check_does_not_exist()?;
        temp.child("bar/images").check_is_dir()
    }

    #[test]
    fn subpath_is_empty() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── bar/
        // |  └── images/
        // └── foo/
        //    └── colors/
        temp.child("bar/images").create_dir_all()?;
        temp.child("foo/colors").create_dir_all()?;
        launch_work(&temp, "foo", "bar", [""])?;
        // After:
        // .
        // ├── bar/
        // |  └── colors/
        // └── foo/
        //    └── colors/
        temp.child("bar/colors").check_is_dir()?;
        temp.child("bar/images").check_does_not_exist()
    }

    #[test]
    fn subpath_is_point() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── bar/
        // |  └── images/
        // └── foo/
        //    └── colors/
        temp.child("bar/images").create_dir_all()?;
        temp.child("foo/colors").create_dir_all()?;
        launch_work(&temp, "foo", "bar", ["."])?;
        // After:
        // .
        // ├── bar/
        // |  └── colors/
        // └── foo/
        //    └── colors/
        temp.child("bar/colors").check_is_dir()?;
        temp.child("bar/images").check_does_not_exist()
    }

    #[test]
    fn subpath_is_parent() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // Before:
        // .
        // ├── bar/
        // |  ├── images/
        // |  └── sun
        // └── foo/
        //    ├── colors/
        //    |  └── red
        //    └── sea
        temp.child("bar/images").create_dir_all()?;
        temp.child("bar/sun").write_str("star")?;
        temp.child("foo/colors/red").write_str("blood")?;
        temp.child("foo/sea").write_str("massive")?;
        launch_work(&temp, "foo/colors", "bar/images", [".."])?;
        // After:
        // .
        // ├── bar/
        // |  ├── colors/
        // |  |  └── red
        // |  └── sea
        // └── foo/
        //    ├── colors/
        //    |  └── red
        //    └── sea
        temp.child("bar/colors").check_is_dir()?;
        temp.child("bar/colors/red").check_is_file_with_content("blood")?;
        temp.child("bar/images").check_does_not_exist()?;
        temp.child("bar/sea").check_is_file_with_content("massive")?;
        temp.child("bar/sun").check_does_not_exist()
    }

    #[test]
    fn fail_if_src_prefix_path_does_not_exist() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        temp.child("bar").create_dir_all()?;
        let result = launch_work(&temp, "foo", "bar", []);
        check_err_contains(result, "failed to read metadata")
    }

    #[test]
    fn fail_if_dst_prefix_path_does_not_exist() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        temp.child("foo").create_dir_all()?;
        let result = launch_work(&temp, "foo", "bar", []);
        check_err_contains(result, "failed to read metadata")
    }

    #[test]
    fn fail_if_src_prefix_path_is_a_file() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // └── foo
        temp.child("bar").create_dir_all()?;
        temp.child("foo").write_str("whatever")?;
        let result = launch_work(&temp, "foo", "bar", []);
        check_err_contains(result, "is not a directory")
    }

    #[test]
    fn fail_if_dst_prefix_path_is_a_file() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar
        // └── foo/
        temp.child("bar").write_str("whatever")?;
        temp.child("foo").create_dir_all()?;
        let result = launch_work(&temp, "foo", "bar", []);
        check_err_contains(result, "is not a directory")
    }

    #[test]
    fn fail_if_src_prefix_path_is_a_symlink_to_a_file() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // ├── foo -> fox
        // └── fox
        temp.child("bar").create_dir_all()?;
        temp.child("foo").symlink_to_file("fox")?;
        temp.child("fox").write_str("whatever")?;
        let result = launch_work(&temp, "foo", "bar", []);
        check_err_contains(result, "is not a directory")
    }

    #[test]
    fn fail_if_dst_prefix_path_is_a_symlink_to_a_file() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar -> baz
        // ├── baz
        // └── foo/
        temp.child("bar").symlink_to_file("baz")?;
        temp.child("baz").write_str("whatever")?;
        temp.child("foo").create_dir_all()?;
        let result = launch_work(&temp, "foo", "bar", []);
        check_err_contains(result, "is not a directory")
    }

    #[test]
    fn fail_if_src_prefix_path_is_a_broken_symlink() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // ├── foo -> fox
        // └── fox -> non_existent_path
        temp.child("bar").create_dir_all()?;
        temp.child("foo").symlink_to_file("fox")?;
        temp.child("fox").symlink_to_file("non_existent_path")?;
        let result = launch_work(&temp, "foo", "bar", []);
        check_err_contains(result, "failed to read metadata")
    }

    #[test]
    fn fail_if_dst_prefix_path_is_a_broken_symlink() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar -> baz
        // ├── baz -> non_existent_path
        // └── foo/
        temp.child("bar").symlink_to_file("baz")?;
        temp.child("baz").symlink_to_file("non_existent_path")?;
        temp.child("foo").create_dir_all()?;
        let result = launch_work(&temp, "foo", "bar", []);
        check_err_contains(result, "failed to read metadata")
    }

    #[test]
    fn fail_if_subpath_is_absolute() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // └── foo/
        //    ├── colors/
        //    └── picture
        temp.child("bar").create_dir_all()?;
        temp.child("foo/colors").create_dir_all()?;
        temp.child("foo/picture").write_str("photo")?;
        let result = launch_work(&temp, "foo", "bar", ["colors", "/picture"]);
        check_err_contains(result, "is absolute")?;
        temp.child("bar/colors").check_does_not_exist()?;
        temp.child("bar/picture").check_does_not_exist()
    }

    #[test]
    fn fail_if_src_path_does_not_exist() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // └── foo/
        //    └── colors/
        temp.child("bar").create_dir_all()?;
        temp.child("foo/colors").create_dir_all()?;
        let result = launch_work(&temp, "foo", "bar", ["colors", "picture"]);
        check_err_contains(result, "failed to read metadata")?;
        temp.child("bar/colors").check_does_not_exist()?;
        temp.child("bar/picture").check_does_not_exist()
    }

    #[test]
    fn fail_if_src_path_is_a_broken_symlink() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // └── foo/
        //    ├── colors/
        //    ├── picture -> sea
        //    └── sea -> non_existent_path
        temp.child("bar").create_dir_all()?;
        temp.child("foo/colors").create_dir_all()?;
        temp.child("foo/picture").symlink_to_file("sea")?;
        temp.child("foo/sea").symlink_to_file("non_existent_path")?;
        let result = launch_work(&temp, "foo", "bar", ["colors", "picture"]);
        check_err_contains(result, "failed to read metadata")?;
        temp.child("bar/colors").check_does_not_exist()?;
        temp.child("bar/picture").check_does_not_exist()
    }

    #[test]
    fn fail_to_replace_a_symlink_to_a_file_with_a_directory() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // │  ├── picture -> sun
        // │  └── sun
        // └── foo/
        //    ├── colors/
        //    └── picture/
        temp.child("bar").create_dir_all()?;
        temp.child("bar/picture").symlink_to_file("sun")?;
        temp.child("bar/sun").write_str("star")?;
        temp.child("foo/colors").create_dir_all()?;
        temp.child("foo/picture").create_dir_all()?;
        let result = launch_work(&temp, "foo", "bar", ["colors", "picture"]);
        check_err_contains(result, "is a symlink whose final target is a file")?;
        temp.child("bar/colors").check_does_not_exist()?;
        temp.child("bar/picture").check_is_symlink_to("sun")?;
        temp.child("bar/sun").check_is_file_with_content("star")
    }

    #[test]
    fn fail_to_replace_a_symlink_to_a_directory_with_a_file() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // │  ├── picture -> sun
        // │  └── sun/
        // └── foo/
        //    ├── colors/
        //    └── picture
        temp.child("bar").create_dir_all()?;
        temp.child("bar/picture").symlink_to_dir("sun")?;
        temp.child("bar/sun").create_dir_all()?;
        temp.child("foo/colors").create_dir_all()?;
        temp.child("foo/picture").write_str("photo")?;
        let result = launch_work(&temp, "foo", "bar", ["colors", "picture"]);
        check_err_contains(result, "is a symlink whose final target is a directory")?;
        temp.child("bar/colors").check_does_not_exist()?;
        temp.child("bar/picture").check_is_symlink_to("sun")?;
        temp.child("bar/sun").check_is_dir()
    }

    #[test]
    fn fail_to_replace_a_symlink_to_a_symlink_to_a_file_with_a_directory() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // │  ├── picture -> sky
        // │  ├── sky -> sun
        // │  └── sun
        // └── foo/
        //    ├── colors/
        //    └── picture/
        temp.child("bar").create_dir_all()?;
        temp.child("bar/picture").symlink_to_file("sky")?;
        temp.child("bar/sky").symlink_to_file("sun")?;
        temp.child("bar/sun").write_str("star")?;
        temp.child("foo/colors").create_dir_all()?;
        temp.child("foo/picture").create_dir_all()?;
        let result = launch_work(&temp, "foo", "bar", ["colors", "picture"]);
        check_err_contains(result, "is a symlink whose final target is a file")?;
        temp.child("bar/colors").check_does_not_exist()?;
        temp.child("bar/picture").check_is_symlink_to("sky")?;
        temp.child("bar/sky").check_is_symlink_to("sun")?;
        temp.child("bar/sun").check_is_file_with_content("star")
    }

    #[test]
    fn fail_to_replace_a_symlink_to_a_symlink_to_a_directory_with_a_file() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // │  ├── picture -> sky
        // │  ├── sky -> sun
        // │  └── sun/
        // └── foo/
        //    ├── colors/
        //    └── picture
        temp.child("bar").create_dir_all()?;
        temp.child("bar/picture").symlink_to_dir("sky")?;
        temp.child("bar/sky").symlink_to_dir("sun")?;
        temp.child("bar/sun").create_dir_all()?;
        temp.child("foo/colors").create_dir_all()?;
        temp.child("foo/picture").write_str("photo")?;
        let result = launch_work(&temp, "foo", "bar", ["colors", "picture"]);
        check_err_contains(result, "is a symlink whose final target is a directory")?;
        temp.child("bar/colors").check_does_not_exist()?;
        temp.child("bar/picture").check_is_symlink_to("sky")?;
        temp.child("bar/sky").check_is_symlink_to("sun")?;
        temp.child("bar/sun").check_is_dir()
    }

    #[test]
    fn fail_to_replace_a_broken_symlink_with_a_directory() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // │  ├── picture -> sun
        // │  └── sun -> non_existent_path
        // └── foo/
        //    ├── colors/
        //    └── picture/
        temp.child("bar").create_dir_all()?;
        temp.child("bar/picture").symlink_to_file("sun")?;
        temp.child("bar/sun").symlink_to_file("non_existent_path")?;
        temp.child("foo/colors").create_dir_all()?;
        temp.child("foo/picture").create_dir_all()?;
        let result = launch_work(&temp, "foo", "bar", ["colors", "picture"]);
        check_err_contains(result, "is a broken symlink")?;
        temp.child("bar/colors").check_does_not_exist()?;
        temp.child("bar/picture").check_is_symlink_to("sun")?;
        temp.child("bar/sun").check_is_symlink_to("non_existent_path")
    }

    #[test]
    fn fail_to_replace_a_broken_symlink_with_a_file() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        // .
        // ├── bar/
        // │  ├── picture -> sun
        // │  └── sun -> non_existent_path
        // └── foo/
        //    ├── colors/
        //    └── picture
        temp.child("bar").create_dir_all()?;
        temp.child("bar/picture").symlink_to_file("sun")?;
        temp.child("bar/sun").symlink_to_file("non_existent_path")?;
        temp.child("foo/colors").create_dir_all()?;
        temp.child("foo/picture").write_str("photo")?;
        let result = launch_work(&temp, "foo", "bar", ["colors", "picture"]);
        check_err_contains(result, "is a broken symlink")?;
        temp.child("bar/colors").check_does_not_exist()?;
        temp.child("bar/picture").check_is_symlink_to("sun")?;
        temp.child("bar/sun").check_is_symlink_to("non_existent_path")
    }

    fn launch_work<const N: usize>(
        temp: &TempDir,
        src_path: &str,
        dst_path: &str,
        subpaths: [&str; N],
    ) -> anyhow::Result<()> {
        let src_prefix_path = temp.child(src_path);
        let src_prefix_path = src_prefix_path.to_str().unwrap(); // hoping it is an UTF-8 sequence
        let dst_prefix_path = temp.child(dst_path);
        let subpaths = subpaths.map(String::from);
        work(src_prefix_path, &dst_prefix_path, &subpaths)
    }
}
