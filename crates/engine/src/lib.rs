//! The loop of the engine, for one work.
//!
//! Everything kept of a work is its [`Work`]: the records the deciding code reads, and the content those records
//! name. It is defined in the crate `work`, beneath the engine and storage both, and its names are given again here.
//! The engine holds nothing else, so a stop of the engine loses nothing. [`handle`] takes one thing that happened to
//! the work, adds it to the records, asks the deciding code what follows until nothing more does, and gives back what
//! is to be done outside: a round to give an agent or to act out, a call to make, a call to withdraw, a round to stop.
//! Doing those, and saying when they are done, is the work of whatever drives the engine; [`pending`] gives again what
//! was to be done and is not yet known as done, for a driver that starts anew (principle `effects-known-as-done`).
//!
//! A pause of the work holds back only what would begin a round, and not the rounds of a run that answers a note, a
//! person's words to its cell: that run starts, and goes on to its end, while the work stays paused. A round may fail: it ends
//! as any round does, nothing having come back, and the engine pauses its work with the reason in the same step, so
//! that the owner sees the cause before the round is given again, of any run; who paused a work and why are in its
//! records, and what is to be done is worked out from them alone, after a stop as before it.
//!
//! - [`event`]: what happens to a work, and what is to be done for it.
//! - [`handle`](mod@handle): the loop.

pub mod event;
pub mod handle;

pub use event::{Came, Effect, Event, Refused};
pub use handle::{handle, pending};
pub use work::{Ask, Back, Call, Given, Handed, Moment, Spent, Time, Work};
