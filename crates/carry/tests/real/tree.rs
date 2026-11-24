//! The design tree on the work's branch as a block of the work: a version where the start made the branch, where the
//! branch took main in, and where a round of the work committed to the tree, which that round's run hands in; and a
//! change handed in only with the owner's answers to the questions marked `tree` in the tree; and a block whose lines
//! name entries of the tree, checked as it is handed in and given as the entries it names; and a change of the tree
//! held to the forms of the tree's rules.

use decide::run::Unmet;
use decide::{RunId, Writer};
use service::Act;
use store::WorkNumber;
use work::Given;

use crate::{FORMED_FILE, Fake, GREETING_ENTRY, GREETING_FILE, NO_SUCH_ENTRY, On, Real, ended, git_in};

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

// The build commits a decision of the design tree that has no line of an approach turned down, which the tree's rules
// require, and hands in its change: the return is sent back, told the file, the line and what is wrong, and told once;
// its next return, with the decision written as the rules say, is taken. The change to the tree is the work's own, and
// the check before the merge, on the commit that would merge, finds it in its forms.
#[tokio::test]
async fn a_change_that_leaves_the_design_tree_out_of_its_forms_is_sent_back_until_it_is_in_them() {
    let fake = Fake {
        ill_formed_tree: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let kept = real.work(work);
    let build = real.run_of(work, "build");
    let line = format!(
        "{FORMED_FILE}:6: decision formed-wrong: a decision has a `rejected` or a `replaced` line, or more, under its \
         first line"
    );
    let unformed = Unmet::TreeForm {
        lines: vec![line.clone()],
    };
    assert_eq!(kept.given.get(&(build, 2)), Some(&Given::SentBack(vec![unformed])));
    assert_eq!(kept.given.get(&(build, 3)), None, "taken at its second round");
    let calls = real.calls();
    let told = calls
        .iter()
        .filter(|call| call.of("Build") && call.stdin.contains(&format!("fix them all:\n{line}")));
    assert_eq!(told.count(), 1, "the build is told once what is wrong with the tree");
}

// A return that asks for a call asks for nothing else, and the run entered for the results goes on in the same
// worktree: the tree the build committed as it asked is held to its forms in the return that hands in, which is the
// run's second, and not before. Well under a second alone, on the fake agent; it catches a look at the tree made on a
// return that asks for calls, which sends the run that asked back and asks of it what it cannot do yet.
#[tokio::test]
async fn a_build_that_asks_for_a_call_with_the_tree_out_of_its_forms_is_held_to_them_only_in_the_return_that_hands_in()
{
    let fake = Fake {
        ill_formed_tree: true,
        build_then_asks: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let kept = real.work(work);
    let runs: Vec<RunId> = (kept.records.started.iter())
        .filter(|started| started.cell.as_str() == "build")
        .map(|started| started.run)
        .collect();
    let [asked, results] = runs[..] else {
        panic!("the build ran twice: {runs:?}");
    };
    assert_eq!(kept.given.get(&(asked, 2)), None, "the return that asked was not held");
    let line = format!(
        "{FORMED_FILE}:6: decision formed-wrong: a decision has a `rejected` or a `replaced` line, or more, under its \
         first line"
    );
    let unformed = Unmet::TreeForm { lines: vec![line] };
    assert_eq!(kept.given.get(&(results, 2)), Some(&Given::SentBack(vec![unformed])));
    assert_eq!(
        kept.given.get(&(results, 3)),
        None,
        "the round that put the tree right left it in its forms"
    );
}

// A line that only reads changes nothing of the work, and a commit of the tree it leaves is the outside's: its return
// is taken as it is, whatever the tree holds. Well under a second alone, on the fake agent; it catches a look at the
// tree made on the return of a line that cannot put it right.
#[tokio::test]
async fn a_review_that_leaves_the_design_tree_out_of_its_forms_is_not_sent_back() {
    let fake = Fake {
        review_commits_an_ill_formed_tree: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    let reviewed = |real: &Real| {
        let kept = real.work(work);
        let Some(started) = (kept.records.started.iter().rev()).find(|started| started.cell.as_str() == "review")
        else {
            return false;
        };
        let run = started.run;
        (kept.records.finished.iter()).any(|finished| finished.run == run) || kept.given.contains_key(&(run, 2))
    };
    real.until(reviewed).await;
    let kept = real.work(work);
    let review = real.run_of(work, "review");
    assert_eq!(
        kept.given.get(&(review, 2)),
        None,
        "the review's return was taken as it was"
    );
    assert!(
        (kept.records.finished.iter()).any(|finished| finished.run == review),
        "the review's run ended with its return"
    );
    let calls = real.calls();
    let reviewed = calls.iter().find(|call| call.of("Review")).expect("the review ran");
    let head = git_in(&reviewed.dir, &["log", "-1", "--format=%s"]);
    assert_eq!(
        head, "Write the tree",
        "the review left the ill-formed tree as the work's head"
    );
}

// The build hands in its code, which is handed in only with the owner's answer in the design tree, with a change not
// committed and the tree as main has it, and is sent back for both. Its next return takes the change back and gives
// nothing else, so that the code it wrote stands as the return before it left it: that return is held to the answer in
// the tree as the one that wrote the code was, and is sent back for it alone. A look that took only what a return
// writes itself would take it, and the work would end with the answer nowhere in the tree.
#[tokio::test]
async fn a_return_that_completes_one_that_wrote_the_code_is_held_to_the_answer_in_the_tree_for_what_it_carries() {
    let fake = Fake {
        question_first: true,
        tree_question: true,
        build_leaves_an_edit: true,
        build_mends_the_edit_and_nothing_else: true,
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
    let Some(Given::SentBack(first)) = kept.given.get(&(build, 2)) else {
        panic!("the first return was not sent back: {:?}", kept.given.get(&(build, 2)));
    };
    let left_edit = first.iter().any(|unmet| matches!(unmet, Unmet::Uncommitted { .. }));
    assert!(left_edit && first.contains(&unchanged), "{first:?}");
    // The second wrote nothing, and was sent back for the answer in the tree alone.
    assert_eq!(kept.given.get(&(build, 3)), Some(&Given::SentBack(vec![unchanged])));
    assert_eq!(kept.given.get(&(build, 4)), None, "taken at its third round");
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
    let entry = format!(
        "### {GREETING_ENTRY}\nGreet by name.\n- rejected: the owner proposed a plain hello; the owner rejected it, because it names no one."
    );
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
    let names: Vec<&str> = (versions.iter())
        .map(|v| kept.contents[&(block.clone(), v.at)].as_str())
        .collect();
    assert_eq!(
        names,
        [GREETING_ENTRY],
        "the work keeps the names as they were handed in"
    );
    let calls = real.calls();
    let review = (calls.iter()).find(|c| c.of("Review")).expect("the review ran");
    let given = format!("From `{GREETING_FILE}`:\n\n### {GREETING_ENTRY}\nGreet by the first name.");
    assert!(
        review.stdin.contains(&given),
        "{given} is missing from:\n{}",
        review.stdin
    );
    assert!(!review.stdin.contains("a plain hello"), "{}", review.stdin);
    assert!(
        !review.stdin.contains("greet-once"),
        "an entry not named is not given:\n{}",
        review.stdin
    );
}
