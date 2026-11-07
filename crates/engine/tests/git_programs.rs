//! The engine's git commands start no program that the repository's configuration names. A work's agent can write the
//! git configuration its worktree shares with the project's checkout (`core.sshCommand`, `core.fsmonitor`,
//! `credential.helper`, `filter.<name>`, `merge.<name>.driver`, `commit.gpgsign` with `gpg.program`, the programs of a
//! remote, `core.askPass`, `core.gitProxy`) and a `.gitattributes` file in its branch; git would then start that
//! program as the engine's user, with the engine's credentials, on the engine's own `fetch`, `push`, `merge` and
//! checkout. The reference is git itself: the same repositories, set up the same way, run by plain git commands, do
//! start the programs; a test that has no such control passes as well when its program never had a way to run.
//!
//! The settings and the way each is stopped are `config::shared_git`'s; `git_hooks.rs` covers hooks.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use engine::git::{catch_up, create_worktree, git, head, is_clean};
use engine::github::{delete_ref, push};

/// Git as a person runs it, with nothing of the engine's settings, and the host's own configuration out of the way: the
/// system and global files of the machine the test runs on are not read, and no question is asked on a terminal.
fn plain(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
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
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .unwrap()
}

/// Plain git that must succeed, with signing off, so that what ran after the programs were named is only the commands
/// under test. Gives what git wrote.
fn setup(dir: &Path, args: &[&str]) -> String {
    let out = plain(dir, &[&["-c", "commit.gpgsign=false"][..], args].concat());
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A bare origin, a clone that moves it on (the other people working on the project) and the project's checkout. The
/// programs the configuration will name are written in `programs`; each one records its own name in `markers` when it
/// starts, and then does what its body says.
struct Fixture {
    root: PathBuf,
    markers: PathBuf,
    programs: PathBuf,
    seed: PathBuf,
    checkout: PathBuf,
}

impl Fixture {
    /// The origin's main holds a file `first` and the given files, a commit for each, when the checkout is cloned.
    fn new(name: &str, files: &[(&str, &str)]) -> Fixture {
        let root = std::env::temp_dir().join(format!("wf-agents-git-programs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (markers, programs) = (root.join("markers"), root.join("programs"));
        std::fs::create_dir_all(&markers).unwrap();
        std::fs::create_dir_all(&programs).unwrap();
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
        fixture.advance("first", "first\n");
        for (file, text) in files {
            fixture.advance(file, text);
        }
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

    /// The origin's main moves on by a commit that writes the file.
    fn advance(&self, file: &str, text: &str) {
        std::fs::write(self.seed.join(file), text).unwrap();
        setup(&self.seed, &["add", "-A"]);
        setup(&self.seed, &["commit", "-q", "-m", file]);
        setup(&self.seed, &["push", "-q", "origin", "HEAD:main"]);
    }

    /// A program: it records `name`, then runs `body` as a shell script. Gives its path, as the configuration names it.
    fn program(&self, name: &str, body: &str) -> String {
        let file = self.programs.join(name);
        let marker = self.markers.join(name);
        std::fs::write(&file, format!("#!/bin/sh\ntouch \"{}\"\n{body}\n", marker.display())).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        file.to_string_lossy().into_owned()
    }

    /// Writes a setting of the checkout's configuration, which every worktree of the checkout shares.
    fn configure(&self, key: &str, value: &str) {
        setup(&self.checkout, &["config", key, value]);
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

    fn finish(self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A commit in a worktree that writes the file, made with plain git.
fn commit_in(dir: &Path, file: &str, text: &str) {
    std::fs::write(dir.join(file), text).unwrap();
    setup(dir, &["add", "-A"]);
    setup(dir, &["commit", "-q", "-m", file]);
}

/// A worktree of the checkout made by plain git, on a branch of its own at the origin's main.
fn control_worktree(fx: &Fixture) -> PathBuf {
    let control = fx.root.join("worktree-control");
    setup(
        &fx.checkout,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "control",
            &control.to_string_lossy(),
            "origin/main",
        ],
    );
    control
}

const NONE: Vec<String> = Vec::new();

// The program named by `core.sshCommand` is the one git starts to reach a remote over ssh. Plain git starts it for a
// push; the engine's push and deletion of a branch start the `ssh` of the machine instead, which finds no such host.
#[test]
fn a_ssh_command_in_the_configuration_is_not_run_by_the_engines_git() {
    let fx = Fixture::new("ssh", &[]);
    let work = fx.root.join("worktree-work");
    create_worktree(&fx.checkout, &work, "work", "main").unwrap();
    let ssh = fx.program("ssh", "exit 1");
    fx.configure("core.sshCommand", &ssh);
    fx.configure("remote.origin.pushurl", "ssh://git.invalid/project.git");

    // The reference: plain git on the same worktree runs the program.
    let control = plain(&work, &["push", "-q", "origin", "HEAD:refs/heads/control"]);
    assert!(
        !control.status.success(),
        "a push to a host that does not exist went through"
    );
    assert_eq!(
        fx.ran(),
        ["ssh"],
        "plain git did not run the ssh command: the fixture does not make it run"
    );

    let sha = head(&work).unwrap();
    assert!(push(&work, &sha, "refs/heads/work", false).is_err());
    assert!(delete_ref(&work, "refs/heads/work").is_err());
    assert_eq!(fx.ran(), NONE, "the engine's git ran the ssh command");
    fx.finish();
}

/// A server that asks for credentials and takes none, on a port of the machine: the answer to every request is
/// `401 Unauthorized`, which makes git ask its credential helpers, and then its askpass program, for a password.
struct Challenge {
    port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Challenge {
    fn start() -> Challenge {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                // The request's head is read to its end, whatever git sends; its body, if any, is not wanted.
                let mut head = Vec::new();
                let mut chunk = [0u8; 4096];
                while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => head.extend_from_slice(&chunk[..n]),
                    }
                }
                let _ = stream.write_all(
                    b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"test\"\r\n\
                      Content-Length: 0\r\nConnection: close\r\n\r\n",
                );
            }
        });
        Challenge {
            port,
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for Challenge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// What the configuration names for git to ask for credentials with.
#[derive(Clone, Copy)]
enum Asker {
    /// `credential.helper`, for every URL.
    Helper,
    /// `credential.<url>.helper`, for the URL of the server only.
    UrlHelper,
    /// `core.askPass`, which git starts to ask for a user name and a password when no helper gives them.
    AskPass,
}

fn engine_never_asks_a_program_for_credentials(name: &str, asker: Asker) {
    let server = Challenge::start();
    let fx = Fixture::new(name, &[]);
    let work = fx.root.join("worktree-work");
    create_worktree(&fx.checkout, &work, "work", "main").unwrap();
    // A helper answers with a user and a password, so that git asks nothing more; askpass answers each question.
    let (marker, key, value) = match asker {
        Asker::Helper => (
            "credential",
            "credential.helper".to_string(),
            fx.program("credential", "cat > /dev/null\nprintf 'username=u\\npassword=p\\n'"),
        ),
        Asker::UrlHelper => (
            "credential",
            format!("credential.http://127.0.0.1:{}.helper", server.port),
            fx.program("credential", "cat > /dev/null\nprintf 'username=u\\npassword=p\\n'"),
        ),
        Asker::AskPass => ("askpass", "core.askPass".to_string(), fx.program("askpass", "printf x")),
    };
    fx.configure(&key, &value);
    fx.configure(
        "remote.origin.pushurl",
        &format!("http://127.0.0.1:{}/project.git", server.port),
    );

    // The reference: plain git pushing to the server runs the program, and the push fails, since the server takes no
    // credentials.
    let control = plain(&work, &["push", "-q", "origin", "HEAD:refs/heads/control"]);
    assert!(!control.status.success(), "the server took the credentials");
    assert_eq!(
        fx.ran(),
        [marker],
        "plain git did not run the program: the fixture does not make it run"
    );

    let sha = head(&work).unwrap();
    assert!(push(&work, &sha, "refs/heads/work", false).is_err());
    assert!(delete_ref(&work, "refs/heads/work").is_err());
    assert_eq!(fx.ran(), NONE, "the engine's git ran a program to ask for credentials");
    fx.finish();
}

// A helper named for every URL by `credential.helper`. It is a list that the engine's git empties, so that the
// helper of the system's configuration (the keychain of the machine) goes too.
#[test]
fn a_credential_helper_in_the_configuration_is_not_run_by_the_engines_git() {
    engine_never_asks_a_program_for_credentials("helper", Asker::Helper);
}

// The same for a helper that the configuration names for one URL: the list that is emptied holds it too.
#[test]
fn a_credential_helper_for_one_url_is_not_run_by_the_engines_git() {
    engine_never_asks_a_program_for_credentials("url-helper", Asker::UrlHelper);
}

// `core.askPass` is read only when `GIT_ASKPASS` is unset, and the engine's git sets it empty.
#[test]
fn an_askpass_program_in_the_configuration_is_not_run_by_the_engines_git() {
    engine_never_asks_a_program_for_credentials("askpass", Asker::AskPass);
}

// `core.fsmonitor` names a program git asks which files changed, on a status, a merge and a checkout. The engine's
// worktree setup, its status and its merge of main run none.
#[test]
fn a_file_system_monitor_in_the_configuration_is_not_run_by_the_engines_git() {
    let fx = Fixture::new("fsmonitor", &[]);
    let monitor = fx.program("fsmonitor", "exit 1");
    fx.configure("core.fsmonitor", &monitor);

    // The reference: plain git asks the program on a status.
    plain(&fx.checkout, &["status", "--short"]);
    assert_eq!(
        fx.ran(),
        ["fsmonitor"],
        "plain git did not run the monitor: the fixture does not make it run"
    );

    let work = fx.root.join("worktree-work");
    create_worktree(&fx.checkout, &work, "work", "main").unwrap();
    assert_eq!(fx.ran(), NONE, "the engine's worktree setup ran the monitor");
    commit_in(&work, "work-own", "work-own\n");
    fx.ran();
    fx.advance("second", "second\n");
    assert!(is_clean(&work).unwrap());
    let caught_up = catch_up(&work, "main").unwrap();
    assert!(caught_up.is_ok(), "the merge of main did not go through: {caught_up:?}");
    assert_eq!(fx.ran(), NONE, "the engine's status or merge ran the monitor");
    fx.finish();
}

// A `filter.<name>` program converts a file as it is checked out or added, and a `merge.<name>.driver` program merges a
// file; both run where a `.gitattributes` file of the branch names them. The names here are `f` and `m`, which nothing
// in the engine knows: the engine's git reads no attributes, so any name is stopped. The engine's worktree setup and
// its merge of a main that changed the same file as the branch run neither, and the merge is a plain conflict.
#[test]
fn a_filter_and_a_merge_driver_in_the_configuration_are_not_run_by_the_engines_git() {
    let fx = Fixture::new(
        "attributes",
        &[(".gitattributes", "*.txt filter=f merge=m\n"), ("shared.txt", "base\n")],
    );
    let filter = fx.program("filter", "exec cat");
    let driver = fx.program("driver", "exit 1");
    fx.configure("filter.f.clean", &filter);
    fx.configure("filter.f.smudge", &filter);
    fx.configure("merge.m.driver", &format!("{driver} %O %A %B"));

    // The reference: plain git runs the filter on a checkout and on an add.
    let control = control_worktree(&fx);
    commit_in(&control, "shared.txt", "control\n");
    assert_eq!(
        fx.ran(),
        ["filter"],
        "plain git did not run the filter: the fixture does not make it run"
    );

    // The engine's worktree setup checks the files out without it.
    let work = fx.root.join("worktree-work");
    create_worktree(&fx.checkout, &work, "work", "main").unwrap();
    assert_eq!(fx.ran(), NONE, "the engine's worktree setup ran the filter");

    // Main and both worktrees' branches changed the same file.
    commit_in(&work, "shared.txt", "work\n");
    fx.advance("shared.txt", "main\n");
    fx.ran();

    // The reference again: plain git merges main into the control's branch by the driver.
    plain(
        &control,
        &["fetch", "-q", "origin", "+refs/heads/main:refs/remotes/origin/main"],
    );
    plain(&control, &["merge", "--no-edit", "-q", "origin/main"]);
    let ran = fx.ran();
    assert!(
        ran.iter().any(|r| r == "driver"),
        "plain git did not run the merge driver: the fixture does not make it run (ran {ran:?})"
    );

    // The engine's merge of main into the work's branch is a plain merge of text: the file is in conflict, and no
    // program ran.
    let caught_up = catch_up(&work, "main").unwrap();
    assert_eq!(caught_up, Err(vec!["shared.txt".to_string()]));
    assert_eq!(fx.ran(), NONE, "the engine's merge ran the filter or the driver");
    fx.finish();
}

// `commit.gpgsign` makes git start `gpg.program` for each commit it makes, a merge's too. The engine's merge of main
// and its revert make their commits unsigned, and the program does not run.
#[test]
fn a_signing_program_in_the_configuration_is_not_run_by_the_engines_git() {
    let fx = Fixture::new("signing", &[]);
    let work = fx.root.join("worktree-work");
    create_worktree(&fx.checkout, &work, "work", "main").unwrap();
    let control = control_worktree(&fx);
    commit_in(&work, "work-own", "work-own\n");
    commit_in(&control, "control-own", "control-own\n");
    let signer = fx.program("gpg", "exit 1");
    fx.configure("commit.gpgsign", "true");
    fx.configure("gpg.program", &signer);
    fx.advance("second", "second\n");

    // The reference: plain git signs the commit of the merge, which fails, since the program signs nothing.
    plain(
        &control,
        &["fetch", "-q", "origin", "+refs/heads/main:refs/remotes/origin/main"],
    );
    let merge = plain(&control, &["merge", "--no-edit", "-q", "origin/main"]);
    assert!(!merge.status.success(), "plain git made the commit of the merge");
    assert_eq!(
        fx.ran(),
        ["gpg"],
        "plain git did not run the program: the fixture does not make it run"
    );

    let caught_up = catch_up(&work, "main").unwrap();
    assert!(caught_up.is_ok(), "the merge of main did not go through: {caught_up:?}");
    let commit = setup(&work, &["rev-list", "--parents", "-n", "1", "HEAD"]);
    assert_eq!(
        commit.split_whitespace().count(),
        3,
        "the engine's merge made no merge commit: {commit}"
    );
    git(
        &work,
        &[
            "-c",
            "user.name=wf-agents",
            "-c",
            "user.email=wf-agents@localhost",
            "revert",
            "--no-edit",
            "HEAD^",
        ],
    )
    .unwrap();
    assert_eq!(fx.ran(), NONE, "the engine's commits ran the signing program");
    fx.finish();
}

// `remote.<name>.uploadpack` and `receivepack` name the program git starts to serve a fetch and to take a push, which
// for a remote that is a path on the machine it starts here. Git takes the first one it reads, so a setting on the
// engine's command line does not replace the configuration's: the engine's fetch and push name the programs
// themselves.
#[test]
fn the_programs_of_a_remote_in_the_configuration_are_not_run_by_the_engines_git() {
    let fx = Fixture::new("remote-programs", &[]);
    let upload = fx.program("uploadpack", "exec git-upload-pack \"$@\"");
    let receive = fx.program("receivepack", "exec git-receive-pack \"$@\"");
    fx.configure("remote.origin.uploadpack", &upload);
    fx.configure("remote.origin.receivepack", &receive);

    // The reference: plain git runs the first for a fetch and the second for a push.
    setup(&fx.checkout, &["fetch", "-q", "origin"]);
    setup(&fx.checkout, &["push", "-q", "origin", "HEAD:refs/heads/control"]);
    assert_eq!(
        fx.ran(),
        ["receivepack", "uploadpack"],
        "plain git did not run the programs: the fixture does not make them run"
    );

    let work = fx.root.join("worktree-work");
    create_worktree(&fx.checkout, &work, "work", "main").unwrap();
    commit_in(&work, "work-own", "work-own\n");
    fx.advance("second", "second\n");
    let caught_up = catch_up(&work, "main").unwrap();
    assert!(caught_up.is_ok(), "the merge of main did not go through: {caught_up:?}");
    let sha = head(&work).unwrap();
    push(&work, &sha, "refs/heads/work", false).unwrap();
    delete_ref(&work, "refs/heads/work").unwrap();
    assert_eq!(fx.ran(), NONE, "the engine's fetch or push ran a program of the remote");
    fx.finish();
}

// `core.gitProxy` names the program git starts to reach a `git://` remote. Git takes the first one it reads, as for the
// programs of a remote, so the engine's git sets `GIT_PROXY_COMMAND` empty, which outranks the configuration.
#[test]
fn a_git_proxy_in_the_configuration_is_not_run_by_the_engines_git() {
    let fx = Fixture::new("proxy", &[]);
    let proxy = fx.program("gitproxy", "exit 1");
    // A port nobody listens on: the connection is refused, as the fetch is meant to fail.
    let closed = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    fx.configure("core.gitProxy", &proxy);
    fx.configure("remote.origin.url", &format!("git://127.0.0.1:{closed}/project.git"));

    // The reference: plain git runs the proxy program to reach the remote.
    let control = plain(&fx.checkout, &["fetch", "-q", "origin"]);
    assert!(
        !control.status.success(),
        "a fetch from a port nobody listens on went through"
    );
    assert_eq!(
        fx.ran(),
        ["gitproxy"],
        "plain git did not run the proxy: the fixture does not make it run"
    );

    let refspec = config::main_refspec("main");
    assert!(git(&fx.checkout, &["fetch", "-q", "origin", &refspec]).is_err());
    assert_eq!(fx.ran(), NONE, "the engine's git ran the proxy program");
    fx.finish();
}

// The same `core.sshCommand` for a fetch, which the test of the push does not reach: the engine's `catch_up` and its
// fetch of main start the `ssh` of the machine, which finds nothing at a closed port, not the configured program.
#[test]
fn a_ssh_command_in_the_configuration_is_not_run_by_the_engines_fetch() {
    let fx = Fixture::new("ssh-fetch", &[]);
    let work = fx.root.join("worktree-work");
    create_worktree(&fx.checkout, &work, "work", "main").unwrap();
    let ssh = fx.program("ssh", "exit 1");
    fx.configure("core.sshCommand", &ssh);
    fx.configure("remote.origin.url", "ssh://127.0.0.1:1/project.git");

    // The reference: plain git fetching over ssh runs the program.
    let control = plain(&work, &["fetch", "-q", "origin"]);
    assert!(!control.status.success(), "a fetch over ssh went through");
    assert_eq!(
        fx.ran(),
        ["ssh"],
        "plain git did not run the ssh command: the fixture does not make it run"
    );

    assert!(catch_up(&work, "main").is_err());
    assert!(git(&work, &["fetch", "-q", "origin", &config::main_refspec("main")]).is_err());
    assert_eq!(fx.ran(), NONE, "the engine's fetch ran the ssh command");
    fx.finish();
}

// A remote whose URL is `ext::<command>` makes git run the command as its transport, once `protocol.ext.allow` lets it:
// git does not allow `ext` by default, and a configuration that allows it and points a remote at a program is a way to
// run a program on the engine's `fetch` and `push` that no setting of the program itself shows. The URL here is the
// remote's own, as an agent that writes the shared configuration writes it.
#[test]
fn an_ext_remote_in_the_configuration_is_not_run_by_the_engines_git() {
    let fx = Fixture::new("ext-remote", &[]);
    let work = fx.root.join("worktree-work");
    create_worktree(&fx.checkout, &work, "work", "main").unwrap();
    let transport = fx.program("ext", "exit 1");
    fx.configure("protocol.ext.allow", "always");
    fx.configure("remote.origin.url", &format!("ext::{transport}"));
    fx.configure("remote.origin.pushurl", &format!("ext::{transport}"));

    // The reference: plain git runs the program to fetch, and to push.
    let control = plain(&work, &["fetch", "-q", "origin"]);
    assert!(
        !control.status.success(),
        "a fetch through a program that fails went through"
    );
    assert_eq!(
        fx.ran(),
        ["ext"],
        "plain git did not run the ext program: the fixture does not make it run"
    );

    assert!(catch_up(&work, "main").is_err());
    let sha = head(&work).unwrap();
    assert!(push(&work, &sha, "refs/heads/work", false).is_err());
    assert!(delete_ref(&work, "refs/heads/work").is_err());
    assert_eq!(fx.ran(), NONE, "the engine's git ran the program of an ext remote");
    fx.finish();
}

// `push.gpgSign` makes git sign the push itself, with `gpg.program`, when the receiving end offers to take a signed push
// (`receive.certNonceSeed`). It is not `commit.gpgSign`, and the engine's push must not start the program either. The
// engine's push is a plain push that goes through.
#[test]
fn a_push_signing_program_in_the_configuration_is_not_run_by_the_engines_git() {
    let fx = Fixture::new("push-signing", &[]);
    let work = fx.root.join("worktree-work");
    create_worktree(&fx.checkout, &work, "work", "main").unwrap();
    setup(
        &fx.root.join("origin.git"),
        &["config", "receive.certNonceSeed", "seed"],
    );
    let signer = fx.program("gpg", "exit 1");
    fx.configure("push.gpgSign", "true");
    fx.configure("gpg.program", &signer);
    // An identity for the push certificate, so that what stops a push that signs is the program, not a missing name.
    fx.configure("user.name", "t");
    fx.configure("user.email", "t@t");

    // The reference: plain git signs the push, which fails, since the program signs nothing.
    let control = plain(&work, &["push", "-q", "origin", "HEAD:refs/heads/control"]);
    assert!(!control.status.success(), "plain git pushed without a signature");
    assert_eq!(
        fx.ran(),
        ["gpg"],
        "plain git did not run the program: the fixture does not make it run"
    );

    let sha = head(&work).unwrap();
    let pushed = push(&work, &sha, "refs/heads/work", false);
    let deleted = delete_ref(&work, "refs/heads/work");
    assert_eq!(fx.ran(), NONE, "the engine's push ran the signing program");
    pushed.unwrap();
    deleted.unwrap();
    fx.finish();
}

// `diff.external` names a program that `git diff` runs in place of its own diff, for every file that changed; the
// attributes do not come into it, so the engine's reading of no attributes does not stop it. The engine's `diff` is the
// text the cells read, and it has to be git's diff.
#[test]
fn an_external_diff_in_the_configuration_is_not_run_by_the_engines_git() {
    let fx = Fixture::new("external-diff", &[]);
    let work = fx.root.join("worktree-work");
    create_worktree(&fx.checkout, &work, "work", "main").unwrap();
    let (from, to) = {
        let from = head(&work).unwrap();
        commit_in(&work, "first", "changed\n");
        (from, head(&work).unwrap())
    };
    let external = fx.program("diff", "exit 0");
    fx.configure("diff.external", &external);

    // The reference: plain git runs the program for the changed file.
    plain(&work, &["diff", &format!("{from}..{to}")]);
    assert_eq!(
        fx.ran(),
        ["diff"],
        "plain git did not run the external diff: the fixture does not make it run"
    );

    let text = engine::git::diff(&work, &from, &to, &[]).unwrap();
    let of_paths = engine::git::diff(&work, &from, &to, &["first".to_string()]).unwrap();
    assert_eq!(fx.ran(), NONE, "the engine's diff ran the external diff");
    assert!(text.contains("+changed"), "the engine's diff is not git's: {text:?}");
    assert!(
        of_paths.contains("+changed"),
        "the engine's diff is not git's: {of_paths:?}"
    );
    fx.finish();
}

/// The commit `HEAD` of the worktree, made again with a signature header that no key made, so that git, asked to check
/// signatures, starts `gpg.program` for it. `HEAD` of the worktree is moved to it.
fn sign_head(fx: &Fixture, dir: &Path, name: &str) {
    let text = setup(dir, &["cat-file", "commit", "HEAD"]);
    let signature = "gpgsig -----BEGIN PGP SIGNATURE-----\n \n fake\n -----END PGP SIGNATURE-----";
    let signed = text.replacen("\n\n", &format!("\n{signature}\n\n"), 1) + "\n";
    let file = fx.root.join(format!("signed-{name}"));
    std::fs::write(&file, signed).unwrap();
    let id = setup(dir, &["hash-object", "-t", "commit", "-w", &file.to_string_lossy()]);
    setup(dir, &["reset", "-q", "--hard", &id]);
}

// `log.showSignature` makes `git log` check each signed commit with `gpg.program` and print the result among the log;
// `merge.verifySignatures` makes `git merge` check the commit it takes. The engine's reading of the branch's log and its
// merge of main run neither, and the log is the commits and nothing else.
#[test]
fn a_signature_check_in_the_configuration_is_not_run_by_the_engines_git() {
    let fx = Fixture::new("verify", &[]);
    let work = fx.root.join("worktree-work");
    create_worktree(&fx.checkout, &work, "work", "main").unwrap();
    let control = control_worktree(&fx);
    let base = head(&work).unwrap();
    commit_in(&work, "work-own", "work-own\n");
    sign_head(&fx, &work, "work");
    commit_in(&control, "control-own", "control-own\n");
    sign_head(&fx, &control, "control");
    // Main moves on by a signed commit.
    fx.advance("second", "second\n");
    std::fs::write(fx.seed.join("third"), "third\n").unwrap();
    setup(&fx.seed, &["add", "-A"]);
    setup(&fx.seed, &["commit", "-q", "-m", "third"]);
    sign_head(&fx, &fx.seed, "main");
    setup(&fx.seed, &["push", "-q", "--force", "origin", "HEAD:main"]);
    fx.ran();

    let checker = fx.program("gpg", "exit 1");
    fx.configure("gpg.program", &checker);
    fx.configure("log.showSignature", "true");
    fx.configure("merge.verifySignatures", "true");

    // The reference: plain git checks the signature of the commit in the log, and of the commit it merges.
    plain(&control, &["log", "--format=%H", "HEAD~1..HEAD"]);
    assert_eq!(
        fx.ran(),
        ["gpg"],
        "plain git did not check the signature in a log: the fixture does not make it"
    );
    plain(
        &control,
        &["fetch", "-q", "origin", "+refs/heads/main:refs/remotes/origin/main"],
    );
    let merge = plain(&control, &["merge", "--no-edit", "-q", "origin/main"]);
    assert!(
        !merge.status.success(),
        "plain git merged a commit whose signature fails"
    );
    assert_eq!(
        fx.ran(),
        ["gpg"],
        "plain git did not check the signature of a merge: the fixture does not make it"
    );

    let log = engine::git::log_since(&work, &base, "HEAD").unwrap();
    assert_eq!(fx.ran(), NONE, "the engine's log ran the signature check");
    assert_eq!(
        log.commits.iter().map(|c| c.subject.as_str()).collect::<Vec<_>>(),
        ["work-own"],
        "the engine's log is not the commits"
    );
    let caught_up = catch_up(&work, "main").unwrap();
    assert!(caught_up.is_ok(), "the merge of main did not go through: {caught_up:?}");
    assert_eq!(fx.ran(), NONE, "the engine's merge ran the signature check");
    fx.finish();
}
