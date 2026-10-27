//! What the owner asks of the running engine (whether every work is paused, the round of one work, what that round
//! would be), shared by the interface's handlers and the engine's loop, with the signal that wakes the loop at once
//! when the owner asks; and the work whose round the engine runs now, which the page shows.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Duration;

use crate::{NextRound, Status};

/// What the owner has asked, as the loop last saw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asked {
    /// Whether the owner paused every work: no work goes on by itself and no new work starts. The engine starts so.
    pub paused: bool,
    /// The work whose round the owner asked for, until the loop has taken it.
    pub step: Option<i64>,
    /// How many times the owner asked what a work's next round would be: each ask wakes the loop.
    pub previews: u64,
    /// How many times the owner decided something the engine reads, such as an answer or a ruling: each wakes the
    /// loop, so that works that go on go on at once.
    pub woken: u64,
}

/// What the loop does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// The round of the one work the owner asked for, first.
    Step(i64),
    /// One step of the works that go on by themselves.
    Pass,
    /// Nothing, until the owner asks for something or the idle time passes.
    Wait,
}

impl Asked {
    pub fn next(self) -> Next {
        match (self.step, self.paused) {
            (Some(w), _) => Next::Step(w),
            (None, false) => Next::Pass,
            (None, true) => Next::Wait,
        }
    }
}

/// An ask of what a work's next round would be, with where the loop answers it.
type Preview = (i64, Sender<Result<NextRound, String>>);

pub struct Control {
    asked: Mutex<Asked>,
    changed: Condvar,
    previews: Mutex<Vec<Preview>>,
    /// The work the engine is taking a step of now.
    running: Mutex<Option<i64>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Default for Control {
    /// As the engine starts: every work paused.
    fn default() -> Control {
        Control {
            asked: Mutex::new(Asked {
                paused: true,
                step: None,
                previews: 0,
                woken: 0,
            }),
            changed: Condvar::new(),
            previews: Mutex::new(Vec::new()),
            running: Mutex::new(None),
        }
    }
}

impl Control {
    pub fn asked(&self) -> Asked {
        *lock(&self.asked)
    }

    /// Pauses every work, or lets them go on: a round under way ends first, and nothing starts after it.
    pub fn set_paused(&self, paused: bool) {
        lock(&self.asked).paused = paused;
        self.changed.notify_all();
    }

    /// Asks for the round of one work, which does not go on by itself; only once the round asked for before has been
    /// taken.
    pub fn ask_step(&self, work: i64) -> Result<(), String> {
        let mut a = lock(&self.asked);
        if let Some(w) = a.step {
            return Err(format!("the round of work {w} asked for before is still under way"));
        }
        a.step = Some(work);
        self.changed.notify_all();
        Ok(())
    }

    /// Says that the round asked for has been taken.
    pub fn stepped(&self) {
        lock(&self.asked).step = None;
        self.changed.notify_all();
    }

    /// Says which work the engine is taking a step of now, or none once the step has ended.
    pub fn set_running(&self, work: Option<i64>) {
        *lock(&self.running) = work;
    }

    /// The engine's status as the loop last built it, with what the owner asked and the work whose round runs, as
    /// they are now.
    pub fn status(&self, mut s: Status) -> Status {
        let a = self.asked();
        s.paused = a.paused;
        s.stepping = a.step;
        s.running = *lock(&self.running);
        s
    }

    /// Asks what a work's next round would be; the loop answers on the channel given back.
    pub fn ask_preview(&self, work: i64) -> Receiver<Result<NextRound, String>> {
        let (tx, rx) = channel();
        lock(&self.previews).push((work, tx));
        lock(&self.asked).previews += 1;
        self.changed.notify_all();
        rx
    }

    /// Wakes the loop: the owner decided something the engine reads.
    pub fn wake(&self) {
        lock(&self.asked).woken += 1;
        self.changed.notify_all();
    }

    /// The asks of what a work's next round would be that wait for an answer.
    pub fn take_previews(&self) -> Vec<Preview> {
        std::mem::take(&mut *lock(&self.previews))
    }

    /// Waits until what is asked differs from `seen`, or `idle` has passed.
    pub fn wait(&self, seen: Asked, idle: Duration) {
        let a = lock(&self.asked);
        let _ = self.changed.wait_timeout_while(a, idle, |now| *now == seen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_work_starts_paused_and_a_round_of_one_work_is_asked_once_at_a_time() {
        let c = Control::default();
        assert_eq!(c.asked().next(), Next::Wait);
        c.ask_step(3).unwrap();
        assert!(c.ask_step(4).is_err());
        assert_eq!(c.asked().next(), Next::Step(3));
        c.stepped();
        assert_eq!(c.asked().next(), Next::Wait);
        c.set_paused(false);
        assert_eq!(c.asked().next(), Next::Pass);
        // The round asked for comes before the works that go on.
        c.ask_step(4).unwrap();
        assert_eq!(c.asked().next(), Next::Step(4));
        c.stepped();
        c.set_paused(true);
        assert_eq!(c.asked().next(), Next::Wait);
    }

    #[test]
    fn the_status_says_what_the_owner_asked_and_which_work_runs_now() {
        let c = Control::default();
        let base = Status {
            paused: false,
            running: None,
            stepping: None,
            last_step: None,
            slots: 2,
            slots_used: 1,
            flow_version: 3,
            config_commit: "c".into(),
            started_at: 1,
            version: "v".into(),
            halt: None,
        };
        let s = c.status(base.clone());
        assert!(s.paused);
        assert_eq!((s.running, s.stepping), (None, None));
        c.set_paused(false);
        c.set_running(Some(5));
        c.ask_step(6).unwrap();
        let s = c.status(base);
        assert!(!s.paused);
        assert_eq!((s.running, s.stepping), (Some(5), Some(6)));
        assert_eq!(s.flow_version, 3);
    }

    #[test]
    fn the_loop_wakes_as_soon_as_the_owner_asks() {
        let c = std::sync::Arc::new(Control::default());
        let seen = c.asked();
        let asker = std::sync::Arc::clone(&c);
        let t = std::thread::spawn(move || asker.ask_step(1).unwrap());
        let started = std::time::Instant::now();
        c.wait(seen, Duration::from_secs(30));
        t.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(10));
        // What changed before the wait began does not make the loop wait.
        let started = std::time::Instant::now();
        c.wait(seen, Duration::from_secs(30));
        assert!(started.elapsed() < Duration::from_secs(10));
        // Asking what a round would be wakes it too, and the ask waits to be answered.
        let seen = c.asked();
        let rx = c.ask_preview(2);
        let started = std::time::Instant::now();
        c.wait(seen, Duration::from_secs(30));
        assert!(started.elapsed() < Duration::from_secs(10));
        let asks = c.take_previews();
        assert_eq!(asks.len(), 1);
        asks[0].1.send(Err("none".into())).unwrap();
        assert_eq!(rx.recv().unwrap(), Err("none".into()));
        // So do what the owner decided, and letting every work go on.
        let asks: [fn(&Control); 2] = [|c| c.wake(), |c| c.set_paused(false)];
        for ask in asks {
            let seen = c.asked();
            ask(&c);
            let started = std::time::Instant::now();
            c.wait(seen, Duration::from_secs(30));
            assert!(started.elapsed() < Duration::from_secs(10));
        }
    }
}
