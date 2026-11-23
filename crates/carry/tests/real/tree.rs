//! The design tree on the work's branch as a block of the work: a version where the start made the branch, where the
//! branch took main in, and where a round of the work committed to the tree, which that round's run hands in; and a
//! change handed in only with the owner's answers to the questions marked `tree` in the tree; and a block whose lines
//! name entries of the tree, checked as it is handed in and given as the entries it names.

use decide::Writer;
use decide::run::Unmet;
use service::Act;
use store::WorkNumber;
use work::Given;

use crate::{Fake, GREETING_ENTRY, GREETING_FILE, NO_SUCH_ENTRY, On, Real, ended, git_in};

/// A check that fails the first time, moving main by a commit of a file of the design tree as it does, and passes after.
const MOVES_DESIGN_ONCE: &str = "test -f ../moved || { touch ../moved; \
    r=$(dirname \"$(git rev-parse --path-format=absolute --git-common-dir)\"); \
    printf 'Good day.\\n' > \"$r/design/other.md\"; git -C \"$r\" add design/other.md; \
    git -C \"$r\" -c user.name=Tester -c user.email=tester@example.com commit -q -m 'Main moved'; exit 1; }";

/// Each version of the tree block of the work, as its content, the commit and the id of the tree, with whether a run
/// wrote it.
fn versions(real: &Real, work: WorkNumber) -> Vec<(String, String, bool)> {
    let kept = real.work(work);
    let block = decide::BlockId::from("tree");
    let versions = decide::versions(&kept.records).remove(&block).unwrap_or_default();
    (versions.into_iter())
        .map(|v| {
            let content = &kept.contents[&(block.clone(), v.at)];
            let (commit, tree) = content.split_once(' ').expect("a commit and a tree");
            (commit.to_string(), tree.to_string(), matches!(v.by, Writer::Run(_)))
        })
        .collect()
}

/// The subject of the commit and the number of its parents.
fn commit(real: &Real, commit: &str) -> (String, usize) {
    let subject = git_in(&real.repo, &["log", "-1", "--format=%s", commit]);
    let parents = git_in(&real.repo, &["log", "-1", "--format=%P", commit]);
    (subject, parents.split_whitespace().count())
}

#[tokio::test]
async fn the_start_and_a_round_that_commits_to_the_design_tree_each_make_a_version() {
    let fake = Fake {
        design: true,
        ..Fake::default()
    };
    let mut real = Real::open(On::Acts, fake, None, "true");
    let main = git_in(&real.repo, &["rev-parse", "main"]);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let versions = versions(&real, work);
    assert_eq!(versions.len(), 2, "{versions:?}");
    // The start made the branch from main: the first version is main's, before any round of an agent, and no round's
    // commit, written from outside.
    assert_eq!(versions[0].0, main);
    let tree = git_in(&real.repo, &["rev-parse", &format!("{main}:design")]);
    assert_eq!(versions[0].1, tree);
    assert!(!versions[0].2, "{versions:?}");
    // The build committed to the tree: the second is that commit's, which the build's run handed in.
    assert_eq!(commit(&real, &versions[1].0), ("Build the greeting".to_string(), 1));
    assert_ne!(versions[1].1, tree);
    assert!(versions[1].2, "{versions:?}");
}

#[tokio::test]
async fn the_branch_taking_main_in_as_a_run_starts_makes_a_version_before_the_round_ends() {
    let mut real = Real::open(On::Acts, Fake::default(), None, MOVES_DESIGN_ONCE);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let versions = versions(&real, work);
    assert_eq!(versions.len(), 2, "{versions:?}");
    // The second build committed nothing of the tree: the version is the merge of the catch-up, which it made as the
    // run started, and not the build's own commit that ended the round.
    let (subject, parents) = commit(&real, &versions[1].0);
    assert!(
        subject.starts_with("Merge main at ") && subject.contains(" as run "),
        "{subject}"
    );
    assert_eq!(parents, 2);
    // The run took the version in as it began, and was told of it: it is put into the round of its run.
    let kept = real.work(work);
    let second = kept
        .records
        .started
        .iter()
        .filter(|s| s.cell.as_str() == "build")
        .nth(1);
    let run = second.expect("a second build").run;
    let put = kept.records.put_in.iter().filter(|p| p.run == run);
    assert_eq!(put.map(|p| p.block.as_str()).collect::<Vec<_>>(), ["tree"]);
}

#[tokio::test]
async fn the_check_that_takes_main_in_makes_a_version() {
    let fake = Fake {
        moves_main: true,
        ..Fake::default()
    };
    let mut real = Real::open(On::Acts, fake, None, "true");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let versions = versions(&real, work);
    assert_eq!(versions.len(), 2, "{versions:?}");
    // The build moved main after it took main in, and committed nothing of the tree; the check merged main.
    let (subject, parents) = commit(&real, &versions[1].0);
    assert!(
        subject.starts_with("Merge main at ") && !subject.contains(" as run "),
        "{subject}"
    );
    assert_eq!(parents, 2);
}

// The build asks the owner a question marked `tree`, and once the owner answers, hands in its change with the design
// tree as main has it: the return is sent back for the question, naming it, and taken once the build writes the answer
// into the tree, the work then going on to its end.
#[tokio::test]
async fn a_change_handed_in_while_an_answer_marked_tree_is_not_in_the_design_tree_is_sent_back_until_it_is() {
    let fake = Fake {
        question_first: true,
        tree_question: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.waited_question(work).is_some()).await;
    real.answer(work);
    real.until(|real| ended(&real.decision(work))).await;
    let kept = real.work(work);
    let build = real.run_of(work, "build");
    let unchanged = Unmet::TreeUnchanged {
        name: "which_name".to_string(),
    };
    assert_eq!(kept.given.get(&(build, 2)), Some(&Given::SentBack(vec![unchanged])));
    assert_eq!(kept.given.get(&(build, 3)), None, "taken at its second round");
    let sent_back = "- `which_name` is marked `tree` and has the owner's answer, but the work's change against main \
                     changes nothing in the design tree: write the option taken into the tree, as its rules say.";
    let calls = real.calls();
    let told = calls
        .iter()
        .filter(|call| call.of("Build") && call.stdin.contains(sent_back));
    assert_eq!(told.count(), 1, "the build is told once why its return was sent back");
}

// The plan names entries of the design tree with a line that names none, and is sent back for that line alone; its
// next return is taken. The review, on another line, reads the entries: it is given each named entry whole, under its
// file, as the tree's latest version has it, the build having rewritten the entry since the plan named it.
#[tokio::test]
async fn a_line_naming_no_entry_is_sent_back_and_a_reader_is_given_each_entry_named_as_the_latest_tree_has_it() {
    let fake = Fake {
        names_entries: true,
        ..Fake::default()
    };
    let mut real = Real::open(On::Entries, fake, None, "true");
    let entry = format!("### {GREETING_ENTRY}\nGreet by name.\n- rejected: the owner proposed a plain hello; the owner rejected it, because it names no one.");
    let file = format!("# Greeting\n\nHow the work greets.\n\n{entry}\n\n### decision greet-once\nGreet once.\n");
    let path = real.repo.join(GREETING_FILE);
    std::fs::create_dir_all(path.parent().expect("a directory")).unwrap();
    std::fs::write(&path, file).unwrap();
    git_in(&real.repo, &["add", GREETING_FILE]);
    git_in(&real.repo, &["commit", "-q", "-m", "Decide the greeting"]);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let kept = real.work(work);
    let plan = real.run_of(work, "plan");
    let unknown = Unmet::NotAnEntry {
        line: NO_SUCH_ENTRY.to_string(),
    };
    assert_eq!(kept.given.get(&(plan, 2)), Some(&Given::SentBack(vec![unknown])));
    assert_eq!(kept.given.get(&(plan, 3)), None, "taken at its second round");
    let block = decide::BlockId::from("entries");
    let versions = decide::versions(&kept.records).remove(&block).unwrap_or_default();
    let names: Vec<&str> = (versions.iter()).map(|v| kept.contents[&(block.clone(), v.at)].as_str()).collect();
    assert_eq!(names, [GREETING_ENTRY], "the work keeps the names as they were handed in");
    let calls = real.calls();
    let review = (calls.iter()).find(|c| c.of("Review")).expect("the review ran");
    let given = format!("From `{GREETING_FILE}`:\n\n### {GREETING_ENTRY}\nGreet by the first name.");
    assert!(review.stdin.contains(&given), "{given} is missing from:\n{}", review.stdin);
    assert!(!review.stdin.contains("a plain hello"), "{}", review.stdin);
    assert!(!review.stdin.contains("greet-once"), "an entry not named is not given:\n{}", review.stdin);
}
