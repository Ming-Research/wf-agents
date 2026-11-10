//! The messages of a small flow, its records made by the engine's own loop: a plan and a build on one line, a review
//! on another, and the merge.

use std::collections::{BTreeMap, BTreeSet};

use decide::{BlockId, CellId, Outsider, RunId};
use engine::{Came, Effect, Event, handle};
use flow::{Compiled, Has, Source};
use work::{Ask, Given, Handed, Spent, Time, Work};

use super::message;

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
line = "reviewer"
task = "Review the code."
inputs = ["code"]
outputs = ["opinions", "verdict"]
"#,
    ),
    (
        "merge",
        r#"
title = "Merge"
kind = "engine"
act = "merge"
requires = ["content"]
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
    };
    let has = Has {
        sources: BTreeSet::new(),
        acts: BTreeSet::from(["merge".to_string()]),
    };
    flow::compile(&source, &has).expect("the small flow compiles")
}

/// A work of the small flow, taken one thing at a time through its loop.
struct Small {
    flow: Compiled,
    work: Work,
    /// What the last thing taken gave to be done.
    effects: Vec<Effect>,
}

impl Small {
    /// A work whose request a person wrote.
    fn new() -> Small {
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

    fn take(&mut self, event: Event) {
        self.effects = handle(&self.flow, &mut self.work, Time(0), event).expect("the loop takes it");
    }

    /// The round given to be carried out.
    fn round(&self) -> (RunId, CellId, usize, Given) {
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

    /// What the round given is told.
    fn told(&self) -> String {
        let (run, cell, number, given) = self.round();
        let spec = self.flow.cells.iter().find(|spec| spec.id == cell).expect("the cell");
        message(&self.flow, &self.work, spec, run, number, &given)
    }

    /// The round given hands these blocks in, each with its value where it has one.
    fn hands_in(&mut self, blocks: &[(&str, &str, Option<&str>)]) {
        let came = Came::HandsIn(
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

    fn back(&mut self, came: Came) {
        let (run, ..) = self.round();
        self.take(Event::Returned {
            run,
            came,
            spent: Spent::default(),
        });
    }

    /// The version the run of this cell that finished last wrote.
    fn wrote(&self, cell: &str) -> u64 {
        let runs: Vec<RunId> = (self.work.records.started.iter())
            .filter(|s| s.cell.as_str() == cell)
            .map(|s| s.run)
            .collect();
        let finished = self.work.records.finished.iter().rev().find(|f| runs.contains(&f.run));
        finished.expect("the cell finished").at.0
    }

    /// The version of the request.
    fn request(&self) -> u64 {
        self.work.records.written[0].at.0
    }
}

#[test]
fn a_first_round_is_given_the_task_and_every_input() {
    let small = Small::new();
    let request = small.request();
    let expected = format!(
        "# Plan (cell `plan`), round 1

## Your task

Write a plan for the request.

## Why it runs now

This is the cell's first run in this work.

## What it reads

### `request`, version {request}, written by a person

Greet the user.

## What it may hand in

- `plan`
"
    );
    assert_eq!(small.told(), expected);
}

#[test]
fn a_later_cell_of_the_line_is_not_sent_again_what_the_line_read_or_wrote() {
    let mut small = Small::new();
    small.hands_in(&[("plan", "Greet by name.", None)]);
    let (request, plan) = (small.request(), small.wrote("plan"));
    let expected = format!(
        "# Build (cell `build`), round 1

## Your task

Build what the plan says.

## Why it runs now

This is the cell's first run in this work.

## What it reads

### `request`, version {request}, written by a person: unchanged since you read it.

### `plan`, version {plan}, written by the cell `plan`: as you wrote it.

### `opinions`: not written yet.

## What it may hand in

- `code`
"
    );
    assert_eq!(small.told(), expected);
}

/// The small work with its review objecting to the first build: the build runs a second time.
fn objected() -> Small {
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
fn a_second_run_is_sent_what_changed_and_not_what_did_not() {
    let small = objected();
    let (request, plan, opinions) = (small.request(), small.wrote("plan"), small.wrote("review"));
    let expected = format!(
        "# Build (cell `build`), round 1

## Your task

Build what the plan says.

## Why it runs now

- `opinions` changed: it was not written when you last worked, and it is now version {opinions}. A cell after this one changed it.

## What it reads

### `request`, version {request}, written by a person: unchanged since you read it.

### `plan`, version {plan}, written by the cell `plan`: as you wrote it.

### `opinions`, version {opinions}, written by the cell `review`

Name the error.

## What it may hand in

- `code`
"
    );
    assert_eq!(small.told(), expected);
}

#[test]
fn a_round_after_calls_is_given_their_results() {
    let mut small = objected();
    small.back(Came::Asks(vec![Ask::Tool {
        name: "check".to_string(),
        input: "the branch".to_string(),
    }]));
    let call = (small.effects.iter())
        .find_map(|effect| match effect {
            Effect::Call { call, .. } => Some(*call),
            _ => None,
        })
        .expect("the tool is called");
    small.take(Event::Done {
        call,
        result: "The check passed.".to_string(),
    });
    let expected = "# Build (cell `build`), round 2

## The results of what you asked for

### The tool `check`, on: the branch

The check passed.

Go on with the task.
";
    assert_eq!(small.told(), expected);
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
fn a_message_built_again_is_the_same() {
    let small = objected();
    assert_eq!(small.told(), small.told());
}
