//! How a round ended, defined here once for every reader: the health numbers, and the owner's interface that shows
//! each round.

use serde_json::Value;

/// How a round ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// It has not ended.
    Running,
    /// Its return took a row of its cell.
    Took,
    /// Its return was sent back: it did not fit the cell's schema or failed a check.
    SentBack,
    /// Its run failed before it handed in a return.
    Failed,
    /// The engine stopped while it ran; its cell runs again.
    Interrupted,
    /// It ended without a row: the end of the graph.
    Ended,
}

/// How a round ended, from the row it took (`transition`), why it was rejected (`rejected`) and whether it has ended.
/// A failed run is recorded under `failed` with its kind, or, in a round recorded before the kinds were, under `error`;
/// any other rejection sent its return back.
pub fn outcome(transition: Option<&str>, rejected: Option<&Value>, ended: bool) -> Outcome {
    match (transition, rejected) {
        (Some(_), _) => Outcome::Took,
        (None, Some(r)) if r.get("interrupted").is_some() => Outcome::Interrupted,
        (None, Some(r)) if r.get("failed").is_some() || r.get("error").is_some() => Outcome::Failed,
        (None, Some(_)) => Outcome::SentBack,
        (None, None) if ended => Outcome::Ended,
        (None, None) => Outcome::Running,
    }
}
