use clap::Parser;

use common::{exists, get_now, get_size, rename};
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
    let size = get_size(&file_path)?;
    let mut coroutine = work(&file_path, size);
    match loop {
        coroutine = match coroutine {
            Yield::WantsNow(coroutine) => {
                let now = get_now()?;
                coroutine.resume(now)
            }
            Yield::WantsExists(coroutine) => {
                let exists = exists(coroutine.get_arg())?;
                coroutine.resume(exists)
            }
            Yield::Return(action) => break action,
        }
    } {
        Some(RenameTo(dst_path)) => rename(&file_path, &dst_path),
        None => Ok(()),
    }
}
