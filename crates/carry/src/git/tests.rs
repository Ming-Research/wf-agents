//! The acts' git against a repository made for each test, with a file committed on its main branch.

use std::path::{Path, PathBuf};

use store::WorkNumber;

use super::*;

/// A repository with one commit on `main`, a data directory beside it, and the project they make.
pub(crate) struct Repo {
    _dir: tempfile::TempDir,
    pub project: Project,
    pub data: PathBuf,
}

/// Runs git in the directory, which must pass, and gives its output.
pub(crate) fn run(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim_end().to_string()
}

/// Writes the file in the directory and commits it there.
pub(crate) fn commit(dir: &Path, file: &str, text: &str) -> String {
    std::fs::write(dir.join(file), text).expect("the file is written");
    run(dir, &["add", file]);
    run(dir, &["commit", "-q", "-m", &format!("Write {file}")]);
    run(dir, &["rev-parse", "HEAD"])
}

/// The installation's tag the tests' places are named with.
pub(crate) const TAG: &str = "0a1b2c3d";

impl Repo {
    /// The work's place in this repository's data directory.
    pub(crate) fn place(&self, work: WorkNumber) -> Place {
        place(&self.data, TAG, work)
    }

    /// A new repository, whose check is this command.
    pub(crate) fn new(check: &str) -> Repo {
        let dir = tempfile::tempdir().expect("a directory");
        let repo = dir.path().join("repo");
        let data = dir.path().join("data");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        run(&repo, &["init", "-q", "-b", "main"]);
        run(&repo, &["config", "user.name", "Tester"]);
        run(&repo, &["config", "user.email", "tester@example.com"]);
        run(&repo, &["config", "commit.gpgsign", "false"]);
        commit(&repo, "greeting.txt", "Hello.\n");
        Repo {
            _dir: dir,
            project: Project {
                repo,
                main: "main".to_string(),
                check: check.to_string(),
                allowed_config: Vec::new(),
            },
            data,
        }
    }
}

const WORK: WorkNumber = WorkNumber(1);

#[test]
fn a_place_is_named_by_the_work_and_the_installation() {
    let place = place(Path::new("/data"), TAG, WORK);
    assert_eq!(place.branch, "work-1-0a1b2c3d");
    assert_eq!(place.reference(), "refs/heads/work-1-0a1b2c3d");
    assert_eq!(place.dir, Path::new("/data/worktrees/1-0a1b2c3d"));
    let installation = Uuid::parse_str("6a1f3c2e-8d4b-4e7a-9c5f-2b8e1d0a7f43").unwrap();
    assert_eq!(tag(&installation), "6a1f3c2e");
}

#[tokio::test]
async fn start_twice_gives_one_worktree() {
    let repo = Repo::new("true");
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let first = start(&repo.project, &repo.place(WORK), "main")
        .await
        .expect("the work starts");
    assert_eq!(first.place, repo.place(WORK));
    assert_eq!(first.from, main);
    assert!(first.place.dir.join("greeting.txt").is_file());
    let again = start(&repo.project, &repo.place(WORK), "main")
        .await
        .expect("the work starts again");
    assert_eq!(again, first);
    let worktrees = run(&repo.project.repo, &["worktree", "list", "--porcelain"]);
    assert_eq!(worktrees.lines().filter(|l| l.starts_with("worktree ")).count(), 2);
}

#[tokio::test]
async fn the_merge_puts_the_checked_commit_on_main_once() {
    let repo = Repo::new("true");
    let started = start(&repo.project, &repo.place(WORK), "main").await.unwrap();
    let head = commit(&started.place.dir, "farewell.txt", "Bye.\n");
    let merged = merge(&repo.project, &repo.place(WORK), Some(&head))
        .await
        .expect("the work merges");
    assert_eq!(merged, run(&repo.project.repo, &["rev-parse", "main"]));
    let parents = run(&repo.project.repo, &["rev-list", "--parents", "-n", "1", "main"]);
    assert_eq!(
        parents.split(' ').nth(2),
        Some(head.as_str()),
        "a merge commit of the work's head"
    );
    assert!(repo.project.repo.join("farewell.txt").is_file());
    let again = merge(&repo.project, &repo.place(WORK), Some(&head))
        .await
        .expect("merged again");
    assert_eq!(again, merged);
}

#[tokio::test]
async fn the_merge_refuses_a_branch_moved_on_from_what_was_checked() {
    let repo = Repo::new("true");
    let started = start(&repo.project, &repo.place(WORK), "main").await.unwrap();
    let checked = commit(&started.place.dir, "farewell.txt", "Bye.\n");
    commit(&started.place.dir, "later.txt", "Later.\n");
    let refused = merge(&repo.project, &repo.place(WORK), Some(&checked))
        .await
        .expect_err("refused");
    assert!(refused.contains("the commit the check passed"), "{refused}");
    assert!(!repo.project.repo.join("farewell.txt").exists(), "nothing was merged");
}

#[tokio::test]
async fn the_merge_runs_no_hook_of_the_repository() {
    let repo = Repo::new("true");
    let started = start(&repo.project, &repo.place(WORK), "main").await.unwrap();
    let head = commit(&started.place.dir, "farewell.txt", "Bye.\n");
    let hooks = repo.project.repo.join(".git/hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("pre-merge-commit");
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    merge(&repo.project, &repo.place(WORK), Some(&head))
        .await
        .expect("the hook is not run");
}

#[tokio::test]
async fn the_merge_needs_the_checkout_on_main() {
    let repo = Repo::new("true");
    start(&repo.project, &repo.place(WORK), "main").await.unwrap();
    run(&repo.project.repo, &["switch", "-q", "-c", "elsewhere"]);
    let refused = merge(&repo.project, &repo.place(WORK), None)
        .await
        .expect_err("the merge is refused");
    assert!(refused.contains("not on the main branch"), "{refused}");
}

/// Writes a program at the path that leaves a mark at `marked` when it is run.
pub(crate) fn marking(path: &Path, marked: &Path) {
    std::fs::write(path, format!("#!/bin/sh\ntouch '{}'\n", marked.display())).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[tokio::test]
async fn the_start_runs_no_hook_of_the_repository() {
    let repo = Repo::new("true");
    let hooks = repo.project.repo.join(".git/hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let marked = repo.data.join("hook-ran");
    marking(&hooks.join("post-checkout"), &marked);
    start(&repo.project, &repo.place(WORK), "main")
        .await
        .expect("the work starts");
    assert!(!marked.exists(), "the repository's hook was run");
}

#[tokio::test]
async fn the_merge_refuses_a_repository_whose_configuration_names_a_program() {
    for (key, value) in [
        ("core.fsmonitor", "/bin/true"),
        ("core.hooksPath", "hooks"),
        ("merge.ours.driver", "true"),
        ("filter.lfs.clean", "cat"),
    ] {
        let repo = Repo::new("true");
        let started = start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        let head = commit(&started.place.dir, "farewell.txt", "Bye.\n");
        run(&repo.project.repo, &["config", key, value]);
        let refused = merge(&repo.project, &repo.place(WORK), Some(&head))
            .await
            .expect_err(key);
        assert!(refused.contains(&key.to_ascii_lowercase()), "{key}: {refused}");
        assert!(
            !repo.project.repo.join("farewell.txt").exists(),
            "{key}: nothing was merged"
        );
    }
}

#[tokio::test]
async fn the_start_refuses_a_repository_whose_local_configuration_could_run_a_program() {
    let repo = Repo::new("true");
    run(&repo.project.repo, &["config", "core.pager", "less"]);
    let refused = start(&repo.project, &repo.place(WORK), "main")
        .await
        .expect_err("refused");
    assert!(refused.contains("core.pager (local)"), "{refused}");
    assert!(refused.contains("allowed_config"), "{refused}");
    assert!(!repo.place(WORK).dir.exists(), "nothing was made");
}

#[tokio::test]
async fn what_an_included_file_names_is_refused() {
    let mut repo = Repo::new("true");
    let included = repo.data.join("included.config");
    std::fs::write(&included, "[diff \"word\"]\n\ttextconv = cat\n").unwrap();
    run(
        &repo.project.repo,
        &["config", "include.path", &included.display().to_string()],
    );
    let refused = start(&repo.project, &repo.place(WORK), "main")
        .await
        .expect_err("refused");
    assert!(refused.contains("include.path"), "{refused}");
    repo.project.allowed_config = vec!["include.path".to_string()];
    let refused = start(&repo.project, &repo.place(WORK), "main")
        .await
        .expect_err("refused");
    assert!(refused.contains("diff.word.textconv"), "{refused}");
}

#[tokio::test]
async fn an_allowed_key_lets_the_work_start_and_merge() {
    let mut repo = Repo::new("true");
    run(&repo.project.repo, &["config", "filter.lfs.clean", "cat"]);
    run(&repo.project.repo, &["config", "filter.lfs.smudge", "cat"]);
    repo.project.allowed_config = vec!["filter.lfs.*".to_string()];
    let started = start(&repo.project, &repo.place(WORK), "main").await.expect("allowed");
    let head = commit(&started.place.dir, "farewell.txt", "Bye.\n");
    merge(&repo.project, &repo.place(WORK), Some(&head))
        .await
        .expect("allowed");
}

#[test]
fn every_key_that_could_run_a_program_is_named() {
    for key in [
        "core.fsmonitor",
        "core.hookspath",
        "filter.x.clean",
        "merge.x.driver",
        "diff.x.textconv",
        "diff.x.command",
        "diff.external",
        "gpg.program",
        "gpg.ssh.program",
        "include.path",
        "includeif.gitdir:/x.path",
        "extensions.worktreeconfig",
        "core.sshcommand",
        "core.gitproxy",
        "credential.helper",
        "core.editor",
        "sequence.editor",
        "core.pager",
    ] {
        assert!(runs_code(key), "{key}");
    }
    for key in ["user.name", "core.autocrlf", "merge.conflictstyle", "diff.renames"] {
        assert!(!runs_code(key), "{key}");
    }
    let patterns = ["filter.lfs.*".to_string(), "core.pager".to_string()];
    assert!(allowed(&patterns, "filter.lfs.clean"));
    assert!(allowed(&patterns, "core.pager"));
    assert!(!allowed(&patterns, "filter.crypt.clean"));
}
