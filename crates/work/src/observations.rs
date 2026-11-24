//! The observations a run keeps as it finishes: what its agent came across beyond the work, as the `observations` of
//! its returns said it.
//!
//! A run keeps those of the return it was taken on, and those of each return of it that was sent back: a return after
//! one that was sent back says only what was wrong with it, and what it did not name stands, so no later return says
//! again what an earlier one observed (decision a-sent-back-return-is-completed-by-what-is-missing).

use decide::RunId;

use crate::{Back, Work};

/// An observation a run kept as it finished: the `n`th text, counting from one, of the `observations` of what came back
/// at the end of its round of this number, the first being one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Observed {
    pub run: RunId,
    pub round: usize,
    pub n: usize,
}

/// The observations the run keeps as it finishes: of each of its returns, in the order of its rounds, each return's in
/// its order.
pub fn kept(work: &Work, run: RunId) -> Vec<Observed> {
    (work.back.range((run, 0)..=(run, usize::MAX)))
        .filter_map(|(&(_, round), back)| match back {
            Back::Returned { said, .. } => Some((round, said.observations.len())),
            _ => None,
        })
        .flat_map(|(round, said)| (1..=said).map(move |n| Observed { run, round, n }))
        .collect()
}
