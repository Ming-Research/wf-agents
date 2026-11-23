//! What a work changed, against a repository made for each test: what its head and its worktree hold over where its
//! branch left main, the same once its worktree and branch are gone and it merged, and one file's diff. Each test makes
//! a repository of a few commits and runs a few dozen git commands, in well under a second.

use super::*;
use crate::git::tests::{Repo, commit, run};

const WORK: WorkNumber = WorkNumber(1);

/// Main's head as the local host keeps it, which the tests' works branch from.
const MAIN: &str = "refs/heads/main";

/// A started work.
async fn started() -> (Repo, Place) {
    let repo = Repo::new("true");
    let started = git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
    (repo, started.place)
}

/// Each file changed: its path, lines added and removed, whether binary, and whether committed.
fn files(changed: &Changed) -> Vec<(&str, u64, u64, bool, bool)> {
    (changed.changes.files.iter())
        .map(|f| (f.path.as_str(), f.added, f.removed, f.binary, f.committed))
        .collect()
}

#[tokio::test]
async fn a_work_changed_what_its_head_and_its_worktree_hold_over_where_its_branch_left_main() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    let known = Known::default();
    assert!(
        known
            .changed(r, MAIN, WorkNumber(2), &repo.place(WorkNumber(2)))
            .await
            .unwrap()
            .is_none(),
        "a work not started has no head to tell from"
    );
    let left = run(r, &["rev-parse", "main"]);
    // Main moves on after the branch left it: what it brought is none of the work's change.
    commit(r, "elsewhere.txt", "Not the work's.\n");
    let head = commit(&place.dir, "farewell.txt", "Bye.\nSee you.\n");
    // A tracked file changed and not committed, a file git does not track whose last line has no newline, and a
    // binary one.
    std::fs::write(place.dir.join("greeting.txt"), "Hello.\nHi.\n").unwrap();
    std::fs::write(place.dir.join("notes.txt"), "One\nTwo\nThree").unwrap();
    std::fs::write(place.dir.join("blob.bin"), [0u8, 1, 2]).unwrap();
    let changed = known.changed(r, MAIN, WORK, &place).await.unwrap().expect("a head");
    let changes = &changed.changes;
    assert_eq!(
        (changes.base.as_str(), changes.head.as_str(), changes.worktree),
        (left.as_str(), head.as_str(), true)
    );
    assert_eq!(
        files(&changed),
        [
            ("blob.bin", 0, 0, true, false),
            ("farewell.txt", 2, 0, false, true),
            ("greeting.txt", 1, 0, false, false),
            ("notes.txt", 3, 0, false, false),
        ]
    );
    let stat = Diffstat {
        files: 4,
        added: 6,
        removed: 0,
    };
    assert_eq!(changed.stat(), stat);
}

#[tokio::test]
async fn once_its_worktree_and_branch_are_gone_a_merged_work_changed_what_its_kept_head_holds() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    let left = run(r, &["rev-parse", "main"]);
    commit(&place.dir, "greeting.txt", "Hello, you.\n");
    let head = commit(&place.dir, "farewell.txt", "Bye.\n");
    crate::head::keep(&place, &head).await.unwrap();
    // Merged by a commit of the merge, as the local repository merges, after which main moves on.
    git::merge(&repo.project, &place, "Say goodbye", Some(&head))
        .await
        .unwrap();
    commit(r, "later.txt", "Later.\n");
    git::remove(&repo.project, &place).await.unwrap();
    run(r, &["branch", "-D", &place.branch]);
    let changed = Known::default()
        .changed(r, MAIN, WORK, &place)
        .await
        .unwrap()
        .expect("the kept head");
    let changes = &changed.changes;
    assert_eq!(
        (changes.base.as_str(), changes.head.as_str(), changes.worktree),
        (left.as_str(), head.as_str(), false)
    );
    assert_eq!(
        files(&changed),
        [("farewell.txt", 1, 0, false, true), ("greeting.txt", 1, 1, false, true)]
    );
}

/// How many times the work's changes were worked out with git.
fn looked(known: &Known) -> usize {
    known.looked.load(std::sync::atomic::Ordering::Relaxed)
}

/// The repository's own directory stands aside, a file no git can read in its place, so that every git command there
/// and in its worktrees fails whatever directory holds the repository; and comes back.
struct Hidden<'a>(&'a Path);

impl<'a> Hidden<'a> {
    fn of(repo: &'a Path) -> Hidden<'a> {
        std::fs::rename(repo.join(".git"), repo.join(".git-aside")).unwrap();
        std::fs::write(repo.join(".git"), "no repository\n").unwrap();
        Hidden(repo)
    }
}

impl Drop for Hidden<'_> {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0.join(".git"));
        let _ = std::fs::rename(self.0.join(".git-aside"), self.0.join(".git"));
    }
}

// Nothing but a round of the work or a tool run for it changes what its branch and worktree hold, and each is recorded:
// so with nothing under way its numbers are worked out once for its records as they stand, no git running for the
// reads after, and again once its records move on, the head a round moved read with them, or once its worktree goes.
#[tokio::test]
async fn the_numbers_of_a_work_with_nothing_under_way_are_kept_until_its_records_move_on() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    let known = Known::default();
    let at_rest = |at| (Seq(at), false);
    commit(&place.dir, "farewell.txt", "Bye.\n");
    std::fs::write(place.dir.join("notes.txt"), "One\n").unwrap();
    let hidden = Hidden::of(r);
    let failed = known.stat(r, MAIN, (WORK, &place), at_rest(5)).await;
    drop(hidden);
    assert!(failed.is_err(), "{failed:?}");
    let first = known.stat(r, MAIN, (WORK, &place), at_rest(5)).await;
    let stat = |files, added| -> Result<Option<Diffstat>, String> {
        Ok(Some(Diffstat {
            files,
            added,
            removed: 0,
        }))
    };
    assert_eq!(first, stat(2, 2), "a look that failed is taken again");
    assert_eq!(looked(&known), 2);
    std::fs::write(place.dir.join("later.txt"), "Later.\n").unwrap();
    let hidden = Hidden::of(r);
    let again = known.stat(r, MAIN, (WORK, &place), at_rest(5)).await;
    drop(hidden);
    assert_eq!(again, first, "no git ran");
    assert_eq!(looked(&known), 2);
    commit(&place.dir, "more.txt", "More.\nAnd more.\n");
    let moved = known.stat(r, MAIN, (WORK, &place), at_rest(9)).await;
    assert_eq!(moved, stat(4, 5), "the head moved, and what the worktree holds beside");
    assert_eq!(looked(&known), 3);
    git::remove(&repo.project, &place).await.unwrap();
    let gone = known.stat(r, MAIN, (WORK, &place), at_rest(9)).await;
    assert_eq!(gone, stat(2, 3), "only what the head holds, once the worktree is gone");
    assert_eq!(looked(&known), 4);
}

// What a worktree under way holds changes at any moment: each read works it out again, but a read that asks of the work
// as it stands while a look at it runs is given that look's answer.
#[tokio::test]
async fn the_numbers_of_a_work_under_way_are_worked_out_for_each_read_and_shared_while_a_look_runs() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    let known = Known::default();
    let look = || known.stat(r, MAIN, (WORK, &place), (Seq(5), true));
    let (one, two) = tokio::join!(look(), look());
    let none: Result<Option<Diffstat>, String> = Ok(Some(Diffstat::default()));
    assert_eq!((&one, &two), (&none, &none));
    assert_eq!(looked(&known), 1, "the second read was given the first's look");
    std::fs::write(place.dir.join("notes.txt"), "One\n").unwrap();
    let three = look().await;
    assert_eq!(
        three,
        Ok(Some(Diffstat {
            files: 1,
            added: 1,
            removed: 0
        }))
    );
    assert_eq!(looked(&known), 2);
}

#[tokio::test]
async fn the_diff_of_a_file_the_work_changed_is_given_and_of_no_other_path() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    commit(&place.dir, "farewell.txt", "Bye.\n");
    std::fs::write(place.dir.join("notes.txt"), "One\n").unwrap();
    let known = Known::default();
    let diff = |path: &'static str| known.diff(r, MAIN, (WORK, &place), path);
    let committed = diff("farewell.txt").await.unwrap().expect("changed");
    assert!(committed.text.contains("+Bye."), "{}", committed.text);
    assert_eq!((committed.path.as_str(), committed.cut), ("farewell.txt", None));
    let untracked = diff("notes.txt").await.unwrap().expect("changed");
    assert!(untracked.text.contains("+One"), "{}", untracked.text);
    assert_eq!(
        diff("greeting.txt").await.unwrap(),
        None,
        "a file the work did not change"
    );
    assert_eq!(
        diff("../repo/greeting.txt").await.unwrap(),
        None,
        "no path the work changed"
    );
}

#[test]
fn a_diff_longer_than_its_bound_is_cut_at_a_character_saying_its_length() {
    let long = format!("{}é", "a".repeat(DIFF - 1));
    let given = cut("big.txt", long.clone());
    assert_eq!(given.text, "a".repeat(DIFF - 1));
    assert_eq!(given.cut, Some(long.len() as u64));
}

#[test]
fn the_commit_a_merge_made_is_read_back_from_what_it_wrote_by_its_words() {
    let text = crate::acts::words().merge.text;
    let commit = "0123456789abcdef0123456789abcdef01234567";
    let slots = [("branch", "refs/heads/work-1-x"), ("main", "main"), ("head", commit)];
    let wrote = flow::agents::fill(&text, &slots);
    assert_eq!(merged_commit(&wrote, &text).as_deref(), Some(commit));
    assert_eq!(merged_commit("Something else.", &text), None);
}

#[tokio::test]
async fn the_pull_request_a_merge_s_commit_names_at_the_end_of_its_title_is_read() {
    let repo = Repo::new("true");
    let r = &repo.project.repo;
    run(r, &["commit", "-q", "--allow-empty", "-m", "Greet the user (#41)"]);
    let named = run(r, &["rev-parse", "HEAD"]);
    run(r, &["commit", "-q", "--allow-empty", "-m", "Greet the user"]);
    let unnamed = run(r, &["rev-parse", "HEAD"]);
    let known = Known::default();
    assert_eq!(known.pull_named(r, &named).await.unwrap(), Some(41));
    assert_eq!(known.pull_named(r, &unnamed).await.unwrap(), None);
}

/// A work that committed two files and whose worktree is taken away, its branch left and no reference kept under its
/// head: the state of a work whose heads were read before they were kept, merged or not.
async fn committed_and_gone() -> (Repo, Place, String) {
    let (repo, place) = started().await;
    commit(&place.dir, "farewell.txt", "Bye.\n");
    let head = commit(&place.dir, "other.txt", "Other.\n");
    git::remove(&repo.project, &place).await.unwrap();
    (repo, place, head)
}

#[tokio::test]
async fn a_binary_file_the_work_committed_is_listed_as_binary_with_no_lines() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    std::fs::write(place.dir.join("img.png"), [0u8, 1, 2]).unwrap();
    run(&place.dir, &["add", "img.png"]);
    run(&place.dir, &["commit", "-q", "-m", "Add an image"]);
    let changed = Known::default()
        .changed(r, MAIN, WORK, &place)
        .await
        .unwrap()
        .expect("a head");
    // Git says `-` for the lines of each side of a binary file, which are no number.
    assert_eq!(files(&changed), [("img.png", 0, 0, true, true)]);
}

#[tokio::test]
async fn a_link_not_tracked_is_one_line_and_a_large_file_not_tracked_is_counted_whole() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    std::os::unix::fs::symlink("greeting.txt", place.dir.join("link")).unwrap();
    // Far more than the 64 KiB a read takes at a time, so the count crosses the end of each piece.
    std::fs::write(place.dir.join("big.txt"), "x\n".repeat(100_000)).unwrap();
    let changed = Known::default()
        .changed(r, MAIN, WORK, &place)
        .await
        .unwrap()
        .expect("a head");
    assert_eq!(
        files(&changed),
        [("big.txt", 100_000, 0, false, false), ("link", 1, 0, false, false)]
    );
}

#[tokio::test]
async fn a_file_git_ignores_is_no_change_of_the_work() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    commit(&place.dir, ".gitignore", "target/\n");
    std::fs::create_dir(place.dir.join("target")).unwrap();
    std::fs::write(place.dir.join("target/out.o"), "built\n").unwrap();
    std::fs::write(place.dir.join("keep.txt"), "Kept.\n").unwrap();
    let changed = Known::default()
        .changed(r, MAIN, WORK, &place)
        .await
        .unwrap()
        .expect("a head");
    assert_eq!(
        files(&changed),
        [(".gitignore", 1, 0, false, true), ("keep.txt", 1, 0, false, false)]
    );
}

#[tokio::test]
async fn a_file_named_with_spaces_or_accents_is_listed_and_diffed_by_its_real_name() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    commit(&place.dir, "my file.txt", "Spaced.\n");
    std::fs::write(place.dir.join("café é.txt"), "Accented.\n").unwrap();
    let known = Known::default();
    let changed = known.changed(r, MAIN, WORK, &place).await.unwrap().expect("a head");
    // Git quotes and escapes such names unless it is told to end each with a zero byte.
    assert_eq!(
        files(&changed),
        [("café é.txt", 1, 0, false, false), ("my file.txt", 1, 0, false, true)]
    );
    for (path, line) in [("café é.txt", "+Accented."), ("my file.txt", "+Spaced.")] {
        let diff = known
            .diff(r, MAIN, (WORK, &place), path)
            .await
            .unwrap()
            .expect("changed");
        assert!(diff.text.contains(line), "{path}: {}", diff.text);
    }
}

#[tokio::test]
async fn a_file_moved_is_a_file_removed_and_a_file_added_and_no_program_the_repository_names_for_a_diff_runs() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    run(&place.dir, &["mv", "greeting.txt", "moved.txt"]);
    run(&place.dir, &["commit", "-q", "-m", "Move the greeting"]);
    // The repository's own configuration asks git to find renames and to run a program for each diff.
    let program = repo.data.join("diff-program");
    let marked = repo.data.join("diff-program-ran");
    crate::git::tests::marking(&program, &marked);
    run(r, &["config", "diff.renames", "true"]);
    run(r, &["config", "diff.external", &program.display().to_string()]);
    let known = Known::default();
    let changed = known.changed(r, MAIN, WORK, &place).await.unwrap().expect("a head");
    assert_eq!(
        files(&changed),
        [("greeting.txt", 0, 1, false, true), ("moved.txt", 1, 0, false, true)]
    );
    let diff = known
        .diff(r, MAIN, (WORK, &place), "moved.txt")
        .await
        .unwrap()
        .expect("changed");
    assert!(diff.text.contains("+Hello."), "{}", diff.text);
    assert!(!marked.exists(), "the repository's diff program was run");
}

#[tokio::test]
async fn a_work_that_committed_nothing_has_its_head_for_its_base_and_shows_what_its_worktree_holds() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    std::fs::write(place.dir.join("notes.txt"), "Note.\n").unwrap();
    let changed = Known::default()
        .changed(r, MAIN, WORK, &place)
        .await
        .unwrap()
        .expect("a head");
    let main = run(r, &["rev-parse", "main"]);
    let changes = &changed.changes;
    assert_eq!(
        (changes.base.as_str(), changes.head.as_str()),
        (main.as_str(), main.as_str())
    );
    assert_eq!(files(&changed), [("notes.txt", 1, 0, false, false)]);
}

#[tokio::test]
async fn a_work_whose_head_was_never_kept_is_read_at_its_branch_once_its_worktree_is_gone() {
    let (repo, place, head) = committed_and_gone().await;
    let r = &repo.project.repo;
    let changed = Known::default()
        .changed(r, MAIN, WORK, &place)
        .await
        .unwrap()
        .expect("the branch");
    assert_eq!(
        (changed.changes.head.as_str(), changed.changes.worktree),
        (head.as_str(), false)
    );
    assert_eq!(
        files(&changed),
        [("farewell.txt", 1, 0, false, true), ("other.txt", 1, 0, false, true)]
    );
}

#[tokio::test]
async fn the_head_of_a_work_with_no_worktree_is_read_once_and_so_is_what_it_changed_at_a_head() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    let first = commit(&place.dir, "farewell.txt", "Bye.\n");
    let second = commit(&place.dir, "other.txt", "Other.\n");
    crate::head::keep(&place, &first).await.unwrap();
    git::remove(&repo.project, &place).await.unwrap();
    run(r, &["branch", "-D", &place.branch]);
    let known = Known::default();
    let head = |changed: Option<Changed>| changed.expect("the kept head").changes.head;
    assert_eq!(head(known.changed(r, MAIN, WORK, &place).await.unwrap()), first);
    // The reference moves, which nothing but a worktree does: what was read stays what it was.
    run(r, &["update-ref", &crate::head::kept(&place), &second]);
    assert_eq!(head(known.changed(r, MAIN, WORK, &place).await.unwrap()), first);
    assert_eq!(
        head(Known::default().changed(r, MAIN, WORK, &place).await.unwrap()),
        second
    );
}

#[tokio::test]
async fn what_the_worktree_holds_is_worked_out_at_each_ask_so_the_changes_grow_as_the_agent_works() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    let known = Known::default();
    let paths = |changed: Changed| {
        files(&changed)
            .iter()
            .map(|f| (f.0.to_string(), f.1))
            .collect::<Vec<_>>()
    };
    let ask = || known.changed(r, MAIN, WORK, &place);
    assert_eq!(paths(ask().await.unwrap().expect("a head")), []);
    std::fs::write(place.dir.join("notes.txt"), "One\n").unwrap();
    assert_eq!(
        paths(ask().await.unwrap().expect("a head")),
        [("notes.txt".to_string(), 1)]
    );
    std::fs::write(place.dir.join("notes.txt"), "One\nTwo\n").unwrap();
    std::fs::write(place.dir.join("greeting.txt"), "Hello.\nHi.\n").unwrap();
    assert_eq!(
        paths(ask().await.unwrap().expect("a head")),
        [("greeting.txt".to_string(), 1), ("notes.txt".to_string(), 2)]
    );
}

#[tokio::test]
async fn once_the_worktree_is_gone_a_file_s_diff_is_taken_between_the_base_and_the_kept_head() {
    let (repo, place, _) = committed_and_gone().await;
    let r = &repo.project.repo;
    let known = Known::default();
    let diff = known
        .diff(r, MAIN, (WORK, &place), "farewell.txt")
        .await
        .unwrap()
        .expect("changed");
    assert!(diff.text.contains("+Bye."), "{}", diff.text);
    assert_eq!((diff.path.as_str(), diff.cut), ("farewell.txt", None));
    assert_eq!(known.diff(r, MAIN, (WORK, &place), "greeting.txt").await.unwrap(), None);
}

#[tokio::test]
async fn a_diff_over_its_bound_is_cut_at_a_character_with_its_whole_length_said() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    // Each line of the diff is four bytes, one of them a two-byte letter: over a megabyte, with the cut among them.
    commit(&place.dir, "big.txt", &"é\n".repeat(600_000));
    let diff = (Known::default().diff(r, MAIN, (WORK, &place), "big.txt").await.unwrap()).expect("changed");
    assert!(diff.cut.is_some_and(|whole| whole as usize > DIFF), "{:?}", diff.cut);
    assert!(
        diff.text.len() <= DIFF && diff.text.len() + 4 > DIFF,
        "{}",
        diff.text.len()
    );
}

#[tokio::test]
async fn a_look_at_the_worktree_leaves_its_index_as_it_was_so_an_agent_s_own_git_is_never_held_up() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    let index = run(&place.dir, &["rev-parse", "--git-path", "index"]);
    let index = place.dir.join(index);
    // The file is tracked and its content is as committed, but git cannot tell without reading it: its time is newer
    // than the index knows, which a refresh would write down.
    let file = std::fs::File::options()
        .write(true)
        .open(place.dir.join("greeting.txt"))
        .unwrap();
    file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5_000))
        .unwrap();
    drop(file);
    let before = std::fs::read(&index).unwrap();
    let known = Known::default();
    known.changed(r, MAIN, WORK, &place).await.unwrap().expect("a head");
    std::fs::write(place.dir.join("notes.txt"), "One\n").unwrap();
    known
        .diff(r, MAIN, (WORK, &place), "notes.txt")
        .await
        .unwrap()
        .expect("changed");
    assert_eq!(std::fs::read(&index).unwrap(), before, "a look wrote the index");
}
