//! The data directory's lock inside one process. A lock belongs to the open file it was taken on, so a second
//! `Lock::acquire` in the same process meets the same refusal another process would. What needs a second process
//! (a holder killed outright) is in `lock_process.rs`.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use engine::{Holder, Lock, Refused};

/// The text of a lock file, and the process number and time it names as holder; none when it names no one.
type Case<'a> = (&'a [u8], Option<(u32, i64)>);

/// A new empty directory for one test.
fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wf-agents-lock-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Takes the directory's lock file with `text` in it and the lock held, as a process that has not finished starting
/// (or one that writes something else) would.
fn hold(dir: &Path, text: &[u8]) -> File {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(config::engine_lock(dir))
        .unwrap();
    file.try_lock().unwrap();
    file.write_all(text).unwrap();
    file
}

// A2. About 5 ms.
#[test]
fn a_second_lock_on_a_directory_is_refused_naming_the_holder_until_the_first_is_released() {
    let d = dir("second");
    let first = Lock::acquire(&d).unwrap();
    assert_eq!(first.dir(), d.as_path());
    let refused = Lock::acquire(&d).unwrap_err();
    match &refused {
        Refused::Held {
            path,
            holder: Some(Holder { pid, .. }),
        } => {
            assert_eq!(*pid, std::process::id());
            assert_eq!(*path, config::engine_lock(&d));
        }
        other => panic!("{other:?}"),
    }
    let said = refused.to_string();
    let expected = format!("engine already running: process {} holds ", std::process::id());
    assert!(said.starts_with(&expected), "{said}");
    drop(first);
    Lock::acquire(&d).unwrap();
    let _ = std::fs::remove_dir_all(&d);
}

// A4. About 5 ms.
#[test]
fn a_lock_file_with_an_old_holders_text_and_no_holder_is_taken_and_rewritten_whole() {
    let d = dir("stale");
    // Longer than what a holder writes, so a rewrite that does not cut the file leaves a tail.
    std::fs::write(config::engine_lock(&d), "999999 1\n".repeat(100)).unwrap();
    let _lock = Lock::acquire(&d).unwrap();
    let text = std::fs::read_to_string(config::engine_lock(&d)).unwrap();
    let mut lines = text.lines();
    let line = lines.next().unwrap();
    assert!(lines.next().is_none(), "{text:?}");
    assert!(text.ends_with('\n'), "{text:?}");
    let (pid, since) = line.split_once(' ').unwrap();
    assert_eq!(pid, std::process::id().to_string());
    assert!(since.parse::<i64>().unwrap() > 0, "{text:?}");
    let _ = std::fs::remove_dir_all(&d);
}

// A5. About 5 ms.
#[test]
fn a_lock_held_by_a_process_that_wrote_nothing_is_refused_without_naming_one() {
    let d = dir("silent");
    let _held = hold(&d, b"");
    let refused = Lock::acquire(&d).unwrap_err();
    assert!(matches!(refused, Refused::Held { holder: None, .. }), "{refused:?}");
    let said = refused.to_string();
    assert!(said.contains("has not recorded who it is"), "{said}");
    assert!(!said.contains("process "), "{said}");
    let _ = std::fs::remove_dir_all(&d);
}

// A5, for the text a holder could leave that is not exactly a process number and a time on one line: it names no one,
// and reading it never fails the refusal. About 20 ms for all the texts.
#[test]
fn text_in_a_held_lock_file_that_is_not_a_process_and_a_time_names_no_holder() {
    let long = format!("1 2\n{}", "x".repeat(1000));
    let texts: [Case<'_>; 11] = [
        (b"12 345\n", Some((12, 345))),
        (b"12 345", None),
        (b"12 345\n\n", None),
        (b"12 345 6\n", None),
        (b"12  345\n", None),
        (b"12\n", None),
        (b"-1 5\n", None),
        (b"a b\n", None),
        (&[0xff, 0xfe, b'\n'], None),
        (long.as_bytes(), None),
        (b"\n", None),
    ];
    for (i, (text, named)) in texts.iter().enumerate() {
        let d = dir(&format!("text{i}"));
        let _held = hold(&d, text);
        let refused = Lock::acquire(&d).unwrap_err();
        let Refused::Held { holder, .. } = &refused else {
            panic!("{text:?}: {refused:?}");
        };
        let got = holder.as_ref().map(|h| (h.pid, h.since));
        assert_eq!(got, *named, "{text:?}");
        let _ = std::fs::remove_dir_all(&d);
    }
}

// A6. About 5 ms.
#[test]
fn a_lock_file_that_cannot_be_opened_is_refused_with_the_system_error() {
    // The directory is not there.
    let missing = std::env::temp_dir().join(format!("wf-agents-lock-missing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&missing);
    let refused = Lock::acquire(&missing).unwrap_err();
    let Refused::Unusable { path, error } = &refused else {
        panic!("{refused:?}");
    };
    assert_eq!(*path, config::engine_lock(&missing));
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert!(refused.to_string().ends_with(&error.to_string()), "{refused}");
    // A directory where the file should be.
    let d = dir("isdir");
    std::fs::create_dir(config::engine_lock(&d)).unwrap();
    let refused = Lock::acquire(&d).unwrap_err();
    assert!(matches!(refused, Refused::Unusable { .. }), "{refused:?}");
    let _ = std::fs::remove_dir_all(&d);
}
