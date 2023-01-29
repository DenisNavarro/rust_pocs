#![forbid(unsafe_code)]
#![warn(clippy::nursery, clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

//! Utility to write unit tests with a temporary directory

use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use tempfile::{tempdir, TempDir};

pub struct TemporaryDirectory {
    tmp_dir: TempDir,
}

impl Default for TemporaryDirectory {
    fn default() -> Self {
        Self::new()
    }
}

impl TemporaryDirectory {
    #[allow(clippy::missing_panics_doc)]
    #[must_use]
    pub fn new() -> Self {
        Self { tmp_dir: tempdir().unwrap() }
    }

    #[must_use]
    pub fn get_path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.tmp_dir.path().join(path)
    }

    pub fn create_dirs(
        &self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> anyhow::Result<()> {
        let tmp_dir_path = self.tmp_dir.path();
        for path in paths {
            let path = tmp_dir_path.join(path);
            fs::create_dir(&path)
                .with_context(|| format!("failed to create directory {path:?}"))?;
            println!("Created directory {path:?}.");
        }
        Ok(())
    }

    pub fn create_files(
        &self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> anyhow::Result<()> {
        let tmp_dir_path = self.tmp_dir.path();
        for path in paths {
            let path = tmp_dir_path.join(path);
            File::create(&path).with_context(|| format!("failed to create file {path:?}"))?;
            println!("Created file {path:?}.");
        }
        Ok(())
    }

    #[cfg(unix)]
    pub fn create_symlinks(
        &self,
        symlinks: impl IntoIterator<Item = (impl AsRef<Path>, impl AsRef<Path>)>,
    ) -> anyhow::Result<()> {
        let tmp_dir_path = self.tmp_dir.path();
        for (from, to) in symlinks {
            let from = tmp_dir_path.join(from);
            let to = to.as_ref();
            std::os::unix::fs::symlink(to, &from)
                .with_context(|| format!("failed to create symlink from {from:?} to {to:?}"))?;
            println!("Created symlink from {from:?} to {to:?}.");
        }
        Ok(())
    }

    pub fn check_the_following_dirs_exist_and_are_not_symlinks(
        &self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
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

    pub fn check_the_following_files_exist_and_are_not_symlinks(
        &self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
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

    pub fn check_the_following_symlinks_exist(
        &self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
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

    pub fn check_the_following_paths_do_not_exist(
        &self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> anyhow::Result<()> {
        let tmp_dir_path = self.tmp_dir.path();
        for path in paths {
            let path = tmp_dir_path.join(path);
            if path.symlink_metadata().is_ok() {
                bail!("{path:?} exists")
            }
        }
        Ok(())
    }
}
