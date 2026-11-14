//! What a run that goes on does next, rule by rule: each test builds a run's log as plain data and says what the run
//! does next, as a whole. What each test expects is taken from the module's documentation and the entries of the
//! design, never from its code. The tests are kept in modules by subject, so that a new test goes into the module of
//! what it tests.

mod calls;
mod failing;
mod handing_in;
mod limits;
mod malformed;
mod paused;
mod paused_answers;
mod rounds;

use std::collections::BTreeMap;

use decide::run::{Call, Failure, Hold, Limit, Log, Requires, Return, Round, Standing, Step, Told, Unmet, next};
use decide::{BlockId, CallId, Seq, Write};

/// What the flow requires of a cell, owned so that tests can lend it as `Requires`.
struct Spec {
    outputs: Vec<BlockId>,
    must_write: Vec<BlockId>,
    choices: BTreeMap<BlockId, Vec<String>>,
    returns: u32,
    told: Told,
}

impl Spec {
    fn requires(&self) -> Requires<'_> {
        Requires {
            outputs: &self.outputs,
            must_write: &self.must_write,
            choices: &self.choices,
            returns: self.returns,
            told: &self.told,
        }
    }
}

/// A cell that may write the plan and the verdict, a choice of yes or no, and need hand in neither; its returns may be
/// sent back twice. Its round was run by the request and the opinions, and was told of the latest version of each
/// block it may write: the plan's of version 4, the verdict's, yes, of version 5.
fn spec() -> Spec {
    let standing = |at: u64, value: Option<&str>| Standing {
        told: Some(Seq(at)),
        latest: Some(Seq(at)),
        value: value.map(str::to_string),
    };
    Spec {
        outputs: vec!["plan".into(), "verdict".into()],
        must_write: vec![],
        choices: BTreeMap::from([("verdict".into(), vec!["yes".to_string(), "no".to_string()])]),
        returns: 2,
        told: Told {
            ran_by: vec!["request".into(), "opinions".into()],
            outputs: BTreeMap::from([
                ("plan".into(), standing(4, None)),
                ("verdict".into(), standing(5, Some("yes"))),
            ]),
            waits_for_calls: false,
            questions: BTreeMap::new(),
        },
    }
}

/// The same cell, which must hand in these blocks each time.
fn must_write(blocks: &[&str]) -> Spec {
    Spec {
        must_write: blocks.iter().map(|&b| b.into()).collect(),
        ..spec()
    }
}

/// The same cell with another limit of returns sent back.
fn limited(returns: u32) -> Spec {
    Spec { returns, ..spec() }
}

/// What the run does next on these rounds, never let go on.
fn step(rounds: Vec<Round>, spec: &Spec) -> Step {
    next(
        &Log {
            rounds,
            ..Log::default()
        },
        spec.requires(),
        Hold::Nothing,
    )
}

/// What the run does next on these rounds, the owner having last let it go on when this many had ended.
fn step_let_go(rounds: Vec<Round>, let_go: usize, spec: &Spec) -> Step {
    next(
        &Log {
            rounds,
            let_go,
            let_on: 0,
        },
        spec.requires(),
        Hold::Nothing,
    )
}

/// What the run does next on these rounds while its work is paused, never let go on.
fn step_paused(rounds: Vec<Round>, spec: &Spec) -> Step {
    next(
        &Log {
            rounds,
            ..Log::default()
        },
        spec.requires(),
        Hold::Rounds,
    )
}

/// What the run, started for what a person wrote, does next on these rounds while its work is paused, never let go
/// on.
fn step_paused_answering(rounds: Vec<Round>, spec: &Spec) -> Step {
    next(
        &Log {
            rounds,
            ..Log::default()
        },
        spec.requires(),
        Hold::Retry,
    )
}

/// A write of prose.
fn prose(block: &str) -> Write {
    Write {
        block: block.into(),
        meaning_unchanged: false,
        value: None,
    }
}

/// A write of a value.
fn valued(block: &str, value: &str) -> Write {
    Write {
        block: block.into(),
        meaning_unchanged: false,
        value: Some(value.to_string()),
    }
}

/// A round under way.
fn under_way() -> Round {
    Round { returned: None }
}

/// A round that failed for a cause of this kind, the engine having made no wait after it.
fn failed_as(failure: Failure) -> Round {
    waited_after(failure, &[])
}

/// A round that failed for a cause of this kind, the engine having made these waits after it, each with its result or
/// not.
fn waited_after(failure: Failure, waits: &[(u64, bool)]) -> Round {
    Round {
        returned: Some(Return::Failed {
            failure,
            why: "it failed".to_string(),
            waits: (waits.iter())
                .map(|&(call, done)| Call {
                    call: CallId(call),
                    done,
                })
                .collect(),
        }),
    }
}

/// A round whose return did the cell's work: it dealt with everything that ran the round, and wrote these.
fn handed_in(writes: Vec<Write>) -> Round {
    works(&["request", "opinions"], writes)
}

/// A round whose return dealt with these, and wrote these.
fn works(dealt_with: &[&str], writes: Vec<Write>) -> Round {
    came(dealt_with, writes, vec![], false)
}

/// A round whose return dealt with these, wrote these, was found to have these wrong as it was read, and asked a
/// question that waits or none.
fn came(dealt_with: &[&str], writes: Vec<Write>, found: Vec<Unmet>, waits: bool) -> Round {
    Round {
        returned: Some(Return::Came {
            dealt_with: dealt_with.iter().map(|&b| b.into()).collect(),
            writes,
            calls: 0,
            found,
            waits,
            questions: Vec::new(),
        }),
    }
}

/// What a run finishes with that dealt with everything that ran its round and wrote these.
fn finish(writes: Vec<Write>) -> Step {
    Step::Finish {
        dealt_with: vec!["request".into(), "opinions".into()],
        writes,
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

/// A round whose return asked for this many calls, and for nothing else.
fn asks(calls: usize) -> Round {
    Round {
        returned: Some(Return::Came {
            dealt_with: vec![],
            writes: vec![],
            calls,
            found: vec![],
            waits: false,
            questions: Vec::new(),
        }),
    }
}

/// A round whose return dealt with nothing, wrote nothing and asked for nothing.
fn asks_nothing() -> Round {
    asks(0)
}

/// What a run finishes with whose return asked for calls: it dealt with nothing and writes nothing.
fn finish_asking() -> Step {
    Step::Finish {
        dealt_with: vec![],
        writes: vec![],
    }
}

fn send_back(unmet: Vec<Unmet>) -> Step {
    Step::SendBack { unmet }
}

fn returns_limit(sent_back: u32) -> Step {
    Step::AtLimit(Limit::Returns { sent_back })
}
