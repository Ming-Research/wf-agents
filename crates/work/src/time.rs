//! What a work's time went to, as the engine keeps it as the work ends, while the version of its flow it ended on is
//! installed: once that version is not, nothing can work it out again, and the work's time bar is drawn from what was
//! kept (decision `a-work-keeps-what-its-time-went-to`).

use decide::Seq;
use serde::{Deserialize, Serialize};

use crate::Time;

/// What a stretch of a work's time went to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Went {
    /// A round of an agent's cell: a model ran.
    Model,
    /// A round of an engine's cell, or a call of a tool from when it was made to when it was done.
    Tools,
    /// The work waited for the owner, as the one definition of what waits on the owner read it then.
    Owner,
    /// A pause held the work, its own or the pause of everything.
    Paused,
    /// A stop of the engine held the work, from when it was asked for to when the engine started again; the bar draws
    /// it as a pause.
    Stopping,
    /// The work waited for the items its item depends on.
    Items,
    /// The work waited for its turn to check and merge: another work of its project held the turn.
    Turn,
    /// The work would begin a round that takes a slot, and no round of it began: the engine had no slot free for it.
    /// The walk, asked in a room with every slot free, says the round would begin; that it did not is the slots'.
    Slot,
    /// Anything else, a stalled work among it. In what was kept before `Slot` was told apart ([`KeptTime::separated`]) a
    /// wait for a slot is here too.
    Other,
}

/// A stretch of a work's time, from a time up to another, and what it went to. Stretches of what ran may overlap one
/// another; stretches of what the work waited for lie between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stretch {
    pub from: Time,
    pub until: Time,
    pub went: Went,
}

/// A stretch of time in which a work held the turn of its project, from a time up to another: none while it holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnSpan {
    pub from: Time,
    pub until: Option<Time>,
}

/// What a work's time went to from its first round to its last, kept as it ended, at a place of its own in the order of
/// what happened to it, with each stretch it held the turn of its project, which the time of the project's other works
/// that waited for the turn is read by. A work reopened and ended again keeps it again; the latest is the work's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeptTime {
    pub at: Seq,
    pub stretches: Vec<Stretch>,
    /// Each stretch the work held the turn, in order, as the flow it ended on decided it. None for a work that kept its
    /// time before the turn was worked out from records: it is read as one that never held it.
    pub turn: Option<Vec<TurnSpan>>,
    /// Whether the stretches tell a wait for a slot apart from the other waits ([`Went::Slot`]): what was kept before
    /// that was told apart has such waits among its `Other`, and they cannot be told again, the version it ended on no
    /// longer being compiled.
    pub separated: bool,
}
