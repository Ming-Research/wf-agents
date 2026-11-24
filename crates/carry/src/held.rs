//! What a carrier holds in this process: each round or call it was given, with the reply that answers it while it is
//! not answered, and the way to stop what carries it out. A round or a call is answered once: by whichever of its end
//! and its stop comes first.
//!
//! What was answered is held until the carrier is shown that the loop has taken the answer, so that a stop or a
//! second giving that comes before then is known for what it is: the carrier prunes it as it is given anything more
//! of the same work, whose records then show the answer taken. What is held is so bounded by the rounds and calls of
//! each work that were answered since the work was last given something.
//!
//! A round that failed is answered by saying so: its end is in the work's records, and the loop neither gives it again
//! nor stops it. Once its work is let on, the run's next round, a round of its own, is carried out anew. A round that
//! fails after it was told to stop has been stopped, and ends as a stop does.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use service::{Along, Hosting, Saying};
use tokio::sync::oneshot;
use work::Left;

use crate::host::Asking;

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

/// What a carrier does with a round that could not be carried out.
pub(crate) enum Failing<R> {
    /// Says it failed, with this reply.
    Fail(R),
    /// Ends it with nothing, with this reply: it was told to stop.
    End(R),
}

/// What carries a round or a call out is told by this to stop. It may be waited for again and again, as between the
/// looks of a wait: once told, it comes at once each time. Through it, too, what carries it out says where it stands as
/// it goes, and what the host found of the work, where it was given the way to.
pub(crate) struct Stop {
    /// The way it is told, until it was told or can no longer be.
    told: Option<oneshot::Receiver<()>>,
    /// Whether it was told.
    stopped: bool,
    /// The way to say where it stands, and what was said last; none where nobody listens.
    saying: Option<(Saying, Option<Along>)>,
    /// The way to say what the host found of the work; none where nobody listens.
    hosting: Option<Hosting>,
    /// How long one test of the project's CI may run before it is stopped, in milliseconds, as the history of the
    /// project's merged checks says it; none where that history is too short.
    test_time: Option<f64>,
}

impl Stop {
    /// A stop that is never told: of what runs to its end once begun, as a keep-warm ping does.
    pub(crate) fn never() -> Stop {
        Stop::new(oneshot::channel().1)
    }

    fn new(told: oneshot::Receiver<()>) -> Stop {
        Stop {
            told: Some(told),
            stopped: false,
            saying: None,
            hosting: None,
            test_time: None,
        }
    }

    /// The stop, through which where it stands is said this way.
    pub(crate) fn saying(mut self, saying: Saying) -> Stop {
        self.saying = Some((saying, None));
        self
    }

    /// The stop, through which what the host found of the work is said this way.
    pub(crate) fn hosting(mut self, hosting: Hosting) -> Stop {
        self.hosting = Some(hosting);
        self
    }

    /// The stop, which gives the host how long one test may run before it is stopped.
    pub(crate) fn test_time(mut self, test_time: Option<f64>) -> Stop {
        self.test_time = test_time;
        self
    }

    /// How long one test may run before it is stopped, in whole seconds rounded up, at least one; none where the
    /// history says none.
    pub(crate) fn test_seconds(&self) -> Option<u64> {
        let millis = self.test_time.filter(|millis| millis.is_finite() && *millis > 0.0)?;
        Some(((millis / 1000.0).ceil() as u64).max(1))
    }

    /// Says that the host checks the work through the pull request of this number, at this address.
    pub(crate) fn pulled(&self, number: u64, url: &str) {
        if let Some(hosting) = &self.hosting {
            hosting.pulled(number, url.to_string());
        }
    }

    /// Says how long the tests and jobs of a run of the project's CI took, as the host read it.
    pub(crate) fn timed(&self, reading: work::Reading) {
        if let Some(hosting) = &self.hosting {
            hosting.timed(reading);
        }
    }

    /// Says that the project's check the host ran for what asked it ended by itself and left these many processes
    /// running in its group, which were stopped; nothing where it left none.
    pub(crate) fn left_behind(&self, asking: Asking, processes: usize) {
        let of = match asking {
            Asking::Call(call) => Left::Call { call },
            Asking::Round(run, round) => Left::Act { run, round },
        };
        if let Some(hosting) = &self.hosting
            && processes > 0
        {
            hosting.left_behind(of, processes);
        }
    }

    /// Says where what is carried out stands now, or that it no longer stands anywhere it says; said once for each
    /// change.
    pub(crate) fn say(&mut self, along: Option<Along>) {
        if let Some((saying, said)) = self.saying.as_mut()
            && *said != along
        {
            saying.say(along.clone());
            *said = along;
        }
    }

    /// Comes once the round or the call is to stop, and at once each time after; never, where it is not.
    pub(crate) async fn told(&mut self) {
        if self.stopped {
            return;
        }
        if let Some(told) = self.told.as_mut() {
            let said = told.await;
            self.told = None;
            if said.is_ok() {
                self.stopped = true;
                return;
            }
        }
        std::future::pending::<()>().await;
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
        Some(Stop::new(told))
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

    /// What to do with what could not be carried out: say it failed; or, where it was told to stop, end it as a stop
    /// does. None where its reply was used already. Either way it is answered, and held as any answered round is.
    pub(crate) fn fail(&self, key: K) -> Option<Failing<R>> {
        let mut slots = self.slots();
        let slot = slots.get_mut(&key)?;
        let reply = slot.reply.take()?;
        if slot.stop.take().is_none() {
            return Some(Failing::End(reply));
        }
        Some(Failing::Fail(reply))
    }

    /// Stops what was given: what carries it out is told to stop, and answers with the reply it holds. Gives the reply
    /// to answer with now where nothing carries it out: where it was not given in this process, the reply given with
    /// the stop; where it is no longer carried out and its reply is still held, that one; none where it was answered.
    pub(crate) fn stop(&self, key: K, reply: R) -> Option<R> {
        self.stopped(key, reply).map(|(reply, _)| reply)
    }

    /// Stops what was given, as [`Held::stop`] does, saying beside the reply to answer with whether it was given in this
    /// process: where it was, what carried it out has ended, and said what came of it, though its reply waits still.
    pub(crate) fn stopped(&self, key: K, reply: R) -> Option<(R, bool)> {
        let mut slots = self.slots();
        let Some(slot) = slots.get_mut(&key) else {
            slots.insert(
                key,
                Slot {
                    reply: None,
                    stop: None,
                },
            );
            return Some((reply, false));
        };
        match slot.stop.take().map(|stop| stop.send(())) {
            Some(Ok(())) => None,
            _ => slot.reply.take().map(|reply| (reply, true)),
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
    fn a_failed_round_is_answered_once_and_forgotten_once_its_answer_was_taken() {
        let held: Held<u8, &str> = Held::new();
        held.hold(1, "first").expect("held");
        assert!(matches!(held.fail(1), Some(Failing::Fail("first"))));
        assert!(held.fail(1).is_none(), "it is answered once");
        assert_eq!(held.stop(1, "stop"), None, "a stop after its end is dropped");
        assert!(
            held.hold(1, "again").is_none(),
            "given again before its answer was taken, it is not carried out"
        );
        held.prune(|_| true);
        assert!(held.hold(1, "again").is_some(), "its answer taken, it is forgotten");
    }

    #[tokio::test]
    async fn a_stop_may_be_waited_for_again_and_comes_at_once_once_told() {
        let held: Held<u8, ()> = Held::new();
        let mut stop = held.hold(1, ()).expect("held");
        let soon = std::time::Duration::from_millis(50);
        assert!(tokio::time::timeout(soon, stop.told()).await.is_err(), "not yet told");
        held.stop(1, ());
        for _ in 0..3 {
            assert!(tokio::time::timeout(soon, stop.told()).await.is_ok(), "told");
        }
        let mut never = Stop::never();
        for _ in 0..3 {
            assert!(tokio::time::timeout(soon, never.told()).await.is_err(), "never told");
        }
    }

    #[test]
    fn where_it_stands_is_said_once_for_each_change() {
        let (to, mut came) = tokio::sync::mpsc::unbounded_channel();
        let reply = service::CallReply::new(store::WorkNumber(1), decide::CallId(2), to);
        let saying = reply.saying();
        let held: Held<u8, service::CallReply> = Held::new();
        let mut stop = held.hold(1, reply).expect("held").saying(saying);
        let first = Along::Runs {
            at: "https://github.example/actions/runs/1".to_string(),
        };
        let runs = Along::Runs {
            at: "https://github.example/actions/runs/2".to_string(),
        };
        for along in [
            Some(first.clone()),
            Some(first.clone()),
            None,
            Some(runs.clone()),
            Some(runs.clone()),
        ] {
            stop.say(along);
        }
        let mut said = Vec::new();
        while let Ok(message) = came.try_recv() {
            if let service::Message::Along { along, .. } = message {
                said.push(along);
            }
        }
        assert_eq!(said, [Some(first.clone()), None, Some(runs)]);
        // Where nobody listens, nothing is said.
        Stop::never().say(Some(first));
    }

    #[test]
    fn a_round_that_fails_after_its_stop_ends_as_a_stop_does() {
        let held: Held<u8, &str> = Held::new();
        let _told = held.hold(1, "first").expect("held");
        assert_eq!(held.stop(1, "stop"), None, "what carries it out is told");
        assert!(matches!(held.fail(1), Some(Failing::End("first"))));
    }
}
