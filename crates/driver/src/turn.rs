//! The turn of a project to check and merge, as the driver reads it from the records of the project's works (decision
//! `one-work-checks-and-merges-at-a-time`).
//!
//! The deciding code says, of one work's records, whether the work holds the turn and whether it wants it
//! ([`decide::holds_turn`], [`decide::wants_turn`]). The driver alone sees every work, so it alone says whose turn it is
//! in a project: the work that holds it; or, where none does, the one that has wanted it longest, the lower number
//! first where two began to want it at one time. A work that its own pause holds is passed over, and keeps the place it
//! has wanted the turn from.
//!
//! Nothing here is kept beside the records: how a work stands is worked out again each time the work grows, and each
//! stretch it held the turn, from the moments at which something happened to it, so that a driver opened on the store
//! gives the same turn as the driver that ran. The stretches are what the bars of the project's other works that waited
//! for the turn are drawn by ([`crate::time`]), kept as the work ends.

use std::collections::BTreeSet;

use decide::{Flow, RunId, holds_turn, wants_turn};
use store::WorkNumber;
use work::{Time, TurnSpan, Work};

/// How a work stands as to the turn of its project.
#[derive(Clone, Debug, Default)]
pub(crate) struct Turned {
    /// Whether the work holds the turn now ([`decide::holds_turn`]).
    pub holds: bool,
    /// Since when the work has wanted the turn without a break, where it wants it now ([`decide::wants_turn`]) and its
    /// own pause does not hold it. The time of the moment after which it began to, and has after each later one.
    pub wants: Option<Time>,
    /// When the work's latest round of a cell that takes the turn began: of two works that hold the turn, which only
    /// records made before the turn was worked out can give, the one that began first has it.
    pub began: Option<Time>,
    /// Each stretch the work held the turn, in order, the one it holds now open: from the time of the moment after
    /// which it held the turn to the time of the moment after which it did not.
    pub spans: Vec<TurnSpan>,
    /// How many of the work's moments `spans` are worked out for.
    seen: usize,
}

impl Turned {
    /// Works out again how the work stands, as it grew: each moment added since is decided once, and what is decided of
    /// the records now. A work that is not going on, or that no flow decides, holds and wants nothing.
    pub fn update(&mut self, flow: &Flow, work: &Work, going: bool) {
        let r = &work.records;
        let moments = &work.moments;
        if self.seen > moments.len() {
            *self = Turned::default();
        }
        let turn_runs: BTreeSet<RunId> = (r.started.iter())
            .filter(|s| flow.place(&s.cell).is_some_and(|i| flow.cells()[i].turn))
            .map(|s| s.run)
            .collect();
        let rounds: Vec<_> = r.rounds.iter().filter(|round| turn_runs.contains(&round.run)).collect();
        // Nothing before the work's first round of a cell that takes the turn can have it held.
        let first = rounds.iter().map(|round| round.at).min();
        for k in self.seen..moments.len() {
            let after = moments.get(k + 1).map(|next| next.from);
            let may = first.is_some_and(|first| after.is_none_or(|after| after > first));
            let held = may && {
                let then = after.map(|after| r.before(after));
                holds_turn(flow, then.as_ref().unwrap_or(r))
            };
            let at = moments[k].at;
            let open = self.spans.last().is_some_and(|span| span.until.is_none());
            match (held, open) {
                (true, false) => self.spans.push(TurnSpan { from: at, until: None }),
                (false, true) => {
                    if let Some(last) = self.spans.last_mut() {
                        last.until = Some(at);
                    }
                }
                _ => {}
            }
        }
        self.seen = moments.len();
        self.began = rounds
            .iter()
            .map(|round| round.at)
            .max()
            .and_then(|at| work.time_of(at));
        self.holds = going && self.spans.last().is_some_and(|span| span.until.is_none());
        self.wants = going.then(|| since_wanted(flow, work)).flatten();
    }

    /// Says the work holds and wants nothing, as a work that no flow decides now does.
    pub fn clear(&mut self) {
        *self = Turned::default();
    }
}

/// Since when the work has wanted the turn without a break, where it wants it now and its own pause does not hold it:
/// the time of the earliest moment after which it wanted it, and after every later one. Worked out by deciding back over
/// the moments from the newest, stopping at the first after which it did not.
fn since_wanted(flow: &Flow, work: &Work) -> Option<Time> {
    let r = &work.records;
    if decide::paused(r) || !wants_turn(flow, r) {
        return None;
    }
    let moments = &work.moments;
    let mut since = moments.last().map(|moment| moment.at);
    for k in (0..moments.len().saturating_sub(1)).rev() {
        if !wants_turn(flow, &r.before(moments[k + 1].from)) {
            break;
        }
        since = Some(moments[k].at);
    }
    since
}

/// Whose turn it is to check and merge in a project, from how its works stand: the work that holds it, the one that
/// began its latest round first where more than one does; else the one that has wanted it longest, the lower number
/// first where two began to want it at one time. None where no work holds or wants it.
pub(crate) fn whose<'a>(works: impl IntoIterator<Item = (WorkNumber, &'a Turned)>) -> Option<WorkNumber> {
    let works: Vec<(WorkNumber, &Turned)> = works.into_iter().collect();
    let holder = (works.iter())
        .filter(|(_, turned)| turned.holds)
        .min_by_key(|(number, turned)| (turned.began, *number));
    let next = || {
        (works.iter())
            .filter(|(_, turned)| turned.wants.is_some())
            .min_by_key(|(number, turned)| (turned.wants, *number))
    };
    holder.or_else(next).map(|(number, _)| *number)
}
