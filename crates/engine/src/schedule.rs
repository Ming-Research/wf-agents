//! The engine's loop and the jobs it dispatches. Only the loop schedules ([`Engine::dispatch`]): between jobs it alone
//! reads what the owner asked, carries out the outbox, brings works on an older flow version to the version the engine
//! runs, starts works, grants slots and finds the keep-warm pings due; and what the engine does to a work from outside
//! its step waits until that work has no job in flight. Every such action but a pause comes through the outbox, where
//! whoever asks writes it, the command line's process and an architect's step as much as the loop: a start over, a
//! cancel of the work's item and the lifting of a pause, which also waits for a free slot; the loop alone takes a work
//! over on an older flow version. So a work never ends while a step of it runs, and a step that finds its round ended
//! is a fault, which pauses the work. The loop hands each job it dispatches, a step of a work or the pings due to its
//! sessions, to a thread of its own with an execution context of its own ([`Engine::fork`]), one job at most for each
//! work, so that the rounds and tool runs of different works overlap while each work moves one step after another,
//! and a pause of a work takes effect when the round under way ends. A job's thread marks its job as over and says so
//! (the hook `ended`), and the loop reaps it ([`Engine::reap`]).

use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, PoisonError};
use std::thread::JoinHandle;

use crate::{
    ARCHITECT, Advance, ENGINEER, Engine, GlobalPause, RoundId, Stepped, Work, actions, architect, lock, versions, warm,
};

/// What the owner lets go on, as the loop that runs the engine hands it at each turn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Go {
    /// The owner's global pause while it holds: no work goes on by itself but those the owner started and the works of
    /// the architects the owner started, and no new work starts on a ready item; none while every work goes on.
    pub pause: Option<GlobalPause>,
    /// The works whose round the owner asked for, in the order asked: each takes one step whatever the global state,
    /// once it has no job in flight, after which the owner holds it paused.
    pub steps: Vec<i64>,
    /// The projects whose architect's round the owner asked for: the next round of its work that runs, or, when none
    /// runs and a matter is due, of a work started for it paused by the owner.
    pub architects: Vec<String>,
}

/// The owner's round a step was asked as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// The round of a work.
    Work(i64),
    /// The round of a project's architect.
    Architect(String),
}

/// What one pass of the loop came to ([`Engine::pass`]): the rounds the loop's own context started or ended as it
/// dispatched and carried out the outbox, as when it took a work over and cut its round off, and what each step it
/// dispatched came to, in the order of their works' ids.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Passed {
    pub dispatching: Vec<RoundId>,
    pub jobs: Vec<Stepped>,
}

impl Passed {
    /// The work of the first step that started or ended a round; none when no step did.
    pub fn work(&self) -> Option<i64> {
        self.moved().next().and_then(|s| s.work)
    }

    /// The cell of the last round of the first step that started or ended one.
    pub fn cell(&self) -> Option<String> {
        self.moved().next().and_then(|s| s.cell.clone())
    }

    /// The item the loop started a work on in this pass, the first when it started several.
    pub fn started_item(&self) -> Option<i64> {
        self.jobs.iter().find_map(|s| s.started_item)
    }

    /// Every round the pass started or ended: the loop's own, then each step's.
    pub fn rounds(&self) -> Vec<RoundId> {
        let mut out = self.dispatching.clone();
        out.extend(self.jobs.iter().flat_map(|s| s.rounds.iter().cloned()));
        out
    }

    fn moved(&self) -> impl Iterator<Item = &Stepped> {
        self.jobs.iter().filter(|s| !s.rounds.is_empty())
    }
}

/// A job the loop dispatched, until the loop reaps it.
pub(crate) struct Job {
    /// Whether it is a step, whose round the owner's page shows running, and not the pings due to its work's sessions.
    step: bool,
    /// Whether the loop granted the step a free slot for its work to leave its wait by: the work holds the slot until
    /// the loop reaps the step.
    granted: bool,
    /// Whether the job's thread is done with it, so that the loop reaps it.
    over: bool,
    thread: Option<JoinHandle<Outcome>>,
    /// The owner's round the step was asked as, and the item the loop started its work on for it.
    asked: Option<Ask>,
    started_item: Option<i64>,
}

/// What a job's thread came to.
struct Outcome {
    /// The rounds its context's connection started or ended, with the cell of the last.
    rounds: Vec<RoundId>,
    cell: Option<String>,
    /// Whether the step found what its work waited for come and left it waiting, as the loop granted it no slot.
    held: bool,
    /// Whether it was the owner's round asked and ran no round, as a failed run's cause that stops every work stood:
    /// the ask stands.
    deferred: bool,
    /// What went wrong that the job could not deal with itself: an error left once its step paused its work, or the
    /// panic its thread ended in.
    failed: Option<String>,
    /// Whether its thread ended in a panic, which leaves the work for the loop to pause.
    panicked: bool,
}

impl Outcome {
    /// A job whose thread ended in a panic, with what the panic said.
    fn panicked(p: &(dyn std::any::Any + Send)) -> Outcome {
        let what = p
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| p.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "no message".to_string());
        Outcome {
            rounds: Vec::new(),
            cell: None,
            held: false,
            deferred: false,
            failed: Some(format!("its step's thread panicked: {what}")),
            panicked: true,
        }
    }
}

/// What a job does.
enum Kind {
    /// A step of the work; `asked` when it is the owner's round, after which the owner holds the work paused.
    Step { asked: bool },
    /// The pings due to the work's sessions, found due at the time given.
    Pings(Vec<warm::Ping>, i64),
}

/// Whether a work goes on by itself under the owner's global pause `pause`: every work while none holds; while one
/// holds, a work the owner started and a work of an architect the owner started.
fn goes_on(pause: Option<&GlobalPause>, w: &Work) -> bool {
    pause.is_none_or(|p| {
        p.runs.contains(&w.id) || (w.graph == ARCHITECT && w.project.as_ref().is_some_and(|x| p.architects.contains(x)))
    })
}

impl Engine {
    /// The works with a job in flight: dispatched by the loop, and not reaped yet.
    pub fn in_flight(&self) -> BTreeSet<i64> {
        lock(&self.shared.jobs).keys().copied().collect()
    }

    /// Whether a work has a job in flight.
    pub(crate) fn has_job(&self, work: i64) -> bool {
        lock(&self.shared.jobs).contains_key(&work)
    }

    /// The works with a step in flight, and not the keep-warm pings of their sessions: each holds its slot, if it holds
    /// one, until the loop reaps the step, whatever happened to the work meanwhile.
    pub(crate) fn stepping(&self) -> BTreeSet<i64> {
        lock(&self.shared.jobs)
            .iter()
            .filter(|(_, j)| j.step)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Whether the loop granted the step in flight of a work a free slot, for the work to leave its wait by.
    pub(crate) fn granted(&self, work: i64) -> bool {
        lock(&self.shared.jobs).get(&work).is_some_and(|j| j.granted)
    }

    /// Whether a step asked as the owner's round `ask` is in flight.
    fn asked_in_flight(&self, ask: &Ask) -> bool {
        lock(&self.shared.jobs).values().any(|j| j.asked.as_ref() == Some(ask))
    }

    /// Lets every work be dispatched again, the works whose last step moved nothing included, since something may have
    /// moved that they wait for: the loop does so when a step or its own turn moved something, and when the owner asked
    /// for something or its idle time passed.
    pub fn rouse(&self) {
        lock(&self.shared.idle).clear();
    }

    /// One turn of the loop's dispatching, with what the owner lets go on (`go`): it reads the configuration again,
    /// hands the engine the owner's global pause, carries out the outbox but for the works with a job in flight, and,
    /// unless a failed run's cause stops every work, brings the works that go on on an older flow version to the
    /// version the engine runs, lets the work that took over the item of a work the owner started go on as started,
    /// starts a work of each project's architect that goes on when a matter is due for it, and
    /// dispatches, each on a thread of its own: the owner's rounds asked, of the works with no job in flight; a step of
    /// every other work that goes on, has no job in flight and is not idle; while a slot is free and the global pause
    /// does not hold, a work started on a ready item; and the keep-warm pings due to the sessions of the works with no
    /// job in flight. A work that waits, holding no slot, steps without one, to find what it waits for come; one whose
    /// step found it come waits on for a slot, which no new work takes meanwhile ([`Engine::start_ready`]), and is
    /// granted a free slot of its machine for its next step, in the order of the works' ids and before any new work
    /// starts, not dispatched while its machine has none free. Gives the owner's rounds asked that it took without a
    /// step, since none could run: a project's architect with nothing to start a work for, or a work brought to the
    /// version the engine runs that could go on from no cell there, whose step that was.
    pub fn dispatch(&mut self, go: &Go) -> Result<Vec<Stepped>, String> {
        self.reload_config();
        self.set_global_pause(go.pause.clone());
        if actions::run_outbox(self)? {
            self.rouse();
        }
        let mut taken = Vec::new();
        if self.halted() {
            return Ok(taken);
        }
        let old: Vec<Work> = self
            .outdated()?
            .into_iter()
            .filter(|w| goes_on(go.pause.as_ref(), w) && !self.has_job(w.id))
            .collect();
        if !old.is_empty() {
            self.bring_up(&old)?;
            self.rouse();
        }
        // The owner's Start of a work taken over, here or by a start over the outbox held, goes on at once in the work
        // that took its item over, as the loop hands it on after the turn ([`Engine::replaced`]).
        let pause = go.pause.clone().map(|mut p| {
            for (old, new) in lock(&self.shared.replaced).iter() {
                if p.runs.contains(old) {
                    p.runs.insert(*new);
                }
            }
            p
        });
        self.set_global_pause(pause.clone());
        let pause = pause.as_ref();
        self.architects_due(pause)?;
        for &work in &go.steps {
            let ask = Ask::Work(work);
            if self.has_job(work) || self.asked_in_flight(&ask) {
                continue;
            }
            match self.owner_round(work)? {
                Some(id) => self.dispatch_step(id, true, false, Some(ask), None)?,
                None => taken.push(Stepped {
                    work: Some(work),
                    asked: Some(ask),
                    ..Stepped::default()
                }),
            }
        }
        for project in &go.architects {
            let ask = Ask::Architect(project.clone());
            if self.asked_in_flight(&ask) {
                continue;
            }
            match architect::running(self, project)? {
                Some(id) if self.has_job(id) => {}
                Some(id) => match self.owner_round(id)? {
                    Some(n) => self.dispatch_step(n, true, false, Some(ask), None)?,
                    None => taken.push(Stepped {
                        asked: Some(ask),
                        ..Stepped::default()
                    }),
                },
                None => {
                    architect::queue_due(self)?;
                    match architect::start(self, project, true)? {
                        Some(id) => {
                            self.say(&format!(
                                "work {id}: a work of the architect of {project} starts, as the owner steps it"
                            ));
                            self.dispatch_step(id, true, false, Some(ask), None)?;
                        }
                        None => taken.push(Stepped {
                            asked: Some(ask),
                            ..Stepped::default()
                        }),
                    }
                }
            }
        }
        for id in self.active()? {
            if self.has_job(id) || lock(&self.shared.idle).contains(&id) {
                continue;
            }
            let w = self.work(id)?;
            if !goes_on(pause, &w) || w.flow_version != self.flow_now {
                continue;
            }
            // Only a work whose last step found its wait over is granted a slot; while its machine has none free it
            // is not dispatched, and the slots are kept for it ([`Engine::start_ready`]).
            let held = lock(&self.shared.held_for_slot).contains(&id) && self.waiting(&w)?;
            let granted = held && self.room_for(&w)?;
            if held && !granted {
                continue;
            }
            self.dispatch_step(id, false, granted, None, None)?;
        }
        if pause.is_none() {
            self.start_ready()?;
        }
        self.keep_warm(db::now(), pause)?;
        Ok(taken)
    }

    /// Starts works on ready items, each with its first step dispatched, while more slots are free than the works that
    /// wait may need ([`Engine::waits_for_a_slot`]): those go on first, as slots free.
    fn start_ready(&mut self) -> Result<(), String> {
        loop {
            if self.free_slots()? <= self.waits_for_a_slot()? {
                return Ok(());
            }
            let Some(item) = self.next_ready() else {
                return Ok(());
            };
            let id = self.create_work(item)?;
            self.say(&format!("work {id} started on item {item}"));
            self.dispatch_step(id, false, false, None, Some(item))?;
        }
    }

    /// How many works that wait, holding no slot, may need a free slot before any new work starts: those whose last
    /// step found what they waited for come and that wait on for one, and those whose step in flight, granted none,
    /// may find it come; none since ended, paused or gone on.
    fn waits_for_a_slot(&mut self) -> Result<i64, String> {
        let mut ids: BTreeSet<i64> = lock(&self.shared.held_for_slot).clone();
        ids.extend(
            lock(&self.shared.jobs)
                .iter()
                .filter(|(_, j)| j.step && !j.granted)
                .map(|(id, _)| *id),
        );
        let mut n = 0;
        for id in ids {
            let w = self.work(id)?;
            if w.graph == ENGINEER
                && w.ended.is_none()
                && w.paused.is_none()
                && !self.granted(id)
                && self.waiting(&w)?
            {
                n += 1;
            }
        }
        Ok(n)
    }

    /// Starts a work of each project's architect that goes on under the owner's global pause `pause` and runs none,
    /// when the cell its architect waits at to start (`S0`) leads on, once what the engine raises itself is queued.
    fn architects_due(&mut self, pause: Option<&GlobalPause>) -> Result<(), String> {
        let projects: Vec<String> = self
            .projects()
            .into_iter()
            .filter(|p| pause.is_none_or(|g| g.architects.contains(p)))
            .collect();
        if projects.is_empty() {
            return Ok(());
        }
        architect::queue_due(self)?;
        for project in projects {
            if architect::running(self, &project)?.is_some() {
                continue;
            }
            if let Some(id) = architect::start(self, &project, false)? {
                self.say(&format!("work {id}: a work of the architect of {project} starts"));
                self.rouse();
            }
        }
        Ok(())
    }

    /// Makes ready the owner's round of a work, as the loop does before it dispatches it: the work is taken into the
    /// owner's hands ([`Engine::take_in_hand`]) and, on an older flow version, brought up first ([`Engine::bring`]), so
    /// that the round is of the cell it moved to or went back to. Gives the work whose step to dispatch; none when it
    /// could go on from no cell of the version and stays where it was brought, paused, or ended as the version has no
    /// graph for it: that was its step.
    fn owner_round(&mut self, work: i64) -> Result<Option<i64>, String> {
        self.take_in_hand(work)?;
        let w = self.work(work)?;
        if w.ended.is_some() || w.flow_version == self.flow_now || self.done(&w)? {
            return Ok(Some(work));
        }
        Ok(match self.bring(&w)? {
            versions::Plan::Stuck { .. } => None,
            _ => Some(work),
        })
    }

    /// Dispatches a step of a work as its job: the owner's round when `asked`, granted a free slot when `granted`.
    fn dispatch_step(
        &mut self,
        work: i64,
        asked: bool,
        granted: bool,
        ask: Option<Ask>,
        started_item: Option<i64>,
    ) -> Result<(), String> {
        if granted {
            lock(&self.shared.held_for_slot).remove(&work);
        }
        self.spawn(work, Kind::Step { asked }, granted, ask, started_item)
    }

    /// Dispatches the pings found due at `now` to a work's sessions as the work's job.
    pub(crate) fn dispatch_pings(&mut self, work: i64, pings: Vec<warm::Ping>, now: i64) -> Result<(), String> {
        self.spawn(work, Kind::Pings(pings, now), false, None, None)
    }

    /// Hands a job of a work to a thread of its own, on a context forked from this one, and keeps it among the jobs
    /// in flight until the loop reaps it.
    fn spawn(
        &mut self,
        work: i64,
        kind: Kind,
        granted: bool,
        asked: Option<Ask>,
        started_item: Option<i64>,
    ) -> Result<(), String> {
        let mut ctx = self.fork()?;
        let step = matches!(kind, Kind::Step { .. });
        let job = Job {
            step,
            granted,
            over: false,
            thread: None,
            asked,
            started_item,
        };
        lock(&self.shared.jobs).insert(work, job);
        let shared = Arc::clone(&self.shared);
        let spawned = std::thread::Builder::new().name(format!("work {work}")).spawn(move || {
            let outcome =
                catch_unwind(AssertUnwindSafe(|| ctx.run(work, kind))).unwrap_or_else(|p| Outcome::panicked(&*p));
            if let Some(j) = lock(&shared.jobs).get_mut(&work) {
                j.over = true;
            }
            shared.job_over.notify_all();
            (shared.hooks.ended)();
            outcome
        });
        match spawned {
            Ok(t) => {
                if let Some(j) = lock(&self.shared.jobs).get_mut(&work) {
                    j.thread = Some(t);
                }
            }
            Err(x) => {
                lock(&self.shared.jobs).remove(&work);
                return Err(format!("work {work}: no thread for its job: {x}"));
            }
        }
        if step {
            self.tell_busy();
        }
        Ok(())
    }

    /// Does a job on this context's thread: a step of the work, or the pings due to its sessions. The owner's round
    /// asked that ran no round, since a failed run's cause that stops every work stood when it was to start, leaves
    /// the work as it was and the owner's ask standing, for the loop to dispatch once the wait is over.
    fn run(&mut self, work: i64, kind: Kind) -> Outcome {
        let mark = self.db.rounds_mark();
        let mut held = false;
        let mut deferred = false;
        let failed = match kind {
            Kind::Step { asked } => match self.take_step(work, asked) {
                Ok(a) => {
                    held = matches!(a, Advance::Held(_));
                    deferred = asked
                        && matches!(a, Advance::Waits(_))
                        && self.halt().is_some()
                        && self.rounds_since(&mark).is_ok_and(|(r, _)| r.is_empty());
                    if asked && !deferred {
                        self.paused_after_step(work).err()
                    } else {
                        None
                    }
                }
                Err(e) => Some(e),
            },
            Kind::Pings(pings, now) => warm::send(self, work, &pings, now).err(),
        };
        let (rounds, cell) = self.rounds_since(&mark).unwrap_or_default();
        Outcome {
            rounds,
            cell,
            held,
            deferred,
            failed,
            panicked: false,
        }
    }

    /// Tells which works' steps run now, as the jobs in flight stand.
    fn tell_busy(&self) {
        let running: Vec<i64> = lock(&self.shared.jobs)
            .iter()
            .filter(|(_, j)| j.step)
            .map(|(id, _)| *id)
            .collect();
        (self.shared.hooks.busy)(&running);
    }

    /// Reaps the jobs whose threads are done with them, as the loop does at each turn: a work whose step found what it
    /// waited for come but was granted no slot is named as waiting for a free slot until a later step of it ends, and
    /// is granted one as soon as its machine has one free; any other work whose step moved nothing is idle, not
    /// dispatched again until something moves, and a step that moved
    /// something lets every work be dispatched again ([`Engine::rouse`]); a work whose step's thread panicked is paused
    /// with why, unless it ended. Gives what each step came to, in the order of their works' ids.
    pub fn reap(&mut self) -> Result<Vec<Stepped>, String> {
        let over: Vec<(i64, Job)> = {
            let mut jobs = lock(&self.shared.jobs);
            let ids: Vec<i64> = jobs.iter().filter(|(_, j)| j.over).map(|(id, _)| *id).collect();
            ids.into_iter()
                .filter_map(|id| jobs.remove(&id).map(|j| (id, j)))
                .collect()
        };
        if over.is_empty() {
            return Ok(Vec::new());
        }
        self.tell_busy();
        let mut out = Vec::new();
        let mut moved = false;
        for (work, job) in over {
            let outcome = match job.thread.map(JoinHandle::join) {
                Some(Ok(o)) => o,
                Some(Err(p)) => Outcome::panicked(&*p),
                None => Outcome::panicked(&"the loop kept no thread of it"),
            };
            if job.step {
                if outcome.held {
                    lock(&self.shared.held_for_slot).insert(work);
                } else {
                    lock(&self.shared.held_for_slot).remove(&work);
                }
                if !outcome.rounds.is_empty() {
                    moved = true;
                } else if job.asked.is_none() && !outcome.held {
                    lock(&self.shared.idle).insert(work);
                }
            }
            if let Some(why) = &outcome.failed {
                self.say(&format!("work {work}: {why}"));
                if outcome.panicked
                    && self.ended(work)?.is_none()
                    && let Err(x) = self.pause(work, why)
                {
                    self.say(&format!("work {work} is not paused: {x}"));
                }
            }
            if job.step {
                out.push(Stepped {
                    work: Some(work),
                    started_item: job.started_item,
                    rounds: outcome.rounds,
                    cell: outcome.cell,
                    // The owner's round deferred is not the round asked: the ask stands for a later dispatch.
                    asked: job.asked.filter(|_| !outcome.deferred),
                });
            }
        }
        if moved {
            self.rouse();
        }
        Ok(out)
    }

    /// Waits until every job in flight has ended, and reaps them all ([`Engine::reap`]): what the loop does before the
    /// engine stops, and after it dispatched a pass.
    pub fn drain(&mut self) -> Result<Vec<Stepped>, String> {
        {
            let mut jobs = lock(&self.shared.jobs);
            while jobs.values().any(|j| !j.over) {
                jobs = self.shared.job_over.wait(jobs).unwrap_or_else(PoisonError::into_inner);
            }
        }
        self.reap()
    }

    /// One pass of the loop, as `wf-agents run --once` and the tests take it: every work looked at afresh, what `go`
    /// lets go on dispatched ([`Engine::dispatch`]), every job waited for and reaped; then, when no step moved, unless
    /// the global pause holds or a failed run's cause stops every work, works started on ready items in the slots that
    /// the steps of works still waiting kept free while they ran, as the loop's next turn would start them once those
    /// works are idle, and waited for in turn; and the outbox the steps filled carried out. After a step that moved,
    /// the next pass, as the loop's next turn, looks at every work first, so that a work whose wait that step ended is
    /// granted a free slot before a new work starts in it. When dispatching fails, the jobs it dispatched are waited for
    /// first, as the loop does before it stops.
    pub fn pass(&mut self, go: &Go) -> Result<Passed, String> {
        self.rouse();
        let mark = self.db.rounds_mark();
        let mut jobs = match self.dispatch(go) {
            Ok(taken) => taken,
            Err(x) => {
                self.drain()?;
                return Err(x);
            }
        };
        jobs.extend(self.drain()?);
        let moved = jobs.iter().any(|s| !s.rounds.is_empty());
        if !moved && go.pause.is_none() && !self.halted() {
            self.start_ready()?;
            jobs.extend(self.drain()?);
        }
        actions::run_outbox(self)?;
        let (dispatching, _) = self.rounds_since(&mark)?;
        Ok(Passed { dispatching, jobs })
    }
}
