//! What the owner asks of the running engine (whether it runs or holds, the next round of one work while it holds,
//! what that round would be), shared by the interface's handlers and the engine's loop, with the signal that wakes
//! the loop at once when the owner asks.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Duration;

use crate::{Mode, NextRound};

/// What the owner has asked, as the loop last saw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asked {
    pub mode: Mode,
    /// While the engine holds: the work whose next round was asked for and has not been taken yet.
    pub step: Option<i64>,
    /// How many times the owner asked what a work's next round would be: each ask wakes the loop.
    pub previews: u64,
    /// How many times the owner decided something the engine reads, such as an answer or a ruling: each wakes the
    /// loop, so that a running engine goes on at once.
    pub woken: u64,
}

/// What the loop does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// A pass over every work.
    Pass,
    /// The next round of the one work the owner asked for.
    Step(i64),
    /// Nothing, until the owner asks for something or the idle time passes.
    Wait,
}

impl Asked {
    pub fn next(self) -> Next {
        match (self.mode, self.step) {
            (Mode::Run, _) => Next::Pass,
            (Mode::Hold, Some(w)) => Next::Step(w),
            (Mode::Hold, None) => Next::Wait,
        }
    }
}

/// An ask of what a work's next round would be, with where the loop answers it.
type Preview = (i64, Sender<Result<NextRound, String>>);

pub struct Control {
    asked: Mutex<Asked>,
    changed: Condvar,
    previews: Mutex<Vec<Preview>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Control {
    pub fn new(mode: Mode) -> Control {
        Control {
            asked: Mutex::new(Asked {
                mode,
                step: None,
                previews: 0,
                woken: 0,
            }),
            changed: Condvar::new(),
            previews: Mutex::new(Vec::new()),
        }
    }

    pub fn asked(&self) -> Asked {
        *lock(&self.asked)
    }

    /// Switches between running and holding; a round asked for and not yet taken is dropped when the engine runs.
    pub fn set_mode(&self, mode: Mode) {
        let mut a = lock(&self.asked);
        a.mode = mode;
        if mode == Mode::Run {
            a.step = None;
        }
        self.changed.notify_all();
    }

    /// Asks for the next round of one work: only while the engine holds, and only once the round asked for before
    /// has been taken.
    pub fn ask_step(&self, work: i64) -> Result<(), String> {
        let mut a = lock(&self.asked);
        if a.mode != Mode::Hold {
            return Err("the engine runs one work's next round only while it holds".into());
        }
        if let Some(w) = a.step {
            return Err(format!(
                "the next round of work {w} asked for before is still under way"
            ));
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
    fn a_round_of_one_work_is_asked_only_while_holding_and_once_at_a_time() {
        let c = Control::new(Mode::Run);
        assert_eq!(c.asked().next(), Next::Pass);
        assert!(c.ask_step(1).is_err());
        c.set_mode(Mode::Hold);
        assert_eq!(c.asked().next(), Next::Wait);
        c.ask_step(3).unwrap();
        assert!(c.ask_step(4).is_err());
        assert_eq!(c.asked().next(), Next::Step(3));
        c.stepped();
        assert_eq!(c.asked().next(), Next::Wait);
        c.ask_step(4).unwrap();
        c.set_mode(Mode::Run);
        assert_eq!(c.asked().step, None);
        assert_eq!(c.asked().next(), Next::Pass);
    }

    #[test]
    fn the_loop_wakes_as_soon_as_the_owner_asks() {
        let c = std::sync::Arc::new(Control::new(Mode::Hold));
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
        // So does what the owner decided.
        let seen = c.asked();
        c.wake();
        let started = std::time::Instant::now();
        c.wait(seen, Duration::from_secs(30));
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
