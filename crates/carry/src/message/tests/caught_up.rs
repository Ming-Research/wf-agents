//! What a round is told of taking main in as its run started: after the task and the list, wherever they are given.

use super::super::{Opening, message};
use super::{Small, asked, words};
use crate::catch_up::{CaughtUp, Outcome};

/// What the round given is told, opening so, with this come of taking main in.
fn told(small: &Small, begins: bool, caught: &CaughtUp) -> String {
    let (run, cell, number, given) = small.round();
    let spec = small.flow.cells.iter().find(|spec| spec.id == cell).expect("the cell");
    let opening = Opening {
        begins,
        main: Some(caught),
    };
    message(&small.flow, &small.work, spec, run, number, &given, &opening, &words())
}

fn caught(outcome: Outcome) -> CaughtUp {
    CaughtUp {
        name: "main".to_string(),
        outcome,
    }
}

#[test]
fn main_taken_in_is_told_after_the_list_with_the_files_it_changed() {
    let small = Small::new();
    let took = caught(Outcome::TookIn {
        main: "beef".to_string(),
        merge: "c0ffee".to_string(),
        files: vec!["a.rs".to_string(), "b.rs".to_string()],
    });
    // What ran the round is told after it, last.
    let expected = small.told().replace(
        "## Why you are run",
        "## main taken in\nThe engine merged main at beef into the work's branch as this run started, as the commit \
         c0ffee. The files main changed since the branch last took it in:\na.rs\nb.rs\n\n## Why you are run",
    );
    assert_eq!(told(&small, false, &took), expected);
}

#[test]
fn main_in_conflict_is_told_with_its_files_and_how_to_merge_it() {
    let small = Small::new();
    let conflict = caught(Outcome::Conflict {
        main: "beef".to_string(),
        files: vec!["a.rs".to_string()],
    });
    let said = told(&small, false, &conflict);
    assert!(
        said.contains(
            "\n\n## main not taken in\nmain at beef does not merge into the work's branch without a conflict, so the \
             engine took nothing in. The files in conflict:\na.rs\nMerge it yourself, with `git merge beef`, before \
             you change the code.\n\n## Why you are run"
        ),
        "{said}"
    );
}

#[test]
fn changes_not_committed_and_a_failure_are_told_why_nothing_was_taken_in() {
    let small = Small::new();
    let left = caught(Outcome::Uncommitted {
        main: "beef".to_string(),
        changes: " M a.rs".to_string(),
        merging: false,
    });
    let said = told(&small, false, &left);
    assert!(
        said.contains(
            "\n\n## main not taken in\nThe worktree holds changes not committed, which taking main at beef in could \
             lose, so the engine took nothing in:\n M a.rs\nCommit them, then merge it yourself, with `git merge \
             beef`.\n\n## Why you are run"
        ),
        "{said}"
    );
    let merging = caught(Outcome::Uncommitted {
        main: "beef".to_string(),
        changes: "UU a.rs".to_string(),
        merging: true,
    });
    let said = told(&small, false, &merging);
    assert!(
        said.contains(
            "\n\n## main not taken in\nA merge is under way in the worktree and is not committed, so the engine \
             took nothing in:\nUU a.rs\nFinish it and commit it, then merge main yourself, with `git merge beef`.\n\n\
             ## Why you are run"
        ),
        "{said}"
    );
    let failed = caught(Outcome::Failed {
        why: "the network is down".to_string(),
    });
    let said = told(&small, false, &failed);
    assert!(
        said.contains(
            "\n\n## main not taken in\nThe engine could not take main in as this run started: the network is down\n\n\
             ## Why you are run"
        ),
        "{said}"
    );
}

#[test]
fn a_branch_that_held_main_is_told_nothing_of_it() {
    let small = Small::new();
    let held = caught(Outcome::Holds {
        main: "beef".to_string(),
    });
    assert_eq!(told(&small, false, &held), small.told());
}

#[test]
fn a_later_round_is_told_it_only_where_it_begins_a_session() {
    let small = asked();
    let took = caught(Outcome::TookIn {
        main: "beef".to_string(),
        merge: "c0ffee".to_string(),
        files: vec!["a.rs".to_string()],
    });
    assert_eq!(
        told(&small, false, &took),
        small.told(),
        "a session that goes on holds it"
    );
    let begun = told(&small, true, &took);
    let list_ends = begun.find("## main taken in").expect("it is told");
    let results = begun
        .find("## The results of what you asked for")
        .expect("with the results");
    assert!(
        list_ends < results,
        "after the task and the list, before the results: {begun}"
    );
}
