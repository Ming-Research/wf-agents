//! The driver, subject by subject: each test opens a store of its own in a directory of its own, builds small flows,
//! and plays the part of everything outside the driver, answering what it gives to be done by telling it what came of
//! that. What each test expects is taken from the crate's documentation and the entries of the design, never from its
//! code. The harness in `support` checks after every call what must always hold. The tests are kept in modules by
//! subject, so that a new test goes into the module of what it tests.

#[allow(dead_code)]
#[path = "../support/mod.rs"]
mod support;

mod adding;
mod dependencies;
mod events;
mod kept;
mod opening;
mod stopped;
mod stops;

use decide::{CallId, Decision, End, Outsider, Pauser, RunId, Waited, Who};
use driver::{Driver, Failed, Fault, Todo};
use engine::{Call, Came, Effect, Event, Given, Spent, Time};
use store::{ItemNumber, WorkNumber};
use support::*;
