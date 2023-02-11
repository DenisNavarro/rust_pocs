#![forbid(unsafe_code)]
#![warn(clippy::nursery, clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

//! Utility to write unit tests with a temporary directory

use std::fs::{self, File, Metadata};
use std::path::{Path, PathBuf};

use anyhow::Context;
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
        paths.into_iter().try_for_each(|path| self.create_dir(path))
    }

    pub fn create_dir(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = self.get_path(path);
        fs::create_dir(&path).with_context(|| format!("failed to create directory {path:?}"))?;
        println!("Created directory {path:?}.");
        Ok(())
    }

    pub fn create_files(
        &self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> anyhow::Result<()> {
        paths.into_iter().try_for_each(|path| self.create_file(path))
    }

    pub fn create_file(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = self.get_path(path);
        File::create(&path).with_context(|| format!("failed to create file {path:?}"))?;
        println!("Created file {path:?}.");
        Ok(())
    }

    #[cfg(unix)]
    pub fn create_symlinks(
        &self,
        symlinks: impl IntoIterator<Item = (impl AsRef<Path>, impl AsRef<Path>)>,
    ) -> anyhow::Result<()> {
        symlinks.into_iter().try_for_each(|(from, to)| self.create_symlink(from, to))
    }

    #[cfg(unix)]
    pub fn create_symlink(
        &self,
        from: impl AsRef<Path>,
        to: impl AsRef<Path>,
    ) -> anyhow::Result<()> {
        let from = self.get_path(from);
        let to = to.as_ref();
        std::os::unix::fs::symlink(to, &from)
            .with_context(|| format!("failed to create symlink from {from:?} to {to:?}"))?;
        println!("Created symlink from {from:?} to {to:?}.");
        Ok(())
    }

    pub fn check_dirs_exist_and_are_not_symlinks(
        &self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> anyhow::Result<()> {
        paths.into_iter().try_for_each(|path| self.check_dir_exists_and_is_not_a_symlink(path))
    }

    pub fn check_dir_exists_and_is_not_a_symlink(
        &self,
        path: impl AsRef<Path>,
    ) -> anyhow::Result<()> {
        let path = self.get_path(path);
        let metadata = symlink_metadata(&path)?;
        metadata.is_dir().then_some(()).with_context(|| format!("{path:?} is not a directory"))
    }

    pub fn check_files_exist_and_are_not_symlinks(
        &self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> anyhow::Result<()> {
        paths.into_iter().try_for_each(|path| self.check_file_exists_and_is_not_a_symlink(path))
    }

    pub fn check_file_exists_and_is_not_a_symlink(
        &self,
        path: impl AsRef<Path>,
    ) -> anyhow::Result<()> {
        let path = self.get_path(path);
        let metadata = symlink_metadata(&path)?;
        metadata.is_file().then_some(()).with_context(|| format!("{path:?} is not a file"))
    }

    pub fn check_symlinks_exist(
        &self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> anyhow::Result<()> {
        paths.into_iter().try_for_each(|path| self.check_symlink_exists(path))
    }

    pub fn check_symlink_exists(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = self.get_path(path);
        let metadata = symlink_metadata(&path)?;
        metadata.is_symlink().then_some(()).with_context(|| format!("{path:?} is not a symlink"))
    }

    pub fn check_do_not_exist(
        &self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> anyhow::Result<()> {
        paths.into_iter().try_for_each(|path| self.check_does_not_exist(path))
    }

    pub fn check_does_not_exist(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = self.get_path(path);
        path.symlink_metadata().is_err().then_some(()).with_context(|| format!("{path:?} exists"))
    }
}

fn symlink_metadata(path: &Path) -> anyhow::Result<Metadata> {
    path.symlink_metadata().with_context(|| format!("failed to read metadata from {path:?}"))
}
