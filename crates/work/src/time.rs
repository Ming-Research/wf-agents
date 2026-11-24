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
    /// Anything else, a stalled work among it.
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

/// What a work's time went to from its first round to its last, kept as it ended, at a place of its own in the order of
/// what happened to it. A work reopened and ended again keeps it again; the latest is the work's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeptTime {
    pub at: Seq,
    pub stretches: Vec<Stretch>,
}
