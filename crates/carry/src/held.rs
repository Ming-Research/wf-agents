//! What a carrier holds in this process: each round or call it was given, with the reply that answers it while it is
//! not answered, and the way to stop what carries it out. A round or a call is answered once: by whichever of its end
//! and its stop comes first.
//!
//! What was answered is held until the carrier is shown that the loop has taken the answer, so that a stop or a
//! second giving that comes before then is known for what it is: the carrier prunes it as it is given anything more
//! of the same work, whose records then show the answer taken. What is held is so bounded by the rounds and calls of
//! each work that were answered since the work was last given something.
//!
//! A round that failed is held as failed: the loop answers it from then on. Given again, once its work is let on, it
//! is carried out anew; stopped, its stop is dropped, since the loop ends it itself. A round that fails after it was
//! told to stop has been stopped, and ends as a stop does.

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
    /// It failed, and the loop answers it.
    failed: bool,
}

/// What a carrier does with a round that could not be carried out.
pub(crate) enum Failing<R> {
    /// Says it failed, with this reply.
    Fail(R),
    /// Ends it with nothing, with this reply: it was told to stop.
    End(R),
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
    /// already, as when it is given again while it is carried out: it is the same round or call, carried out once. A
    /// round that failed is held anew.
    pub(crate) fn hold(&self, key: K, reply: R) -> Option<Stop> {
        let mut slots = self.slots();
        if slots.get(&key).is_some_and(|slot| !slot.failed) {
            return None;
        }
        let (stop, told) = oneshot::channel();
        let slot = Slot {
            reply: Some(reply),
            stop: Some(stop),
            failed: false,
        };
        slots.insert(key, slot);
        Some(Stop(told))
    }

    /// Forgets each round or call that was answered and that `taken` says the loop has taken the answer of: nothing
    /// more can come for it.
    pub(crate) fn prune(&self, taken: impl Fn(&K) -> bool) {
        self.slots().retain(|key, slot| slot.reply.is_some() || !taken(key));
    }

    /// The reply, to answer with; none where it was used already. What was given stays held, so that given again it
    /// is not carried out a second time.
    pub(crate) fn take(&self, key: K) -> Option<R> {
        self.slots().get_mut(&key).and_then(|slot| slot.reply.take())
    }

    /// What to do with what could not be carried out: say it failed, and hold it as failed; or, where it was told to
    /// stop, end it as a stop does. None where its reply was used already.
    pub(crate) fn fail(&self, key: K) -> Option<Failing<R>> {
        let mut slots = self.slots();
        let slot = slots.get_mut(&key)?;
        let reply = slot.reply.take()?;
        if slot.stop.is_none() {
            return Some(Failing::End(reply));
        }
        slot.stop = None;
        slot.failed = true;
        Some(Failing::Fail(reply))
    }

    /// Stops what was given: what carries it out is told to stop, and answers with the reply it holds. Gives the reply
    /// to answer with now where nothing carries it out: where it was not given in this process, the reply given with
    /// the stop; where it is no longer carried out and its reply is still held, that one. A round that failed is
    /// answered by the loop: none.
    pub(crate) fn stop(&self, key: K, reply: R) -> Option<R> {
        let mut slots = self.slots();
        let Some(slot) = slots.get_mut(&key) else {
            slots.insert(
                key,
                Slot {
                    reply: None,
                    stop: None,
                    failed: false,
                },
            );
            return Some(reply);
        };
        if slot.failed {
            return None;
        }
        match slot.stop.take().map(|stop| stop.send(())) {
            Some(Ok(())) => None,
            _ => slot.reply.take(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_was_answered_is_forgotten_once_its_answer_was_taken() {
        let held: Held<u8, &str> = Held::new();
        held.hold(1, "first").expect("held");
        held.hold(2, "second").expect("held");
        assert_eq!(held.take(1), Some("first"));
        held.prune(|_| true);
        // The one answered is forgotten: given again, it is new. The one not answered is kept.
        assert!(held.hold(1, "again").is_some());
        assert!(held.hold(2, "twice").is_none());
        // Answered and not taken, it stays held.
        assert_eq!(held.take(1), Some("again"));
        held.prune(|_| false);
        assert!(held.hold(1, "third").is_none());
    }

    #[test]
    fn a_failed_round_is_given_again_anew_and_its_stop_is_the_loops() {
        let held: Held<u8, &str> = Held::new();
        held.hold(1, "first").expect("held");
        assert!(matches!(held.fail(1), Some(Failing::Fail("first"))));
        assert_eq!(held.stop(1, "stop"), None, "the loop ends a failed round");
        assert!(held.hold(1, "again").is_some(), "given again, it is carried out anew");
    }

    #[test]
    fn a_round_that_fails_after_its_stop_ends_as_a_stop_does() {
        let held: Held<u8, &str> = Held::new();
        let _told = held.hold(1, "first").expect("held");
        assert_eq!(held.stop(1, "stop"), None, "what carries it out is told");
        assert!(matches!(held.fail(1), Some(Failing::End("first"))));
    }
}
