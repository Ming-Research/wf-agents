//! The data directory's lock under attack: the ways a lock can be held in name only, or refuse in a way that hides
//! why. The plain take, refusal and release are in `lock.rs`, and what needs a second process is in
//! `lock_process.rs`; these look for what those assume.
//!
//! What a kernel lock does is flock(2)'s: the lock belongs to the open file, not to the path it was opened by, so it
//! outlives the file's name and is gone only when the last descriptor of that file is closed. The tests do not rely on
//! the code's own earlier output: a second taker either gets in or does not, which the lock's purpose (one engine on
//! a data directory) decides.

use std::fs::{File, OpenOptions, Permissions};
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Barrier;

use engine::agent::{Agents, Reply, Request};
use engine::hub::{CheckRun, Failure, GitHub};
use engine::{Engine, Lock, Refused};

/// A new empty directory for one test.
fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wf-agents-lock-attack-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Takes the directory's lock file with `text` in it and the lock held, as a process that holds the lock and has
/// written something else would.
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

// A2, for the lock file's name taken away while its lock is held. An owner's `rm`, a `git clean` of the untracked file
// in the data directory (a git repository), or a file moved over the name leave the engine holding a file no path
// reaches; a second start that creates the name again locks a new file and runs beside the first. About 40 ms.
#[test]
fn a_lock_file_taken_away_while_the_lock_is_held_does_not_let_a_second_taker_in() {
    let mut got_in = Vec::new();
    for how in ["removed", "replaced", "git clean"] {
        let d = dir(&format!("away-{}", how.replace(' ', "-")));
        let first = Lock::acquire(&d).unwrap();
        let path = config::engine_lock(&d);
        match how {
            "removed" => std::fs::remove_file(&path).unwrap(),
            "replaced" => {
                let other = d.join("other");
                std::fs::write(&other, "").unwrap();
                std::fs::rename(&other, &path).unwrap();
            }
            _ => {
                git(&d, &["init", "-q"]);
                git(&d, &["clean", "-fdq"]);
                assert!(!path.exists(), "git clean did not take the lock file away");
            }
        }
        match Lock::acquire(&d) {
            Err(Refused::Held { .. }) => {}
            other => got_in.push(format!("{how}: {other:?}")),
        }
        drop(first);
        let _ = std::fs::remove_dir_all(&d);
    }
    assert!(
        got_in.is_empty(),
        "a second taker got in while the first still held its lock:\n{}",
        got_in.join("\n")
    );
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

// A5, for a time in the lock file that is far from now: the text is a process number and a time, so it names a holder,
// and the refusal message must still be produced, never a panic from the arithmetic on the time. The message is the
// one thing `run` prints when it is refused. About 20 ms.
#[test]
fn a_holder_time_far_from_now_still_gives_the_refusal_message() {
    let texts = [
        "7 -9223372036854775808\n",
        "7 -9223372036854775807\n",
        "7 9223372036854775807\n",
        "7 0\n",
    ];
    let mut failed = Vec::new();
    for (i, text) in texts.iter().enumerate() {
        let d = dir(&format!("time{i}"));
        let _held = hold(&d, text.as_bytes());
        let refused = Lock::acquire(&d).unwrap_err();
        assert!(matches!(refused, Refused::Held { .. }), "{text:?}: {refused:?}");
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| refused.to_string())) {
            Ok(said) if said.contains(" holds ") => {}
            Ok(said) => failed.push(format!("{text:?}: {said}")),
            Err(_) => failed.push(format!("{text:?}: making the message panicked")),
        }
        let _ = std::fs::remove_dir_all(&d);
    }
    assert!(failed.is_empty(), "{failed:#?}");
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

/// What the engine's configuration needs of a data directory: the files an engine reads from its last commit.
const INSTALL: &str = r#"
[owner]
language = "English"
[items]
title_max = 40
[machines]
workstation = "here"
heavy = "builds"
poll_interval = 60
[machines.hosts.here]
slots = 1
worktrees = "/tmp"
[agents.clis.claude]
kind = "claude"
command = "claude"
[agents.levels.l]
desc = "all work"
cli = "claude"
model = "m"
[gh]
allow = ["pr view"]
[query]
max_rows = 10
timeout_ms = 1000
"#;

const PROJECT: &str = r#"
repo = "o/p"
checkout = "/tmp/p"
design_goal = "simple"
design_paths = ["design/"]
test_paths = ["tests/"]
rules = ["ground"]
[ci]
fast = "ci"
full = ["ci"]
merge_checks = ["check"]
"#;

/// A data directory with its configuration committed.
fn committed_dir(name: &str) -> PathBuf {
    let d = dir(name);
    std::fs::create_dir_all(d.join("projects")).unwrap();
    std::fs::create_dir_all(d.join("rules")).unwrap();
    std::fs::write(d.join("install.toml"), INSTALL).unwrap();
    std::fs::write(d.join("projects/p.toml"), PROJECT).unwrap();
    std::fs::write(d.join("rules/ground.md"), "Facts first.\n").unwrap();
    git(&d, &["init", "-q"]);
    git(&d, &["add", "-A"]);
    git(&d, &["commit", "-q", "-m", "configuration"]);
    d
}

/// Agents and GitHub for an engine that starts no work.
struct Idle;

impl Agents for Idle {
    fn run(&self, _r: &Request) -> Reply {
        unreachable!("no work starts")
    }
}

impl GitHub for Idle {
    fn pull_request(
        &self,
        _repo: &str,
        _branch: &str,
        _base: &str,
        _title_body: Option<(&str, &str)>,
        _existing: Option<i64>,
    ) -> Result<i64, Failure> {
        unreachable!("no work starts")
    }

    fn checks(&self, _repo: &str, _sha: &str, _workflow: &str) -> Result<Vec<CheckRun>, Failure> {
        unreachable!("no work starts")
    }

    fn rerun(&self, _repo: &str, _sha: &str, _workflow: &str) -> Result<Vec<CheckRun>, Failure> {
        unreachable!("no work starts")
    }

    fn merge(&self, _repo: &str, _pr: i64) -> Result<String, Failure> {
        unreachable!("no work starts")
    }

    fn close(&self, _repo: &str, _pr: i64, _comment: &str) -> Result<(), Failure> {
        unreachable!("no work starts")
    }

    fn gh(&self, _args: &[String]) -> Result<(i32, String), Failure> {
        unreachable!("no work starts")
    }
}

// A1 and A2, for the engine itself: the lock handed to `Engine::new` stays held for as long as the engine lives and
// is freed when it is dropped. A guard dropped at once (`drop(lock)`, or a lock the engine never stores) passes every
// test that holds a `Lock` by itself, while a second engine starts. About 1 s: the engine compiles the flow.
#[test]
fn an_engine_holds_its_directory_until_it_is_dropped() {
    let d = committed_dir("engine");
    let lock = Lock::acquire(&d).unwrap();
    let committed = config::committed(&d).unwrap();
    let engine = Engine::new(lock, committed, Box::new(Idle), Box::new(Idle)).unwrap();
    match Lock::acquire(&d) {
        Err(Refused::Held { holder: Some(h), .. }) => assert_eq!(h.pid, std::process::id()),
        other => panic!("a second engine could take the directory while the first lives: {other:?}"),
    }
    drop(engine);
    if let Err(e) = Lock::acquire(&d) {
        panic!("the directory should be free once the engine is dropped: {e}");
    }
    let _ = std::fs::remove_dir_all(&d);
}
