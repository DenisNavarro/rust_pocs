use std::fmt::Display;
use std::fs;
use std::io::{self, Write as _};

use anyhow::Context as _;
use serde_json::json;
use time::OffsetDateTime;

pub fn get_size(file_path: &str) -> anyhow::Result<u64> {
    let metadata = fs::metadata(file_path)
        .with_context(|| format!("failed to read metadata from {}", quote(file_path)))?;
    Ok(metadata.len())
}

pub fn get_now() -> anyhow::Result<OffsetDateTime> {
    OffsetDateTime::now_local().context("failed to determine the local offset")
}

pub fn exists(path: &str) -> anyhow::Result<bool> {
    fs::exists(path).with_context(|| format!("failed to get the existence of {}", quote(path)))
}

pub fn rename(src_path: &str, dst_path: &str) -> anyhow::Result<()> {
    fs::rename(src_path, dst_path)
        .with_context(|| format!("failed to rename {} to {}", quote(src_path), quote(dst_path)))?;
    writeln!(io::stdout(), "Renamed {} to {}", quote(src_path), quote(dst_path))
        .context("failed to write to stdout")
}

#[must_use]
pub fn quote(string: &str) -> impl Display + '_ {
    // The Rust documentation says:
    //
    // > `Debug` implementations of types provided by the standard library (`std`, `core`, `alloc`,
    // > etc.) are not stable, and may also change with future Rust versions.
    //
    // This is why I use `format!("{}", quote(string))` instead of `format!("{string:?}")`.
    json!(string)
}
