//! The engine's loop, rule by rule: each test takes a small work through `handle` and says what the records hold and
//! what is to be done outside, as a whole. What each test expects is taken from the crate's documentation and the
//! entries of the design, never from its code. The tests are kept in modules by subject, so that a new test goes into
//! the module of what it tests.

mod abandoning;
mod calls;
mod cancelling;
mod ending;
mod events;
mod given;
mod limits;
mod pausing;
mod pending;
mod rebuilt;
mod refused;

use std::collections::{BTreeMap, BTreeSet};

use decide::run::{Limit, Unmet};
use decide::{BlockId, CallId, Cause, Outcome, Outsider, RunId, Seq, TestName, Tests, Who};
use engine::{Ask, Call, Came, Effect, Event, Given, Handed, Refused, Work, handle, pending};
use flow::{Compiled, Source, compile};

/// The named tests: the merge waits for the owner's word until a test sets it otherwise.
struct Named(BTreeMap<TestName, Outcome>);

impl Tests for Named {
    fn test(&self, name: &TestName) -> Outcome {
        self.0.get(name).cloned().unwrap_or(Outcome::Holds)
    }
}

/// A writer that reads a request, the outside's main and the opinions on what it wrote; a judge that writes opinions
/// for it and a verdict, a choice; and a merge, the engine's own cell, that waits for the owner's word. Returns may be
/// sent back twice, a run may make three calls, and a cell may go round three times.
fn judged() -> Compiled {
    let flow = r#"
        name = "judged"
        order = ["write", "judge", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.main]
        [lines.other]
        [tests]
        may_merge = "The work may merge"
        [blocks.request]
        people = true
        [blocks.main]
        outside = true
        [blocks.opinions]
        people = true
        [blocks.text]
        [blocks.verdict]
        values = ["yes", "no"]
        people = true
        [blocks.merged]
        shown = true
    "#;
    let write = r#"
        title = "Write"
        kind = "agent"
        line = "main"
        inputs = ["request", "opinions", "main"]
        outputs = ["text"]
    "#;
    let judge = r#"
        title = "Judge"
        kind = "agent"
        line = "other"
        inputs = ["text"]
        outputs = ["opinions", "verdict"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        requires = ["may_merge"]
        inputs = ["text", "verdict"]
        outputs = ["merged"]
    "#;
    let source = Source {
        flow: flow.to_string(),
        cells: [("write", write), ("judge", judge), ("merge", merge)]
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
    };
    let known: BTreeSet<TestName> = ["may_merge".into()].into();
    compile(&source, &known).expect("the definition compiles")
}

/// A work on the judged flow, with the named tests it is walked with.
struct W {
    flow: Compiled,
    tests: Named,
    work: Work,
}

impl W {
    /// A new work whose merge waits for the owner's word.
    fn new() -> Self {
        Self {
            flow: judged(),
            tests: Named([("may_merge".into(), Outcome::Waits { who: Who::Owner })].into()),
            work: Work::default(),
        }
    }

    /// Takes the event, which the loop must take, and gives what is to be done.
    fn on(&mut self, event: Event) -> Vec<Effect> {
        handle(&self.flow, &self.tests, &mut self.work, event).expect("the event is taken")
    }

    /// The event, which the loop must refuse, leaving the work exactly as it was; and why.
    fn refused(&mut self, event: Event) -> Refused {
        let before = self.work.clone();
        let why = handle(&self.flow, &self.tests, &mut self.work, event).expect_err("the event is refused");
        assert_eq!(self.work, before, "a refused event changed the work");
        why
    }

    /// A person writes the block; the place of the entry names the version.
    fn person_writes(&mut self, block: &str) -> (Vec<Effect>, Seq) {
        let effects = self.on(wrote(block, Outsider::Person, true, None));
        (effects, self.last_written())
    }

    /// The round of the run under way ends with what came back.
    fn returns(&mut self, run: RunId, came: Came) -> Vec<Effect> {
        self.on(Event::Returned { run, came })
    }

    /// The call has its result.
    fn done(&mut self, call: CallId) -> Vec<Effect> {
        self.on(Event::Done {
            call,
            result: format!("the result of {}", call.0),
        })
    }

    /// The place of the last write by a person or the outside.
    fn last_written(&self) -> Seq {
        self.work.records.written.last().expect("a write").at
    }

    /// The place of the last finish of a run.
    fn last_finished(&self) -> Seq {
        self.work.records.finished.last().expect("a finish").at
    }

    /// A work whose writer has a request and its first round under way: the run of that round.
    fn requested() -> (Self, RunId, Seq) {
        let mut w = Self::new();
        let (effects, request) = w.person_writes("request");
        let (run, ..) = round(&effects);
        (w, run, request)
    }
}

/// A write by a person or the outside, its content saying what was written.
fn wrote(block: &str, by: Outsider, matters: bool, value: Option<&str>) -> Event {
    Event::Wrote {
        block: block.into(),
        by,
        content: format!("{block}, as written by {by:?}"),
        matters,
        value: value.map(str::to_string),
    }
}

/// A block handed in, its content naming it.
fn handed(block: &str, value: Option<&str>) -> Handed {
    Handed {
        block: block.into(),
        content: format!("the {block}"),
        matters: true,
        value: value.map(str::to_string),
    }
}

/// What came back: these blocks handed in.
fn hands_in(blocks: &[(&str, Option<&str>)]) -> Came {
    Came::HandsIn(blocks.iter().map(|(b, v)| handed(b, *v)).collect())
}

/// What the writer hands in that does not meet what the flow requires: the judge's verdict.
fn wanting() -> Came {
    hands_in(&[("verdict", Some("yes"))])
}

fn wanting_unmet() -> Vec<Unmet> {
    vec![Unmet::NotAnOutput {
        block: "verdict".into(),
    }]
}

fn tool(name: &str) -> Ask {
    Ask::Tool {
        name: name.to_string(),
        input: format!("what {name} is run on"),
    }
}

fn question(who: Who) -> Ask {
    Ask::Person {
        who,
        question: "which way?".to_string(),
    }
}

/// The one round the engine gives to be done: its run, cell, number and what it is given.
fn round(effects: &[Effect]) -> (RunId, String, usize, Given) {
    match effects {
        [
            Effect::Round {
                run,
                cell,
                number,
                given,
            },
        ] => (*run, cell.0.clone(), *number, given.clone()),
        other => panic!("not one round: {other:?}"),
    }
}

fn round_effect(run: RunId, cell: &str, number: usize, given: Given) -> Effect {
    Effect::Round {
        run,
        cell: cell.into(),
        number,
        given,
    }
}

fn task(because: Vec<Cause>, basis: &[(&str, Option<Seq>)]) -> Given {
    Given::Task {
        because,
        basis: basis.iter().map(|(b, v)| (BlockId::from(*b), *v)).collect(),
    }
}

fn changed(block: &str, was: Option<Seq>, behind: bool) -> Cause {
    Cause::Changed {
        block: block.into(),
        was,
        behind,
    }
}

/// What the run's first round was given.
fn given_of(w: &W, run: RunId) -> Given {
    w.work
        .given
        .get(&run)
        .and_then(|g| g.first())
        .expect("a round was given")
        .clone()
}

/// The calls given to be made, each with its run and what it is.
fn calls_of(effects: &[Effect]) -> Vec<(CallId, RunId, Call)> {
    (effects.iter())
        .map(|e| match e {
            Effect::Call { call, run, what } => (*call, *run, what.clone()),
            other => panic!("not a call: {other:?}"),
        })
        .collect()
}
