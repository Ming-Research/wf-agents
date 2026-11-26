//! When a line of a work begins a new session, as the driver works it out of the work's records after each thing that
//! happened and before the walk: so that a new session is recorded before the line's next round is given.
//!
//! With the installation's [`Policy`], a line begins a new session at its next round when the model its session's
//! rounds ran on, as the work's records say, is not the one its rounds run on now; or when what a new session of it
//! would begin with now, its system prompt or its form of return, as the carriers make it from the flow and the work's
//! worktree, is not what its session began with, or cannot be made ([`Policy::changed`]): a session keeps what it
//! began with, so a flow installed since, main taken in or the work's own change of the project's files reaches the
//! line only in a new session (decision `a-changed-system-prompt-or-form-begins-a-new-session`). No line begins one for
//! the account its rounds run under: the session lives in the one directory of the program's, and the service takes a
//! session made under another account, so the line goes on in it, its next round writing its context to that account's
//! cache (decision a-line-goes-on-in-its-session-under-another-account). Nor for the context its session holds: the
//! agents' program compacts the session at the installation's own number, given it on every round's command line, and
//! the line goes on in it (decision `a-session-is-compacted-at-the-installations-number`). A new session is recorded at
//! once, as the next thing that happens to the work is taken; a round of the line under way then goes on in the session
//! it began in, which its carrier decided from the work as kept as the round began, and the line's next round begins
//! the new one, told what its run was doing where the run ran in the session before.
//!
//! Whatever the policy, a line a round of whose session the service's safety check flagged, the run guard stopped, or
//! whose prompt was too long for the model, begins a new session, in which that round is tried again, so that none is
//! given the same context again.
//!
//! Nothing follows for a work that has ended, and without a policy only the new sessions after failed rounds do. A new
//! session runs no cell: the next round in it is filled with all its cell's data, the entries of the design tree that
//! the work was worked from among them (design document `data-and-context`, 15, 25).

use decide::run::Failure;
use decide::{CellId, Decision, RunId, decide};
use engine::Event;
use flow::CellSpec;
use flow::Compiled;
use store::{Item, WorkNumber};
use work::sessions::{began_on, number_at, of_line, start};
use work::{Back, Why, Work};

use crate::budget::Budget;

/// When a line begins a new session, and how its sessions are kept, as the installation sets it.
pub struct Policy {
    /// How many values of a history a line is worked out from at least ([`crate::history`]).
    pub samples: usize,
    /// The model a round of the cell, for the item, runs on now; none where it cannot run.
    #[allow(clippy::type_complexity)]
    pub model: Box<dyn Fn(&Item, &CellSpec) -> Option<String> + Send>,
    /// Whether a new session of the cell's line, in the work, for the item, would begin with another system prompt or
    /// form of return now than the line's session of this number began with, as the carriers kept them, or with one
    /// that cannot be made: no where the session began with none kept. None where the carriers keep no sessions.
    #[allow(clippy::type_complexity)]
    pub changed: Option<Box<dyn Fn(&Compiled, WorkNumber, &Item, &CellSpec, u32) -> bool + Send>>,
    /// How the sessions of lines that wait are kept warm; none where the agents' program keeps no prompt cache.
    pub warm: Option<Warm>,
}

/// How the sessions of lines that wait for their next round are kept warm ([`crate::warm`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warm {
    /// How long the agents' program's prompt cache keeps a session after its last call, in milliseconds.
    pub life_ms: u64,
    /// How long after a session's last call it is sent a keep-warm ping, in milliseconds: less than the life.
    pub after_ms: u64,
    /// How many pings one wait of a session is worth where the work's own reports do not yet tell its model's prices,
    /// which tell it otherwise ([`crate::prices`]): [`crate::prices::UNMEASURED_PINGS`], the fewest any model is worth.
    pub unmeasured_pings: u32,
    /// The most a work may spend, as the installation says ([`crate::budget`]); none where it sets no budget. A work
    /// the engine paused past the bar its budget sets is pinged no more, a ping counting in its spend.
    pub budget: Option<Budget>,
}

/// What follows, from outside the work of this number, from the work as it now is: each new session a line begins,
/// after a round that failed so that it is tried again in another session, or as the policy says.
pub fn follow(flow: &Compiled, number: WorkNumber, work: &Work, item: &Item, policy: Option<&Policy>) -> Vec<Event> {
    // Nothing more is written to a work that has ended, whose sessions will run no round.
    if matches!(decide(&flow.flow, &work.records), Decision::Ended(_)) {
        return Vec::new();
    }
    lines(flow)
        .filter_map(|line| match tried_anew(flow, work, line) {
            Some(why) => Some(Event::Anew {
                line: line.to_string(),
                why,
            }),
            None => policy.and_then(|policy| due(flow, number, work, item, line, policy)),
        })
        .collect()
}

/// The lines of the flow's cells, each once, in the order of the walk.
fn lines(flow: &Compiled) -> impl Iterator<Item = &str> {
    let mut seen = Vec::new();
    for line in flow.cells.iter().filter_map(|cell| cell.line.as_deref()) {
        if !seen.contains(&line) {
            seen.push(line);
        }
    }
    seen.into_iter()
}

fn line_of_cell<'a>(flow: &'a Compiled, cell: &str) -> Option<&'a str> {
    (flow.cells.iter().find(|spec| spec.id.as_str() == cell)).and_then(|spec| spec.line.as_deref())
}

/// The line of the run's cell.
fn line_of_run<'a>(flow: &'a Compiled, work: &Work, run: RunId) -> Option<&'a str> {
    let started = work.records.started.iter().find(|s| s.run == run)?;
    line_of_cell(flow, started.cell.as_str())
}

/// The new session the line of the work of this number begins at its next round, as the policy says: where its
/// session's rounds ran on another model than the line's rounds run on now; or where a new session of it would begin
/// with another system prompt or form of return than its session began with, or with one that cannot be made, the
/// record holding the context of the session it leaves where one was told. One whose rounds ran under another account
/// goes on (decision a-line-goes-on-in-its-session-under-another-account), and a session past the installation's
/// context is compacted by the agents' program and goes on (decision
/// `a-session-is-compacted-at-the-installations-number`).
pub fn due(
    flow: &Compiled,
    number: WorkNumber,
    work: &Work,
    item: &Item,
    line: &str,
    policy: &Policy,
) -> Option<Event> {
    let on_line = |run: RunId| line_of_run(flow, work, run) == Some(line);
    let session = number_at(work, line, work.tick());
    let began = began_on(work, line, on_line, session);
    // What a round of the line runs on now, as the policy says for a cell of the line; none where it cannot run, which
    // its round says when it fails. Any cell of the line will do: the levels above the item's that a round runs at are
    // the line's, so a line's model does not depend on which cell runs.
    let cell = flow.cells.iter().find(|cell| cell.line.as_deref() == Some(line));
    let now = cell.and_then(|cell| (policy.model)(item, cell));
    if let (Some(began), Some(now)) = (began, &now)
        && began.model != *now
    {
        return Some(Event::Anew {
            line: line.to_string(),
            why: Why::Model {
                was: began.model.clone(),
            },
        });
    }
    // The system prompt and the form are the same for every cell of the line, so any cell of it stands for the line
    // here too.
    if let (Some(changed), Some(cell)) = (&policy.changed, cell)
        && changed(flow, number, item, cell, session)
    {
        let context = {
            let line_of = |cell: &CellId| line_of_cell(flow, cell.as_str()).map(str::to_string);
            crate::history::context_before(&line_of, work, line, session, work.tick())
        };
        return Some(Event::Anew {
            line: line.to_string(),
            why: Why::Changed { context },
        });
    }
    None
}

/// Whether a session of the line with an id of its own was recorded after the round of this number of the run took its
/// session: the session the round ran in has ended already.
fn begun_since(work: &Work, line: &str, run: RunId, number: usize) -> bool {
    let took = start(work, run, number);
    of_line(work, line).any(|s| s.why.own_id() && s.at > took)
}

/// Why the line begins a new session at once, where a round of it failed, in the session the line is in, as one the
/// service's safety check flagged, the run guard stopped, or whose prompt was too long for the model: each is tried
/// again in a session that does not give it the same context. A new session follows each such round as soon as it is
/// taken, so that round is the line's last; any of them is looked at, since a store kept before new sessions were
/// recorded holds flagged rounds that no recorded session follows, the line having gone on in a session of its own that
/// only the failures counted: its line begins a new session as the store is opened, which holds nothing of that one and
/// is told what a session that begins is told.
fn tried_anew(flow: &Compiled, work: &Work, line: &str) -> Option<Why> {
    (work.back.iter()).find_map(|(&(run, number), back)| {
        let why = match back {
            Back::Failed {
                failure: Failure::Flagged,
                ..
            } => Why::Flagged,
            Back::Failed {
                failure: Failure::Guarded,
                ..
            } => Why::Guarded,
            Back::Failed {
                failure: Failure::PromptTooLong,
                ..
            } => Why::TooLong,
            _ => return None,
        };
        (line_of_run(flow, work, run) == Some(line) && !begun_since(work, line, run, number)).then_some(why)
    })
}
