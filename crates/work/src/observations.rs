//! The observations a run keeps as it finishes: what its agent came across beyond the work, as the `observations` of
//! its returns said it.
//!
//! A run keeps those of the return it was taken on, and those of each return of it that was sent back and that no
//! later return of it was made in a session holding. A later return made in the session that held one says again what
//! of it still holds, so keeping both would keep it twice. One made in a session that never saw it cannot: a new
//! session begun after it, as for the context's limit, another model or account, the owner or a flag, holds only why
//! it was sent back, and so does one begun again or compacted under its id. Whether a session holds a return is read
//! as the context manager reads what a session holds (crate `context`, module `manager`): the two rounds ran in one
//! session of the line, which was not begun again or compacted under its id after the earlier one ended and before the
//! later one began.

use decide::RunId;

use crate::sessions::{number_at, of_line, start};
use crate::{Back, Why, Work};

/// An observation a run kept as it finished: the `n`th text, counting from one, of the `observations` of what came back
/// at the end of its round of this number, the first being one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Observed {
    pub run: RunId,
    pub round: usize,
    pub n: usize,
}

/// The observations the run keeps as it finishes, its cell being on this line, or on none as the engine's own acts
/// are: of each of its returns that it keeps, in the order of its rounds, each return's in its order.
pub fn kept(work: &Work, run: RunId, line: Option<&str>) -> Vec<Observed> {
    // The rounds of the run that came back with a return, in order, each with how many observations it said.
    let returns: Vec<(usize, usize)> = (work.back.range((run, 0)..=(run, usize::MAX)))
        .filter_map(|(&(_, round), back)| match back {
            Back::Returned { said, .. } => Some((round, said.observations.len())),
            _ => None,
        })
        .collect();
    let mut kept = Vec::new();
    for (i, &(round, said)) in returns.iter().enumerate() {
        // A return is kept unless the next return of the run was made in a session that held it: with no line there
        // is no session to tell apart, and every return is held by the next.
        let held = match (returns.get(i + 1), line) {
            (None, _) => false,
            (Some(_), None) => true,
            (Some(&(next, _)), Some(line)) => holds(work, line, run, round, next),
        };
        if !held {
            kept.extend((1..=said).map(|n| Observed { run, round, n }));
        }
    }
    kept
}

/// Whether the session the round of the later number of the run ran in, on this line, held what the round of the
/// earlier number returned: both ran in one session of the line, and it was not begun again or compacted under its id
/// after the earlier round ended and before the later one began.
fn holds(work: &Work, line: &str, run: RunId, earlier: usize, later: usize) -> bool {
    let ended = earlier
        .checked_sub(1)
        .and_then(|i| work.records.returned.iter().filter(|r| r.run == run).nth(i));
    let Some(ended) = ended.map(|r| r.at) else {
        return false;
    };
    let began = start(work, run, later);
    let one = number_at(work, line, start(work, run, earlier)) == number_at(work, line, began);
    let reopened =
        (of_line(work, line)).any(|s| matches!(s.why, Why::Lost | Why::Compacted) && s.at > ended && s.at < began);
    one && !reopened
}
