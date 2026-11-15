//! The player of a script (design document `testing`, 14): it plays a script whose agents are all stood in for on an
//! engine made as `wf-agents run --script` makes it, its carriers following the script, in this process, on the clock
//! of the runtime it runs on, whose time the caller pauses so that it passes as the engine waits: a script of rounds of
//! seconds plays in milliseconds, and an act given at a round under way is given at that step.
//!
//! It adds the script's items in order, lets on each that is not held, and takes what the loop is sent one thing at a
//! time ([`Stepper`]); before each, it gives each item's next act whose state the engine shows (module
//! `script::items`), stopping the engine and opening it again on its store where an act says so, and moving main once
//! every item with an act that moves it still to give stands at that act's state, so that the one write the engine
//! makes into every going work of the project is each work's own. It ends once every item stands at its path's end
//! with no act left to give, once each that does not has left its path for good, or once a bound of things taken or of
//! the clock's time has passed; then each item's work is judged against its path (module `judge`), and each act whose
//! state never came is named, as is each the engine refused.

use std::fmt;
use std::path::Path;
use std::time::Duration;

use flow::Compiled;
use store::WorkNumber;
use work::Time;

use crate::judge::Judged;
use crate::script::Script;
use crate::{Fault, Inbox, Message, Service};

/// The loop run in this process one thing at a time, on the clock of the runtime it runs on: what a play takes, and
/// what the service's own tests drive.
pub struct Stepper {
    service: Service,
    inbox: Inbox,
}

impl Stepper {
    /// The loop as it was opened, with its channel.
    pub fn new((service, inbox): (Service, Inbox)) -> Stepper {
        Stepper { service, inbox }
    }

    /// The process stopped and started again: the loop opened anew, with its channel, in place of the one before,
    /// whose carriers' answers no longer reach it.
    pub fn reopen(&mut self, (service, inbox): (Service, Inbox)) {
        self.service = service;
        self.inbox = inbox;
    }

    pub fn service(&self) -> &Service {
        &self.service
    }

    /// The next thing sent to the loop, once it comes, within this much of the runtime's time; none where nothing
    /// comes by then. Where the runtime's time is paused, it passes at once while nothing is under way.
    pub async fn next(&mut self, within: Duration) -> Option<Message> {
        tokio::time::timeout(within, self.inbox.recv()).await.ok().flatten()
    }

    /// Takes it, as the loop takes what comes to it.
    pub fn take(&mut self, message: Message) -> Result<(), Fault> {
        self.service.take(message)
    }
}

/// When a play ends at the latest: once it has taken this many things, or this much of the clock's time has passed
/// since its start, in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub steps: usize,
    pub ms: u64,
}

/// What came of a play.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Played {
    /// Each item, in the script's order.
    pub items: Vec<ItemPlayed>,
    /// How many things the loop took.
    pub steps: usize,
    /// What stopped the loop, where something did, in words.
    pub fault: Option<String>,
}

/// What came of one item of a play.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemPlayed {
    pub work: WorkNumber,
    /// How its work's records stand against its path.
    pub judged: Judged,
    /// Its acts whose state never came, by their places among its acts, counted from one.
    pub unreached: Vec<usize>,
    /// Its acts the engine refused, by their places, counted from one, with why.
    pub refused: Vec<(usize, String)>,
}

impl Played {
    /// Whether the play went as its script says: every item's work took its path, every act was given and taken, and
    /// nothing stopped the loop.
    pub fn took(&self) -> bool {
        self.fault.is_none()
            && (self.items.iter())
                .all(|item| item.judged == Judged::Took && item.unreached.is_empty() && item.refused.is_empty())
    }
}

/// What came of the play, a line for each item and one for what stopped the loop.
impl fmt::Display for Played {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (k, item) in self.items.iter().enumerate() {
            write!(f, "items[{k}], W{}: {}", item.work.0, item.judged)?;
            for act in &item.unreached {
                write!(f, "; its act {act} was not given: its state never came")?;
            }
            for (act, why) in &item.refused {
                write!(f, "; its act {act} was refused: {why}")?;
            }
            writeln!(f)?;
        }
        if let Some(fault) = &self.fault {
            writeln!(f, "The loop stopped: {fault}")?;
        }
        Ok(())
    }
}

/// Plays the script on these flows over the store at this path, which holds nothing yet, its items added to the
/// project of this name, its clock starting at `start`, within the bounds; or says what is wrong with the script for
/// them, each in words with where it is, as its check says, and each item with no path, since a play judges what it
/// plays. It runs on the runtime it is called on, whose time the caller pauses.
pub async fn play(
    script: &Script,
    flows: &[Compiled],
    store: &Path,
    project: &str,
    start: Time,
    bounds: Bounds,
) -> Result<Played, Vec<String>> {
    let _ = (script, flows, store, project, start, bounds);
    Err(vec!["a script is not played yet".to_string()])
}
