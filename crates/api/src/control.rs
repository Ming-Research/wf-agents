//! What the owner asks of the running engine, shared by the interface's handlers and the engine's loop, with the signal
//! that wakes the loop at once when the owner asks: whether every work is paused and whether waiting sessions are kept
//! warm meanwhile, the works and the projects' architects the owner started, the round of one work or of a project's
//! architect and whether it starts a new session, and what a round would be; and the work whose round the engine runs
//! now, which the page shows.
//!
//! Each work follows the global pause and start, unless the owner started it or paused it, and the later of the two
//! wins: the global pause stops every work and makes those the owner started follow it again, a work's Start runs it
//! whatever the global state, and a work's own pause, which the database keeps, holds until the owner starts it again.
//! Each project's architect's works are taken the same way.

use std::collections::BTreeSet;
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Duration;

use crate::Status;

pub use sessions::Fresh;

/// What the owner has asked, as the loop last saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// Whether the owner paused every work: no work goes on by itself and no new work starts, but for those the owner
    /// started since. The engine starts so.
    pub paused: bool,
    /// Whether the sessions of waiting works are kept warm while every work is paused: the owner's switch beside the
    /// pause, which the engine reads from the database as it starts; on at first.
    pub keep_warm: bool,
    /// The works the owner started since every work was last paused: each goes on whatever the global state.
    pub started: BTreeSet<i64>,
    /// The works the owner started whose own pause the engine is still to lift, as the owner: a work on an item holds no
    /// slot while it is paused, so its pause is lifted once its machine has a slot free.
    pub releasing: BTreeSet<i64>,
    /// The projects whose architect the owner started since every work was last paused: its works go on, and one starts
    /// when its queue has a matter, whatever the global state.
    pub architects_started: BTreeSet<String>,
    /// The projects whose architect's work that runs the engine is still to lift the pause of, as the owner, who
    /// started it.
    pub architects_releasing: BTreeSet<String>,
    /// The work whose round the owner asked for, until the loop has taken it.
    pub step: Option<i64>,
    /// The project whose architect's round the owner asked for, until the loop has taken it: the next round of its work
    /// that runs, or a work started paused and its first round when its queue would start one.
    pub architect: Option<String>,
    /// How many times the owner decided something the engine reads, such as an answer or a ruling: each wakes the
    /// loop, so that works that go on go on at once.
    pub woken: u64,
}

/// What the loop does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// The round of the one work the owner asked for, first.
    Step(i64),
    /// The round of a project's architect the owner asked for.
    Architect(String),
    /// One step of the works that go on by themselves.
    Pass,
    /// While every work is paused, one step of the works the owner started, and of the architects the owner started.
    Started,
    /// Nothing, until the owner asks for something or the idle time passes.
    Wait,
}

impl Asked {
    pub fn next(&self) -> Next {
        match (self.step, self.paused, &self.architect) {
            (Some(w), _, _) => Next::Step(w),
            (None, _, Some(p)) => Next::Architect(p.clone()),
            (None, false, None) => Next::Pass,
            (None, true, None) if !self.started.is_empty() || !self.architects_started.is_empty() => Next::Started,
            (None, true, None) => Next::Wait,
        }
    }

    /// Whether the owner's global pause holds a work back now: every work is paused and the owner did not start it
    /// since; `architect` names the project of a work that is its architect's.
    pub fn holds(&self, work: i64, architect: Option<&str>) -> bool {
        self.paused && !self.started.contains(&work) && !architect.is_some_and(|p| self.architects_started.contains(p))
    }
}

pub struct Control {
    asked: Mutex<Asked>,
    changed: Condvar,
    /// The new sessions the owner asked for with a Start or a round, until the loop has handed them to the session
    /// manager: a work's, or a project's architect's.
    fresh: Mutex<Vec<Fresh>>,
    /// The work the engine is taking a step of now.
    running: Mutex<Option<i64>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Default for Control {
    /// As the engine starts: every work paused, waiting sessions kept warm.
    fn default() -> Control {
        Control {
            asked: Mutex::new(Asked {
                paused: true,
                keep_warm: true,
                started: BTreeSet::new(),
                releasing: BTreeSet::new(),
                architects_started: BTreeSet::new(),
                architects_releasing: BTreeSet::new(),
                step: None,
                architect: None,
                woken: 0,
            }),
            changed: Condvar::new(),
            fresh: Mutex::new(Vec::new()),
            running: Mutex::new(None),
        }
    }
}

impl Control {
    pub fn asked(&self) -> Asked {
        lock(&self.asked).clone()
    }

    /// Pauses every work, or lets them go on: a round under way ends first, and nothing starts after it. Pausing makes
    /// the works and the architects the owner started follow the global state again; letting them go on lifts no work's
    /// own pause.
    pub fn set_paused(&self, paused: bool) {
        let mut a = lock(&self.asked);
        a.paused = paused;
        if paused {
            a.started.clear();
            a.architects_started.clear();
        }
        drop(a);
        self.changed.notify_all();
    }

    /// Keeps the sessions of waiting works warm while every work is paused, or not.
    pub fn set_keep_warm(&self, on: bool) {
        lock(&self.asked).keep_warm = on;
        self.changed.notify_all();
    }

    /// The owner's Start of a work or of a project's architect: it goes on whatever the global state, its own pause
    /// lifted; its next round starts a new session when `new_session`.
    pub fn start(&self, which: Fresh, new_session: bool) {
        let mut a = lock(&self.asked);
        if new_session {
            lock(&self.fresh).push(which.clone());
        }
        match which {
            Fresh::Work(w) => {
                a.started.insert(w);
                a.releasing.insert(w);
            }
            Fresh::Architect(p) => {
                a.architects_started.insert(p.clone());
                a.architects_releasing.insert(p);
            }
        }
        drop(a);
        self.changed.notify_all();
    }

    /// The owner's Pause of a work or of a project's architect, whose own pause the database keeps: the owner's Start
    /// of it is over, and so is the lifting of its pause that Start asked.
    pub fn pause(&self, which: &Fresh) {
        let mut a = lock(&self.asked);
        match which {
            Fresh::Work(w) => {
                a.started.remove(w);
                a.releasing.remove(w);
            }
            Fresh::Architect(p) => {
                a.architects_started.remove(p);
                a.architects_releasing.remove(p);
            }
        }
        drop(a);
        self.changed.notify_all();
    }

    /// The engine lifted, or found nothing to lift of, the pauses of these works the owner started, and of the works of
    /// these projects' architects.
    pub fn released(&self, works: &[i64], architects: &[String]) {
        let mut a = lock(&self.asked);
        for w in works {
            a.releasing.remove(w);
        }
        for p in architects {
            a.architects_releasing.remove(p);
        }
    }

    /// The engine took a work over by a new work on its item, as when the owner started it over: the owner's Start of the
    /// old one goes on in the new one, and so does the lifting of its pause that Start asked.
    pub fn replaced(&self, old: i64, new: i64) {
        let mut a = lock(&self.asked);
        if a.started.remove(&old) {
            a.started.insert(new);
        }
        if a.releasing.remove(&old) {
            a.releasing.insert(new);
        }
        drop(a);
        self.changed.notify_all();
    }

    /// The new sessions asked for that the loop has not handed on yet.
    pub fn take_new_sessions(&self) -> Vec<Fresh> {
        std::mem::take(&mut *lock(&self.fresh))
    }

    /// Asks for the round of one work, whatever the global state, in a new session when `new_session`; only once the
    /// round asked for before has been taken.
    pub fn ask_step(&self, work: i64, new_session: bool) -> Result<(), String> {
        let mut a = lock(&self.asked);
        if let Some(w) = a.step {
            return Err(format!("the round of work {w} asked for before is still under way"));
        }
        if let Some(p) = &a.architect {
            return Err(format!(
                "the round of the architect of {p} asked for before is still under way"
            ));
        }
        if new_session {
            lock(&self.fresh).push(Fresh::Work(work));
        }
        a.step = Some(work);
        drop(a);
        self.changed.notify_all();
        Ok(())
    }

    /// Asks for the round of a project's architect: the next round of its work that runs, or, when none runs and its
    /// queue would start one, a work started paused by the owner and its first round; in a new session for its work
    /// when `new_session`; only once the round asked for before has been taken.
    pub fn ask_step_architect(&self, project: &str, new_session: bool) -> Result<(), String> {
        let mut a = lock(&self.asked);
        if let Some(w) = a.step {
            return Err(format!("the round of work {w} asked for before is still under way"));
        }
        if let Some(p) = &a.architect {
            return Err(format!(
                "the round of the architect of {p} asked for before is still under way"
            ));
        }
        if new_session {
            lock(&self.fresh).push(Fresh::Architect(project.to_string()));
        }
        a.architect = Some(project.to_string());
        drop(a);
        self.changed.notify_all();
        Ok(())
    }

    /// Says that the round asked for has been taken. The work it was of is paused by the owner after it, so the owner's
    /// Start of it is over; so is the architect's, after its round.
    pub fn stepped(&self, work: Option<i64>) {
        let mut a = lock(&self.asked);
        if let Some(w) = work {
            a.started.remove(&w);
            a.releasing.remove(&w);
        }
        if let Some(p) = a.architect.take() {
            a.architects_started.remove(&p);
            a.architects_releasing.remove(&p);
        }
        a.step = None;
        drop(a);
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
        s.keep_warm = a.keep_warm;
        s.started = a.started.iter().copied().collect();
        s.releasing = a.releasing.iter().copied().collect();
        s.architects_started = a.architects_started.iter().cloned().collect();
        s.stepping = a.step;
        s.running = *lock(&self.running);
        s
    }

    /// Wakes the loop: the owner decided something the engine reads.
    pub fn wake(&self) {
        lock(&self.asked).woken += 1;
        self.changed.notify_all();
    }

    /// Waits until what is asked differs from `seen`, or `idle` has passed.
    pub fn wait(&self, seen: &Asked, idle: Duration) {
        let a = lock(&self.asked);
        let _ = self.changed.wait_timeout_while(a, idle, |now| *now == *seen);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_work_starts_paused_and_a_round_of_one_work_is_asked_once_at_a_time() {
        let c = Control::default();
        assert_eq!(c.asked().next(), Next::Wait);
        c.ask_step(3, false).unwrap();
        assert!(c.ask_step(4, false).is_err());
        assert_eq!(c.asked().next(), Next::Step(3));
        c.stepped(Some(3));
        assert_eq!(c.asked().next(), Next::Wait);
        c.set_paused(false);
        assert_eq!(c.asked().next(), Next::Pass);
        // The round asked for comes before the works that go on.
        c.ask_step(4, false).unwrap();
        assert_eq!(c.asked().next(), Next::Step(4));
        c.stepped(Some(4));
        c.set_paused(true);
        assert_eq!(c.asked().next(), Next::Wait);
        // A project's architect's round is asked for the same way, while every work is paused, once at a time.
        c.ask_step_architect("p", false).unwrap();
        assert!(c.ask_step_architect("q", false).is_err() && c.ask_step(5, false).is_err());
        assert_eq!(c.asked().next(), Next::Architect("p".into()));
        c.stepped(None);
        assert_eq!(c.asked().next(), Next::Wait);
        assert!(c.take_new_sessions().is_empty());
    }

    // A work the owner started goes on while every work is paused, until the owner pauses it, steps it, or pauses
    // every work again; its pause is to be lifted until the engine says it did. The architect is started the same way.
    #[test]
    fn a_work_the_owner_started_goes_on_until_a_later_pause() {
        let c = Control::default();
        c.start(Fresh::Work(3), false);
        let a = c.asked();
        assert_eq!(a.next(), Next::Started);
        assert!(!a.holds(3, None) && a.holds(4, None) && a.holds(9, Some("p")));
        assert_eq!(a.releasing, BTreeSet::from([3]));
        c.released(&[3], &[]);
        assert!(c.asked().releasing.is_empty() && c.asked().started.contains(&3));
        c.pause(&Fresh::Work(3));
        assert_eq!(c.asked().next(), Next::Wait);
        c.start(Fresh::Work(3), false);
        c.start(Fresh::Architect("p".into()), false);
        assert!(!c.asked().holds(9, Some("p")) && c.asked().holds(9, Some("q")));
        // Every work paused again: the works the owner started follow it; a pause still to lift stays asked.
        c.set_paused(true);
        let a = c.asked();
        assert_eq!(a.next(), Next::Wait);
        assert!(a.started.is_empty() && a.architects_started.is_empty() && a.releasing.contains(&3));
        // Letting every work go on runs every work but those paused by themselves, which the database says.
        c.set_paused(false);
        assert_eq!(c.asked().next(), Next::Pass);
        assert!(!c.asked().holds(4, None));
        // A round the owner asked for leaves the work paused by the owner: its Start is over.
        c.set_paused(true);
        c.start(Fresh::Work(5), false);
        c.ask_step(5, false).unwrap();
        c.stepped(Some(5));
        assert!(!c.asked().started.contains(&5) && !c.asked().releasing.contains(&5));
    }

    // The owner's Start of a work taken over goes on in the work that took its item over, its pause still to lift with
    // it; a work the owner did not start hands on nothing.
    #[test]
    fn the_owner_s_start_goes_on_in_the_work_that_takes_a_work_over() {
        let c = Control::default();
        c.start(Fresh::Work(3), false);
        c.replaced(3, 7);
        c.replaced(4, 8);
        let a = c.asked();
        assert_eq!(a.started, BTreeSet::from([7]));
        assert_eq!(a.releasing, BTreeSet::from([7]));
    }

    // A round or a Start asked in a new session hands the session manager its ask, the architect's with no work, once;
    // a refused round asks nothing.
    #[test]
    fn a_new_session_is_asked_once() {
        let c = Control::default();
        c.ask_step(3, true).unwrap();
        assert!(c.ask_step(4, true).is_err());
        assert_eq!(c.take_new_sessions(), vec![Fresh::Work(3)]);
        assert!(c.take_new_sessions().is_empty());
        c.stepped(Some(3));
        c.ask_step_architect("p", true).unwrap();
        c.start(Fresh::Work(7), true);
        c.start(Fresh::Work(8), false);
        assert_eq!(
            c.take_new_sessions(),
            vec![Fresh::Architect("p".into()), Fresh::Work(7)]
        );
    }

    #[test]
    fn the_status_says_what_the_owner_asked_and_which_work_runs_now() {
        let c = Control::default();
        let base = Status {
            paused: false,
            keep_warm: false,
            started: Vec::new(),
            releasing: Vec::new(),
            architects_started: Vec::new(),
            running: None,
            stepping: None,
            last_step: None,
            slots: 2,
            slots_used: 1,
            flow_version: 3,
            config_commit: "c".into(),
            repos: std::collections::BTreeMap::new(),
            started_at: 1,
            version: "v".into(),
            halt: None,
        };
        let s = c.status(base.clone());
        assert!(s.paused && s.keep_warm);
        assert_eq!((s.running, s.stepping), (None, None));
        c.start(Fresh::Work(4), false);
        c.start(Fresh::Architect("p".into()), false);
        let s = c.status(base.clone());
        assert_eq!(
            (s.started, s.releasing, s.architects_started),
            (vec![4], vec![4], vec!["p".to_string()])
        );
        c.set_paused(false);
        c.set_keep_warm(false);
        c.set_running(Some(5));
        c.ask_step(6, false).unwrap();
        let s = c.status(base);
        assert!(!s.paused && !s.keep_warm);
        assert_eq!((s.running, s.stepping), (Some(5), Some(6)));
        assert_eq!(s.flow_version, 3);
    }

    #[test]
    fn the_loop_wakes_as_soon_as_the_owner_asks() {
        let c = std::sync::Arc::new(Control::default());
        let seen = c.asked();
        let asker = std::sync::Arc::clone(&c);
        let t = std::thread::spawn(move || asker.ask_step(1, false).unwrap());
        let started = std::time::Instant::now();
        c.wait(&seen, Duration::from_secs(30));
        t.join().unwrap();
        assert!(started.elapsed() < Duration::from_secs(10));
        // What changed before the wait began does not make the loop wait.
        let started = std::time::Instant::now();
        c.wait(&seen, Duration::from_secs(30));
        assert!(started.elapsed() < Duration::from_secs(10));
        // So does what the owner decided, letting every work go on, starting or pausing one, and the switch that keeps
        // sessions warm.
        let asks: [fn(&Control); 5] = [
            |c| c.wake(),
            |c| c.set_paused(false),
            |c| c.set_keep_warm(false),
            |c| c.start(Fresh::Work(9), false),
            |c| c.pause(&Fresh::Work(9)),
        ];
        for ask in asks {
            let seen = c.asked();
            ask(&c);
            let started = std::time::Instant::now();
            c.wait(&seen, Duration::from_secs(30));
            assert!(started.elapsed() < Duration::from_secs(10));
        }
    }
}
