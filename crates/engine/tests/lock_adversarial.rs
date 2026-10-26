//! The data directory's lock under attack: the ways a lock can be held in name only, or refuse in a way that hides
//! why. The plain take, refusal and release are in `lock.rs`, and what needs a second process is in
//! `lock_process.rs`; these look for what those assume.
//!
//! What a kernel lock does is flock(2)'s: the lock belongs to the open file, not to the path it was opened by, so it
//! outlives the file's name and is gone only when the last descriptor of that file is closed. The tests do not rely on
//! the code's own earlier output: a second taker either gets in or does not, which the lock's purpose (one engine on
//! a data directory) decides.
//!
//! No test in this file starts a process. The tests run as threads of one process, and a child that is forked while
//! one of them holds a lock keeps a copy of the open file, and with it the lock, until it runs its program: a lock a
//! test has dropped would still look held. The test that needs git is in `lock_held_by_engine.rs`.

use std::fs::{File, OpenOptions, Permissions};
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::sync::Barrier;

use engine::{Lock, Refused};

/// A new empty directory for one test.
fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wf-agents-lock-attack-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

// A2, for takers that start at the same instant: exactly one gets the lock and every other is refused as held, never
// with another error. Twenty-five rounds of eight threads, each thread opening the file itself. About 100 ms.
#[test]
fn takers_racing_at_one_instant_leave_exactly_one_holder() {
    const TAKERS: usize = 8;
    for round in 0..25 {
        let d = dir(&format!("race{round}"));
        let start = &Barrier::new(TAKERS);
        let dir_ref = &d;
        let results: Vec<Result<Lock, Refused>> = std::thread::scope(|s| {
            let takers: Vec<_> = (0..TAKERS)
                .map(|_| {
                    s.spawn(move || {
                        start.wait();
                        Lock::acquire(dir_ref)
                    })
                })
                .collect();
            takers.into_iter().map(|t| t.join().unwrap()).collect()
        });
        assert_eq!(
            results.iter().filter(|r| r.is_ok()).count(),
            1,
            "round {round}: {results:?}"
        );
        for refused in results.iter().filter_map(|r| r.as_ref().err()) {
            let Refused::Held { holder, .. } = refused else {
                panic!("round {round}: {refused:?}");
            };
            // The winner may not have written who it is yet; when it has, it is this process.
            if let Some(h) = holder {
                assert_eq!(h.pid, std::process::id(), "round {round}");
            }
            assert!(!refused.to_string().is_empty());
        }
        drop(results);
        let _ = std::fs::remove_dir_all(&d);
    }
}

// A1 and A5, for the file a refusal reads: a refused start only reads it, so the holder is named again by the next
// refusal. About 5 ms.
#[test]
fn a_refusal_leaves_the_lock_file_as_the_holder_wrote_it() {
    let d = dir("intact");
    let holder = Lock::acquire(&d).unwrap();
    let before = std::fs::read(config::engine_lock(&d)).unwrap();
    assert!(!before.is_empty());
    for _ in 0..3 {
        let said = Lock::acquire(&d).unwrap_err().to_string();
        let expected = format!("engine already running: process {} holds ", std::process::id());
        assert!(said.starts_with(&expected), "{said}");
        assert_eq!(std::fs::read(config::engine_lock(&d)).unwrap(), before);
    }
    drop(holder);
    let _ = std::fs::remove_dir_all(&d);
}

// A2, for a directory reached by another path: the lock is the directory's, not the spelling's. A link to the
// directory and a path through `.` and `..` meet the same lock from either side. About 10 ms.
#[test]
fn another_path_to_the_same_directory_is_the_same_lock() {
    let d = dir("spelling");
    std::fs::create_dir(d.join("sub")).unwrap();
    let link = d.with_extension("link");
    let _ = std::fs::remove_file(&link);
    symlink(&d, &link).unwrap();
    let others = [link.clone(), d.join("sub").join(".."), d.join(".")];
    let first = Lock::acquire(&d).unwrap();
    for other in &others {
        let second = Lock::acquire(other);
        assert!(matches!(second, Err(Refused::Held { .. })), "{other:?}: {second:?}");
    }
    drop(first);
    let first = Lock::acquire(&link).unwrap();
    let second = Lock::acquire(&d);
    assert!(matches!(second, Err(Refused::Held { .. })), "{second:?}");
    drop(first);
    let _ = std::fs::remove_file(&link);
    let _ = std::fs::remove_dir_all(&d);
}

// A6, for a lock file the process may not write: the open fails with the system's error and nothing is written to the
// file. Nothing to test for a process that file modes do not bind (root). About 5 ms.
#[test]
fn a_lock_file_that_may_not_be_written_is_refused_with_the_permission_error() {
    let d = dir("readonly-file");
    let path = config::engine_lock(&d);
    std::fs::write(&path, "1 1\n").unwrap();
    std::fs::set_permissions(&path, Permissions::from_mode(0o444)).unwrap();
    if OpenOptions::new().write(true).open(&path).is_ok() {
        eprintln!("skipped: file modes do not bind this process");
        return;
    }
    let refused = Lock::acquire(&d).unwrap_err();
    let Refused::Unusable { path: named, error } = &refused else {
        panic!("{refused:?}");
    };
    assert_eq!(*named, path);
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(refused.to_string().ends_with(&error.to_string()), "{refused}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "1 1\n");
    let _ = std::fs::remove_dir_all(&d);
}

// A6, for a data directory the process may not create the lock file in. About 5 ms.
#[test]
fn a_directory_that_may_not_be_written_is_refused_with_the_permission_error() {
    let d = dir("readonly-dir");
    std::fs::set_permissions(&d, Permissions::from_mode(0o555)).unwrap();
    if File::create(d.join("probe")).is_ok() {
        std::fs::set_permissions(&d, Permissions::from_mode(0o755)).unwrap();
        let _ = std::fs::remove_dir_all(&d);
        eprintln!("skipped: directory modes do not bind this process");
        return;
    }
    let refused = Lock::acquire(&d).unwrap_err();
    let restore = std::fs::set_permissions(&d, Permissions::from_mode(0o755));
    let Refused::Unusable { error, .. } = &refused else {
        panic!("{refused:?}");
    };
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(refused.to_string().ends_with(&error.to_string()), "{refused}");
    assert!(!config::engine_lock(&d).exists());
    restore.unwrap();
    let _ = std::fs::remove_dir_all(&d);
}

// A2 and A6, for a lock file that is a link: one to a file locks the file it points to, even one not made yet, and one
// to a directory cannot be opened and is refused with the system's error. About 10 ms.
#[test]
fn a_lock_file_that_is_a_link_locks_what_it_points_to_and_one_to_a_directory_is_refused() {
    let d = dir("links");
    let path = config::engine_lock(&d);
    let target = d.join("elsewhere");
    symlink(&target, &path).unwrap();
    let first = Lock::acquire(&d).unwrap();
    assert!(target.exists(), "the lock was not taken on what the link points to");
    let said = Lock::acquire(&d).unwrap_err().to_string();
    let expected = format!("engine already running: process {} holds ", std::process::id());
    assert!(said.starts_with(&expected), "{said}");
    drop(first);
    std::fs::remove_file(&path).unwrap();

    std::fs::create_dir(d.join("a-directory")).unwrap();
    symlink(d.join("a-directory"), &path).unwrap();
    let refused = Lock::acquire(&d).unwrap_err();
    assert!(matches!(refused, Refused::Unusable { .. }), "{refused:?}");
    let _ = std::fs::remove_dir_all(&d);
}
