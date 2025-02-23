use clap::Parser;

use common::{exists, get_now, get_size, rename};
use generic_renamer::{RenameTo, work};

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
    match work(&file_path, size, get_now, exists)? {
        Some(RenameTo(dst_path)) => rename(&file_path, &dst_path),
        None => Ok(()),
    }
}
