//! An agent's git cannot push, however the push is written: the environment a round's command line is given
//! (`carry::cli::environment`) with what the flows give every agent's program (`agents/refused.toml`) makes git refuse
//! every transport, which no spelling of the command and no `-c` changes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use service::Token;

use crate::{beside, git_in};

/// The environment of a command of the agents' program, given the flows' variables or not, with a `home` of its own so
/// that no configuration of whoever runs the test is read.
fn environment(home: &Path, refused: &BTreeMap<String, String>) -> Vec<(String, String)> {
    let settings = carry::Settings::read(include_str!("../../install.sample.toml")).expect("the sample reads");
    let engine = [
        ("PATH".to_string(), std::env::var("PATH").unwrap_or_default()),
        ("HOME".to_string(), home.display().to_string()),
    ];
    carry::cli::environment(
        &settings.agents,
        &home.join("profile"),
        engine,
        &BTreeMap::new(),
        refused,
        &Token::new("sk-ant-oat01-secret".to_string()),
    )
}

/// The path of the git that `PATH` finds first.
fn git_path() -> PathBuf {
    let path = std::env::var("PATH").expect("PATH");
    (std::env::split_paths(&path))
        .map(|dir| dir.join("git"))
        .find(|git| git.is_file())
        .expect("git is on the PATH")
}

/// The refs the remote holds, one per line, read as the test itself, with no limit on git.
fn refs(remote: &Path) -> Vec<String> {
    git_in(remote, &["for-each-ref", "--format=%(refname)"])
        .lines()
        .map(str::to_string)
        .collect()
}

/// Each way of writing a push of `HEAD` to the new branch `{branch}` of the remote that the work repository `{repo}`
/// names `origin`, as a shell command line run in it.
///
/// Each comes with whether it pushes where git is given no variable: an empty `GIT_ALLOW_PROTOCOL` is a value, which
/// git takes as allowing no protocol, so it refuses there too.
fn spellings() -> Vec<(&'static str, String, bool)> {
    let push = "push origin HEAD:refs/heads/{branch}";
    let git = git_path().display().to_string();
    vec![
        ("plain", format!("git {push}"), true),
        ("git -C", format!("git -C {{repo}} {push}"), true),
        ("an environment prefix", format!("env X=1 git {push}"), true),
        ("the path of git", format!("{git} {push}"), true),
        ("a shell", format!("sh -c 'git {push}'"), true),
        ("a script", "sh {repo}/../push.sh".to_string(), true),
        (
            "git -c protocol allowed",
            format!("git -c protocol.allow=always -c protocol.file.allow=always {push}"),
            true,
        ),
        ("--no-verify", format!("git {push} --no-verify"), true),
        ("the variable empty", format!("GIT_ALLOW_PROTOCOL= git {push}"), false),
    ]
}

/// Each spelling run in the work repository `repo` with `environment`: whether it ended well, and what it said.
fn run(repo: &Path, environment: &[(String, String)], branch: &str, line: &str) -> Output {
    let line = line
        .replace("{repo}", &repo.display().to_string())
        .replace("{branch}", branch);
    Command::new("sh")
        .args(["-c", &line])
        .env_clear()
        .envs(environment.iter().cloned())
        .current_dir(repo)
        .output()
        .expect("sh runs")
}

// A push is refused whichever way it is written, and not refused where the flows' variables are left out: each spelling
// (plain, `git -C`, an environment prefix, the path of git, a shell, a script, `-c` that allows every protocol,
// `--no-verify`, the variable set empty) runs the real git against a bare repository in the environment the carrier
// gives a round, and ends with an error and leaves no new ref; the same spelling in an environment without the flows'
// variables pushes, which shows the test would see a push. About a dozen git processes, tens of milliseconds; the one
// test that a push is refused whatever its spelling, which the permission rule that matches a command's start cannot
// show.
#[test]
fn a_push_written_any_way_is_refused_in_the_environment_a_round_is_given() {
    let dir = tempfile::tempdir().unwrap();
    let (remote, repo, home) = (
        dir.path().join("remote.git"),
        dir.path().join("work"),
        dir.path().join("home"),
    );
    std::fs::create_dir_all(&home).unwrap();
    git_in(dir.path(), &["init", "-q", "--bare", "remote.git"]);
    git_in(dir.path(), &["init", "-q", "work"]);
    git_in(&repo, &["remote", "add", "origin", &remote.display().to_string()]);
    git_in(
        &repo,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "One",
        ],
    );

    let flows = beside().refused.environment;
    assert_eq!(
        flows,
        BTreeMap::from([("GIT_ALLOW_PROTOCOL".to_string(), "none".to_string())]),
        "the flows give the agents' program the variable that stops git's transports"
    );
    let (stopped, free) = (environment(&home, &flows), environment(&home, &BTreeMap::new()));
    let spellings = spellings();
    for (n, (what, line, pushes_freely)) in spellings.iter().enumerate() {
        // The script names its branch as the test's own, which the line has no way to give it.
        let script = |branch: &str| {
            std::fs::write(
                dir.path().join("push.sh"),
                format!("git push origin HEAD:refs/heads/{branch}\n"),
            )
            .unwrap();
        };
        let branch = format!("refused-{n}");
        script(&branch);
        let out = run(&repo, &stopped, &branch, line);
        assert!(
            !out.status.success(),
            "{what}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            !refs(&remote).contains(&format!("refs/heads/{branch}")),
            "{what}: a ref came"
        );

        if !pushes_freely {
            continue;
        }
        let branch = format!("allowed-{n}");
        script(&branch);
        let out = run(&repo, &free, &branch, line);
        assert!(
            out.status.success(),
            "{what} without the variable: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            refs(&remote).contains(&format!("refs/heads/{branch}")),
            "{what}: no ref came"
        );
    }
}

/// A work repository `work` with a commit and a bare `remote.git` it names `origin`, and a linked worktree `linked` of
/// it, as the agents' own worktrees are, in `dir`; the `home` the environments use is `dir/home`.
fn repositories(dir: &Path) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let (remote, repo, linked, home) = (
        dir.join("remote.git"),
        dir.join("work"),
        dir.join("linked"),
        dir.join("home"),
    );
    std::fs::create_dir_all(&home).unwrap();
    git_in(dir, &["init", "-q", "--bare", "remote.git"]);
    git_in(dir, &["init", "-q", "work"]);
    git_in(&repo, &["remote", "add", "origin", &remote.display().to_string()]);
    git_in(
        &repo,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "One",
        ],
    );
    git_in(
        &repo,
        &["worktree", "add", "-q", "-b", "agent", &linked.display().to_string()],
    );
    (remote, repo, linked, home)
}

/// A shell command line run in `at` with `environment`, its `{remote}` the bare repository's path and its `{branch}` the
/// branch to push to.
fn run_at(at: &Path, remote: &Path, environment: &[(String, String)], branch: &str, line: &str) -> Output {
    let line = line
        .replace("{remote}", &remote.display().to_string())
        .replace("{branch}", branch);
    Command::new("sh")
        .args(["-c", &line])
        .env_clear()
        .envs(environment.iter().cloned())
        .current_dir(at)
        .output()
        .expect("sh runs")
}

// Git's other ways to push are refused as `git push origin` is, in the environment a round is given: the plumbing
// command that pushes (`send-pack`), a push from the linked worktree an agent works in, an alias, the receive-pack
// option, a push URL set with `-c`, and the remote written as a path or a `file://` URL instead of its name. The
// reference is git's own documentation of `GIT_ALLOW_PROTOCOL` (git(1)): it behaves as if `protocol.allow` is `never`
// and only the listed protocols are `always`, overriding configuration, so no command that makes a connection may
// get past it; each also pushes where the variable is left out, so that the test would see a push. About fifteen git
// processes, tens of milliseconds; the spellings of `a_push_written_any_way_...` do not use these commands.
#[test]
fn the_other_commands_and_places_that_push_are_refused_in_the_environment_a_round_is_given() {
    let dir = tempfile::tempdir().unwrap();
    let (remote, repo, linked, home) = repositories(dir.path());
    let flows = beside().refused.environment;
    let (stopped, free) = (environment(&home, &flows), environment(&home, &BTreeMap::new()));
    let push = "HEAD:refs/heads/{branch}";
    let cases: Vec<(&str, &Path, String)> = vec![
        ("send-pack", &repo, format!("git send-pack {{remote}} {push}")),
        ("a linked worktree", &linked, format!("git push origin {push}")),
        ("an alias", &repo, format!("git -c alias.p='push origin {push}' p")),
        (
            "the receive-pack option",
            &repo,
            format!("git push --receive-pack=git-receive-pack origin {push}"),
        ),
        (
            "a push URL",
            &repo,
            format!("git -c remote.origin.pushurl={{remote}} push origin {push}"),
        ),
        ("a path", &repo, format!("git push {{remote}} {push}")),
        ("a file URL", &repo, format!("git push file://{{remote}} {push}")),
        (
            "a path from the linked worktree by git -C",
            &linked,
            format!("git -C {} push {{remote}} {push}", repo.display()),
        ),
    ];
    for (n, (what, at, line)) in cases.iter().enumerate() {
        let branch = format!("refused-{n}");
        let out = run_at(at, &remote, &stopped, &branch, line);
        assert!(!out.status.success(), "{what}: pushed");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("not allowed"),
            "{what}: it failed, but not because the transport is refused: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            !refs(&remote).contains(&format!("refs/heads/{branch}")),
            "{what}: a ref came"
        );
        let branch = format!("allowed-{n}");
        let out = run_at(at, &remote, &free, &branch, line);
        assert!(
            out.status.success(),
            "{what} without the variable: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            refs(&remote).contains(&format!("refs/heads/{branch}")),
            "{what}: no ref came"
        );
    }
}

// A push to a URL of any other transport is refused by git for its transport, before it connects, not by a connection
// that happens to fail: for each of ssh (as a URL and as `user@host:path`), https, http and git, git says that the
// transport is not allowed, and where the variable is left out it goes on to try (to a port nothing listens on, with
// `false` as its ssh) and fails some other way. The reference is git's documentation of `GIT_ALLOW_PROTOCOL` (git(1)).
// Five failing connections to a closed local port, tens of milliseconds; it is the only test that the transports
// other than the local one are refused by git itself.
#[test]
fn a_push_to_a_url_of_any_transport_is_refused_by_git_before_it_connects() {
    let dir = tempfile::tempdir().unwrap();
    let (remote, repo, _linked, home) = repositories(dir.path());
    let flows = beside().refused.environment;
    let stopped = environment(&home, &flows);
    let mut free = environment(&home, &BTreeMap::new());
    free.push(("GIT_SSH_COMMAND".to_string(), "false".to_string()));
    free.push(("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()));
    for (transport, url) in [
        ("ssh", "ssh://git@127.0.0.1:9/r.git"),
        ("ssh", "git@127.0.0.1:r.git"),
        ("https", "https://127.0.0.1:9/r.git"),
        ("http", "http://127.0.0.1:9/r.git"),
        ("git", "git://127.0.0.1:9/r.git"),
    ] {
        let line = format!("git push {url} HEAD:refs/heads/x");
        let out = run_at(&repo, &remote, &stopped, "x", &line);
        let said = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(!out.status.success(), "{url}: pushed");
        assert!(
            said.contains(&format!("transport '{transport}' not allowed")),
            "{url}: {said}"
        );
        let out = run_at(&repo, &remote, &free, "x", &line);
        let said = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(!said.contains("not allowed"), "{url} without the variable: {said}");
    }
}
