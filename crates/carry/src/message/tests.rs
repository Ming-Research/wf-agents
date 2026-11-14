//! The messages of a small flow, its records made by the engine's own loop: a plan and a build on one line, a review
//! on another, and the merge.

use std::collections::{BTreeMap, BTreeSet};

use decide::{BlockId, CellId, Outsider, RunId};
use engine::{Came, Effect, Event, handle};
use flow::{Compiled, Has, Source};
use work::{Ask, Given, Handed, Spent, Time, Work};

use super::{Opening, message};

mod caught_up;

const FLOW: &str = r#"
name = "small"
order = ["plan", "build", "review", "merge"]

[limits]
runs = 3
returns = 2
calls = 4

[lines.engineer]
[lines.reviewer]

[blocks.request]
people = true

[blocks.plan]
[blocks.opinions]
[blocks.code]

[blocks.verdict]
values = ["yes", "no"]

[blocks.merged]
shown = true

[tests.content]
says = "The review is content"
kind = "all"
reads = ["verdict"]
is = "yes"
"#;

const CELLS: [(&str, &str); 4] = [
    (
        "plan",
        r#"
title = "Plan"
kind = "agent"
effort = "high"
line = "engineer"
task = "Write a plan for the request."
inputs = ["request"]
outputs = ["plan"]
"#,
    ),
    (
        "build",
        r#"
title = "Build"
kind = "agent"
effort = "high"
line = "engineer"
task = "Build what the plan says."
inputs = ["request", "plan", "opinions"]
outputs = ["code"]
"#,
    ),
    (
        "review",
        r#"
title = "Review"
kind = "agent"
effort = "high"
line = "reviewer"
task = "Review the code."
inputs = ["code"]
outputs = ["opinions", "verdict"]
must_write = ["verdict"]
"#,
    ),
    (
        "merge",
        r#"
title = "Merge"
kind = "engine"
act = "merge"
requires = { test = "content" }
inputs = ["code"]
outputs = ["merged"]
"#,
    ),
];

fn flow() -> Compiled {
    let source = Source {
        flow: FLOW.to_string(),
        cells: (CELLS.iter())
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect::<BTreeMap<_, _>>(),
        ..Default::default()
    };
    let has = Has {
        sources: BTreeSet::new(),
        acts: BTreeSet::from(["merge".to_string()]),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the small flow compiles")
}

/// A work of the small flow, taken one thing at a time through its loop.
pub(crate) struct Small {
    pub(crate) flow: Compiled,
    pub(crate) work: Work,
    /// What the last thing taken gave to be done.
    effects: Vec<Effect>,
}

impl Small {
    /// A work whose request a person wrote.
    pub(crate) fn new() -> Small {
        let mut small = Small {
            flow: flow(),
            work: Work::default(),
            effects: Vec::new(),
        };
        small.take(Event::Wrote {
            block: "request".into(),
            by: Outsider::Person,
            content: "Greet the user.".to_string(),
            matters: true,
            value: None,
        });
        small
    }

    pub(crate) fn take(&mut self, event: Event) {
        self.effects = handle(&self.flow, &mut self.work, Time(0), event).expect("the loop takes it");
    }

    /// The round given to be carried out.
    pub(crate) fn round(&self) -> (RunId, CellId, usize, Given) {
        (self.effects.iter())
            .find_map(|effect| match effect {
                Effect::Round {
                    run,
                    cell,
                    number,
                    given,
                } => Some((*run, cell.clone(), *number, given.clone())),
                _ => None,
            })
            .expect("a round is given")
    }

    /// What the round given is told, in a session that goes on.
    pub(crate) fn told(&self) -> String {
        self.told_as(false)
    }

    /// What the round given is told, in a session it begins or one that goes on.
    pub(crate) fn told_as(&self, begins: bool) -> String {
        let (run, cell, number, given) = self.round();
        let spec = self.flow.cells.iter().find(|spec| spec.id == cell).expect("the cell");
        message(
            &self.flow,
            &self.work,
            spec,
            run,
            number,
            &given,
            &Opening { begins, main: None },
        )
    }

    /// The round given hands these blocks in, each with its value where it has one.
    pub(crate) fn hands_in(&mut self, blocks: &[(&str, &str, Option<&str>)]) {
        let came = Came::hands_in(
            (blocks.iter())
                .map(|(block, content, value)| Handed {
                    block: BlockId::from(*block),
                    content: content.to_string(),
                    matters: true,
                    value: value.map(str::to_string),
                })
                .collect(),
        );
        self.back(came);
    }

    pub(crate) fn back(&mut self, came: Came) {
        let (run, ..) = self.round();
        self.take(Event::Returned {
            run,
            came,
            spent: Spent::default(),
        });
    }

    /// The call the last thing taken made has this result.
    pub(crate) fn done(&mut self, result: &str) {
        let call = (self.effects.iter())
            .find_map(|effect| match effect {
                Effect::Call { call, .. } => Some(*call),
                _ => None,
            })
            .expect("a call is made");
        self.take(Event::Done {
            call,
            result: result.to_string(),
        });
    }

    /// The version the run of this cell that finished last wrote.
    pub(crate) fn wrote(&self, cell: &str) -> u64 {
        let runs: Vec<RunId> = (self.work.records.started.iter())
            .filter(|s| s.cell.as_str() == cell)
            .map(|s| s.run)
            .collect();
        let finished = self.work.records.finished.iter().rev().find(|f| runs.contains(&f.run));
        finished.expect("the cell finished").at.0
    }

    /// The version of the request.
    pub(crate) fn request(&self) -> u64 {
        self.work.records.written[0].at.0
    }
}

#[test]
fn a_first_round_is_given_the_task_and_the_list_of_its_blocks_with_no_content() {
    let small = Small::new();
    let request = small.request();
    let expected = format!(
        "# Plan (cell `plan`), round 1

## Your task

Write a plan for the request.

## What it reads

This is the cell's first run in this work.

- `request`: version {request}, written by a person.

## What it may write

- `plan`: not written yet.

To read all of it at once, ask for the tool `read` with the input `request@{request}`.
"
    );
    assert_eq!(small.told(), expected);
    assert!(!small.told().contains("Greet the user."), "no content is sent");
}

#[test]
fn a_cell_of_the_line_is_told_every_block_whoever_wrote_it() {
    let mut small = Small::new();
    small.hands_in(&[("plan", "Greet by name.", None)]);
    let (request, plan) = (small.request(), small.wrote("plan"));
    let expected = format!(
        "# Build (cell `build`), round 1

## Your task

Build what the plan says.

## What it reads

This is the cell's first run in this work.

- `request`: version {request}, written by a person.
- `plan`: version {plan}, written by the cell `plan`.
- `opinions`: not written yet.

## What it may write

- `code`: not written yet.

To read all of it at once, ask for the tool `read` with the input `request@{request} plan@{plan}`.
"
    );
    assert_eq!(small.told(), expected);
}

/// The small work with its review objecting to the first build: the build runs a second time.
pub(crate) fn objected() -> Small {
    let mut small = Small::new();
    small.hands_in(&[("plan", "Greet by name.", None)]);
    small.hands_in(&[("code", "A greeting.", None)]);
    small.hands_in(&[
        ("opinions", "Name the error.", None),
        ("verdict", "Not yet.", Some("no")),
    ]);
    small
}

#[test]
fn a_second_run_is_told_what_changed_since_it_last_worked() {
    let small = objected();
    let (request, plan, code, opinions) = (
        small.request(),
        small.wrote("plan"),
        small.wrote("build"),
        small.wrote("review"),
    );
    let expected = format!(
        "# Build (cell `build`), round 1

## Your task

Build what the plan says.

## What it reads

- `request`: version {request}, written by a person; you last worked from this version.
- `plan`: version {plan}, written by the cell `plan`; you last worked from this version.
- `opinions`: version {opinions}, written by the cell `review`; changed since you last worked, when it was not written.

## What it may write

- `code`: version {code}; its meaning last changed at this version.

To read all of it at once, ask for the tool `read` with the input `request@{request} plan@{plan} opinions@{opinions} code@{code}`.
"
    );
    assert_eq!(small.told(), expected);
}

/// The small work with the review run a second time, on the second build; gives the work with the versions of the first code, the first opinions and the second code.
pub(crate) fn objected_twice() -> (Small, u64, u64, u64) {
    let mut small = objected();
    let (code, opinions) = (small.wrote("build"), small.wrote("review"));
    small.hands_in(&[("code", "A greeting by name.", None)]);
    let later = small.wrote("build");
    (small, code, opinions, later)
}

#[test]
fn a_choice_is_told_with_its_value_and_a_change_with_what_reads_it() {
    let (small, code, opinions, later) = objected_twice();
    let expected = format!(
        "# Review (cell `review`), round 1

## Your task

Review the code.

## What it reads

- `code`: version {later}, written by the cell `build`; changed since you last worked from version {code}.

## What it may write

- `opinions`: version {opinions}; its meaning last changed at this version.
- `verdict`, a choice, one of `yes`, `no` (hand it in every time): version {opinions}, with the value `no`; its meaning last changed at this version.

To read all of it at once, ask for the tool `read` with the input `code@{later} code@{code}..{later} opinions@{opinions} verdict@{opinions}`.
"
    );
    assert_eq!(small.told(), expected);
}

#[test]
fn a_changed_input_with_a_version_before_is_read_as_a_difference() {
    let (mut small, _, first, _) = objected_twice();
    small.hands_in(&[
        ("opinions", "Name the error, and say it.", None),
        ("verdict", "Not yet.", Some("no")),
    ]);
    let second = small.wrote("review");
    let told = small.told();
    assert!(
        told.contains(&format!(
            "- `opinions`: version {second}, written by the cell `review`; changed since you last worked from version \
             {first}."
        )),
        "{told}"
    );
    let (request, plan, code) = (small.request(), small.wrote("plan"), small.wrote("build"));
    let all = format!("`request@{request} plan@{plan} opinions@{second} opinions@{first}..{second} code@{code}`");
    assert!(told.contains(&all), "{told}");
}

#[test]
fn a_run_entered_for_the_results_of_calls_is_given_them() {
    let small = asked();
    let expected = "# Build (cell `build`), round 1

## The results of what you asked for

### The tool `check`, on: the branch

The check passed.

Go on with the task.
";
    assert_eq!(small.told(), expected);
}

#[test]
fn a_round_that_begins_a_session_is_given_the_task_and_the_list_before_the_results() {
    let small = asked();
    let told = small.told_as(true);
    let task = told.find("## Your task").expect("the task");
    let list = told.find("## What it reads").expect("the list");
    let results = told.find("## The results of what you asked for").expect("the results");
    assert!(task < list && list < results, "{told}");
    assert!(told.starts_with("# Build (cell `build`), round 1\n"), "{told}");
    assert!(told.ends_with("The check passed.\n\nGo on with the task.\n"), "{told}");
}

/// The build's second run asks for the check, which passes, and the build is entered again for the result.
fn asked() -> Small {
    let mut small = objected();
    small.back(Came::asks(vec![Ask::Tool {
        name: "check".to_string(),
        input: "the branch".to_string(),
    }]));
    small.done("The check passed.");
    small
}

#[test]
fn a_run_entered_for_results_that_begins_a_session_is_given_its_own_results_and_none_of_an_earlier_run() {
    let mut small = objected();
    small.back(Came::asks(vec![Ask::Person {
        who: decide::Who::Owner,
        question: "Which name?".to_string(),
    }]));
    small.done("By name.");
    small.back(Came::asks(vec![Ask::Tool {
        name: "check".to_string(),
        input: String::new(),
    }]));
    small.done("The check passed.");
    let told = small.told_as(true);
    let expected = "## The results of what you asked for

### The tool `check`

The check passed.

Go on with the task.
";
    assert!(
        told.starts_with("# Build (cell `build`), round 1\n\n## Your task"),
        "{told}"
    );
    assert!(told.ends_with(expected), "{told}");
    assert!(
        !told.contains("Which name?"),
        "an earlier run's results are not given: {told}"
    );
}

#[test]
fn a_read_an_earlier_run_asked_for_is_not_given_again() {
    let mut small = objected();
    small.back(Came::asks(vec![Ask::Tool {
        name: "read".to_string(),
        input: "plan@5 opinions".to_string(),
    }]));
    small.done("### `plan`, version 5\n\nGreet by name.");
    small.back(Came::asks(vec![Ask::Tool {
        name: "read".to_string(),
        input: "request".to_string(),
    }]));
    small.done("### `request`, version 1\n\nGreet the user.");
    let told = small.told_as(true);
    assert!(
        !told.contains("Greet by name."),
        "an earlier read is not given again: {told}"
    );
    assert!(
        told.ends_with("Greet the user.\n\nGo on with the task.\n"),
        "the run's own read is given: {told}"
    );
}

#[test]
fn a_call_the_work_does_not_hold_is_said_so() {
    let small = objected();
    let (run, cell, ..) = small.round();
    let spec = small.flow.cells.iter().find(|spec| spec.id == cell).unwrap();
    let given = Given::Results(vec![decide::CallId(99)]);
    let told = message(&small.flow, &small.work, spec, run, 2, &given, &Opening::default());
    assert!(
        told.contains("### Call 99, which the work does not hold\n\n(It has no result.)"),
        "{told}"
    );
}

#[test]
fn a_round_after_a_return_sent_back_is_told_why() {
    let mut small = objected();
    small.hands_in(&[("verdict", "Fine.", Some("yes"))]);
    let expected = "# Build (cell `build`), round 2

## What you handed in was sent back

- the block verdict is not one the cell may write

Hand in again: everything the cell hands in, in one return.
";
    assert_eq!(small.told(), expected);
}

#[test]
fn a_review_that_leaves_out_the_block_it_must_hand_in_is_told_so() {
    let (mut small, ..) = objected_twice();
    small.hands_in(&[("opinions", "Name the error, and say it.", None)]);
    let expected = "# Review (cell `review`), round 2

## What you handed in was sent back

- the block verdict is not handed in, and the cell must hand it in every time

Hand in again: everything the cell hands in, in one return.
";
    assert_eq!(small.told(), expected);
}

#[test]
fn a_message_built_again_is_the_same() {
    let small = objected();
    assert_eq!(small.told(), small.told());
    assert_eq!(small.told_as(true), small.told_as(true));
}
