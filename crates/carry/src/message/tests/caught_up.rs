//! What a round is told of taking main in as its run started: after the task and the list, wherever they are given, in
//! one line, in the words of the part `main` of the message.

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
        task: None,
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
fn main_taken_in_is_told_as_the_commit_merged_in_and_the_command_that_shows_what_it_changed() {
    let small = Small::new();
    let took = caught(Outcome::TookIn {
        main: "beef".to_string(),
        merge: "c0ffee".to_string(),
    });
    // What ran the round is told after it, last.
    let expected = small.told().replace(
        "## Why you are run",
        "main at beef was merged into the work's branch as the commit c0ffee; `git diff --name-only c0ffee^1 c0ffee` \
         shows what it changed.\n\n## Why you are run",
    );
    assert_eq!(told(&small, false, &took), expected);
}

#[test]
fn main_in_conflict_is_told_with_its_files_and_no_instruction() {
    let small = Small::new();
    let conflict = caught(Outcome::Conflict {
        main: "beef".to_string(),
        files: vec!["a.rs".to_string(), "b.rs".to_string()],
    });
    let said = told(&small, false, &conflict);
    assert!(
        said.contains(
            "\n\nmain at beef was not merged into the work's branch: it conflicts in a.rs, b.rs.\n\n## Why you are run"
        ),
        "{said}"
    );
}

#[test]
fn changes_not_committed_and_a_failure_are_told_why_nothing_was_taken_in() {
    let small = Small::new();
    let left = caught(Outcome::Uncommitted {
        main: "beef".to_string(),
        changes: " M a.rs\n?? b.rs".to_string(),
        merging: false,
    });
    let said = told(&small, false, &left);
    assert!(
        said.contains(
            "\n\nmain at beef was not merged into the work's branch: the worktree holds changes not committed, a.rs, \
             b.rs.\n\n## Why you are run"
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
            "\n\nmain at beef was not merged into the work's branch: a merge is under way in the worktree, not \
             committed, a.rs.\n\n## Why you are run"
        ),
        "{said}"
    );
    let failed = caught(Outcome::Failed {
        why: "the network is down".to_string(),
    });
    let said = told(&small, false, &failed);
    assert!(
        said.contains("\n\nmain was not merged into the work's branch.\n\n## Why you are run"),
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
    });
    assert_eq!(
        told(&small, false, &took),
        small.told(),
        "a session that goes on holds it"
    );
    let begun = told(&small, true, &took);
    let list_ends = begun.find("was merged into the work's branch").expect("it is told");
    let results = begun
        .find("## The results of what you asked for")
        .expect("with the results");
    assert!(
        list_ends < results,
        "after the task and the list, before the results: {begun}"
    );
}
