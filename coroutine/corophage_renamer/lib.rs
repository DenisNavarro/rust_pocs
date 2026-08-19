#![no_std]
extern crate alloc;

use alloc::format;
use alloc::string::String;

use time::OffsetDateTime;
use time::macros::format_description;

use corophage::{CoSend, Effects, Program, Yielder, effect};

#[effect(OffsetDateTime)]
pub struct Now;

#[effect(bool)]
pub struct Exists(pub String);

type Effs = Effects![Now, Exists];

#[must_use]
pub struct RenameTo(pub String);

#[must_use]
pub fn work(file_path: &str, size: u64) -> CoSend<'_, Effs, Option<RenameTo>> {
    CoSend::new(move |mut yi: Yielder<'_, Effs>| async move {
        if size >= 42 {
            let dst_path = yi.invoke(Program::from_co(get_destination_path(file_path))).await;
            return Some(RenameTo(dst_path));
        }
        None
    })
}

#[must_use]
fn get_destination_path(file_path: &str) -> CoSend<'_, Effs, String> {
    CoSend::new(move |mut yi: Yielder<'_, Effs>| async move {
        let formatted_date = {
            let now = yi.yield_(Now).await;
            now.format(&format_description!("[year]-[month]-[day]")).unwrap()
        };
        let mut number = 1;
        loop {
            let candidate = format!("{file_path}.{formatted_date}.{number}");
            if !yi.yield_(Exists(candidate.clone())).await {
                break candidate;
            }
            number += 1;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{Exists, Now, RenameTo, work};

    use alloc::collections::BTreeMap;
    use alloc::string::String;

    use corophage::{Control, Program};
    use time::OffsetDateTime;
    use time::macros::datetime;

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
        let action = Program::from_co(work(file_path, size))
            .handle(|_: Now| Control::resume(now))
            .handle(|Exists(path)| Control::resume(files.contains_key(&path)))
            .run_sync()
            .unwrap(); // no Err(Cancelled)
        if let Some(RenameTo(dst_path)) = action {
            let file_size = files.remove(file_path).unwrap();
            files.insert(dst_path, file_size);
        }
    }
}
