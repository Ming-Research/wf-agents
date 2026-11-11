//! The loop of the engine, for one work.
//!
//! Everything kept of a work is its [`Work`]: the records the deciding code reads, and the content those records
//! name. It is defined in the crate `work`, beneath the engine and storage both, and its names are given again here.
//! The engine holds nothing else, so a stop of the engine loses nothing. [`handle`] takes one thing that happened to
//! the work, adds it to the records, asks the deciding code what follows until nothing more does, and gives back what
//! is to be done outside: a round to give an agent or to act out, a call to make, a call to withdraw, a round to stop,
//! and, once a work has ended and nothing of it is under way, what it holds outside the engine to let go.
//! Doing those, and saying when they are done, is the work of whatever drives the engine; [`pending`] gives again what
//! was to be done and is not yet known as done, for a driver that starts anew (principle `effects-known-as-done`).
//!
//! A pause of the work holds back only what would begin a round, and not the rounds of a run that answers a note, a
//! person's words to its cell: that run starts, and goes on to its end, while the work stays paused. A round may fail:
//! it ends as any round does, nothing having come back, and the engine handles it as the kind of its cause says. Where
//! that is to pause the work, the engine pauses it with the reason in the same step, so that the owner sees the cause
//! before the round is given again, of any run; a failure in passing has the run wait, as a call the engine makes and
//! the clock answers, before the round is tried again; one whose cause every work shares holds the agents' rounds of
//! every work, which whatever drives the engine works out from all their records and says by a [`Gate`]. Who paused a
//! work and why, and what it waits for, are in its records, and what is to be done is worked out from them alone,
//! after a stop as before it.
//!
//! The owner's pause of everything and the slots hold back every work at once, and are the scheduler's: the loop is
//! given them as they stand ([`decide::Room`]), and begins no round that they hold back.
//!
//! - [`event`]: what happens to a work, and what is to be done for it.
//! - [`handle`](mod@handle): the loop.
//! - [`held`](mod@held): what holds a work back, as the loop leaves it.

pub mod event;
pub mod handle;
pub mod held;

pub use event::{Came, Effect, Event, Gate, Refused};
pub use handle::{handle, handle_held, handle_in, handle_with, pending};
pub use held::held;
pub use work::{Ask, Back, Call, Given, Handed, Moment, Spent, Time, Work};
