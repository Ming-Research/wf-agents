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
