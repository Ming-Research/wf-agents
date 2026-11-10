//! What a carrier holds in this process: each round or call it was given, with the reply that answers it while it is
//! not answered, and the way to stop what carries it out. A round or a call is answered once: by whichever of its end
//! and its stop comes first.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use tokio::sync::oneshot;

/// The rounds or the calls a carrier was given, by their key.
pub(crate) struct Held<K, R> {
    slots: Mutex<BTreeMap<K, Slot<R>>>,
}

/// One round or call given.
struct Slot<R> {
    /// The reply, until it is used.
    reply: Option<R>,
    /// The way to tell what carries it out to stop, until it is told.
    stop: Option<oneshot::Sender<()>>,
}

/// What carries a round or a call out is told by this to stop.
pub(crate) struct Stop(oneshot::Receiver<()>);

impl Stop {
    /// A stop that is never told.
    #[cfg(test)]
    pub(crate) fn never() -> Stop {
        Stop(oneshot::channel().1)
    }

    /// Comes once the round or the call is to stop; never, where it is not.
    pub(crate) async fn told(&mut self) {
        if (&mut self.0).await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

impl<K: Ord + Copy, R> Held<K, R> {
    pub(crate) fn new() -> Self {
        Held {
            slots: Mutex::new(BTreeMap::new()),
        }
    }

    fn slots(&self) -> MutexGuard<'_, BTreeMap<K, Slot<R>>> {
        // What is held stays whole whatever panicked while it was held: each change is one insert, take or remove.
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Holds what was given, with its reply, and gives the way it is told to stop; gives nothing where it is held
    /// already, as when it is given again while it is carried out: it is the same round or call, carried out once.
    pub(crate) fn hold(&self, key: K, reply: R) -> Option<Stop> {
        let mut slots = self.slots();
        if slots.contains_key(&key) {
            return None;
        }
        let (stop, told) = oneshot::channel();
        let slot = Slot {
            reply: Some(reply),
            stop: Some(stop),
        };
        slots.insert(key, slot);
        Some(Stop(told))
    }

    /// The reply, to answer with; none where it was used already. What was given stays held, so that given again it
    /// is not carried out a second time.
    pub(crate) fn take(&self, key: K) -> Option<R> {
        self.slots().get_mut(&key).and_then(|slot| slot.reply.take())
    }

    /// The reply, to say it failed with; none where it was used already. What was given is let go of, so that given
    /// again it is carried out anew.
    pub(crate) fn forget(&self, key: K) -> Option<R> {
        let mut slots = self.slots();
        let reply = slots.get_mut(&key).and_then(|slot| slot.reply.take());
        if reply.is_some() {
            slots.remove(&key);
        }
        reply
    }

    /// Stops what was given: what carries it out is told to stop, and answers with the reply it holds. Gives the reply
    /// to answer with now where nothing carries it out: where it was not given in this process, the reply given with
    /// the stop; where it is no longer carried out and its reply is still held, that one.
    pub(crate) fn stop(&self, key: K, reply: R) -> Option<R> {
        let mut slots = self.slots();
        let Some(slot) = slots.get_mut(&key) else {
            slots.insert(
                key,
                Slot {
                    reply: None,
                    stop: None,
                },
            );
            return Some(reply);
        };
        match slot.stop.take().map(|stop| stop.send(())) {
            Some(Ok(())) => None,
            _ => slot.reply.take(),
        }
    }
}
