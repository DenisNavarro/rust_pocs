use anyhow::Context as _;
use clap::Parser;
use time::OffsetDateTime;

use common::{exists_async, get_now, get_size_async, rename_async};
use generic_renamer::{RenameTo, work};

use core::future::ready;

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
    // `get_now()` is called early because `OffsetDateTime::now_local()` cannot be called in a
    // multithread context: https://github.com/time-rs/time/issues/457
    let now = get_now()?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to build the Tokio runtime")?
        .block_on(main_impl(&file_path, now))
}

async fn main_impl(file_path: &str, now: OffsetDateTime) -> anyhow::Result<()> {
    let size = get_size_async(file_path).await?;
    match work(file_path, size, || ready(Ok(now)), async |path| exists_async(&path).await).await? {
        Some(RenameTo(dst_path)) => rename_async(file_path, &dst_path).await,
        None => Ok(()),
    }
}
