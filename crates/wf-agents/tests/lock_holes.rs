//! What the data directory's lock must keep when its file is taken away, and what a refusal must say whatever the lock
//! file holds. These go through `engine::Lock`, the lock the `run` command takes before anything else.
//!
//! What a kernel lock does is flock(2)'s: the lock belongs to the open file, not to the path it was opened by, so it
//! outlives the file's name and is gone only when the last descriptor of that file is closed. Whether a second taker
//! gets in is decided by the lock's purpose, one engine on a data directory, not by the code's own earlier output.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use engine::{Lock, Refused};

/// A new empty directory for one test.
fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wf-agents-lock-holes-{name}-{}", std::process::id()));
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
