#![no_std]
extern crate alloc;

use alloc::format;
use alloc::string::String;

use time::macros::format_description;
use time::OffsetDateTime;

#[must_use]
pub enum Yield<'a> {
    WantsNow(WantsNow<'a>),
    WantsExists(WantsExists<'a>),
    Return(Option<RenameTo>),
}

#[must_use]
pub struct WantsNow<'a> {
    file_path: &'a str,
}

#[must_use]
pub struct WantsExists<'a> {
    file_path: &'a str,
    formatted_date: String,
    number: usize,
    candidate: String,
}

#[must_use]
pub struct RenameTo(pub String);

pub const fn work(file_path: &str, size: u64) -> Yield {
    if size >= 42 {
        return Yield::WantsNow(WantsNow { file_path });
    }
    Yield::Return(None)
}

impl<'a> WantsNow<'a> {
    pub fn resume(self, now: OffsetDateTime) -> Yield<'a> {
        let file_path = self.file_path;
        let formatted_date = now.format(&format_description!("[year]-[month]-[day]")).unwrap();
        let number = 1;
        let candidate = get_candidate(file_path, &formatted_date, number);
        Yield::WantsExists(WantsExists { file_path, formatted_date, number, candidate })
    }
}

impl<'a> WantsExists<'a> {
    #[must_use]
    pub fn get_arg(&self) -> &str {
        &self.candidate
    }
    pub fn resume(self, exists: bool) -> Yield<'a> {
        if exists {
            let number = self.number + 1;
            let candidate = get_candidate(self.file_path, &self.formatted_date, number);
            Yield::WantsExists(WantsExists { number, candidate, ..self })
        } else {
            Yield::Return(Some(RenameTo(self.candidate)))
        }
    }
}

fn get_candidate(file_path: &str, formatted_date: &str, number: usize) -> String {
    format!("{file_path}.{formatted_date}.{number}")
}

#[cfg(test)]
mod tests {
    use super::{work, RenameTo, Yield};

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
        launch_work(&mut files, "app.log", datetime!(2011-12-13 14:15:16 UTC));
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
        launch_work(&mut files, "app.log", datetime!(2011-12-13 14:15:16 UTC));
        assert_eq!(files, BTreeMap::from([("app.log.2011-12-13.1".into(), Size(42))]));
    }

    #[test]
    fn noop_because_the_file_is_small() {
        let mut files = BTreeMap::from([("app.log".into(), Size(41))]);
        launch_work(&mut files, "app.log", datetime!(2011-12-13 14:15:16 UTC));
        assert_eq!(files, BTreeMap::from([("app.log".into(), Size(41))]));
    }

    fn launch_work(files: &mut BTreeMap<String, Size>, file_path: &str, now: OffsetDateTime) {
        let size = files[file_path].0;
        let mut coroutine = work(file_path, size);
        let action = loop {
            coroutine = match coroutine {
                Yield::WantsNow(coroutine) => coroutine.resume(now),
                Yield::WantsExists(coroutine) => {
                    let exists = files.contains_key(coroutine.get_arg());
                    coroutine.resume(exists)
                }
                Yield::Return(action) => break action,
            };
        };
        if let Some(RenameTo(dst_path)) = action {
            let file_size = files.remove(file_path).unwrap();
            files.insert(dst_path, file_size);
        }
    }
}
