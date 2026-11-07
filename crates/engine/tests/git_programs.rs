//! The engine's git commands run no program that the git configuration names. A work's agent can write the
//! configuration its worktree shares with the project's checkout (`core.sshCommand`, `core.fsmonitor`,
//! `credential.helper`, `filter.<name>.smudge`, `merge.<name>.driver` and the like); git would then run that program as
//! the engine's user, with the engine's credentials, on the engine's own fetch, merge, push and checkout. The reference
//! is git itself: the same repositories, set up the same way, run by plain git commands, do run the programs (each case
//! first checks that, so that a program which never ran for any reason cannot pass the case).
//!
//! Each case has two tests: `*_engine`, the engine's own functions, and `*_pins`, plain git with the settings the
//! engine is to pin written as `-c` options, which shows that those settings stop the program in git itself.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use engine::git::{catch_up, changed_since, create_worktree, diff, head};
use engine::hub::{delete_ref, push};

/// The programs the configurations name: each records its own name in the markers directory and exits, as a program
/// that does what it likes and leaves the exit code alone would.
const PROGRAMS: &[&str] = &[
    "fsmonitor",
    "ssh",
    "credential-helper",
    "git-proxy",
    "ext-transport",
    "upload-pack",
    "receive-pack",
    "smudge",
    "clean",
    "merge-driver",
    "textconv",
    "ext-diff",
    "gpg-program",
];

/// The settings the engine's git commands are to be run with, so far as research found them (the object of the empty
/// tree is the tree git reads attributes from, so that no attribute names a driver).
const CANDIDATE_PINS: &[(&str, &str)] = &[
    ("core.hooksPath", "/dev/null"),
    ("core.fsmonitor", "false"),
    ("core.sshCommand", "ssh"),
    ("core.gitProxy", ""),
    ("core.attributesFile", "/dev/null"),
    ("attr.tree", "4b825dc642cb6eb9a060e54bf8d69288fbee4904"),
    ("credential.helper", ""),
    ("protocol.ext.allow", "never"),
    ("remote.origin.uploadpack", "git-upload-pack"),
    ("remote.origin.receivepack", "git-receive-pack"),
    ("commit.gpgsign", "false"),
];

/// One way a configuration can name a program, and what plain git runs of it.
struct Case {
    /// The `.gitattributes` of the project's main.
    attributes: &'static str,
    /// Whether the attributes are in a file the configuration names (`core.attributesFile`) and not in the tree.
    attributes_file: bool,
    /// Whether the work and main change the same file, so that a merge needs a merge driver.
    same_file: bool,
    /// The address of the work's origin when the configuration changes it; `{port}` is a server that answers 401.
    remote: &'static str,
    /// The configuration written in the checkout; `{name}` is the path of the program of that name.
    config: &'static [(&'static str, &'static str)],
    /// The programs plain git runs with that configuration.
    expected: &'static [&'static str],
}

const FSMONITOR: Case = Case {
    attributes: "",
    attributes_file: false,
    same_file: false,
    remote: "",
    config: &[("core.fsmonitor", "{fsmonitor}")],
    expected: &["fsmonitor"],
};

const SSH: Case = Case {
    attributes: "",
    attributes_file: false,
    same_file: false,
    remote: "ssh://127.0.0.1:1/repo.git",
    config: &[("core.sshCommand", "{ssh}")],
    expected: &["ssh"],
};

const CREDENTIAL: Case = Case {
    attributes: "",
    attributes_file: false,
    same_file: false,
    remote: "http://u:p@127.0.0.1:{port}/repo.git",
    config: &[("credential.helper", "{credential-helper}")],
    expected: &["credential-helper"],
};

const GIT_PROXY: Case = Case {
    attributes: "",
    attributes_file: false,
    same_file: false,
    remote: "git://127.0.0.1:1/repo.git",
    config: &[("core.gitProxy", "{git-proxy}")],
    expected: &["git-proxy"],
};

const EXT_TRANSPORT: Case = Case {
    attributes: "",
    attributes_file: false,
    same_file: false,
    remote: "ext::{ext-transport} %S",
    config: &[("protocol.ext.allow", "always")],
    expected: &["ext-transport"],
};

const PACK_PROGRAMS: Case = Case {
    attributes: "",
    attributes_file: false,
    same_file: false,
    remote: "",
    config: &[
        ("remote.origin.uploadpack", "{upload-pack}"),
        ("remote.origin.receivepack", "{receive-pack}"),
    ],
    expected: &["upload-pack", "receive-pack"],
};

const FILTER: Case = Case {
    attributes: "*.dat filter=x\n",
    attributes_file: false,
    same_file: false,
    remote: "",
    config: &[("filter.x.smudge", "{smudge}"), ("filter.x.clean", "{clean}")],
    expected: &["smudge"],
};

const FILTER_IN_ATTRIBUTES_FILE: Case = Case {
    attributes: "*.dat filter=x\n",
    attributes_file: true,
    same_file: false,
    remote: "",
    config: &[
        ("core.attributesFile", "{attributes}"),
        ("filter.x.smudge", "{smudge}"),
        ("filter.x.clean", "{clean}"),
    ],
    expected: &["smudge"],
};

const MERGE_DRIVER: Case = Case {
    attributes: "*.txt merge=x\n",
    attributes_file: false,
    same_file: true,
    remote: "",
    config: &[("merge.x.driver", "{merge-driver} %O %A %B")],
    expected: &["merge-driver"],
};

const TEXTCONV: Case = Case {
    attributes: "*.txt diff=x\n",
    attributes_file: false,
    same_file: false,
    remote: "",
    config: &[("diff.x.textconv", "{textconv}")],
    expected: &["textconv"],
};

const EXTERNAL_DIFF: Case = Case {
    attributes: "",
    attributes_file: false,
    same_file: false,
    remote: "",
    config: &[("diff.external", "{ext-diff}")],
    expected: &["ext-diff"],
};

const GPG: Case = Case {
    attributes: "",
    attributes_file: false,
    same_file: false,
    remote: "",
    config: &[("commit.gpgsign", "true"), ("gpg.program", "{gpg-program}")],
    expected: &["gpg-program"],
};

/// Git as the test sets repositories up: with its hooks off and nothing signed.
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
        .args(["-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false"])
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

/// An address that answers every request with 401, so that git asks its credential helpers to forget the credentials
/// it was given.
fn serve_unauthorized() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut seen = Vec::new();
            let mut buf = [0u8; 1024];
            while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => seen.extend_from_slice(&buf[..n]),
                }
            }
            let _ = stream.write_all(
                b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"t\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        }
    });
    port
}

struct Fixture {
    root: PathBuf,
    markers: PathBuf,
    seed: PathBuf,
    checkout: PathBuf,
    /// The worktrees of the reference run, of the run with the pins and of the engine's.
    control: PathBuf,
    probe: PathBuf,
    work: PathBuf,
    /// The commit main was at before it moved on.
    first: String,
}

impl Fixture {
    fn new(name: &str, case: &Case) -> Fixture {
        let root = std::env::temp_dir().join(format!("wf-agents-git-programs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (markers, programs) = (root.join("markers"), root.join("programs"));
        std::fs::create_dir_all(&markers).unwrap();
        std::fs::create_dir_all(&programs).unwrap();
        for program in PROGRAMS {
            let body = match *program {
                "smudge" | "clean" => "cat\n",
                "textconv" => "cat \"$1\"\n",
                _ => "",
            };
            let file = programs.join(program);
            let marker = markers.join(program);
            std::fs::write(&file, format!("#!/bin/sh\ntouch \"{}\"\n{body}", marker.display())).unwrap();
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let (origin, seed, checkout) = (root.join("origin.git"), root.join("seed"), root.join("checkout"));
        setup(&root, &["init", "-q", "--bare", &origin.to_string_lossy()]);
        setup(
            &root,
            &["clone", "-q", &origin.to_string_lossy(), &seed.to_string_lossy()],
        );
        std::fs::write(seed.join("a.txt"), "hi\n").unwrap();
        std::fs::write(seed.join("b.dat"), "hi\n").unwrap();
        if !case.attributes_file {
            std::fs::write(seed.join(".gitattributes"), case.attributes).unwrap();
        }
        setup(&seed, &["add", "-A"]);
        setup(&seed, &["commit", "-q", "-m", "first"]);
        setup(&seed, &["push", "-q", "origin", "HEAD:main"]);
        let first = setup(&seed, &["rev-parse", "HEAD"]);
        setup(
            &root,
            &["clone", "-q", &origin.to_string_lossy(), &checkout.to_string_lossy()],
        );
        let fx = Fixture {
            root: root.clone(),
            markers,
            seed,
            checkout,
            control: root.join("worktree-control"),
            probe: root.join("worktree-probe"),
            work: root.join("worktree-work"),
            first,
        };
        for (dir, branch) in [(&fx.control, "control"), (&fx.probe, "probe"), (&fx.work, "work")] {
            setup(
                &fx.checkout,
                &[
                    "worktree",
                    "add",
                    "-q",
                    "-b",
                    branch,
                    &dir.to_string_lossy(),
                    "origin/main",
                ],
            );
            // The work's own change; in the merge driver's case it is to a file main changes too.
            let file = if case.same_file { "a.txt" } else { "own.txt" };
            std::fs::write(dir.join(file), format!("{branch}\n")).unwrap();
            setup(dir, &["add", "-A"]);
            setup(dir, &["commit", "-q", "-m", branch]);
        }
        // Main moves on, in the files the attributes name.
        std::fs::write(fx.seed.join("a.txt"), "main\n").unwrap();
        std::fs::write(fx.seed.join("b.dat"), "main\n").unwrap();
        setup(&fx.seed, &["add", "-A"]);
        setup(&fx.seed, &["commit", "-q", "-m", "second"]);
        setup(&fx.seed, &["push", "-q", "origin", "HEAD:main"]);

        // The configuration an agent could write, in the checkout's, which every worktree shares.
        let port = if case.remote.contains("{port}") {
            serve_unauthorized()
        } else {
            0
        };
        let attributes = root.join("attributes");
        if case.attributes_file {
            std::fs::write(&attributes, case.attributes).unwrap();
        }
        let expand = |text: &str| {
            let mut text = text.replace("{port}", &port.to_string());
            text = text.replace("{attributes}", &attributes.to_string_lossy());
            for program in PROGRAMS {
                text = text.replace(&format!("{{{program}}}"), &programs.join(program).to_string_lossy());
            }
            text
        };
        if !case.remote.is_empty() {
            setup(&fx.checkout, &["config", "remote.origin.url", &expand(case.remote)]);
        }
        for (key, value) in case.config {
            setup(&fx.checkout, &["config", key, &expand(value)]);
        }
        fx
    }

    /// Which programs ran since the last call, by name; the markers are cleared.
    fn ran(&self) -> Vec<String> {
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
}

/// The steps of the engine, by plain git commands, with the pins as `-c` options: fetch main, merge it, look at the
/// worktree, diff, push and delete a branch, and add a worktree. A command that fails is no failure of the test: a
/// program that ran and failed has run.
fn plain_steps(fx: &Fixture, label: &str, work: &Path, pins: &[(&str, &str)]) {
    let attempt = |dir: &Path, args: &[&str]| {
        let _ = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(pins.iter().flat_map(|(k, v)| ["-c".to_string(), format!("{k}={v}")]))
            .args(args)
            .output()
            .unwrap();
    };
    attempt(
        work,
        &["fetch", "-q", "origin", "+refs/heads/main:refs/remotes/origin/main"],
    );
    attempt(work, &["merge", "--no-edit", "-q", "origin/main"]);
    attempt(work, &["status", "--porcelain"]);
    attempt(work, &["diff", "--name-only", "origin/main"]);
    // What the engine asks of diff when it shows a diff: no external program, no conversion.
    let flags: &[&str] = if pins.is_empty() {
        &[]
    } else {
        &["--no-ext-diff", "--no-textconv"]
    };
    attempt(
        work,
        &[&["diff"], flags, &[&format!("{}..origin/main", fx.first)]].concat(),
    );
    attempt(work, &["push", "-q", "origin", &format!("HEAD:refs/heads/{label}")]);
    attempt(work, &["push", "-q", "origin", &format!(":refs/heads/{label}")]);
    let fresh = fx.root.join(format!("worktree-{label}-fresh"));
    attempt(
        &fx.checkout,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            &format!("{label}-fresh"),
            &fresh.to_string_lossy(),
            "origin/main",
        ],
    );
}

/// The same steps, by the engine's functions.
fn engine_steps(fx: &Fixture) {
    let work = &fx.work;
    let _ = catch_up(work, "main");
    let _ = changed_since(work, "origin/main");
    let _ = diff(work, &fx.first, "origin/main", &[]);
    if let Ok(sha) = head(work) {
        let _ = push(work, &sha, "refs/heads/work", false);
    }
    let _ = delete_ref(work, "refs/heads/work");
    let _ = create_worktree(
        &fx.checkout,
        &fx.root.join("worktree-engine-fresh"),
        "engine-fresh",
        "main",
    );
}

/// The case with plain git first, which must run the programs it expects; then the engine's functions, or plain git
/// with the pins, which must run none.
fn run(name: &str, case: &Case, engine: bool) {
    let fx = Fixture::new(name, case);
    plain_steps(&fx, "control", &fx.control, &[]);
    let ran = fx.ran();
    for program in case.expected {
        assert!(
            ran.iter().any(|r| r == program),
            "plain git did not run {program}: the fixture does not make git run it (ran {ran:?})"
        );
    }
    if engine {
        engine_steps(&fx);
        assert_eq!(fx.ran(), Vec::<String>::new(), "the engine's git ran a program");
    } else {
        plain_steps(&fx, "probe", &fx.probe, CANDIDATE_PINS);
        assert_eq!(fx.ran(), Vec::<String>::new(), "git with the pins ran a program");
    }
    let _ = std::fs::remove_dir_all(&fx.root);
}

macro_rules! case {
    ($engine:ident, $pins:ident, $case:expr) => {
        #[test]
        fn $engine() {
            run(stringify!($engine), &$case, true);
        }

        #[test]
        fn $pins() {
            run(stringify!($pins), &$case, false);
        }
    };
}

// `core.fsmonitor`: the program git asks which files changed, on every command that looks at the worktree.
case!(fsmonitor_engine, fsmonitor_pins, FSMONITOR);
// `core.sshCommand`: the program git starts instead of ssh for an ssh address.
case!(ssh_command_engine, ssh_command_pins, SSH);
// `credential.helper`: the program git asks for, and tells to forget, the credentials of an http address.
case!(credential_helper_engine, credential_helper_pins, CREDENTIAL);
// `core.gitProxy`: the program git starts to reach a git:// address.
case!(git_proxy_engine, git_proxy_pins, GIT_PROXY);
// `protocol.ext.allow` with an `ext::` address, which is a program.
case!(ext_transport_engine, ext_transport_pins, EXT_TRANSPORT);
// `remote.origin.uploadpack` and `receivepack`: the programs git starts on the other side of a fetch and a push.
case!(pack_programs_engine, pack_programs_pins, PACK_PROGRAMS);
// `filter.<name>.smudge` and `clean`, named by an attribute of the tree.
case!(filter_engine, filter_pins, FILTER);
// The same, the attribute in a file the configuration names.
case!(
    filter_attributes_file_engine,
    filter_attributes_file_pins,
    FILTER_IN_ATTRIBUTES_FILE
);
// `merge.<name>.driver`, named by an attribute of the tree.
case!(merge_driver_engine, merge_driver_pins, MERGE_DRIVER);
// `diff.<name>.textconv`, named by an attribute of the tree.
case!(textconv_engine, textconv_pins, TEXTCONV);
// `diff.external`.
case!(external_diff_engine, external_diff_pins, EXTERNAL_DIFF);
// `commit.gpgsign` with `gpg.program`: the merge commit of the engine is signed by the program.
case!(gpg_engine, gpg_pins, GPG);
