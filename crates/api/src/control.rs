//! What the owner asks of the running engine (its mode, and in step mode the next step), shared by the interface's
//! handlers and the engine's loop, with the signal that wakes the loop at once when the owner asks.

use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Duration;

use crate::Mode;

/// What the owner has asked, as the loop last saw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asked {
    pub mode: Mode,
    /// In step mode: a step was asked for and has not been taken yet.
    pub step: bool,
}

/// What the loop does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// A pass over every work.
    Pass,
    /// The one step the owner asked for.
    Step,
    /// Nothing, until the owner asks for something or the idle time passes.
    Wait,
}

impl Asked {
    pub fn next(self) -> Next {
        match self.mode {
            Mode::Run => Next::Pass,
            Mode::Step if self.step => Next::Step,
            Mode::Step | Mode::Hold => Next::Wait,
        }
    }
}

pub struct Control {
    asked: Mutex<Asked>,
    changed: Condvar,
}

fn lock(m: &Mutex<Asked>) -> MutexGuard<'_, Asked> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Control {
    pub fn new(mode: Mode) -> Control {
        Control {
            asked: Mutex::new(Asked { mode, step: false }),
            changed: Condvar::new(),
        }
    }

    pub fn asked(&self) -> Asked {
        *lock(&self.asked)
    }

    /// Switches the mode; a step asked for and not yet taken is dropped when the mode is no longer step.
    pub fn set_mode(&self, mode: Mode) {
        let mut a = lock(&self.asked);
        a.mode = mode;
        if mode != Mode::Step {
            a.step = false;
        }
        self.changed.notify_all();
    }

    /// Asks for the next step: only in step mode, and only once the step asked for before has been taken.
    pub fn ask_step(&self) -> Result<(), String> {
        let mut a = lock(&self.asked);
        if a.mode != Mode::Step {
            return Err("the engine takes single steps only in step mode".into());
        }
        if a.step {
            return Err("the step asked for before is still under way".into());
        }
        a.step = true;
        self.changed.notify_all();
        Ok(())
    }

    /// Says that the step asked for has been taken.
    pub fn stepped(&self) {
        lock(&self.asked).step = false;
        self.changed.notify_all();
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
    fn a_step_is_asked_only_in_step_mode_and_once_at_a_time() {
        let c = Control::new(Mode::Hold);
        assert_eq!(c.asked().next(), Next::Wait);
        assert!(c.ask_step().is_err());
        c.set_mode(Mode::Step);
        assert_eq!(c.asked().next(), Next::Wait);
        c.ask_step().unwrap();
        assert!(c.ask_step().is_err());
        assert_eq!(c.asked().next(), Next::Step);
        c.stepped();
        assert_eq!(c.asked().next(), Next::Wait);
        c.ask_step().unwrap();
        c.set_mode(Mode::Run);
        assert_eq!(
            c.asked(),
            Asked {
                mode: Mode::Run,
                step: false
            }
        );
        assert_eq!(c.asked().next(), Next::Pass);
    }

    #[test]
    fn the_loop_wakes_as_soon_as_the_owner_asks() {
        let c = std::sync::Arc::new(Control::new(Mode::Step));
        let seen = c.asked();
        let asker = std::sync::Arc::clone(&c);
        let t = std::thread::spawn(move || asker.ask_step().unwrap());
        let started = std::time::Instant::now();
        c.wait(seen, Duration::from_secs(30));
        t.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(10));
        // What changed before the wait began does not make the loop wait.
        let started = std::time::Instant::now();
        c.wait(seen, Duration::from_secs(30));
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
