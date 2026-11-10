//! What a run that goes on does next, rule by rule: each test builds a run's log as plain data and says what the run
//! does next, as a whole. What each test expects is taken from the module's documentation and the entries of the
//! design, never from its code. The tests are kept in modules by subject, so that a new test goes into the module of
//! what it tests.

mod calls;
mod handing_in;
mod limits;
mod malformed;
mod rounds;

use std::collections::BTreeMap;

use decide::run::{Call, Limit, Log, Requires, Return, Round, Step, Unmet, next};
use decide::{BlockId, CallId, Write};

/// What the flow requires of a cell, owned so that tests can lend it as `Requires`.
struct Spec {
    outputs: Vec<BlockId>,
    choices: BTreeMap<BlockId, Vec<String>>,
    returns: u32,
    calls: u32,
}

impl Spec {
    fn requires(&self) -> Requires<'_> {
        Requires {
            outputs: &self.outputs,
            choices: &self.choices,
            returns: self.returns,
            calls: self.calls,
        }
    }
}

/// A cell that may write the plan and the verdict, a choice of yes or no; its returns may be sent back twice, and it
/// may make three calls.
fn spec() -> Spec {
    Spec {
        outputs: vec!["plan".into(), "verdict".into()],
        choices: BTreeMap::from([("verdict".into(), vec!["yes".to_string(), "no".to_string()])]),
        returns: 2,
        calls: 3,
    }
}

/// The same cell with other limits.
fn limited(returns: u32, calls: u32) -> Spec {
    Spec {
        returns,
        calls,
        ..spec()
    }
}

/// What the run does next on these rounds, never let go on.
fn step(rounds: Vec<Round>, spec: &Spec) -> Step {
    next(&Log { rounds, let_go: 0 }, spec.requires())
}

/// What the run does next on these rounds, the owner having last let it go on when this many had ended.
fn step_let_go(rounds: Vec<Round>, let_go: usize, spec: &Spec) -> Step {
    next(&Log { rounds, let_go }, spec.requires())
}

/// A write of prose.
fn prose(block: &str) -> Write {
    Write {
        block: block.into(),
        matters: true,
        value: None,
    }
}

/// A write of a value.
fn valued(block: &str, value: &str) -> Write {
    Write {
        block: block.into(),
        matters: true,
        value: Some(value.to_string()),
    }
}

/// A round under way.
fn under_way() -> Round {
    Round { returned: None }
}

/// A round that handed in these writes.
fn handed_in(writes: Vec<Write>) -> Round {
    Round {
        returned: Some(Return::HandedIn(writes)),
    }
}

/// A round that handed in what meets nothing: the plan with a value.
fn wanting() -> Round {
    handed_in(vec![valued("plan", "yes")])
}

/// What `wanting` does not meet.
fn wanting_unmet() -> Vec<Unmet> {
    vec![Unmet::NotAChoice { block: "plan".into() }]
}

/// A round that asked for this many calls, none of them made yet.
fn asks(asked: usize) -> Round {
    Round {
        returned: Some(Return::Asked { asked, made: vec![] }),
    }
}

/// A round that asked for no calls at all.
fn asks_nothing() -> Round {
    asks(0)
}

/// A round whose calls were made, each with whether it has its result.
fn made(calls: &[(u64, bool)]) -> Round {
    Round {
        returned: Some(Return::Asked {
            asked: calls.len(),
            made: calls
                .iter()
                .map(|(id, done)| Call {
                    call: CallId(*id),
                    done: *done,
                })
                .collect(),
        }),
    }
}

/// A round that asked for this many calls, each made and with its result, the ids starting at `from`.
fn asked_done(from: u64, n: u64) -> Round {
    made(&(from..from + n).map(|id| (id, true)).collect::<Vec<_>>())
}

fn send_back(unmet: Vec<Unmet>) -> Step {
    Step::SendBack { unmet }
}

fn calls_limit(made: u32) -> Step {
    Step::AtLimit(Limit::Calls { made })
}

fn returns_limit(sent_back: u32) -> Step {
    Step::AtLimit(Limit::Returns { sent_back })
}
