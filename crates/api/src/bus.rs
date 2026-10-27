//! The bus every live event goes through. Each event gets the next number in this process; its id is the process's
//! start and that number, so a client that reconnects with the last id it saw is given the events after it while
//! they are kept, and is told to read everything again when they are not, or when the engine started again since.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};

use crate::{Event, Status};

/// How many of the latest events are kept for clients that reconnect.
const KEPT: usize = 1000;

/// An event with its id and number.
#[derive(Debug, Clone, PartialEq)]
pub struct Envelope {
    pub id: String,
    pub seq: u64,
    pub event: Event,
}

type Listener = Box<dyn Fn(&Envelope) + Send>;

#[derive(Default)]
struct Inner {
    seq: u64,
    recent: VecDeque<Envelope>,
    status: Option<Status>,
    listeners: Vec<Listener>,
}

pub struct Bus {
    /// When this process started, in milliseconds: the first part of every id.
    epoch: i64,
    inner: Mutex<Inner>,
}

impl Default for Bus {
    fn default() -> Bus {
        Bus::new(db::now())
    }
}

impl Bus {
    pub fn new(epoch: i64) -> Bus {
        Bus {
            epoch,
            inner: Mutex::new(Inner::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn id(&self, seq: u64) -> String {
        format!("{}-{seq}", self.epoch)
    }

    /// Hands the event to every listener; a status the same as the last one is not an event.
    pub fn publish(&self, event: Event) {
        let mut x = self.lock();
        if let Event::Status { status } = &event {
            if x.status.as_ref() == Some(status) {
                return;
            }
            x.status = Some(status.clone());
        }
        x.seq += 1;
        let env = Envelope {
            id: self.id(x.seq),
            seq: x.seq,
            event,
        };
        x.recent.push_back(env.clone());
        if x.recent.len() > KEPT {
            x.recent.pop_front();
        }
        for l in &x.listeners {
            l(&env);
        }
    }

    /// The engine's status as it last published it.
    pub fn status(&self) -> Option<Status> {
        self.lock().status.clone()
    }

    /// Hands `f` every event published from now on, in order.
    pub fn listen(&self, f: Listener) {
        self.lock().listeners.push(f);
    }

    /// What a client is given first, and the number of the last event that covers: with no id, an event saying the
    /// stream starts here; with the id of an event it saw, the events after it, or an event saying to read
    /// everything again when they are not all kept. Events published after this call reach the client's listener.
    pub fn since(&self, last_seen: Option<&str>) -> (Vec<Envelope>, u64) {
        let x = self.lock();
        let now = x.seq;
        let mark = |event| Envelope {
            id: self.id(now),
            seq: now,
            event,
        };
        let Some(last) = last_seen else {
            return (vec![mark(Event::Ready)], now);
        };
        let seen = last
            .split_once('-')
            .filter(|(epoch, _)| *epoch == self.epoch.to_string())
            .and_then(|(_, n)| n.parse::<u64>().ok());
        match seen {
            Some(s) if s <= now && x.recent.front().is_none_or(|f| f.seq <= s + 1) => {
                (x.recent.iter().filter(|e| e.seq > s).cloned().collect(), now)
            }
            _ => (vec![mark(Event::Resync)], now),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    fn work(w: i64) -> Event {
        Event::Work { work: w }
    }

    #[test]
    fn a_client_is_given_what_it_missed_or_told_to_read_again() {
        let bus = Bus::new(7);
        let heard: Arc<Mutex<Vec<u64>>> = Arc::default();
        let h = Arc::clone(&heard);
        bus.listen(Box::new(move |e| h.lock().unwrap().push(e.seq)));
        let (first, upto) = bus.since(None);
        assert_eq!(first[0].event, Event::Ready);
        assert_eq!((first[0].id.as_str(), upto), ("7-0", 0));
        for w in 1..=3 {
            bus.publish(work(w));
        }
        assert_eq!(*heard.lock().unwrap(), [1, 2, 3]);
        let (missed, upto) = bus.since(Some("7-1"));
        assert_eq!(
            missed.iter().map(|e| e.event.clone()).collect::<Vec<_>>(),
            [work(2), work(3)]
        );
        assert_eq!(upto, 3);
        assert!(bus.since(Some("7-3")).0.is_empty());
        // Another process's ids, or ids from the future, mean everything is read again.
        assert_eq!(bus.since(Some("6-2")).0[0].event, Event::Resync);
        assert_eq!(bus.since(Some("7-9")).0[0].event, Event::Resync);
        assert_eq!(bus.since(Some("nonsense")).0[0].event, Event::Resync);
        for w in 0..KEPT as i64 {
            bus.publish(work(w));
        }
        assert_eq!(bus.since(Some("7-1")).0[0].event, Event::Resync);
        assert_eq!(bus.since(Some("7-3")).0.len(), KEPT);
    }
}
