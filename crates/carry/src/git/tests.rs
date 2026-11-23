//! The acts' git against a repository made for each test, with a file committed on its main branch.

use std::path::{Path, PathBuf};

use store::WorkNumber;

use super::*;

/// Why a merge was refused, where it was refused for another cause than a moved main.
fn failed(refused: NotMerged) -> String {
    match refused {
        NotMerged::Failed(failed) => failed.why,
        moved => panic!("refused for a moved main: {moved:?}"),
    }
}

/// A repository with one commit on `main`, a data directory beside it, and the project they make.
pub(crate) struct Repo {
    _dir: tempfile::TempDir,
    pub project: Project,
    /// The command that checks the project where its repository is the host, as its declaration names it.
    pub check: String,
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
                allowed_config: Vec::new(),
                github: None,
                commits: crate::acts::writes().commits,
                tree: "design/".to_string(),
            },
            check: check.to_string(),
            data,
        }
    }
}

const WORK: WorkNumber = WorkNumber(1);

/// The name of the work's merge.
const NAME: &str = "Say goodbye as well";

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
    assert_eq!((&again.place, &again.from), (&first.place, &first.from));
    // The branch was there already the second time.
    assert!(!first.kept && again.kept);
    let worktrees = run(&repo.project.repo, &["worktree", "list", "--porcelain"]);
    assert_eq!(worktrees.lines().filter(|l| l.starts_with("worktree ")).count(), 2);
}

// Git is started by its path, so that the standard library starts it with `posix_spawn`, not `fork`, and is given of
// the engine's environment only what is kept, with what the command is given beside. It builds a command and starts
// nothing.
#[test]
fn git_is_started_by_its_path_given_only_what_is_kept_of_the_environment() {
    let with = With {
        settings: Vec::new(),
        env: vec![("GIT_SSH_COMMAND".to_string(), "ssh".to_string())],
    };
    let git = command(Path::new("/"), &with, &["status"]);
    let program = Path::new(git.as_std().get_program());
    assert!(program.is_absolute(), "{}", program.display());
    assert_eq!(program.file_name(), Some(std::ffi::OsStr::new("git")));
    assert_eq!(crate::programs::named(program.as_os_str()), "git");
    let given: Vec<String> = (git.as_std().get_envs())
        .filter(|(_, value)| value.is_some())
        .map(|(name, _)| name.to_string_lossy().into_owned())
        .collect();
    let kept = |name: &str| crate::cli::KEPT.contains(&name) || name.starts_with("LC_");
    let beside = ["GIT_SSH_COMMAND", "GIT_TERMINAL_PROMPT"];
    assert!(
        (given.iter()).all(|name| kept(name) || beside.contains(&name.as_str())),
        "{given:?}"
    );
    assert!(given.iter().any(|name| name == "PATH"), "{given:?}");
}

// A command run in a worktree may leave directories that do not let their owner take a file out, at any depth: the
// worktree is still taken away, since git alone cannot, and the repository no longer names it.
#[tokio::test]
async fn a_worktree_with_directories_that_cannot_be_written_or_read_is_taken_away() {
    use std::os::unix::fs::PermissionsExt;
    let repo = Repo::new("true");
    let started = start(&repo.project, &repo.place(WORK), "main").await.unwrap();
    let (outer, inner) = (started.place.dir.join("sealed"), started.place.dir.join("sealed/inner"));
    std::fs::create_dir_all(&inner).unwrap();
    std::fs::write(inner.join("entry"), "x").unwrap();
    for (dir, mode) in [(&inner, 0o555), (&outer, 0o000)] {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode)).unwrap();
    }
    remove(&repo.project, &repo.place(WORK))
        .await
        .expect("the worktree is taken away");
    assert!(!started.place.dir.exists());
    let worktrees = run(&repo.project.repo, &["worktree", "list", "--porcelain"]);
    assert_eq!(worktrees.lines().filter(|l| l.starts_with("worktree ")).count(), 1);
}

#[tokio::test]
async fn the_merge_puts_the_checked_commit_on_main_once() {
    let repo = Repo::new("true");
    let started = start(&repo.project, &repo.place(WORK), "main").await.unwrap();
    let head = commit(&started.place.dir, "farewell.txt", "Bye.\n");
    let merged = merge(&repo.project, &repo.place(WORK), NAME, Some(&head))
        .await
        .expect("the work merges");
    assert_eq!(merged, run(&repo.project.repo, &["rev-parse", "main"]));
    let parents = run(&repo.project.repo, &["rev-list", "--parents", "-n", "1", "main"]);
    assert_eq!(
        parents.split(' ').nth(2),
        Some(head.as_str()),
        "a merge commit of the work's head"
    );
    assert_eq!(
        run(&repo.project.repo, &["log", "-1", "--format=%s", "main"]),
        NAME,
        "named by the work's merge"
    );
    assert!(repo.project.repo.join("farewell.txt").is_file());
    let again = merge(&repo.project, &repo.place(WORK), NAME, Some(&head))
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
    let refused = merge(&repo.project, &repo.place(WORK), NAME, Some(&checked))
        .await
        .expect_err("refused");
    let refused = failed(refused);
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
    merge(&repo.project, &repo.place(WORK), NAME, Some(&head))
        .await
        .expect("the hook is not run");
}

#[tokio::test]
async fn the_merge_needs_the_checkout_on_main() {
    let repo = Repo::new("true");
    start(&repo.project, &repo.place(WORK), "main").await.unwrap();
    run(&repo.project.repo, &["switch", "-q", "-c", "elsewhere"]);
    let refused = merge(&repo.project, &repo.place(WORK), NAME, None)
        .await
        .expect_err("the merge is refused");
    let refused = failed(refused);
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
        let refused = merge(&repo.project, &repo.place(WORK), NAME, Some(&head))
            .await
            .expect_err(key);
        let refused = failed(refused);
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
    merge(&repo.project, &repo.place(WORK), NAME, Some(&head))
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
        "remote.https://github.com/o/r.git.uploadpack",
        "remote.origin.receivepack",
        "remote.origin.vcs",
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

// What a worktree holds that is not committed is the change of a file git tracks, staged or not; a file git does not
// track is none, however many there are, and is named apart: a directory of them as one line, what git ignores not at
// all.
#[tokio::test]
async fn a_file_git_does_not_track_is_no_change_and_an_edit_of_a_tracked_one_is() {
    let repo = Repo::new("true");
    let dir = &repo.project.repo;
    commit(dir, ".gitignore", "target/\n");
    assert_eq!(uncommitted(dir).await.unwrap(), "");
    assert_eq!(untracked(dir).await.unwrap(), "");

    std::fs::write(dir.join("notes.txt"), "Notes.\n").unwrap();
    std::fs::create_dir_all(dir.join("cache/deep")).unwrap();
    std::fs::write(dir.join("cache/one"), "1\n").unwrap();
    std::fs::write(dir.join("cache/deep/two"), "2\n").unwrap();
    std::fs::create_dir_all(dir.join("target")).unwrap();
    std::fs::write(dir.join("target/out"), "built\n").unwrap();
    assert_eq!(
        uncommitted(dir).await.unwrap(),
        "",
        "files git does not track are no change"
    );
    assert_eq!(untracked(dir).await.unwrap(), "cache/\nnotes.txt");

    std::fs::write(dir.join("greeting.txt"), "Hello, there.\n").unwrap();
    assert_eq!(uncommitted(dir).await.unwrap(), " M greeting.txt");
    run(dir, &["add", "greeting.txt"]);
    assert_eq!(
        uncommitted(dir).await.unwrap(),
        "M  greeting.txt",
        "a staged edit is one too"
    );
    assert_eq!(
        untracked(dir).await.unwrap(),
        "cache/\nnotes.txt",
        "a tracked file is not untracked"
    );
}

// Reference: the output of `git status --porcelain` as git's documentation gives its status codes (`A ` added, ` D`
// deleted, `R ` renamed, ` T` type changed). Whatever git tracks and the worktree or index changes is not committed;
// the one definition the check, the round's end and the repository's merge share must list each kind of it, and must
// not depend on the repository's own configuration of how status shows files git does not track.
#[tokio::test]
async fn every_change_to_a_tracked_file_is_uncommitted_whatever_the_repository_says_of_untracked_files() {
    let repo = Repo::new("true");
    let dir = &repo.project.repo;
    commit(dir, "old.txt", "Old.\n");
    commit(dir, "gone.txt", "Gone.\n");
    commit(dir, "same.txt", "Same.\n");
    // Where the repository's configuration says to show every untracked file, the list still holds none.
    run(dir, &["config", "status.showUntrackedFiles", "all"]);
    std::fs::write(dir.join("notes.txt"), "Notes.\n").unwrap();
    assert_eq!(uncommitted(dir).await.unwrap(), "");

    // A new file that is staged is tracked, and not committed.
    std::fs::write(dir.join("new.txt"), "New.\n").unwrap();
    run(dir, &["add", "new.txt"]);
    assert_eq!(uncommitted(dir).await.unwrap(), "A  new.txt");
    // A tracked file deleted, not staged, and one removed from the index.
    std::fs::remove_file(dir.join("gone.txt")).unwrap();
    run(dir, &["rm", "-q", "same.txt"]);
    // A rename, staged.
    run(dir, &["mv", "old.txt", "renamed.txt"]);
    let listed = uncommitted(dir).await.unwrap();
    let mut lines: Vec<&str> = listed.lines().collect();
    lines.sort_unstable();
    assert_eq!(
        lines,
        [" D gone.txt", "A  new.txt", "D  same.txt", "R  old.txt -> renamed.txt"],
        "{listed}"
    );
    // The untracked file is still not among them, and is the one `untracked` gives.
    assert_eq!(untracked(dir).await.unwrap(), "notes.txt");
}

// Reference: git's documentation of `ls-files --others --exclude-standard --directory`: a directory classified as
// "other" is shown by its name alone, and one that holds a tracked file is not, its untracked files shown one by one;
// a name with a space is a line of its own, unquoted.
#[tokio::test]
async fn untracked_names_a_directory_with_a_tracked_file_by_its_files_and_a_name_with_a_space_whole() {
    let repo = Repo::new("true");
    let dir = &repo.project.repo;
    std::fs::create_dir_all(dir.join("src")).unwrap();
    commit(dir, "src/lib.txt", "Lib.\n");
    std::fs::write(dir.join("src/extra.txt"), "Extra.\n").unwrap();
    std::fs::write(dir.join("my notes.txt"), "Notes.\n").unwrap();
    std::fs::create_dir_all(dir.join("out dir")).unwrap();
    std::fs::write(dir.join("out dir/one"), "1\n").unwrap();
    assert_eq!(untracked(dir).await.unwrap(), "my notes.txt\nout dir/\nsrc/extra.txt");
    assert_eq!(uncommitted(dir).await.unwrap(), "");
}
