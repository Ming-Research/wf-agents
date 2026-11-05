//! The lock that keeps one engine on a data directory, taken in one process. What a process that dies or starts a child
//! does to it is in `lock_process.rs`, and the command refusing a second engine is in the command's tests.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use engine::{Holder, Lock, Refused};

/// A new data directory for one test, with the `db/` directory that `wf-agents init` makes.
fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wf-agents-lock-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("db")).unwrap();
    d
}

// A second lock on a data directory is refused while the first is held, and says which process holds it; once the first
// is dropped the lock is taken again.
#[test]
fn a_second_lock_on_a_directory_is_refused_naming_the_holder_until_the_first_is_dropped() {
    let d = dir("twice");
    let first = Lock::acquire(&d).unwrap();
    assert_eq!(first.dir(), d);
    match Lock::acquire(&d) {
        Err(refused @ Refused::Held { .. }) => {
            let Refused::Held { path, holder } = &refused else {
                unreachable!()
            };
            assert_eq!(path, &config::engine_lock(&d));
            let held = holder.as_ref().expect("the file says who holds it");
            assert_eq!(held.pid, std::process::id());
            assert!(
                refused.to_string().contains(&format!("process {}", std::process::id())),
                "{refused}"
            );
        }
        other => panic!("{other:?}"),
    }
    drop(first);
    Lock::acquire(&d).unwrap();
    let _ = std::fs::remove_dir_all(&d);
}

// Text left in the lock file by an engine that is gone is no lock: with no process holding it, the lock is taken and
// the file says who holds it now.
#[test]
fn a_lock_file_with_an_old_holders_text_and_no_process_holding_it_is_free_and_rewritten() {
    let d = dir("stale");
    let path = config::engine_lock(&d);
    std::fs::write(
        &path,
        "999999 1000\nlonger than what replaces it, so the old text must be cut off\n",
    )
    .unwrap();
    let lock = Lock::acquire(&d).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let mut words = text.strip_suffix('\n').expect("one line").split(' ');
    assert_eq!(words.next().unwrap(), std::process::id().to_string());
    let since: i64 = words.next().unwrap().parse().unwrap();
    assert!((db::now() - since).abs() < 60_000, "{text:?}");
    assert!(words.next().is_none(), "{text:?}");
    drop(lock);
    let _ = std::fs::remove_dir_all(&d);
}

// A process that holds the lock and has not written who it is yet, or whose text is no process and time, is refused as
// held by someone not recorded, not as any process number.
#[test]
fn a_lock_held_by_a_process_that_has_recorded_nothing_names_no_one() {
    for (name, text) in [("empty", ""), ("garbage", "not a holder\n"), ("no-time", "42\n")] {
        let d = dir(name);
        let path = config::engine_lock(&d);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .unwrap();
        file.write_all(text.as_bytes()).unwrap();
        file.try_lock().unwrap();
        match Lock::acquire(&d) {
            Err(refused @ Refused::Held { .. }) => {
                let Refused::Held { holder, .. } = &refused else {
                    unreachable!()
                };
                assert_eq!(holder, &None::<Holder>, "{name}");
                let said = refused.to_string();
                assert!(said.contains("has not recorded who it is"), "{name}: {said}");
                assert!(!said.contains("process "), "{name}: {said}");
            }
            other => panic!("{name}: {other:?}"),
        }
        drop(file);
        let _ = std::fs::remove_dir_all(&d);
    }
}

// A lock file that cannot be opened or locked, and is not held by anyone, fails with what the system said, and takes no
// lock: an engine is never run without one.
#[test]
fn a_lock_file_that_cannot_be_opened_fails_with_the_systems_error() {
    // The file is a directory.
    let d = dir("directory");
    let path = config::engine_lock(&d);
    std::fs::create_dir(&path).unwrap();
    let refused = Lock::acquire(&d).unwrap_err();
    let Refused::Unusable { path: at, error } = &refused else {
        panic!("{refused:?}")
    };
    assert_eq!(at, &path);
    let said = refused.to_string();
    assert!(said.starts_with(&path.display().to_string()), "{said}");
    assert!(said.ends_with(&error.to_string()), "{said}");
    let _ = std::fs::remove_dir_all(&d);

    // The directory the file lives in is not there.
    let d = dir("no-db-directory");
    std::fs::remove_dir(d.join("db")).unwrap();
    let refused = Lock::acquire(&d).unwrap_err();
    let Refused::Unusable { error, .. } = &refused else {
        panic!("{refused:?}")
    };
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound, "{refused}");
    assert!(!config::engine_lock(&d).exists());
    let _ = std::fs::remove_dir_all(&d);
}
