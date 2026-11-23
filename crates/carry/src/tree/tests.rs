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
    let started = git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
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

/// The format of the tree this repository's own declaration gives.
fn kinds_of_this_repository() -> TreeFormat {
    TreeFormat {
        kinds: vec![
            flow::agents::EntryKind {
                kind: "principle".to_string(),
                dir: "principles/".to_string(),
            },
            flow::agents::EntryKind {
                kind: "decision".to_string(),
                dir: "decisions/".to_string(),
            },
        ],
        heading: "### {kind} {name}".to_string(),
        reference: "{kind} {name}".to_string(),
    }
}

/// The words the tests give the entries in.
fn words() -> Entries {
    Entries {
        file: "In `{path}`:".to_string(),
        gone: "{reference} is gone.".to_string(),
    }
}

/// Writes the files of a tree of two kinds, a concept's file beside its subdirectory's, and gives the commit.
fn a_tree(dir: &Path) -> String {
    write(
        dir,
        "design/decisions/flow.md",
        "# Flow\n\nWhat the flow covers.\n\n### decision b-two\nTwo.\n- rejected: an agent proposed three.\n\n\n\
         ### decision a-one\nOne.\n",
    );
    write(
        dir,
        "design/decisions/flow/engineer.md",
        "### decision c-three\nThree.\n",
    );
    write(
        dir,
        "design/principles/general.md",
        "# General\n\n### principle p-one\nA rule.\nWhy: a reason.\n",
    )
}

// What is given in place of the names: each named once, whole to the line before the next heading less its blank
// lines, the principles before the decisions as the kinds are ordered, a concept's file before its subdirectory's, and
// in a file in the file's order, whatever the order of the names, their blank lines and the white space around them.
#[tokio::test]
async fn each_entry_named_is_given_whole_once_under_its_file_in_the_order_of_the_kinds_and_then_of_the_paths() {
    let (_repo, place) = started().await;
    let commit = a_tree(&place.dir);
    let names = "decision c-three\n  decision a-one  \n\nprinciple p-one\ndecision b-two\ndecision a-one\n";
    let given = entries(
        &place.dir,
        "design/",
        &kinds_of_this_repository(),
        Some(&commit),
        names,
        &words(),
    )
    .await;
    let expected = "In `design/principles/general.md`:\n\n### principle p-one\nA rule.\nWhy: a reason.\n\n\
                    In `design/decisions/flow.md`:\n\n### decision b-two\nTwo.\n- rejected: an agent proposed three.\n\n\
                    ### decision a-one\nOne.\n\n\
                    In `design/decisions/flow/engineer.md`:\n\n### decision c-three\nThree.";
    assert_eq!(given.as_deref(), Ok(expected));
}

// The entries are read at the commit given, not at the branch's head: of two commits, each gives its own text, and an
// entry the later one removes is given in the words for a name the tree does not hold, after the entries, as a line
// that is no reference is, and as every line is where the tree has no version.
#[tokio::test]
async fn each_entry_is_given_as_the_commit_given_has_it_and_a_name_it_does_not_hold_in_the_words_for_that() {
    let (_repo, place) = started().await;
    let first = a_tree(&place.dir);
    let later = write(
        &place.dir,
        "design/decisions/flow.md",
        "# Flow\n\n### decision b-two\nTwo, changed.\n",
    );
    let format = kinds_of_this_repository();
    let names = "decision a-one\ndecision b-two\nnot a reference\n";
    let at = |commit: Option<String>| {
        let (dir, format) = (place.dir.clone(), format.clone());
        async move { entries(&dir, "design/", &format, commit.as_deref(), names, &words()).await }
    };
    let before = "In `design/decisions/flow.md`:\n\n### decision b-two\nTwo.\n- rejected: an agent proposed three.\n\n\
                  ### decision a-one\nOne.\n\nnot a reference is gone.";
    assert_eq!(at(Some(first)).await.as_deref(), Ok(before));
    let after = "In `design/decisions/flow.md`:\n\n### decision b-two\nTwo, changed.\n\ndecision a-one is gone.\n\n\
                 not a reference is gone.";
    assert_eq!(at(Some(later)).await.as_deref(), Ok(after));
    let none = "decision a-one is gone.\n\ndecision b-two is gone.\n\nnot a reference is gone.";
    assert_eq!(at(None).await.as_deref(), Ok(none));
}

// What a return that writes the names is sent back for: each line, trimmed and once, not written as a reference to an
// entry, or naming one the tree lacks, of its kind; a blank line is none.
#[tokio::test]
async fn a_line_not_written_as_a_reference_or_naming_an_entry_the_tree_lacks_of_its_kind_is_unknown() {
    let (_repo, place) = started().await;
    let commit = a_tree(&place.dir);
    let names = "decision a-one\ndecision no-such-entry\nprinciple a-one\nsee decision a-one\n\n decision a-one \n\
                 decision no-such-entry\ndecision two words\n";
    let unknown = unknown(&place.dir, "design/", &kinds_of_this_repository(), &commit, names).await;
    let expected = [
        "decision no-such-entry",
        "principle a-one",
        "see decision a-one",
        "decision two words",
    ];
    assert_eq!(unknown, Ok(expected.map(str::to_string).to_vec()));
}

// Nothing of how an entry is found is the engine's own: a tree elsewhere, of one kind in a directory of its own, with
// its own heading and way of referring, is checked and given by those, and an entry in this repository's form in it is
// none.
#[tokio::test]
async fn a_tree_of_another_format_is_checked_and_given_by_its_own_kinds_heading_and_reference() {
    let (_repo, place) = started().await;
    write(
        &place.dir,
        "docs/tree/decisions/old.md",
        "### decision keep-it\nKept.\n",
    );
    let commit = write(
        &place.dir,
        "docs/tree/rules/a.md",
        "## rule: keep-it\nKeep it.\n## rule: drop-it\nDrop it.\n",
    );
    let format = TreeFormat {
        kinds: vec![flow::agents::EntryKind {
            kind: "rule".to_string(),
            dir: "rules".to_string(),
        }],
        heading: "## {kind}: {name}".to_string(),
        reference: "{kind} {name}".to_string(),
    };
    let names = "rule keep-it\ndecision keep-it\n";
    let unknown = unknown(&place.dir, "docs/tree", &format, &commit, names).await;
    assert_eq!(unknown, Ok(vec!["decision keep-it".to_string()]));
    let given = entries(&place.dir, "docs/tree", &format, Some(&commit), names, &words()).await;
    let expected = "In `docs/tree/rules/a.md`:\n\n## rule: keep-it\nKeep it.\n\ndecision keep-it is gone.";
    assert_eq!(given.as_deref(), Ok(expected));
}

// A look at a commit the repository does not have says why, rather than take every name as unknown or as gone.
#[tokio::test]
async fn a_look_at_the_entries_that_cannot_be_made_says_why() {
    let (_repo, place) = started().await;
    let format = kinds_of_this_repository();
    let missing = "0123456789abcdef0123456789abcdef01234567";
    let unknown = unknown(&place.dir, "design/", &format, missing, "decision a-one").await;
    assert!(unknown.is_err(), "{unknown:?}");
    let given = entries(
        &place.dir,
        "design/",
        &format,
        Some(missing),
        "decision a-one",
        &words(),
    )
    .await;
    assert!(given.is_err(), "{given:?}");
}

// The files of a commit are read as git-cat-file(1) says `--batch` writes them ("BATCH OUTPUT": `<oid> SP <type> SP
// <size> LF <contents> LF` for each object, `<object> SP missing LF` for one it does not find): in the order asked, an
// empty file as empty, a path the commit lacks as none, and a file whose text looks like such a header, or whose path
// holds a space, as its own bytes, each object after it read from where the one before ends. A path that holds a line
// break, which that input cannot carry, is refused.
#[tokio::test]
async fn the_files_of_a_commit_are_read_in_one_batch_as_git_frames_them_an_empty_or_missing_one_or_one_like_a_header() {
    let (_repo, place) = started().await;
    let like_a_header = "0123456789abcdef0123456789abcdef01234567 blob 3\nabc\n\nHEAD:t/x.md missing\n";
    write(&place.dir, "t/empty.md", "");
    write(&place.dir, "t/like a header.md", like_a_header);
    let commit = write(&place.dir, "t/last.md", "no line break at the end");
    let paths = [
        "t/empty.md",
        "t/nope.md",
        "t/like a header.md",
        "t/last.md",
        "t/empty.md",
    ];
    let read = crate::git::blobs(&place.dir, &commit, &paths).await;
    let expected = vec![
        Some(Vec::new()),
        None,
        Some(like_a_header.as_bytes().to_vec()),
        Some(b"no line break at the end".to_vec()),
        Some(Vec::new()),
    ];
    assert_eq!(read, Ok(expected));
    let refused = crate::git::blobs(&place.dir, &commit, &["t/a\nb.md"]).await;
    assert!(refused.is_err(), "{refused:?}");
}

/// Each file of the directory, by its path from `root`, with its text.
fn files_under(root: &Path, dir: &Path, into: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(root.join(dir)).expect("the directory is read") {
        let entry = entry.expect("the directory is read");
        let path = dir.join(entry.file_name());
        if entry.file_type().expect("the file is looked at").is_dir() {
            files_under(root, &path, into);
        } else {
            let text = std::fs::read_to_string(root.join(&path)).expect("the file is read");
            into.push((path.to_string_lossy().into_owned(), text));
        }
    }
}

// This repository's own declaration (`[tree_format]` of `flow/project.toml`, with its words in
// `flow/agents/message/given.toml`) finds every entry of this repository's own design tree as the tree's rules
// (`design/README.md`) write one, and gives each whole. By those rules the principles are in `principles/` and the
// decisions in `decisions/`, an entry begins `### principle <name>` or `### decision <name>`, "each line of an entry"
// is "one line of the file" and one blank line is between entries: so an entry is its heading and the lines after it
// up to a blank line or the file's end. Each is given under its file, the principles' files before the decisions', in
// the order of their paths (acceptance 2), and none of the names is unknown. The rules' own templates, which
// `design/README.md` writes as headings in its code fences, are no entry: that file is under neither directory.
#[tokio::test]
async fn this_repository_s_declaration_finds_and_gives_whole_every_entry_of_its_own_design_tree() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let agents = flow::agents::read(&root.join("flow")).expect("the files beside the flows are read");
    let format = (agents.project.tree_format.clone()).expect("the declaration gives the format of the tree");
    let tree = agents.project.tree.clone();
    let words = agents.given.entries.clone();
    let design = tree.trim_end_matches('/');
    let readme = std::fs::read_to_string(root.join(design).join("README.md")).expect("the tree's rules are read");
    for template in ["### principle <a-short-name>", "### decision <a-short-name>"] {
        assert!(
            readme.lines().any(|line| line == template),
            "the rules write {template}"
        );
    }
    let (_repo, place) = started().await;
    let mut parts = Vec::new();
    let mut references = Vec::new();
    for (dir, kind) in [("principles", "principle"), ("decisions", "decision")] {
        let mut files = Vec::new();
        files_under(&root, &Path::new(design).join(dir), &mut files);
        files.sort();
        let before = references.len();
        for (path, text) in &files {
            let at = place.dir.join(path);
            std::fs::create_dir_all(at.parent().expect("a directory")).expect("the directory is made");
            std::fs::write(&at, text).expect("the file is written");
            let lines: Vec<&str> = text.lines().collect();
            let mut of_file = Vec::new();
            for (i, line) in lines.iter().enumerate() {
                if let Some(name) = line.strip_prefix(&format!("### {kind} ")) {
                    let whole: Vec<&str> = lines[i..]
                        .iter()
                        .take_while(|l| !l.trim().is_empty())
                        .copied()
                        .collect();
                    of_file.push(whole.join("\n"));
                    references.push(format!("{kind} {name}"));
                }
            }
            if !of_file.is_empty() {
                parts.push(flow::agents::fill(&words.file, &[("path", path.as_str())]));
                parts.extend(of_file);
            }
        }
        assert!(references.len() > before, "the tree holds entries of the kind {kind}");
    }
    std::fs::write(place.dir.join(design).join("README.md"), &readme).expect("the rules are written");
    run(&place.dir, &["add", design]);
    run(&place.dir, &["commit", "-q", "-m", "Write the design tree"]);
    let commit = run(&place.dir, &["rev-parse", "HEAD"]);
    let names = references.join("\n");
    let unknown_of = |names: String| {
        let (dir, tree, format, commit) = (place.dir.clone(), tree.clone(), format.clone(), commit.clone());
        async move { unknown(&dir, &tree, &format, &commit, &names).await }
    };
    assert_eq!(unknown_of(names.clone()).await, Ok(Vec::new()));
    let templates = ["principle <a-short-name>", "decision <a-short-name>"].map(str::to_string);
    assert_eq!(unknown_of(templates.join("\n")).await, Ok(templates.to_vec()));
    let given = entries(&place.dir, &tree, &format, Some(&commit), &names, &words).await;
    assert_eq!(given, Ok(parts.join("\n\n")));
}
