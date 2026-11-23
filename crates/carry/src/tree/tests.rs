//! The tree's versions against a work's branch in a repository made for each test, and the look at whether the work's
//! change holds the owner's answers marked `tree` there. Each test makes a repository of a few commits and runs a few
//! git commands, in well under a second.

use std::path::Path;

use decide::{Answered, Questioned, Returned, RunId, Seq, Started};
use store::WorkNumber;
use work::{Back, Way};

use super::*;
use crate::git::tests::{Repo, run};

const WORK: WorkNumber = WorkNumber(1);

/// Writes the file, in a directory of the worktree where it names one, commits it, and gives the commit.
fn write(dir: &Path, file: &str, text: &str) -> String {
    let path = dir.join(file);
    std::fs::create_dir_all(path.parent().expect("a directory")).expect("the directory is made");
    std::fs::write(&path, text).expect("the file is written");
    run(dir, &["add", file]);
    run(dir, &["commit", "-q", "-m", &format!("Write {file}")]);
    run(dir, &["rev-parse", "HEAD"])
}

/// Removes the path, commits that, and gives the commit.
fn remove(dir: &Path, path: &str) -> String {
    run(dir, &["rm", "-q", "-r", path]);
    run(dir, &["commit", "-q", "-m", &format!("Remove {path}")]);
    run(dir, &["rev-parse", "HEAD"])
}

/// A started work.
async fn started() -> (Repo, Place) {
    let repo = Repo::new("true");
    let started = git::start(&repo.project, &repo.place(WORK), "main", "main")
        .await
        .unwrap();
    (repo, started.place)
}

/// The looks at a work's branch for the tree's block, each against the latest version, as the looks before it left it.
#[derive(Default)]
struct Looks {
    last: Option<String>,
}

impl Looks {
    /// Looks at the branch, and gives the content of the new version it makes, where it makes one.
    async fn next(&mut self, place: &Place) -> Option<String> {
        let new = read(place, "design/", self.last.as_deref()).await.unwrap();
        if new.is_some() {
            self.last = new.clone();
        }
        new
    }
}

/// The content a version of the work's branch at its head has, where it has a design tree.
fn content(place: &Place) -> String {
    let branch = place.reference();
    let tree = run(&place.dir, &["rev-parse", &format!("{branch}:design")]);
    format!("{} {tree}", run(&place.dir, &["rev-parse", &branch]))
}

#[tokio::test]
async fn a_branch_with_no_design_tree_makes_no_version() {
    let (_repo, place) = started().await;
    assert_eq!(Looks::default().next(&place).await, None);
}

#[tokio::test]
async fn the_first_look_at_a_design_tree_makes_a_version_of_the_commit_and_the_tree_and_the_next_makes_none() {
    let (_repo, place) = started().await;
    let mut looks = Looks::default();
    write(&place.dir, "design/a.md", "One.\n");
    assert_eq!(looks.next(&place).await, Some(content(&place)));
    assert_eq!(looks.next(&place).await, None, "nothing changed since");
}

#[tokio::test]
async fn a_commit_that_leaves_the_design_tree_as_it_was_makes_no_version() {
    let (_repo, place) = started().await;
    let mut looks = Looks::default();
    write(&place.dir, "design/a.md", "One.\n");
    looks.next(&place).await;
    write(&place.dir, "other.txt", "Elsewhere.\n");
    assert_eq!(looks.next(&place).await, None);
    write(&place.dir, "design/a.md", "Two.\n");
    assert_eq!(looks.next(&place).await, Some(content(&place)));
}

#[tokio::test]
async fn a_tree_put_back_as_an_earlier_version_had_it_is_a_new_version_after_the_last() {
    let (_repo, place) = started().await;
    let mut looks = Looks::default();
    write(&place.dir, "design/a.md", "One.\n");
    looks.next(&place).await;
    write(&place.dir, "design/a.md", "Two.\n");
    looks.next(&place).await;
    write(&place.dir, "design/a.md", "One.\n");
    assert_eq!(looks.next(&place).await, Some(content(&place)));
}

#[tokio::test]
async fn a_branch_that_removes_its_design_tree_makes_a_version_and_one_that_never_had_it_makes_none() {
    let (_repo, place) = started().await;
    let mut looks = Looks::default();
    write(&place.dir, "design/a.md", "One.\n");
    looks.next(&place).await;
    let removed = remove(&place.dir, "design");
    assert_eq!(looks.next(&place).await, Some(format!("{removed} -")));
    assert_eq!(looks.next(&place).await, None, "it is gone still");
    write(&place.dir, "design/a.md", "One.\n");
    assert_eq!(looks.next(&place).await, Some(content(&place)));
}

#[tokio::test]
async fn the_look_is_at_the_work_s_branch_and_not_at_what_the_worktree_has_checked_out() {
    let (_repo, place) = started().await;
    let start = run(&place.dir, &["rev-parse", "HEAD"]);
    write(&place.dir, "design/a.md", "One.\n");
    let on_the_branch = content(&place);
    run(&place.dir, &["checkout", "-q", "--detach", &start]);
    assert_eq!(Looks::default().next(&place).await, Some(on_the_branch));
}

/// The content of the tree's version at each of these commits.
fn versions(dir: &Path, commits: &[&str]) -> Vec<String> {
    (commits.iter())
        .map(|commit| {
            let tree = run(dir, &["rev-parse", &format!("{commit}:design")]);
            format!("{commit} {tree}")
        })
        .collect()
}

#[tokio::test]
async fn the_files_that_changed_between_two_versions_are_those_the_diff_of_the_design_tree_gives_between_their_commits()
{
    let (_repo, place) = started().await;
    let dir = &place.dir;
    let first = write(dir, "design/a.md", "One.\n");
    write(dir, "other.txt", "Elsewhere.\n");
    let second = write(dir, "design/decisions/b.md", "Two.\n");
    let third = write(dir, "design/decisions/c.md", "Three.\n");
    let v = versions(dir, &[&first, &second, &third]);
    let files = changed(dir, "design/", &v[0], &v[2]).await.unwrap();
    assert_eq!(files, "design/decisions/b.md\ndesign/decisions/c.md");
    let given = run(dir, &["diff", "--name-only", &first, &third, "--", "design"]);
    assert_eq!(files, given);
    let later = changed(dir, "design/", &v[1], &v[2]).await.unwrap();
    assert_eq!(later, "design/decisions/c.md");
}

#[tokio::test]
async fn a_rename_and_a_deletion_are_named_by_every_path_they_touch() {
    let (_repo, place) = started().await;
    let dir = &place.dir;
    let first = write(dir, "design/a.md", "One, which is long enough to be known again.\n");
    write(dir, "design/b.md", "Two, which goes.\n");
    let second = write(dir, "design/keep.md", "Three.\n");
    run(dir, &["mv", "design/a.md", "design/c.md"]);
    run(dir, &["commit", "-q", "-m", "Rename a.md"]);
    let third = remove(dir, "design/b.md");
    let v = versions(dir, &[&first, &second, &third]);
    let files = changed(dir, "design/", &v[1], &v[2]).await.unwrap();
    assert_eq!(files, "design/a.md\ndesign/b.md\ndesign/c.md");
}

#[tokio::test]
async fn a_version_that_is_not_the_trees_has_no_files() {
    let (_repo, place) = started().await;
    let files = changed(&place.dir, "design/", "nothing", "not of the tree at all").await;
    assert_eq!(files.unwrap(), "");
}

/// A question of the cell `build`, marked `tree` as given, recommending the first of two options.
pub(crate) fn asked(name: &str, tree: Option<bool>) -> Question {
    let way = |does: &str| Way {
        does: does.to_string(),
        follows: format!("what follows from {does}"),
    };
    Question::Asked {
        name: name.to_string(),
        waits: false,
        tree,
        recommends: Some(1),
        ask: format!("Which way for {name}?"),
        options: vec![way("the short way"), way("the long way")],
        about: None,
        reasons: Default::default(),
    }
}

/// A work whose one run of `build` asked these questions as its first round ended, as the loop keeps them, each
/// answered with its first option where it says so.
pub(crate) fn asked_in(questions: Vec<(Question, bool)>) -> Work {
    let run = RunId(1);
    let mut work = Work::default();
    work.records.started.push(Started {
        run,
        cell: "build".into(),
        at: Seq(1),
        because: Vec::new(),
        basis: Default::default(),
    });
    work.records.returned.push(Returned { run, at: Seq(2) });
    let mut said = Said::default();
    for (i, (question, answered)) in questions.into_iter().enumerate() {
        let at = Seq(3 + 2 * i as u64);
        work.records.questions.push(Questioned {
            name: question.name().to_string(),
            cell: "build".into(),
            run,
            engine: false,
            waits: false,
            withdrawn: false,
            at,
        });
        if answered {
            work.records.answers.push(Answered {
                question: question.name().to_string(),
                version: at,
                option: 1,
                why: None,
                at: Seq(at.0 + 1),
            });
        }
        said.questions.push(question);
    }
    let back = Back::Returned {
        dealt_with: Vec::new(),
        handed: Vec::new(),
        asks: Vec::new(),
        found: Vec::new(),
        said,
        engine: Vec::new(),
        waits_for: None,
        read: Vec::new(),
    };
    work.back.insert((run, 1), back);
    work
}

/// The work, as [`asked_in`] keeps it, with the question of this name withdrawn after all it keeps.
pub(crate) fn withdrawn(mut work: Work, name: &str) -> Work {
    work.records.questions.push(Questioned {
        name: name.to_string(),
        cell: "build".into(),
        run: RunId(1),
        engine: false,
        waits: false,
        withdrawn: true,
        at: Seq(100),
    });
    work
}

/// The questions whose answer the tree does not hold, as a round ends with the work's branch as it is, its return
/// saying this beside its work, main where the reference `main` holds it.
async fn unwritten_in(repo: &Repo, place: &Place, work: &Work, said: &Said) -> Result<Vec<String>, String> {
    unwritten(&repo.project, place, "main", "design/", work, said).await
}

// The work's own change is held against main where the branch left it: main changing its own tree since is not the
// work's change.
#[tokio::test]
async fn an_answered_question_marked_tree_is_named_while_the_work_changes_nothing_in_the_tree() {
    let (repo, place) = started().await;
    write(&place.dir, "src/greeting.rs", "pub fn greet() {}\n");
    write(&repo.project.repo, "design/main.md", "Main's own entry.\n");
    let work = asked_in(vec![(asked("placement", Some(true)), true)]);
    let named = unwritten_in(&repo, &place, &work, &Said::default()).await;
    assert_eq!(named, Ok(vec!["placement".to_string()]));
}

#[tokio::test]
async fn once_the_work_changes_the_tree_no_question_is_named() {
    let (repo, place) = started().await;
    write(&place.dir, "design/decisions/greeting.md", "The option taken.\n");
    let work = asked_in(vec![(asked("placement", Some(true)), true)]);
    assert_eq!(
        unwritten_in(&repo, &place, &work, &Said::default()).await,
        Ok(Vec::new())
    );
}

// Each with the tree as main has it: a question not answered, one marked no, one kept before questions said `tree`,
// one withdrawn since its answer, and one the return asks anew, whose latest version then has no answer. Of several
// answered and marked, each is named, in the order of the names.
#[tokio::test]
async fn only_a_standing_question_marked_tree_with_an_answer_to_its_latest_version_is_named() {
    let (repo, place) = started().await;
    write(&place.dir, "src/greeting.rs", "pub fn greet() {}\n");
    let none: Result<Vec<String>, String> = Ok(Vec::new());
    let unanswered = asked_in(vec![(asked("placement", Some(true)), false)]);
    assert_eq!(unwritten_in(&repo, &place, &unanswered, &Said::default()).await, none);
    let marked_no = asked_in(vec![(asked("placement", Some(false)), true)]);
    assert_eq!(unwritten_in(&repo, &place, &marked_no, &Said::default()).await, none);
    let not_marked = asked_in(vec![(asked("placement", None), true)]);
    assert_eq!(unwritten_in(&repo, &place, &not_marked, &Said::default()).await, none);
    let taken_back = withdrawn(asked_in(vec![(asked("placement", Some(true)), true)]), "placement");
    assert_eq!(unwritten_in(&repo, &place, &taken_back, &Said::default()).await, none);
    let answered = asked_in(vec![(asked("placement", Some(true)), true)]);
    let anew = Said {
        questions: vec![asked("placement", Some(true))],
        observations: Vec::new(),
    };
    assert_eq!(unwritten_in(&repo, &place, &answered, &anew).await, none);
    let several = asked_in(vec![
        (asked("wording", Some(true)), true),
        (asked("colour", Some(false)), true),
        (asked("placement", Some(true)), true),
    ]);
    let named = unwritten_in(&repo, &place, &several, &Said::default()).await;
    assert_eq!(named, Ok(vec!["placement".to_string(), "wording".to_string()]));
}

// A question kept before questions said `tree` is read as not marked, and is written again the same, with no mark.
#[tokio::test]
async fn a_question_kept_before_questions_said_tree_is_read_as_not_marked_and_named_by_none() {
    let kept = r#"{"Asked":{"name":"placement","waits":false,"recommends":1,"ask":"Where?","options":[{"does":"here","follows":"near"}]}}"#;
    let question: Question = serde_json::from_str(kept).expect("a question kept before is read");
    assert!(matches!(&question, Question::Asked { tree: None, .. }), "{question:?}");
    assert_eq!(serde_json::to_string(&question).expect("it is written"), kept);
    let (repo, place) = started().await;
    let work = asked_in(vec![(question, true)]);
    assert_eq!(
        unwritten_in(&repo, &place, &work, &Said::default()).await,
        Ok(Vec::new())
    );
}

// Where the look cannot be made, it says why, rather than take the work as changing the tree or as not.
#[tokio::test]
async fn a_look_that_cannot_be_made_says_why() {
    let (repo, place) = started().await;
    let work = asked_in(vec![(asked("placement", Some(true)), true)]);
    let looked = unwritten(
        &repo.project,
        &place,
        "no-such-main",
        "design/",
        &work,
        &Said::default(),
    )
    .await;
    assert!(matches!(&looked, Err(why) if why.contains("merge-base")), "{looked:?}");
}
