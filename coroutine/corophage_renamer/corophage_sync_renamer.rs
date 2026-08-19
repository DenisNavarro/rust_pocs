use clap::Parser;
use corophage::{Cancelled, Control, Program};

use common::{exists, get_now, get_size, rename};
use corophage_renamer::{Exists, Now, RenameTo, work};

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
    let mut error: Option<anyhow::Error> = None;
    let result = Program::from_co(work(&file_path, size))
        .handle(|error: &mut Option<anyhow::Error>, _: Now| match get_now() {
            Ok(now) => Control::resume(now),
            Err(err) => {
                *error = Some(err);
                Control::Cancel
            }
        })
        .handle(|error: &mut Option<anyhow::Error>, Exists(path)| match exists(&path) {
            Ok(path_exists) => Control::resume(path_exists),
            Err(err) => {
                *error = Some(err);
                Control::Cancel
            }
        })
        .run_sync_stateful(&mut error);
    match result {
        Ok(Some(RenameTo(dst_path))) => rename(&file_path, &dst_path),
        Ok(None) => Ok(()),
        Err(Cancelled) => Err(error.unwrap()),
    }
}
