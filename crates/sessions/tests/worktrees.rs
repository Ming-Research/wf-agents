//! Git commands on one project's repository from several threads at once, as the engine's threads run them: those that
//! change what every worktree of the repository shares take turns, so git refuses none for a lock another holds.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Barrier};

use sessions::worktree::{create_worktree, git};

/// How many times each thread runs its command.
const TIMES: usize = 20;

fn run(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A project's checkout cloned from a bare origin whose main has one commit, and a clone of its own that pushes to it.
struct Repo {
    root: PathBuf,
    checkout: PathBuf,
    source: PathBuf,
}

impl Repo {
    fn new(name: &str) -> Repo {
        let root = std::env::temp_dir().join(format!("wf-agents-worktrees-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let origin = root.join("origin.git");
        let source = root.join("source");
        let checkout = root.join("checkout");
        run(&root, &["init", "-q", "--bare", &origin.to_string_lossy()]);
        run(&root, &["init", "-q", &source.to_string_lossy()]);
        std::fs::write(source.join("a"), "0").unwrap();
        run(&source, &["add", "-A"]);
        run(&source, &["commit", "-q", "-m", "start"]);
        run(
            &source,
            &["push", "-q", &origin.to_string_lossy(), "HEAD:refs/heads/main"],
        );
        run(&origin, &["symbolic-ref", "HEAD", "refs/heads/main"]);
        run(
            &root,
            &["clone", "-q", &origin.to_string_lossy(), &checkout.to_string_lossy()],
        );
        Repo { root, checkout, source }
    }

    /// Main moves on by one commit on the origin.
    fn main_moves_on(&self, n: usize) {
        std::fs::write(self.source.join("a"), n.to_string()).unwrap();
        run(&self.source, &["commit", "-q", "-am", &format!("change {n}")]);
        let origin = self.root.join("origin.git");
        run(
            &self.source,
            &["push", "-q", &origin.to_string_lossy(), "HEAD:refs/heads/main"],
        );
    }

    /// Where a work's worktree goes, by its name.
    fn worktree(&self, name: &str) -> PathBuf {
        self.root.join("worktrees").join(name)
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The failures among what the threads' commands gave.
fn failures(results: &[Result<String, String>]) -> Vec<&String> {
    results.iter().filter_map(|r| r.as_ref().err()).collect()
}

// Two threads each set up a work's worktree on the same project's checkout at once, as the session manager does: fetch
// main, then add a worktree on a new branch from it, which writes the branch's tracking into the repository's
// configuration. Main moves on before each turn, as it does between the starts of works, so that both fetches move the
// same remote-tracking branch. Every one of the worktrees is added.
#[test]
fn worktrees_set_up_at_once_on_one_checkout_are_all_added() {
    let r = Repo::new("add");
    let start = Arc::new(Barrier::new(3));
    let threads: Vec<_> = ["a", "b"]
        .into_iter()
        .map(|t| {
            let (checkout, start) = (r.checkout.clone(), Arc::clone(&start));
            let dirs: Vec<PathBuf> = (0..TIMES).map(|n| r.worktree(&format!("{t}-{n}"))).collect();
            std::thread::spawn(move || {
                dirs.iter()
                    .enumerate()
                    .map(|(n, dir)| {
                        start.wait();
                        create_worktree(&checkout, dir, &format!("work/{t}-{n}"), "main")
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    for n in 0..TIMES {
        r.main_moves_on(n + 1);
        start.wait();
    }
    let results: Vec<Result<String, String>> = threads.into_iter().flat_map(|h| h.join().unwrap()).collect();
    assert_eq!(failures(&results), Vec::<&String>::new());
    assert_eq!(results.len(), 2 * TIMES);
}

// One thread fetches main in a work's worktree, as a work catching up with main does, while another sets up worktrees
// on the same checkout, main moving on before each turn so that both fetches move the same remote-tracking branch.
// Every command succeeds.
#[test]
fn a_fetch_in_a_worktree_beside_worktrees_being_set_up_succeeds() {
    let r = Repo::new("fetch");
    let first = r.worktree("first");
    create_worktree(&r.checkout, &first, "work/first", "main").unwrap();
    let start = Arc::new(Barrier::new(3));
    let fetching = {
        let start = Arc::clone(&start);
        std::thread::spawn(move || {
            (0..TIMES)
                .map(|_| {
                    start.wait();
                    git(&first, &["fetch", "-q", "origin", "main"])
                })
                .collect::<Vec<_>>()
        })
    };
    let adding = {
        let (checkout, start) = (r.checkout.clone(), Arc::clone(&start));
        let dirs: Vec<PathBuf> = (0..TIMES).map(|n| r.worktree(&format!("next-{n}"))).collect();
        std::thread::spawn(move || {
            dirs.iter()
                .enumerate()
                .map(|(n, dir)| {
                    start.wait();
                    create_worktree(&checkout, dir, &format!("work/next-{n}"), "main")
                })
                .collect::<Vec<_>>()
        })
    };
    for n in 0..TIMES {
        r.main_moves_on(n + 1);
        start.wait();
    }
    let mut results = fetching.join().unwrap();
    results.extend(adding.join().unwrap());
    assert_eq!(failures(&results), Vec::<&String>::new());
    assert_eq!(results.len(), 2 * TIMES);
}
