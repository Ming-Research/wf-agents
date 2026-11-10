//! The walk, rule by rule: each test builds a small work through the helper, which records exactly what each decision
//! gives, and says what the walk decides, as a whole. What each test expects is taken from the entries of the design,
//! never from the deciding code. The tests are kept in modules by subject, so that a new test goes into the module of
//! what it tests; this file holds what the modules build their expectations from, and the works several of them start
//! from.

#[path = "../support/mod.rs"]
mod support;

mod changed_flow;
mod failing;
mod faults;
mod flows;
mod inputs;
mod limit;
mod meaning;
mod named_tests;
mod opinions;
mod order;
mod preconditions;
mod rounds;
mod skipping;
mod stale;

use std::collections::BTreeMap;

use decide::{
    Abandoned, Abandoning, BlockId, CallId, Cause, Cell, Condition, Decision, Done, End, Flow, FlowFault, Pauser,
    RecordFault, Returned, Round, RunId, Seq, Waited, Who, Write, Written,
};
use support::Says::{DoesNotMatter, Matters, Value};
use support::{World, cell, gate, merge_word, resting};

/// Versions of blocks, by name.
type Versions<'a> = &'a [(&'a str, Option<Seq>)];

/// The decision to run the cell, because of these, from these versions of the blocks it reads or may write.
fn runs(cell: &str, because: Vec<Cause>, basis: Versions<'_>) -> Decision {
    Decision::Run {
        cell: cell.into(),
        because,
        basis: basis
            .iter()
            .map(|(b, v)| (BlockId::from(*b), *v))
            .collect::<BTreeMap<_, _>>(),
    }
}

/// A block the cell reads has another meaning than the version its record knows; `behind` where a cell after it
/// wrote the current version.
fn changed(block: &str, was: Option<Seq>, behind: bool) -> Cause {
    Cause::Changed {
        block: block.into(),
        was,
        behind,
    }
}

fn go_on(cell: &str, run: RunId) -> Decision {
    Decision::GoOn { cell: cell.into(), run }
}

fn busy(cell: &str, run: RunId) -> Decision {
    Decision::Busy { cell: cell.into(), run }
}

fn abandon(cell: &str, run: RunId, because: Abandoning) -> Decision {
    Decision::Abandon {
        cell: cell.into(),
        run,
        because,
    }
}

fn wait(at: &str, on: Vec<Waited>) -> Decision {
    Decision::Wait { at: at.into(), on }
}

fn outdated(block: &str) -> Abandoning {
    Abandoning::Outdated { block: block.into() }
}

fn precondition(test: &str, who: Who) -> Waited {
    Waited::Precondition { test: test.into(), who }
}

/// A cell that runs only when the block's current value is this one.
fn when(block: &str, value: &str, mut c: Cell) -> Cell {
    c.applies = Some(Condition::Is {
        block: block.into(),
        value: value.into(),
    });
    c
}

/// A cell that runs only while the named test holds.
fn on_test(test: &str, mut c: Cell) -> Cell {
    c.applies = Some(Condition::Test(test.into()));
    c
}

fn with_limit(limit: u32, mut c: Cell) -> Cell {
    c.limit = limit;
    c
}

fn requiring(test: &str, mut c: Cell) -> Cell {
    c.requires.push(test.into());
    c
}

/// The cell the walk runs and, for each reason, its block and whether a cell after it wrote the change.
fn reasons(decision: &Decision) -> (String, Vec<(String, bool)>) {
    let Decision::Run { cell, because, .. } = decision else {
        panic!("the walk runs nothing: {decision:?}")
    };
    let because = (because.iter())
        .map(|c| match c {
            Cause::Changed { block, behind, .. } => (block.0.clone(), *behind),
            Cause::NoRecord => ("no record".to_string(), false),
        })
        .collect();
    (cell.0.clone(), because)
}

const NO_RECORD: Cause = Cause::NoRecord;

/// The implementation and its review, which writes its opinions for the implementation; both ran once, the review
/// with nothing to say.
fn reviewed() -> (World, Seq, Seq) {
    let cells = vec![
        cell("implement", &["plan", "opinions"], &["code"]),
        cell("review", &["code"], &["opinions"]),
    ];
    let mut w = World::with_gate(cells);
    let plan = w.person_writes("plan");
    let code = w.run("implement", &[("code", Matters)]);
    w.run("review", &[]);
    assert_eq!(w.decide(), resting());
    (w, plan, code)
}

/// The same two cells, the review not yet run: it judges the implementation's first code.
fn judged() -> (World, Seq, Seq) {
    let cells = vec![
        cell("implement", &["plan", "opinions"], &["code"]),
        cell("review", &["code"], &["opinions"]),
    ];
    let mut w = World::with_gate(cells);
    let plan = w.person_writes("plan");
    let code = w.run("implement", &[("code", Matters)]);
    (w, plan, code)
}

/// A research and the implementation of its plan, both run once: the background, the plan and the code.
fn planned() -> (World, Seq, Seq, Seq) {
    let cells = vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &["code"]),
    ];
    let mut w = World::with_gate(cells);
    let background = w.person_writes("background");
    let plan = w.run("research", &[("plan", Matters)]);
    let code = w.run("implement", &[("code", Matters)]);
    assert_eq!(w.decide(), resting());
    (w, background, plan, code)
}

/// A cell that writes a choice, and one that reads it.
fn chosen() -> (World, Seq) {
    let mut w = World::with_gate(vec![
        cell("ask", &["topic"], &["choice"]),
        cell("use", &["choice"], &[]),
    ]);
    w.person_writes("topic");
    let choice = w.run("ask", &[("choice", Value("yes"))]);
    w.run("use", &[]);
    assert_eq!(w.decide(), resting());
    (w, choice)
}

/// An implementation, a review that says whether to test more, the testing that runs only then, and a check that
/// reads the code and the tests; all ran once. Returns the code and the tests.
fn tested() -> (World, Seq, Seq) {
    let cells = vec![
        cell("implement", &["brief"], &["code"]),
        cell("review", &["code"], &["more"]),
        when("more", "yes", cell("testing", &["code"], &["tests"])),
        cell("check", &["code", "tests"], &[]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("brief");
    let code = w.run("implement", &[("code", Matters)]);
    w.run("review", &[("more", Value("yes"))]);
    let tests = w.run("testing", &[("tests", Matters)]);
    w.run("check", &[]);
    assert_eq!(w.decide(), resting());
    (w, code, tests)
}

/// The review writes an opinion, and the implementation answers it with new code.
fn send_back(w: &mut World) {
    w.run("review", &[("opinions", Matters)]);
    w.run("implement", &[("code", Matters)]);
}

fn at_limit(w: &World, runs: u32) {
    assert_eq!(w.decide(), wait("implement", vec![Waited::Limit { runs }]));
}
