//! The engine's git commands run no program the repository's git configuration names. A work's agent can write the
//! configuration its worktree shares with the project's checkout; git would then run the program it names as the
//! engine's user, with the engine's credentials, on the engine's own commands. Each setting below names a program that
//! records its own name in the markers directory. The reference is git itself: plain git, on the same repository set
//! up the same way and taking the same steps, does run each of them, so that a program that never ran for any reason
//! cannot pass the test.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use engine::git::{catch_up, create_worktree, diff, head, is_clean};
use engine::hub::{delete_ref, push};

/// The programs written, each by the name its marker takes, and what it does after it has left its marker: a filter
/// passes its input on, the transports pass on to the git program they stand for, so that the steps after them go
/// through when they run; the others fail, as a program that does what it likes does.
const PROGRAMS: &[(&str, &str)] = &[
    ("clean", "cat"),
    ("smudge", "cat"),
    ("merge", "exit 1"),
    ("fsmonitor", "exit 1"),
    ("ssh", "exit 1"),
    ("proxy", "exit 1"),
    ("credential", "exit 1"),
    ("extdiff", "exit 1"),
    ("gpg", "exit 1"),
    ("uploadpack", "exec git upload-pack \"$@\""),
    ("receivepack", "exec git receive-pack \"$@\""),
];

#[derive(Clone, Copy, PartialEq)]
enum Side {
    /// Git as a person runs it.
    Plain,
    /// Git as the engine runs it.
    Engine,
}

struct Fixture {
    root: PathBuf,
    markers: PathBuf,
    programs: PathBuf,
    seed: PathBuf,
    checkout: PathBuf,
}

/// Git as the test sets repositories up: with the programs off, so that what ran after the settings were written is only
/// the commands under test.
fn setup(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(["-c", "core.hooksPath=/dev/null", "-c", "core.fsmonitor=false"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Git as a person runs it (`Side::Plain`) or as the engine does (`Side::Engine`), failing allowed: what a step does is
/// told by the markers, not by its result.
fn run(side: Side, dir: &Path, args: &[&str]) {
    match side {
        Side::Plain => {
            let _ = Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .output();
        }
        Side::Engine => {
            let who = ["-c", "user.name=t", "-c", "user.email=t@t"];
            let _ = engine::git::git(dir, &[&who[..], args].concat());
        }
    }
}

/// A server that answers every request with 401, so that git asks its credential helpers to forget what it tried.
fn unauthorized_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().take(32) {
            let Ok(mut stream) = stream else { continue };
            let mut request = [0u8; 4096];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(
                b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"t\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        }
    });
    port
}

impl Fixture {
    fn new(name: &str) -> Fixture {
        let root = std::env::temp_dir().join(format!("wf-agents-git-programs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (markers, programs) = (root.join("markers"), root.join("programs"));
        std::fs::create_dir_all(&markers).unwrap();
        std::fs::create_dir_all(&programs).unwrap();
        for (program, tail) in PROGRAMS {
            let file = programs.join(program);
            std::fs::write(
                &file,
                format!("#!/bin/sh\ntouch \"{}\"\n{tail}\n", markers.join(program).display()),
            )
            .unwrap();
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let (origin, seed, checkout) = (root.join("origin.git"), root.join("seed"), root.join("checkout"));
        setup(&root, &["init", "-q", "--bare", &origin.to_string_lossy()]);
        setup(
            &root,
            &["clone", "-q", &origin.to_string_lossy(), &seed.to_string_lossy()],
        );
        let fixture = Fixture {
            root,
            markers,
            programs,
            seed,
            checkout,
        };
        // The attributes name the drivers; nothing defines them yet, so they do nothing until the configuration does.
        std::fs::write(
            fixture.seed.join(".gitattributes"),
            "*.txt filter=evil merge=evil diff=evil\n",
        )
        .unwrap();
        let lines: String = (1..=9).map(|n| format!("line {n}\n")).collect();
        std::fs::write(fixture.seed.join("f.txt"), lines).unwrap();
        setup(&fixture.seed, &["add", "-A"]);
        setup(&fixture.seed, &["commit", "-q", "-m", "first"]);
        setup(&fixture.seed, &["push", "-q", "origin", "HEAD:main"]);
        setup(
            &fixture.root,
            &[
                "clone",
                "-q",
                &origin.to_string_lossy(),
                &fixture.checkout.to_string_lossy(),
            ],
        );
        fixture
    }

    fn program(&self, name: &str) -> String {
        self.programs.join(name).to_string_lossy().into_owned()
    }

    /// What an agent can write: the settings that name programs, in the configuration the checkout's worktrees share, and
    /// three remotes that reach git over a transport that runs one.
    fn poison(&self, http_port: u16) {
        let c = &self.checkout;
        let set = |key: &str, value: String| setup(c, &["config", key, &value]);
        set("filter.evil.clean", self.program("clean"));
        set("filter.evil.smudge", self.program("smudge"));
        set("merge.evil.driver", format!("{} %O %A %B", self.program("merge")));
        set("core.fsmonitor", self.program("fsmonitor"));
        set("core.sshCommand", self.program("ssh"));
        set("core.gitProxy", self.program("proxy"));
        set("credential.helper", format!("!{}", self.program("credential")));
        set("diff.external", self.program("extdiff"));
        set("remote.origin.uploadpack", self.program("uploadpack"));
        set("remote.origin.receivepack", self.program("receivepack"));
        set("gpg.program", self.program("gpg"));
        set("remote.ssh.url", "ssh://wf-agents-test.invalid/repo.git".into());
        set("remote.proxy.url", "git://wf-agents-test.invalid/repo.git".into());
        set("remote.web.url", format!("http://u:p@127.0.0.1:{http_port}/repo.git"));
        for remote in ["ssh", "proxy", "web"] {
            set(
                &format!("remote.{remote}.fetch"),
                format!("+refs/heads/*:refs/remotes/{remote}/*"),
            );
        }
    }

    /// The origin's main moves on by a line of the file the work also changes, so that merging it needs a merge of the
    /// file's content.
    fn advance(&self, line: &str) {
        let text = std::fs::read_to_string(self.seed.join("f.txt")).unwrap();
        let moved: Vec<String> = text
            .lines()
            .map(|l| {
                if l == "line 8" || l.starts_with("main ") {
                    format!("main {line}")
                } else {
                    l.to_string()
                }
            })
            .collect();
        std::fs::write(self.seed.join("f.txt"), moved.join("\n") + "\n").unwrap();
        setup(&self.seed, &["add", "-A"]);
        setup(&self.seed, &["commit", "-q", "-m", line]);
        setup(&self.seed, &["push", "-q", "origin", "HEAD:main"]);
    }

    /// Which programs ran since the last call, by name; the markers are cleared.
    fn programs_run(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.markers)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        for name in &names {
            std::fs::remove_file(self.markers.join(name)).unwrap();
        }
        names
    }

    /// The steps the engine takes on a work, taken by `side` on a worktree and branch of its own named `name`: set up,
    /// committed in, read (status, diff), brought up to a main that moved on, pushed, deleted, and the commands that
    /// reach other remotes.
    fn steps(&self, side: Side, name: &str) {
        let wt = self.root.join(format!("worktree-{name}"));
        let spec = format!("refs/heads/{name}");
        match side {
            Side::Plain => {
                run(
                    side,
                    &self.checkout,
                    &["fetch", "-q", "origin", "+refs/heads/main:refs/remotes/origin/main"],
                );
                run(
                    side,
                    &self.checkout,
                    &[
                        "worktree",
                        "add",
                        "-q",
                        "-b",
                        name,
                        &wt.to_string_lossy(),
                        "origin/main",
                    ],
                );
            }
            Side::Engine => {
                let _ = create_worktree(&self.checkout, &wt, name, "main");
            }
        }
        // The work's own change to the file, committed.
        let text = std::fs::read_to_string(wt.join("f.txt")).unwrap_or_default();
        let _ = std::fs::write(wt.join("f.txt"), text.replace("line 2", &format!("{name} 2")));
        run(side, &wt, &["add", "-A"]);
        run(side, &wt, &["commit", "-q", "-m", "own"]);
        match side {
            Side::Plain => {
                run(side, &wt, &["status", "--porcelain"]);
                run(side, &wt, &["diff", "HEAD~1", "HEAD"]);
            }
            Side::Engine => {
                let _ = is_clean(&wt);
                let _ = diff(&wt, "HEAD~1", "HEAD", &[]);
            }
        }
        self.advance(name);
        match side {
            Side::Plain => {
                run(
                    side,
                    &wt,
                    &["fetch", "-q", "origin", "+refs/heads/main:refs/remotes/origin/main"],
                );
                run(side, &wt, &["merge", "--no-edit", "-q", "origin/main"]);
                run(side, &wt, &["merge", "--abort"]);
                run(side, &wt, &["push", "-q", "origin", &format!("HEAD:{spec}")]);
                run(side, &wt, &["push", "-q", "origin", &format!(":{spec}")]);
            }
            Side::Engine => {
                let _ = catch_up(&wt, "main");
                if let Ok(sha) = head(&wt) {
                    let _ = push(&wt, &sha, &spec, false);
                }
                let _ = delete_ref(&wt, &spec);
            }
        }
        for remote in ["ssh", "proxy", "web"] {
            run(side, &wt, &["fetch", "-q", remote]);
        }
        // A signed commit: the setting that asks for it goes in for this step only.
        setup(&self.checkout, &["config", "commit.gpgSign", "true"]);
        run(side, &wt, &["commit", "-q", "--allow-empty", "-m", "signed"]);
        setup(&self.checkout, &["config", "--unset", "commit.gpgSign"]);
    }
}

// A checkout whose configuration names a program for each setting that makes git run one: the engine's worktree setup,
// commits, status, diff, merge of main, push, deletion of a branch and fetches from other remotes run none of them,
// while the same steps by plain git run every one. It catches a command of the engine's started without pinning one
// of these settings, whichever command it is.
//
// Alone it runs under a second; it is worth it because no other test starts git against a configuration that names a
// program, and the settings are the only way an agent's `git config` reaches the engine's own credentials.
#[test]
fn programs_named_in_the_configuration_run_in_no_git_command_of_the_engine() {
    let fx = Fixture::new("all");
    let port = unauthorized_server();
    fx.poison(port);

    // The reference: plain git takes the steps and runs every program. Without this, a program that never ran for any
    // reason would pass the test too.
    fx.steps(Side::Plain, "control");
    let ran = fx.programs_run();
    let missing: Vec<&str> = PROGRAMS
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !ran.iter().any(|r| r == name))
        .collect();
    assert!(
        missing.is_empty(),
        "plain git did not run {missing:?}: the fixture does not make git run them (ran {ran:?})"
    );

    // The engine's git takes the same steps.
    fx.steps(Side::Engine, "work");
    assert_eq!(
        fx.programs_run(),
        Vec::<String>::new(),
        "the engine's git ran a program named in the configuration"
    );

    let _ = std::fs::remove_dir_all(&fx.root);
}
