//! When a line of a work begins a new session, as the driver works it out of the work's records after each thing that
//! happened and before the walk: so that a new session is recorded, and the cells it makes run again are run, before
//! the line's next round is given.
//!
//! With the installation's [`Policy`], a line begins a new session at its next round when the account or the model its
//! session's rounds ran on, as the work's records say, is not the one its rounds run on now, or when its last round
//! ended with a context past the limit. The limit is the line the line's own rounds give, in every work of the flow, of
//! the context they ended with ([`crate::history::context_line`]), so that a session is replaced where its size is more
//! than its line's history explains (principle `judged-by-own-history`); but never more than the policy's
//! [`Policy::limit`], which is the limit while that history is too short to give a line. The program compacts a
//! session near its model's window, so a line drawn from history stands past the window once one session came near it:
//! only a fixed number still bounds what each call of a long session reads. The limit waits while a run of the line has
//! not ended, as one rewriting a return that was sent back, or a cell of the line waits for the results of the calls
//! its run asked for: it goes on in the session that holds what it did, and the next round of the line after that
//! begins the new one. Nor does the limit end a session whose last round was of a cell that reads the [`SESSION`]
//! block: that cell loads the session, and a new session for its own size would load it again without end. A new
//! session for any other reason begins at once, and a round whose run ran in the session before is told what the run
//! was doing.
//!
//! Whatever the policy, a line a round of whose session the service's safety check flagged, the run guard stopped, or
//! whose prompt was too long for the model, begins a new session, in which that round is tried again, so that none is
//! given the same context again.
//!
//! Nothing follows for a work that has ended. Without a policy, nothing more is worked out, and only what others record
//! is followed: whatever records a new session, the driver then writes each of the work's [`SESSION`] blocks that a
//! cell of the line reads, so that the cell runs again and what it reads is in the new session. A flow gives each line
//! a block of its own, so that a new session of one line runs again only the cells of that line; a block that cells of
//! several lines read is written for a new session of any of them. A line's first session, which begins with its first
//! round, writes nothing: that cell runs then anyway.

use std::collections::BTreeMap;

use decide::run::Failure;
use decide::{Decision, Outsider, RunId, decide};
use engine::Event;
use flow::CellSpec;
use flow::agents::fill;
use flow::{Compiled, Texts};
use store::Item;
use work::sessions::{began_on, number_at, of_line, start};
use work::{Back, RanOn, Why, Work};

use crate::history::Line;

/// When a line begins a new session, and how its sessions are kept, as the installation sets it.
pub struct Policy {
    /// How many values of a history a line is worked out from at least ([`crate::history`]).
    pub samples: usize,
    /// The most a session's context may hold, in tokens: the limit where the line's history gives none, and the most it
    /// may be where it does.
    pub limit: u64,
    /// What a round of the cell, for the item, runs on now: its model and account among it; none where it cannot run.
    #[allow(clippy::type_complexity)]
    pub on: Box<dyn Fn(&Item, &CellSpec) -> Option<RanOn> + Send>,
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
    /// The most pings one wait of a session on each model is worth, by the model's name: as many as cost, each reading
    /// the session, no more than writing it to the cache again would beyond reading it. A session on a model not here
    /// is not pinged, its worth not being known.
    pub most: BTreeMap<String, u32>,
    /// Whether sessions are kept warm while the owner holds everything.
    pub while_paused: bool,
}

/// The line of the context each line's rounds end with, by line, where its history gives one.
pub type Limits = BTreeMap<String, Line>;

/// The source of the blocks the driver writes as a line begins a new session: each a block a cell reads that loads
/// what the line's other cells work from into the session, a line's own where the flow gives each line one.
pub const SESSION: &str = "session";

/// What follows, from outside the work, from the work as it now is: each new session the policy says a line begins,
/// and the write of each [`SESSION`] block where a line that has a cell reading it began one since the block was last
/// written.
pub fn follow(flow: &Compiled, work: &Work, item: &Item, policy: Option<&Policy>, limits: &Limits) -> Vec<Event> {
    // Nothing more is written to a work that has ended, whose sessions will run no round.
    if matches!(decide(&flow.flow, &work.records), Decision::Ended(_)) {
        return Vec::new();
    }
    let mut events: Vec<Event> = lines(flow)
        .filter_map(|line| match tried_anew(flow, work, line) {
            Some(why) => Some(Event::Anew {
                line: line.to_string(),
                why,
            }),
            None => policy.and_then(|policy| due(flow, work, item, line, policy, limits.get(line))),
        })
        .collect();
    let mut writes = Vec::new();
    for (block, declared) in (flow.blocks.iter()).filter(|(_, b)| b.writers.outside.as_deref() == Some(SESSION)) {
        let read_on =
            |line: &str| (declared.readers.cells.iter()).any(|cell| line_of_cell(flow, cell.as_str()) == Some(line));
        // The latest new session of a line the block stands for: one about to be recorded, or else one recorded since
        // the block was last written.
        let coming = events.iter().rev().find_map(|event| match event {
            Event::Anew { line, why } if read_on(line) => Some(said(why, line, &declared.writes)),
            _ => None,
        });
        let written = (work.records.written.iter())
            .filter(|w| w.block == *block && w.by == Outsider::Outside)
            .map(|w| w.at)
            .max();
        let loads = |cell: &str| declared.readers.cells.iter().any(|c| c.as_str() == cell);
        let recorded = (work.sessions.iter().rev())
            .find(|s| read_on(&s.line) && !in_the_loader(flow, work, s, &loads))
            .filter(|s| written.is_none_or(|w| w < s.at))
            .map(|s| said(&s.why, &s.line, &declared.writes));
        if let Some(content) = coming.or(recorded) {
            writes.push(Event::Wrote {
                block: block.clone(),
                by: Outsider::Outside,
                content,
                matters: true,
                value: None,
            });
        }
    }
    events.extend(writes);
    events
}

/// Whether the session is a compaction during a round of a cell that loads the session: that cell's own compaction
/// does not have it run again, as the limit ends no session at its round, or it would load again without end. The
/// round a session was recorded in is the latest that began before it.
fn in_the_loader(flow: &Compiled, work: &Work, session: &work::Session, loads: &dyn Fn(&str) -> bool) -> bool {
    if session.why != Why::Compacted {
        return false;
    }
    // The round of the session's line under way as it was compacted: one that began before and had not ended. A
    // compaction that came while none was, as of a keep-warm ping, is in no round of the loading cell's.
    let r = &work.records;
    let mut counted: Vec<RunId> = Vec::new();
    let mut under_way = None;
    for round in &r.rounds {
        counted.push(round.run);
        let k = counted.iter().filter(|x| **x == round.run).count();
        let ended = (r.returned.iter()).filter(|e| e.run == round.run).nth(k - 1);
        let on_line = line_of_run(flow, work, round.run) == Some(session.line.as_str());
        if on_line && round.at < session.at && ended.is_none_or(|e| e.at > session.at) {
            under_way = Some(round.run);
        }
    }
    let cell = under_way.and_then(|run| r.started.iter().find(|s| s.run == run));
    cell.is_some_and(|started| loads(started.cell.as_str()))
}

/// What the block says of a new session of the line, in the words its definition gives (`writes`): `text`, with the
/// line and the words of why it began one; of a compaction, which begins none, the words of `compacted` alone.
fn said(why: &Why, line: &str, writes: &Texts) -> String {
    let says = |name: &str| writes.get(name).unwrap_or_default();
    let (kind, slots) = why.named();
    if *why == Why::Compacted {
        return fill(says(kind), &[("line", line)]);
    }
    let slots: Vec<(&str, &str)> = slots.iter().map(|(name, value)| (*name, value.as_str())).collect();
    let why = fill(says(kind), &slots);
    fill(says("text"), &[("line", line), ("why", &why)])
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

/// The new session the line begins at its next round, as the policy says: where its session's rounds ran under
/// another account or on another model than the line's rounds run on now, or where its last round ended past its
/// limit: the line of its history, where it has one, never more than the policy's limit.
pub fn due(
    flow: &Compiled,
    work: &Work,
    item: &Item,
    line: &str,
    policy: &Policy,
    limit: Option<&Line>,
) -> Option<Event> {
    let on_line = |run: RunId| line_of_run(flow, work, run) == Some(line);
    let session = number_at(work, line, work.tick());
    let began = began_on(work, line, on_line, session);
    // What a round of the line runs on now, as the policy says for a cell of the line; none where it cannot run, which
    // its round says when it fails. Any cell of the line will do: the levels above the item's that a round runs at are
    // the line's, and the account the installation's, so a line's model and account do not depend on which cell runs.
    let cell = flow.cells.iter().find(|cell| cell.line.as_deref() == Some(line));
    let now = cell.and_then(|cell| (policy.on)(item, cell));
    let anew = |why| {
        Some(Event::Anew {
            line: line.to_string(),
            why,
        })
    };
    if let (Some(began), Some(now)) = (began, &now)
        && began.account != now.account
    {
        return anew(Why::Account {
            was: began.account.clone(),
        });
    }
    // The line is worked out in tokens, which a count of them can only round down from.
    let limit = (limit.map(|l| l.value.max(0.0).floor() as u64)).map_or(policy.limit, |l| l.min(policy.limit));
    if let Some(context) = past_limit(flow, work, line, limit) {
        return anew(Why::Limit { context, limit });
    }
    if let (Some(began), Some(now)) = (began, &now)
        && began.model != now.model
    {
        return anew(Why::Model {
            was: began.model.clone(),
        });
    }
    None
}

/// The line's last round that ended, by its run and number: the latest end of a round of the line.
fn last_ended(flow: &Compiled, work: &Work, line: &str) -> Option<(RunId, usize)> {
    let mut ended: Vec<(RunId, usize)> = Vec::new();
    for returned in &work.records.returned {
        let number = ended.iter().filter(|(run, _)| *run == returned.run).count() + 1;
        ended.push((returned.run, number));
    }
    (ended.into_iter().rev()).find(|(run, _)| line_of_run(flow, work, *run) == Some(line))
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

/// The context the line's last round ended with, where it was past the limit in the session the line is in, and the
/// limit ends that session.
fn past_limit(flow: &Compiled, work: &Work, line: &str, limit: u64) -> Option<u64> {
    let (run, number) = last_ended(flow, work, line)?;
    let context = work.spent.get(&(run, number))?.context_tokens?;
    if context <= limit || begun_since(work, line, run, number) {
        return None;
    }
    let started = work.records.started.iter().find(|s| s.run == run)?;
    let loads = (flow.blocks.values())
        .filter(|b| b.writers.outside.as_deref() == Some(SESSION))
        .any(|b| b.readers.cells.contains(&started.cell));
    // A run of the line that has not ended, as on a return sent back, goes on in the session it ran in, and so does a
    // cell of the line that applies and is owed the results of the calls its run asked for: the limit takes effect
    // once the line's runs have ended and none of its cells waits so. A cell skipped since it asked waits for nothing.
    let r = &work.records;
    let of_line = || (r.started.iter()).filter(|s| line_of_cell(flow, s.cell.as_str()) == Some(line));
    let open =
        of_line().any(|s| !(r.finished.iter().any(|f| f.run == s.run) || r.abandoned.iter().any(|a| a.run == s.run)));
    let waiting = (decide::owed(&flow.flow, r).keys()).any(|cell| line_of_cell(flow, cell.as_str()) == Some(line));
    (!loads && !open && !waiting).then_some(context)
}
