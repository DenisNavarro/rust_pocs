#![no_std]
extern crate alloc;

use alloc::format;
use alloc::string::String;

use time::macros::format_description;
use time::OffsetDateTime;

#[must_use]
pub struct RenameTo(pub String);

pub fn work<E>(
    file_path: &str,
    size: u64,
    get_now: impl FnOnce() -> Result<OffsetDateTime, E>,
    exists: impl FnMut(&str) -> Result<bool, E>,
) -> Result<Option<RenameTo>, E> {
    if size >= 42 {
        let dst_path = get_destination_path(file_path, get_now, exists)?;
        return Ok(Some(RenameTo(dst_path)));
    }
    Ok(None)
}

fn get_destination_path<E>(
    file_path: &str,
    get_now: impl FnOnce() -> Result<OffsetDateTime, E>,
    mut exists: impl FnMut(&str) -> Result<bool, E>,
) -> Result<String, E> {
    let formatted_date = {
        let now = get_now()?;
        now.format(&format_description!("[year]-[month]-[day]")).unwrap()
    };
    let mut number = 1;
    loop {
        let candidate = format!("{file_path}.{formatted_date}.{number}");
        if !exists(&candidate)? {
            break Ok(candidate);
        }
        number += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::{work, RenameTo};

    use alloc::collections::BTreeMap;
    use alloc::string::String;

    use time::macros::datetime;
    use time::OffsetDateTime;

    #[derive(Debug, PartialEq, Eq)]
    struct Size(u64);

    #[test]
    fn demo() {
        let mut files = BTreeMap::from([
            ("app.log".into(), Size(300)),
            ("app.log.2011-12-13.1".into(), Size(100)),
            ("app.log.2011-12-13.2".into(), Size(200)),
        ]);
        launch_work(&mut files, "app.log", datetime!(2011-12-13 14:15:16 UTC)).unwrap();
        assert_eq!(
            files,
            BTreeMap::from([
                ("app.log.2011-12-13.1".into(), Size(100)),
                ("app.log.2011-12-13.2".into(), Size(200)),
                ("app.log.2011-12-13.3".into(), Size(300)),
            ])
        );
    }

    #[test]
    fn first_backup_of_the_day() {
        let mut files = BTreeMap::from([("app.log".into(), Size(42))]);
        launch_work(&mut files, "app.log", datetime!(2011-12-13 14:15:16 UTC)).unwrap();
        assert_eq!(files, BTreeMap::from([("app.log.2011-12-13.1".into(), Size(42))]));
    }

    #[test]
    fn noop_because_the_file_is_small() {
        let mut files = BTreeMap::from([("app.log".into(), Size(41))]);
        launch_work(&mut files, "app.log", datetime!(2011-12-13 14:15:16 UTC)).unwrap();
        assert_eq!(files, BTreeMap::from([("app.log".into(), Size(41))]));
    }

    fn launch_work(
        files: &mut BTreeMap<String, Size>,
        file_path: &str,
        now: OffsetDateTime,
    ) -> Result<(), &'static str> {
        let size = files[file_path].0;
        let get_now = || Ok(now);
        let exists = |path: &str| Ok(files.contains_key(path));
        if let Some(RenameTo(dst_path)) = work(file_path, size, get_now, exists)? {
            let file_size = files.remove(file_path).unwrap();
            files.insert(dst_path, file_size);
        }
        Ok(())
    }
}
