use std::io::{self, Write as _};

use anyhow::Context as _;
use clap::Parser;
use time::OffsetDateTime;
use tokio::fs;

use common::{get_now, quote};
use renamer::{RenameTo, Yield, work};

#[derive(Parser)]
/// If the file has 42 bytes or more, move it by appending a suffix.
///
/// The suffix is `.YYYY-MM-DD.number` with `YYYY-MM-DD` the current date and
/// `number` the smallest positive integer such that the destination path does
/// not exist before the move.
struct Cli {
    /// UTF-8 file path
    file_path: String,
}

fn main() -> anyhow::Result<()> {
    let Cli { file_path } = Cli::parse();
    // `get_now()` fails when it is called just before `coroutine.resume(now)`.
    // The error is "The system's UTC offset could not be determined".
    // The issue may be: https://github.com/time-rs/time/issues/457
    let now = get_now()?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to build the Tokio runtime")?
        .block_on(main_impl(&file_path, now))
}

async fn main_impl(file_path: &str, now: OffsetDateTime) -> anyhow::Result<()> {
    let size = get_size(file_path).await?;
    let mut coroutine = work(file_path, size);
    match loop {
        coroutine = match coroutine {
            Yield::WantsNow(coroutine) => coroutine.resume(now),
            Yield::WantsExists(coroutine) => {
                let exists = exists(coroutine.get_arg()).await?;
                coroutine.resume(exists)
            }
            Yield::Return(action) => break action,
        }
    } {
        Some(RenameTo(dst_path)) => rename(file_path, &dst_path).await,
        None => Ok(()),
    }
}

async fn get_size(file_path: &str) -> anyhow::Result<u64> {
    let metadata = fs::metadata(file_path)
        .await
        .with_context(|| format!("failed to read metadata from {}", quote(file_path)))?;
    Ok(metadata.len())
}

async fn exists(path: &str) -> anyhow::Result<bool> {
    fs::try_exists(path)
        .await
        .with_context(|| format!("failed to get the existence of {}", quote(path)))
}

async fn rename(src_path: &str, dst_path: &str) -> anyhow::Result<()> {
    fs::rename(src_path, dst_path)
        .await
        .with_context(|| format!("failed to rename {} to {}", quote(src_path), quote(dst_path)))?;
    writeln!(io::stdout(), "Renamed {} to {}", quote(src_path), quote(dst_path))
        .context("failed to write to stdout")
}
