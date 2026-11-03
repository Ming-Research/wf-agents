//! The engine: takes each work through the compiled flow, one cell at a time. At an agent's cell it runs a round of
//! the agent, whose message gives the always inputs every time and each conditional input when the session lacks it; at its own cells and
//! the tool cells it does the step itself; at the owner's it waits for the owner's answer. Each return is checked
//! against the cell's schema and checks, takes exactly one row, and the round, the work's new position and what the
//! row does in the database are written in one transaction. What must happen outside the database is put in the
//! outbox in that transaction and carried out after it, so a crash loses none. A round that ends in no return the
//! cell takes is handled as the flow's rounds section says.
//!
//! Each project's architect's works run its graph on no item, one at a time for each project and holding no slot,
//! started as the cell the project's architect waits at to start (`S0`) says, when the project's queue of matters has
//! one ([`architect`]).
//!
//! In this version the engine and the session manager are one process on one machine. The engine's loop is the only
//! scheduler ([`schedule`]): it hands each step it dispatches to a thread of its own, never two jobs of one work at once,
//! so that the rounds and tool runs of different works overlap while each work moves one step after another. What
//! lasts across steps and is the same for every step is one [`Shared`] part behind an `Arc`, safe to share between
//! threads, and each thread that takes a step holds an [`Engine`] of its own ([`Engine::fork`]), with its own connection
//! to the database and the configuration as its step began. Its git commands that change what every worktree of a
//! project's repository shares take turns ([`sessions::worktree::git`]).

pub mod backup;
pub mod blockers;
pub mod database;
pub mod ending;
pub mod git;
pub mod hub;
pub mod limits;

mod actions;
mod arch_changes;
mod architect;
mod cards;
mod checks;
mod decisions;
mod failed;
mod faults;
mod inputs;
pub mod message;
mod open_questions;
mod reasoning;
mod reminders;
pub mod schedule;
mod steps;
mod test_name;
mod tools;
mod unparsed;
mod values;
mod versions;
mod warm;
mod watch;
mod work;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, RwLock};
use std::time::Duration;

use db::rusqlite::{OptionalExtension, params};
use db::{Actor, Db};
use flow::{Compiled, CompiledCell, CompiledGraph};
use serde_json::Value;

/// The command line agents run in, which the session manager starts.
pub use sessions::cli as agent;

pub use checks::DESIGN_CARD_RULING;
pub use failed::{Halt, network_reachable, utc};
pub use schedule::{Ask, Go, Passed};
pub use steps::{cell_request, next_message};
pub use warm::GlobalPause;
pub use work::{Counted, Item, Round, RoundId, Work, json_col, query_json, query_json_counted};

use agent::Agents;
use hub::GitHub;

/// The graph works on items run.
pub const ENGINEER: &str = "engineer";

/// The graph the architect's works run, on no item, and the line its session goes on in.
pub const ARCHITECT: &str = "architect";

/// The graph a flow version before the setting of a project's policy was a stage of the architect's works ran it as, a
/// work of its own: the engine takes such a work over when it next goes on, ending it and letting go the hold on new
/// starts it kept, and the architect's next check of the policy decides again whether to set it.
pub const POLICY: &str = "policy";

/// The line of the search of observations, which runs at the observer's level.
pub const OBSERVER: &str = "observer";

/// The engine's version: the commit it was built from, which every agent's round records.
pub const VERSION: &str = env!("WF_AGENTS_COMMIT");

/// What one step of a work came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Advance {
    /// A round or step ended: the work moved on, or its return was sent back to be done again.
    Moved,
    /// The work waits: for the owner, for other works, or for its turn.
    Waits(String),
    /// What the work waited for came, and it waits on, holding no slot, since the loop granted its step none: its
    /// machine had no slot free.
    Held(String),
    /// The work is done, was ended or taken over, or is paused.
    Stopped,
}

/// Where the engine tells what the agent of a running round is doing: the round's work, its number in the work, and
/// what it does.
pub type Activity = dyn Fn(i64, i64, &str) + Send + Sync;

/// Where the engine tells how a running round stands: the round's work, its number in the work, and what its calls to
/// the model come to so far.
pub type Measured = dyn Fn(i64, i64, sessions::guard::Live) + Send + Sync;

/// Where the loop tells which works it is taking a step of, in the order of their ids.
pub type Busy = dyn Fn(&[i64]) + Send + Sync;

/// Where the engine tells what runs, given when it is made and shared by every thread's steps.
#[derive(Clone)]
pub struct Hooks {
    /// Where the engine says what it does and what went wrong.
    pub say: Arc<dyn Fn(&str) + Send + Sync>,
    /// What the agent of a running round is doing, by its work and round, as its command line reports it.
    pub activity: Arc<Activity>,
    /// How a running round stands, by its work and round, after each of its calls to the model.
    pub measure: Arc<Measured>,
    /// Where the loop says which works it is taking a step of, in the order of their ids, each time it dispatches a
    /// step or reaps one: the owner's page shows those works' rounds as running, and what stops once each ends.
    pub busy: Arc<Busy>,
    /// Where a job's thread says that the job has ended, a step or a work's keep-warm pings: the loop waits on it, to
    /// reap the job and dispatch what its end lets go on.
    pub ended: Arc<dyn Fn() + Send + Sync>,
    /// What is told the rows each committed write touched, registered on every connection of the engine: how the
    /// owner's page hears of each change.
    pub commit: Option<db::Listener>,
    /// Whether the agents' service can be reached from here, which the engine looks at itself while no network stops
    /// every work ([`failed::network_reachable`]); none to let a round find out, as before.
    pub network: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
}

impl Default for Hooks {
    /// Says what the engine does on the standard error, and tells nothing else.
    fn default() -> Hooks {
        Hooks {
            say: Arc::new(|s: &str| eprintln!("{s}")),
            activity: Arc::new(|_: i64, _: i64, _: &str| {}),
            measure: Arc::new(|_: i64, _: i64, _: sessions::guard::Live| {}),
            busy: Arc::new(|_: &[i64]| {}),
            ended: Arc::new(|| {}),
            commit: None,
            network: None,
        }
    }
}

/// What a graph as a project's agents are asked it depends on: the flow's version, the graph, the project and the
/// configuration's commit.
type AskedGraph = (i64, String, Option<String>, String);

/// What every thread's steps share and what lasts from one step to the next: one for the engine's process. Each lock
/// is held only while what it guards is read or changed, never across a round, a command or a write.
struct Shared {
    /// The compiled flows read so far, by version.
    flows: RwLock<BTreeMap<i64, Arc<Compiled>>>,
    /// The graphs as projects' agents are asked them, by flow version, graph, project and configuration's commit
    /// ([`Engine::graph_of`]).
    asked: RwLock<BTreeMap<AskedGraph, Arc<CompiledGraph>>>,
    /// The configuration in use: the last good one read ([`Engine::reload_config`]).
    config: RwLock<Arc<config::Committed>>,
    /// While a failed run's cause stops every work alike (a usage limit, no network), why and until when: no new
    /// round starts until then.
    halt: Mutex<Option<Halt>>,
    /// The kinds of failed run the owner was told stop every work, until a round runs to a return again: the owner is
    /// told of each once, though the rounds of several works meet several causes in turn.
    told: Mutex<BTreeSet<String>>,
    /// How many times in a row the engine looked at the network itself and found it down, since a round last tried.
    looked: Mutex<u32>,
    /// The owner's global pause while it holds, as the loop that runs the engine hands it each turn: no work takes a
    /// step but those the owner started, and no new work starts; none while works go on. The architects that judge
    /// what runs and what starts are told it with the slots.
    global_pause: Mutex<Option<GlobalPause>>,
    /// The works taken over, each with the new work that took its item over, until the engine's loop has read them
    /// ([`Engine::replaced`]): the owner's Start of a work goes on in the work that replaced it.
    replaced: Mutex<Vec<(i64, i64)>>,
    /// The works whose last step found what they waited for come and waited on, holding no slot, since the loop granted
    /// it none ([`Advance::Held`]); a work leaves it when the loop grants its step a slot or reaps its next step. Only
    /// the loop changes it.
    held_for_slot: Mutex<BTreeSet<i64>>,
    /// The jobs the loop dispatched and has not reaped yet, by their works: one at most for each work. Only the loop
    /// adds and removes them; a job's thread only marks its own as over ([`schedule`]).
    jobs: Mutex<BTreeMap<i64, schedule::Job>>,
    /// Woken each time a job's thread marks its job as over, for a loop that waits until every job has ended.
    job_over: Condvar,
    /// The works whose last step moved nothing, which the loop does not dispatch again until something moved since
    /// ([`Engine::rouse`]): a work that waits and finds nothing come would otherwise be dispatched again at once.
    idle: Mutex<BTreeSet<i64>>,
    /// The session manager: the agents' sessions and the command line they run in.
    sessions: sessions::Manager,
    github: Box<dyn GitHub>,
    hooks: Hooks,
}

impl Shared {
    fn new(
        config: config::Committed,
        flows: BTreeMap<i64, Arc<Compiled>>,
        agents: Box<dyn Agents>,
        github: Box<dyn GitHub>,
        hooks: Hooks,
    ) -> Shared {
        Shared {
            flows: RwLock::new(flows),
            asked: RwLock::default(),
            config: RwLock::new(Arc::new(config)),
            halt: Mutex::default(),
            told: Mutex::default(),
            looked: Mutex::default(),
            global_pause: Mutex::default(),
            replaced: Mutex::default(),
            held_for_slot: Mutex::default(),
            jobs: Mutex::default(),
            job_over: Condvar::new(),
            idle: Mutex::default(),
            sessions: sessions::Manager::new(agents),
            github,
            hooks,
        }
    }

    /// The configuration in use.
    fn config(&self) -> Arc<config::Committed> {
        self.config.read().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// A compiled flow read before, by its version.
    fn flow(&self, version: i64) -> Option<Arc<Compiled>> {
        self.flows
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&version)
            .cloned()
    }
}

/// What a level's rounds run on ([`Engine::runs_on`]), which [`RunsOn::on`] lends to the session manager.
#[derive(Debug, Clone)]
pub struct RunsOn {
    model: String,
    home: Option<PathBuf>,
    cache: Option<i64>,
}

impl RunsOn {
    pub fn on(&self) -> sessions::On<'_> {
        sessions::On {
            model: &self.model,
            home: self.home.as_deref(),
            cache: self.cache,
        }
    }
}

/// One thread's context for taking steps: the part every thread shares, a connection to the database of its own, with
/// the shared commit listener registered on it, and the configuration as its step began. A step writes only through
/// its context's connection, so what one step wrote ([`Stepped`]) is what that connection wrote.
pub struct Engine {
    shared: Arc<Shared>,
    pub db: Db,
    /// The data directory: the configuration, the database and the worktrees.
    pub data: PathBuf,
    /// The configuration as this context's step began, from the shared part; it stays the same throughout the step.
    pub config: Arc<config::Committed>,
    /// The flow version new works start on: the one built into this engine.
    pub flow_now: i64,
    /// How long a run that failed in passing waits before it runs again the first time; twice as long each time
    /// after, up to five minutes.
    pub retry_wait: Duration,
    /// How long a second of the flow's backoff for a step of the engine's that failed in passing lasts: one second,
    /// which a test shortens so that it does not wait.
    pub backoff_unit: Duration,
}

// A thread that takes a step owns its context, and every thread shares one `Shared`: a field added to either that may
// not cross threads fails here, when the engine is compiled.
const _: () = {
    const fn sendable<T: Send>() {}
    const fn shareable<T: Send + Sync>() {}
    sendable::<Engine>();
    shareable::<Shared>();
};

/// What a lock guards, also after a thread panicked while it held it: each value the shared part guards is whole after
/// every change, so it is still good.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Engine {
    /// An engine on a data directory, with its committed configuration, telling `hooks` what runs; the flow built into
    /// it is stored as a flow version if it is new.
    pub fn new(
        data: &Path,
        config: config::Committed,
        agents: Box<dyn Agents>,
        github: Box<dyn GitHub>,
        hooks: Hooks,
    ) -> Result<Engine, String> {
        let mut db = database::open(data)?;
        let compiled = compile_for(&config.config)?;
        let digest = compiled.digest();
        let json = compiled.json();
        let flow_now = db
            .write(Actor::Engine, |tx| {
                tx.execute(
                    "INSERT INTO flow_versions (digest, compiled) VALUES (?1, ?2) ON CONFLICT (digest) DO NOTHING",
                    params![digest, json],
                )?;
                Ok(
                    tx.query_row("SELECT id FROM flow_versions WHERE digest = ?1", [&digest], |r| {
                        r.get(0)
                    })?,
                )
            })
            .map_err(|e| e.to_string())?;
        // What is written from here on is told.
        if let Some(f) = &hooks.commit {
            db.on_commit(Arc::clone(f)).map_err(|e| e.to_string())?;
        }
        let flows = BTreeMap::from([(flow_now, Arc::new(compiled))]);
        let e = Engine::on(Shared::new(config, flows, agents, github, hooks), db, data, flow_now);
        let names: Vec<String> = e.config.config.install.agents.clis.keys().cloned().collect();
        for name in names {
            e.cli_version(&name);
        }
        Ok(e)
    }

    /// The first context on a shared part, on the connection given.
    fn on(shared: Shared, db: Db, data: &Path, flow_now: i64) -> Engine {
        let config = shared.config();
        Engine {
            shared: Arc::new(shared),
            db,
            data: data.to_path_buf(),
            config,
            flow_now,
            retry_wait: Duration::from_secs(10),
            backoff_unit: Duration::from_secs(1),
        }
    }

    /// Another context on the same engine, for another thread to take steps on: a connection of its own to the data
    /// directory's database, with the shared commit listener registered on it, the same shared part, and the
    /// configuration in use.
    pub fn fork(&self) -> Result<Engine, String> {
        let mut db = database::open(&self.data)?;
        if let Some(f) = &self.shared.hooks.commit {
            db.on_commit(Arc::clone(f)).map_err(|e| e.to_string())?;
        }
        Ok(Engine {
            shared: Arc::clone(&self.shared),
            db,
            data: self.data.clone(),
            config: self.shared.config(),
            flow_now: self.flow_now,
            retry_wait: self.retry_wait,
            backoff_unit: self.backoff_unit,
        })
    }

    /// The engine made only to read, for a dry run such as building a work's next message beside an engine that may be
    /// running: the database is opened read-only and as it is, nothing is recorded (not even the built-in flow's
    /// version), no work is stopped or started, and no command line is asked its version. Steps on it fail.
    pub fn reading(
        data: &Path,
        config: config::Committed,
        agents: Box<dyn Agents>,
        github: Box<dyn GitHub>,
    ) -> Result<Engine, String> {
        let db = database::open_read_only(data)?;
        let compiled = compile_for(&config.config)?;
        // The built-in flow's version, if the database has it; a work reads its own version from the database.
        let flow_now: i64 = db
            .conn()
            .query_row(
                "SELECT id FROM flow_versions WHERE digest = ?1",
                [compiled.digest()],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let mut flows = BTreeMap::new();
        if flow_now != 0 {
            flows.insert(flow_now, Arc::new(compiled));
        }
        let shared = Shared::new(config, flows, agents, github, Hooks::default());
        Ok(Engine::on(shared, db, data, flow_now))
    }

    /// Says what the engine does or what went wrong, where its hooks say.
    pub fn say(&self, text: &str) {
        (self.shared.hooks.say)(text);
    }

    /// Tells what the agent of a running round is doing: the round's work, its number in the work, and what it does.
    pub fn activity(&self, work: i64, round: i64, what: &str) {
        (self.shared.hooks.activity)(work, round, what);
    }

    /// The session manager: the agents' sessions and the command line they run in.
    pub fn sessions(&self) -> &sessions::Manager {
        &self.shared.sessions
    }

    pub fn github(&self) -> &dyn GitHub {
        self.shared.github.as_ref()
    }

    /// While a failed run's cause stops every work alike (a usage limit, no network), why and until when: no new round
    /// starts until then.
    pub fn halt(&self) -> Option<Halt> {
        lock(&self.shared.halt).clone()
    }

    /// Sets, or lifts with none, what stops every work.
    pub fn set_halt(&self, halt: Option<Halt>) {
        *lock(&self.shared.halt) = halt;
    }

    /// Stops every work as `halt` says, unless the wait that stands, which another work's round may have set, ends
    /// later; gives whether the owner is to be told, which is the first time since a round last ran to a return that
    /// this kind stops them.
    fn halt_every_work(&self, halt: Halt) -> bool {
        let kind = halt.kind.clone();
        {
            let mut standing = lock(&self.shared.halt);
            if !standing.as_ref().is_some_and(|h| h.until >= halt.until) {
                *standing = Some(halt);
            }
        }
        lock(&self.shared.told).insert(kind)
    }

    /// The owner's global pause while it holds, as the loop that runs the engine last handed it: no work takes a step
    /// but those the owner started, and no new work starts; none while works go on. The architects that judge what
    /// runs and what starts are told it with the slots.
    pub fn global_pause(&self) -> Option<GlobalPause> {
        lock(&self.shared.global_pause).clone()
    }

    /// Hands the engine the owner's global pause, none while works go on.
    pub fn set_global_pause(&self, pause: Option<GlobalPause>) {
        *lock(&self.shared.global_pause) = pause;
    }

    /// Brings works on an older flow version to the version the engine runs, which the loop does when each next goes
    /// on, before it dispatches its step: a work that can move moves ([`Engine::move_up`]), and the rest are taken over
    /// ([`Engine::take_over`]). Gives the new works started on the items of those taken over.
    fn bring_up(&mut self, works: &[Work]) -> Result<Vec<i64>, String> {
        let mut over = Vec::new();
        for w in works {
            match self.cannot_move(w)? {
                None => self.move_up(w)?,
                Some(cannot) => over.push((w.clone(), self.moved_on(&cannot))),
            }
        }
        self.take_over(&over)
    }

    /// Why a work on an older flow version cannot move to the version the engine runs ([`versions::why_not`]): what
    /// changed at the cell it stands at, or a return it handed in that its cell there does not take; none when it can
    /// move. Both versions' graphs are read as the work's project's agents are asked them, which its returns were
    /// checked against.
    fn cannot_move(&mut self, w: &Work) -> Result<Option<String>, String> {
        let old = self.flow(w.flow_version)?.graph(&w.graph).cloned();
        let now = self.flow_now;
        let new = self.flow(now)?.graph(&w.graph).cloned();
        let (Some(old), Some(new)) = (old, new) else {
            return Ok(Some(format!("there is no graph {}", w.graph)));
        };
        let (old, new) = (self.as_asked(w, old)?, self.as_asked(w, new)?);
        let handed: Vec<versions::Handed> = work::rounds(self.db.conn(), w.id)?
            .into_iter()
            .filter(|r| r.transition.is_some())
            .filter_map(|r| {
                Some(versions::Handed {
                    round: r.id,
                    cell: r.cell?,
                    output: r.output?,
                })
            })
            .collect();
        Ok(versions::why_not(&old, &new, &w.pos, &handed))
    }

    /// Moves a work on an older flow version to the version the engine runs: the same work, at the same cell, in the
    /// same sessions, recording the version it moved from. Its sessions are given again what the version changed of
    /// what they hold, as any changed input is given.
    fn move_up(&mut self, w: &Work) -> Result<(), String> {
        let (id, from, now) = (w.id, w.flow_version, self.flow_now);
        self.db
            .write(Actor::Engine, |tx| {
                tx.execute(
                    "UPDATE works SET flow_version = ?2, moved_from = ?3 WHERE id = ?1",
                    params![id, now, from],
                )?;
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        self.say(&format!(
            "work {id} moved from flow version {from} to {now}, at cell {}",
            w.pos.at
        ));
        Ok(())
    }

    /// Why a work on an older flow version that cannot move is taken over: the version, and what changed there.
    fn moved_on(&self, cannot: &str) -> String {
        format!(
            "the flow moved to version {}, where {cannot}, and a new work on it takes the item over",
            self.flow_now
        )
    }

    /// Takes works over, each with why: a work on an older flow version that cannot move, which the engine does when
    /// it next goes on, instead of taking its step, and a work the owner or its project's architect started over
    /// ([`Engine::start_over`]). Each stops, saying why, as every work ends ([`ending::end`]: the round it was in is
    /// cut off, its decision cards are replaced, its asks of the owner need no answer, its questions to the architect are
    /// withdrawn and its sessions end), and its item gets a new work on the version the engine runs, whose cell `S1`
    /// takes over the stopped work's branch, worktree (with what is not committed in it) and pull request and whose
    /// cell `R1` is told what exists, the questions withdrawn included. Until then the owner still rules on its design
    /// cards and answers its asks, and the new work is given them. A new work is paused as the owner had paused the old
    /// one, so that a work the owner steps stays in the owner's hands, and goes on under the owner's Start of the old
    /// one ([`Engine::replaced`]). Every work given is stopped before any new one starts, so that the slots they held
    /// are free; a new work takes a slot only when one is free: in the order of the works stopped, those that find one
    /// start now, and the items of the rest go to the head of the queue, ahead of the items queued before, in the same
    /// order, to start as slots free. Gives the new works started.
    fn take_over(&mut self, works: &[(Work, String)]) -> Result<Vec<i64>, String> {
        let mut takeovers = Vec::new();
        let projects = self.config.config.projects.clone();
        for (w, why) in works {
            let id = w.id;
            let stopped = format!("stopped: {why}");
            self.db
                .write(Actor::Engine, |tx| {
                    ending::end(tx, id, ending::End::TakenOver(&stopped), &projects).map_err(db::Error::Abort)?;
                    actions::notify(tx, Some(id), &format!("Work {id} stopped: {why}.")).map_err(db::Error::Abort)?;
                    Ok(())
                })
                .map_err(|e| e.to_string())?;
            self.say(&format!("work {id} stopped: {why}"));
            // An architect work stopped leaves no hold of its own behind: setting its project straight is the work's,
            // as a policy being set was the setting's, a work of its own in a flow version before; the architect's
            // next work asks its questions afresh.
            if w.graph == ARCHITECT || w.graph == POLICY {
                let project = w.project.clone();
                self.db
                    .write(Actor::Engine, |tx| {
                        tx.execute(
                            "DELETE FROM dispatch_holds WHERE project = ?1 AND reason = 'set_straight'",
                            [&project],
                        )?;
                        Ok(())
                    })
                    .map_err(|e| e.to_string())?;
            }
            // The worktree a setting of a policy read in, as a work of its own, is no one's any longer.
            if w.graph == POLICY
                && let Some(p) = w.project.as_deref().and_then(|p| projects.get(p))
            {
                let dir = architect::worktree(&self.data, POLICY, w.project.as_deref().unwrap_or_default());
                if let Err(x) = sessions::worktree::remove_worktree(&p.checkout, &dir) {
                    self.say(&format!("work {id}: its worktree {} stays: {x}", dir.display()));
                }
            }
            if let Some(item) = w.item.filter(|_| w.graph == ENGINEER) {
                let held = w.paused.clone().filter(|_| w.paused_by.as_deref() == Some("owner"));
                takeovers.push((id, item, held));
            }
        }
        // Once one waits, those after it wait too, so that they start in the order of the works they replace.
        let mut started = Vec::new();
        let mut waiting = Vec::new();
        for (id, item, held) in takeovers {
            if waiting.is_empty() && self.free_slots()? > 0 {
                let new = self.start_work(item, held.as_deref())?;
                self.say(&format!("work {new} started on item {item}, after work {id}"));
                lock(&self.shared.replaced).push((id, new));
                started.push(new);
            } else {
                waiting.push((id, item));
            }
        }
        if waiting.is_empty() {
            return Ok(started);
        }
        let items: Vec<i64> = waiting.iter().map(|(_, item)| *item).collect();
        self.db
            .write(Actor::Engine, |tx| {
                let first: f64 = tx.query_row("SELECT coalesce(min(position), 1.0) FROM dispatch_queue", [], |r| {
                    r.get(0)
                })?;
                for (n, item) in items.iter().enumerate() {
                    let position = first - (items.len() - n) as f64;
                    tx.execute(
                        "INSERT OR REPLACE INTO dispatch_queue (item, position) VALUES (?1, ?2)",
                        params![item, position],
                    )?;
                }
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        for (id, item) in waiting {
            self.say(&format!(
                "item {item} waits for a free slot at the head of the queue, to take over from work {id}"
            ));
        }
        Ok(started)
    }

    /// Starts a work over, as the owner or its project's architect asked ([`db::start_over`]), by taking it over as a
    /// work on an older flow version is taken over ([`Engine::take_over`]), saying who started it over and, for the
    /// architect, why. A work that can no longer be started over, as one that ended since it was asked, is left as it
    /// is, and one whose merge came first goes on to watch main, whoever asked told.
    fn start_over(&mut self, id: i64, by: &str, why: Option<&str>) -> Result<(), String> {
        let w = self.work(id)?;
        if w.ended.is_some() || w.graph != ENGINEER || self.done(&w)? {
            self.say(&format!("work {id} is not started over: it is over"));
            return Ok(());
        }
        if let Some(item) = w.item
            && db::merge_stands(self.db.conn(), item).map_err(|e| e.to_string())?
        {
            self.db
                .write(Actor::Engine, |tx| {
                    ending::merge_came_first(tx, item, "started over", &[by]).map_err(db::Error::Abort)
                })
                .map_err(|e| e.to_string())?;
            self.say(&format!("work {id} is not started over: its merge came first"));
            return Ok(());
        }
        let who = if by == db::Actor::Architect.name() {
            "the architect"
        } else {
            "the owner"
        };
        let why = match why.map(|w| db::without_final_terminal(w.trim())) {
            Some(why) => format!("{who} started it over, and a new work on it takes the item over: {why}"),
            None => format!("{who} started it over, and a new work on it takes the item over"),
        };
        self.take_over(&[(w, why)])?;
        Ok(())
    }

    /// Carries out a cancel of an item the outbox holds, as the loop does: not yet while a work on the item has a job in
    /// flight, which may be a work that took the item over since the cancel was asked.
    pub(crate) fn cancel_asked(&mut self, item: i64, why: &str, by: &str) -> Result<actions::Carried, String> {
        let works = ending::not_ended(self.db.conn(), item)?;
        if works.iter().any(|w| self.has_job(*w)) {
            return Ok(actions::Carried::NotYet);
        }
        let projects = self.config.config.projects.clone();
        self.db
            .write(Actor::Engine, |tx| {
                ending::cancel_asked(tx, item, why, by, &projects).map_err(db::Error::Abort)
            })
            .map_err(|e| e.to_string())?;
        Ok(actions::Carried::Done)
    }

    /// Lifts a work's pause as the outbox holds it asked by `by`, the owner or its project's architect, as the loop does:
    /// not yet while a work on an item finds no slot free on its machine, the bar the owner's Start meets
    /// ([`Engine::release_started`]). A work that ended or goes on since, that the architect asked for once the pause is
    /// no longer its own, or whose request a later word withdrew, is left as it is.
    pub(crate) fn resume_asked(&mut self, id: i64, by: &str) -> Result<actions::Carried, String> {
        let w = self.work(id)?;
        let architect = by == db::Actor::Architect.name();
        let gone = w.ended.is_some()
            || w.paused.is_none()
            || (architect && !db::stopped_by_architect(w.paused.as_deref(), w.paused_by.as_deref()));
        if gone {
            return Ok(actions::Carried::Done);
        }
        if w.graph == ENGINEER && !self.room_for(&w)? {
            return Ok(actions::Carried::NotYet);
        }
        let actor = if architect { Actor::Architect } else { Actor::Owner };
        let lifted = self
            .db
            .write(actor, |tx| {
                // Read with the lifting, not as the loop read what was due, which an action before may have held up: the
                // owner's Pause or Step since withdrew the request and stands, and the owner's own resume joined it.
                let Some(by) = db::resume_asker(tx, id)? else {
                    return Ok(None);
                };
                if by != db::Actor::Architect.name() {
                    return db::lift_pause(tx, id).map(|told| Some(owner_started(id, told)));
                }
                // No word to the architect, which asked for it itself ([`db::lift_pause`] is the owner's).
                tx.execute("UPDATE works SET paused = NULL, paused_by = NULL WHERE id = ?1", [id])?;
                db::withdraw_resume(tx, id)?;
                Ok(Some(format!("work {id} goes on, as the architect let it")))
            })
            .map_err(|e| e.to_string())?;
        if let Some(said) = lifted {
            self.say(&said);
        }
        Ok(actions::Carried::Done)
    }

    /// The works taken over since the engine's loop last read them, each with the new work that took its item over.
    pub fn replaced(&self) -> Vec<(i64, i64)> {
        std::mem::take(&mut *lock(&self.shared.replaced))
    }

    /// The works that go on by themselves on an older flow version, in the order of their ids: each moves or is taken
    /// over before anything else is started.
    fn outdated(&mut self) -> Result<Vec<Work>, String> {
        let mut out = Vec::new();
        for id in self.active()? {
            let w = self.work(id)?;
            if w.flow_version != self.flow_now {
                out.push(w);
            }
        }
        Ok(out)
    }

    /// Takes a work the owner asked a round of into the owner's hands: a work the engine paused is let go on and at
    /// once paused by the owner, so that its cell counts afresh, as after any time the owner lets a work go on, and it
    /// stays paused after the round. So a resume that still waits for a slot is over: the owner's Step is the later word.
    fn take_in_hand(&mut self, id: i64) -> Result<(), String> {
        self.db
            .write(Actor::Owner, |tx| db::withdraw_resume(tx, id))
            .map_err(|e| e.to_string())?;
        let by: Option<String> = self
            .db
            .conn()
            .query_row("SELECT paused_by FROM works WHERE id = ?1", [id], |r| r.get(0))
            .map_err(|e| format!("work {id}: {e}"))?;
        if by.as_deref() != Some("engine") {
            return Ok(());
        }
        self.db
            .write(Actor::Owner, |tx| {
                tx.execute("UPDATE works SET paused = NULL, paused_by = NULL WHERE id = ?1", [id])?;
                tx.execute(
                    "UPDATE works SET paused = ?2, paused_by = 'owner' WHERE id = ?1",
                    params![id, db::PAUSED_BY_OWNER],
                )?;
                Ok(())
            })
            .map_err(|e| e.to_string())
    }

    /// What a level's rounds run on, by which the session manager chooses their session: the level's model, the
    /// configuration directory its CLI runs agents with, as the configuration names it (the login account the agents'
    /// calls are made under, which holds their sessions; [`Engine::cli_home`] makes it), and how long the CLI's prompt
    /// cache keeps a session's prompt after its last call. The one place the engine works this out, for a round, the
    /// check before it, the keep-warm pings and the question whether a session understood the project.
    pub fn runs_on(&self, level: &config::Level) -> RunsOn {
        let cli = self.config.config.install.agents.clis.get(&level.cli);
        RunsOn {
            model: level.model.clone(),
            home: cli.map(|c| c.home(&self.data, &level.cli)),
            cache: cli.and_then(|c| c.kind.cache_lifetime()),
        }
    }

    /// The configuration directory a CLI runs agents with, made when it is not there yet, and kept out of the data
    /// directory's history even when the owner made it first by logging in there.
    pub fn cli_home(&self, name: &str) -> Option<PathBuf> {
        let cli = self.config.config.install.agents.clis.get(name)?;
        let (dir, failed) = agent::home(&self.data, cli, name);
        if let Some(x) = failed {
            self.say(&x);
        }
        Some(dir)
    }

    /// A CLI's version as it reports it, read the first time it is asked for: when the engine starts.
    pub fn cli_version(&self, name: &str) -> String {
        self.sessions().version(&self.data, &self.config.config, name)
    }

    /// Keeps warm the sessions of the works with no job in flight, as the loop does at each turn: the session manager
    /// finds each session due a ping at `now`, whose work's next round will go on in it 55 minutes after its last call,
    /// and the loop dispatches the pings due to each work's sessions as that work's one job, which sends and records
    /// them on a thread of its own; gives how many pings it dispatched. While the owner's global pause holds, `pause`
    /// says whether it keeps them warm and which works run on despite it.
    pub fn keep_warm(&mut self, now: i64, pause: Option<&GlobalPause>) -> Result<usize, String> {
        let mut sent = 0;
        for (work, pings) in warm::due(self, now, pause)? {
            sent += pings.len();
            self.dispatch_pings(work, pings, now)?;
        }
        Ok(sent)
    }

    /// Keeps a copy of the database once a day while the engine runs ([`db::backup`]): when the newest daily copy is a
    /// day old, or there is none, it copies the database whole, as one snapshot read on a connection of its own while
    /// the works' writes go on, and keeps the newest copies the installation's configuration says (`[backup] keep`);
    /// gives the copy's path when it took one.
    pub fn back_up(&self, now: i64) -> Result<Option<PathBuf>, String> {
        const DAY_MS: i64 = 24 * 60 * 60 * 1000;
        let database = config::database(&self.data);
        let dir = db::backup::dir(&database);
        if db::backup::newest(&dir, db::backup::DAILY).is_some_and(|at| now - at < DAY_MS) {
            return Ok(None);
        }
        let copy = db::backup::copy_file(&database, db::backup::DAILY).map_err(|e| e.to_string())?;
        db::backup::prune(&dir, db::backup::DAILY, self.config.config.install.backup.keep())
            .map_err(|e| e.to_string())?;
        Ok(Some(copy))
    }

    /// Keeps a daily copy off the machine as well, on a thread of its own ([`backup`]): compressed, uploaded to the data
    /// directory's repository on GitHub, the newest kept there as beside the database, the compressed file removed once
    /// uploaded. A data directory with no remote on GitHub keeps its copies on this machine only, as the log says; a
    /// failed upload is said, and the next day's copy goes up all the same.
    pub fn keep_off_machine(&self, copy: PathBuf) {
        let say = Arc::clone(&self.shared.hooks.say);
        let data = self.data.clone();
        let keep = self.config.config.install.backup.keep();
        std::thread::spawn(move || {
            let Some(repo) = backup::repository(&data) else {
                say("the data directory has no repository on GitHub: the database's copies stay on this machine only");
                return;
            };
            let kept = backup::compress(&copy).and_then(|gz| {
                let up = backup::upload(&repo, &gz, keep);
                let _ = std::fs::remove_file(&gz);
                up
            });
            match kept {
                Ok(()) => say(&format!("kept the database's copy off this machine, in {repo}")),
                Err(x) => say(&format!("keeping the database's copy off this machine: {x}")),
            }
        });
    }

    /// Reminds the owner of the asks waiting for the owner's answer when a reminder is due at `now`, the installation's
    /// interval after the oldest was put and after the last reminder ([`reminders::due`]): one notice for all of them,
    /// whatever the works do, then carries out the outbox, which writes it to the notifications log. Gives whether it
    /// reminded.
    pub fn remind(&mut self, now: i64) -> Result<bool, String> {
        let every = self.config.config.install.owner.remind_interval;
        let interval = i64::try_from(every.saturating_mul(1000)).unwrap_or(i64::MAX);
        let Some(text) = reminders::due(self.db.conn(), now, interval)? else {
            return Ok(false);
        };
        self.db
            .write(Actor::Engine, |tx| {
                actions::remind(tx, now, &text).map_err(db::Error::Abort)
            })
            .map_err(|e| e.to_string())?;
        actions::run_outbox(self)?;
        Ok(true)
    }

    /// Whether no new round may start now, because a failed run's cause stops every work; a wait that is over ends.
    /// While no network stops them, the engine looks at the network itself when the wait is over, without running the
    /// command line, which would put the round's message in its session once more, and waits again while it is down;
    /// every [`failed::LOOKS`]th look in a row a round tries anyway, in case what it looks at cannot be reached while
    /// the service can. The look ends or prolongs only the wait it looked at: a wait a step on another thread set
    /// meanwhile holds, and is looked at afresh.
    pub fn halted(&self) -> bool {
        loop {
            let seen = match lock(&self.shared.halt).clone() {
                Some(h) if db::now() < h.until => return true,
                Some(h) => h,
                None => return false,
            };
            // Whether the network is down, when the engine looks at it itself; the look is taken with no lock held.
            let down = match &self.shared.hooks.network {
                Some(reachable) if seen.kind == "offline" => Some(!reachable()),
                _ => None,
            };
            let mut halt = lock(&self.shared.halt);
            if halt.as_ref() != Some(&seen) {
                continue;
            }
            if let Some(down) = down {
                let mut looked = lock(&self.shared.looked);
                if down && *looked + 1 < failed::LOOKS {
                    *looked += 1;
                    if let Some(h) = halt.as_mut() {
                        h.until = db::now() + failed::PROBE;
                    }
                    return true;
                }
                *looked = 0;
            }
            *halt = None;
            drop(halt);
            // A work whose step found the wait standing as its round was to start is idle: every work goes on now.
            self.rouse();
            self.say(&format!("the wait for {} is over: works go on", seen.kind));
            return false;
        }
    }

    /// A round ran to a return: whatever stopped every work has passed.
    fn returned(&self) {
        lock(&self.shared.told).clear();
    }

    /// The compiled flow a work runs on, read from the database the first time any thread asks for it.
    pub fn flow(&self, version: i64) -> Result<Arc<Compiled>, String> {
        if let Some(f) = self.shared.flow(version) {
            return Ok(f);
        }
        let text: String = self
            .db
            .conn()
            .query_row("SELECT compiled FROM flow_versions WHERE id = ?1", [version], |r| {
                r.get(0)
            })
            .map_err(|e| format!("flow version {version}: {e}"))?;
        let c: Compiled = serde_json::from_str(&text).map_err(|e| format!("flow version {version}: {e}"))?;
        let mut flows = self.shared.flows.write().unwrap_or_else(PoisonError::into_inner);
        Ok(Arc::clone(flows.entry(version).or_insert_with(|| Arc::new(c))))
    }

    /// The graph a work runs on, as its project's agents are asked it: without the fields of every return whose need
    /// the project does not meet, such as flow feedback the project's configuration turned off. Shared, so the engine
    /// can be borrowed while it is in use; made once for each flow, graph, project and configuration, since the loop
    /// asks for it for every work many times a pass and a copy of a graph is large.
    pub fn graph_of(&mut self, w: &Work) -> Result<Arc<CompiledGraph>, String> {
        let project = self.project_named(w)?;
        let key = (w.flow_version, w.graph.clone(), project, self.config.commit.clone());
        if let Some(g) = self
            .shared
            .asked
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
        {
            return Ok(Arc::clone(g));
        }
        let g = self
            .flow(w.flow_version)?
            .graph(&w.graph)
            .cloned()
            .ok_or_else(|| format!("work {}: its flow has no graph {}", w.id, w.graph))?;
        let g = Arc::new(self.as_asked(w, g)?);
        let mut asked = self.shared.asked.write().unwrap_or_else(PoisonError::into_inner);
        Ok(Arc::clone(asked.entry(key).or_insert(g)))
    }

    /// The project a work's agents answer to: its own, or its item's; none for a work on neither.
    fn project_named(&self, w: &Work) -> Result<Option<String>, String> {
        match (&w.project, w.item) {
            (Some(p), _) => Ok(Some(p.clone())),
            (None, Some(item)) => self
                .db
                .conn()
                .query_row("SELECT project FROM items WHERE id = ?1", [item], |r| r.get(0))
                .optional()
                .map_err(|e| format!("item {item}: {e}")),
            (None, None) => Ok(None),
        }
    }

    /// A graph as the agents of a work's project are asked it ([`CompiledGraph::as_asked`]).
    fn as_asked(&self, w: &Work, g: CompiledGraph) -> Result<CompiledGraph, String> {
        let project = self.project_named(w)?.and_then(|p| self.config.config.projects.get(&p));
        Ok(g.as_asked(|need| project.is_some_and(|p| p.has(need).unwrap_or(false))))
    }

    /// The project of the configuration that is wf-agents itself, whose repository wf-agents is built from: its
    /// architect takes in the flow feedback of every project that belongs to wf-agents. None when no project is.
    pub fn wf_agents_project(&self) -> Option<String> {
        self.config.config.wf_agents_project()
    }

    pub fn work(&mut self, id: i64) -> Result<Work, String> {
        let w = Work::load(self.db.conn(), id, None)?;
        let g = self.graph_of(&w)?;
        Work::load(self.db.conn(), id, Some(&g))
    }

    pub fn item_of(&self, w: &Work) -> Result<Item, String> {
        let id = w.item.ok_or_else(|| format!("work {} has no item", w.id))?;
        Item::load(self.db.conn(), id)
    }

    /// The item a work is on; none for a work of the architect, which is on no item.
    pub fn subject(&self, w: &Work) -> Result<Option<Item>, String> {
        match w.item {
            Some(id) => Item::load(self.db.conn(), id).map(Some),
            None => Ok(None),
        }
    }

    /// The configuration of the project an item is in; none for no item, or a project the configuration lacks.
    pub fn project_of_item(&self, item: Option<&Item>) -> Option<config::Project> {
        item.and_then(|i| self.config.config.projects.get(&i.project).cloned())
    }

    pub fn project_of(&self, w: &Work) -> Result<config::Project, String> {
        let item = self.item_of(w)?;
        self.config
            .config
            .projects
            .get(&item.project)
            .cloned()
            .ok_or_else(|| format!("item {}: no project {} in the configuration", item.id, item.project))
    }

    /// Reads the committed configuration again, for every thread; a broken one is not used, and the last good one
    /// stays. This context's step goes on with the configuration in use from here, which another thread may have read.
    pub fn reload_config(&mut self) {
        let mut now = self.shared.config();
        match config::committed(&self.data) {
            Ok(c) if c.commit != now.commit => {
                let flow = self.shared.flow(self.flow_now);
                let problems = flow.map(|f| c.config.check(&f)).unwrap_or_default();
                if problems.is_empty() {
                    self.say(&format!("configuration {} in use", c.commit));
                    now = Arc::new(c);
                    *self.shared.config.write().unwrap_or_else(PoisonError::into_inner) = Arc::clone(&now);
                } else {
                    self.say(&format!("configuration {} not used: {}", c.commit, problems.join("; ")));
                }
            }
            Ok(_) => {}
            Err(e) => self.say(&format!("configuration not read, the last good one stays: {e}")),
        }
        self.config = now;
    }

    /// Whether the work is at its graph's end and the end has run.
    pub fn done(&mut self, w: &Work) -> Result<bool, String> {
        let g = self.graph_of(w)?;
        if w.pos.at != g.end {
            return Ok(false);
        }
        let rs = work::rounds_at(self.db.conn(), w.id, &g.end)?;
        Ok(rs.iter().any(|r| r.ended_at.is_some() && r.rejected.is_none()))
    }

    /// The works still to take further: not ended, not paused, not done.
    pub fn active(&mut self) -> Result<Vec<i64>, String> {
        let ids: Vec<i64> = {
            let mut st = self
                .db
                .conn()
                .prepare("SELECT id FROM works WHERE ended IS NULL AND paused IS NULL ORDER BY id")
                .map_err(|e| e.to_string())?;
            st.query_map([], |r| r.get(0))
                .map_err(|e| e.to_string())?
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?
        };
        let mut out = Vec::new();
        for id in ids {
            let w = self.work(id)?;
            if !self.done(&w)? {
                out.push(id);
            }
        }
        Ok(out)
    }

    /// Takes a work one step further, as a job the loop dispatched ([`schedule`]): a paused work only when the owner
    /// asked for its round. A work asked to stop, or on an older flow version, takes no step: the loop stops it or
    /// brings it up once this job has ended, since it does what the engine does to a work from outside its step only
    /// while the work has no job in flight.
    fn take_step(&mut self, id: i64, asked: bool) -> Result<Advance, String> {
        let w = self.work(id)?;
        if w.ended.is_some() || (w.paused.is_some() && !asked) || self.done(&w)? {
            return Ok(Advance::Stopped);
        }
        if w.flow_version != self.flow_now || db::stop_asked(self.db.conn(), id).map_err(|e| e.to_string())? {
            return Ok(Advance::Stopped);
        }
        let g = self.graph_of(&w)?;
        let cell = g
            .cell(&w.pos.at)
            .cloned()
            .ok_or_else(|| format!("work {id} is at no such cell: {}", w.pos.at))?;
        // A work on an item that waits holds no slot: it goes on from its wait, once what it waits for came, only
        // when the loop granted its step a free slot, unless the owner asked for its round.
        let slotted = !asked && w.graph == ENGINEER;
        let result = if cell.kind == "agent" {
            steps::agent(self, &w, &g, &cell)
        } else {
            steps::engine(self, &w, &g, &cell, slotted)
        };
        match result {
            Ok(a) => Ok(a),
            Err(e) => {
                let why = format!("cell {}: {e}", cell.id);
                // A step that failed while the network is down failed for want of it: every work waits for it.
                if self.network_down() {
                    return self.wait_offline(id, &why);
                }
                // A step that cannot be taken is the engine's or the configuration's fault, not the work's: the work
                // is paused with the reason, for the owner to see.
                self.say(&format!("work {id} paused: {why}"));
                self.pause(id, &why)?;
                Ok(Advance::Stopped)
            }
        }
    }

    /// Why a work ended, as it stands now; none while it has not.
    fn ended(&self, id: i64) -> Result<Option<String>, String> {
        self.db
            .conn()
            .query_row("SELECT ended FROM works WHERE id = ?1", [id], |r| r.get(0))
            .map_err(|e| format!("work {id}: {e}"))
    }

    /// Whether the engine's own look at the network finds it down; with no look ([`Hooks::network`]), it is taken as
    /// up.
    pub(crate) fn network_down(&self) -> bool {
        self.shared.hooks.network.as_ref().is_some_and(|reachable| !reachable())
    }

    /// Stops every work while the network is down, for a step of work `id` that failed for want of it, as a run of the
    /// command line that cannot reach the network does (`offline`): the round the step left open is closed as such a
    /// failed run, so that its cell runs again once the network is back, and the owner is told once; nothing goes to
    /// the architect, and the work is not paused.
    pub(crate) fn wait_offline(&mut self, id: i64, why: &str) -> Result<Advance, String> {
        let open = work::open_round(self.db.conn(), id)?;
        self.db
            .write(Actor::Engine, |tx| {
                if let Some(r) = &open {
                    let ended = work::Ended {
                        rejected: Some(serde_json::json!({ "failed": { "kind": "offline", "what": why } })),
                        transition: None,
                        ..work::Ended::default()
                    };
                    work::finish_round(tx, r.id, &ended).map_err(db::Error::Abort)?;
                }
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        let first = self.halt_every_work(Halt {
            kind: "offline".into(),
            until: db::now() + failed::PROBE,
            why: why.to_string(),
        });
        if first {
            let text = format!(
                "No step starts for any work while the network cannot be reached ({why}): the engine looks at it every half minute, and each work goes on from its cell once it is back."
            );
            self.db
                .write(Actor::Engine, |tx| {
                    actions::notify(tx, Some(id), &text).map_err(db::Error::Abort)
                })
                .map_err(|e| e.to_string())?;
        }
        self.say(&format!("work {id} waits for the network: {why}"));
        Ok(Advance::Waits(format!("the network: {why}")))
    }

    /// Pauses a work whose step cannot be taken: the round the step left open is closed with the reason, so it shows
    /// as what it was and not as running, and the owner is told once. A question waiting for its answer stays open.
    pub fn pause(&mut self, id: i64, why: &str) -> Result<(), String> {
        let w = self.work(id)?;
        let g = self.graph_of(&w)?;
        let asks = |r: &Round| {
            r.cell
                .as_deref()
                .and_then(|c| g.cell(c))
                .is_some_and(|c| matches!(c.step.as_deref(), Some("ask_owner" | "ask_architect")))
        };
        let text = match self.subject(&w) {
            Ok(Some(item)) => format!("Work {id} on item {} ({}) is paused: {why}", item.id, item.title),
            Ok(None) => format!("Work {id} ({}) is paused: {why}", w.of_architect()),
            Err(_) => format!("Work {id} is paused: {why}"),
        };
        self.db
            .write(Actor::Engine, |tx| {
                let open = work::open_round(tx, id).map_err(db::Error::Abort)?.filter(|r| !asks(r));
                if let Some(r) = &open {
                    let ended = work::Ended {
                        rejected: Some(serde_json::json!({ "error": why })),
                        ..work::Ended::default()
                    };
                    work::finish_round(tx, r.id, &ended).map_err(db::Error::Abort)?;
                }
                tx.execute(
                    "UPDATE works SET paused = ?2, paused_by = 'engine' WHERE id = ?1",
                    params![id, why],
                )?;
                db::withdraw_resume(tx, id)?;
                actions::notify(tx, Some(id), &text).map_err(db::Error::Abort)?;
                Ok(())
            })
            .map_err(|e| e.to_string())
    }

    /// Starts a work on the item: at the start cell of the engineer's graph, on the flow built into the engine. The
    /// item leaves the queue: it is no longer waiting to start.
    pub fn create_work(&mut self, item: i64) -> Result<i64, String> {
        self.start_work(item, None)
    }

    /// Starts a work on the item, paused by the owner for the reason given, if one is. The work starts where the cell
    /// an item waits at to start (`S0`) leads once nothing holds it back: no work stands at that cell, since no work
    /// exists while the item waits there.
    fn start_work(&mut self, item: i64, paused: Option<&str>) -> Result<i64, String> {
        let v = self.flow_now;
        let flow = self.flow(v)?;
        let g = flow.graph(ENGINEER).ok_or("the flow has no engineer graph")?;
        let mut ready = serde_json::Map::new();
        ready.insert("start".into(), Value::String("ready".into()));
        let start = match g.cell(&g.start) {
            Some(c) if c.inenums.contains_key("start") => {
                let row = g.take(&g.start, &serde_json::json!({}), &ready)?;
                g.step(&g.start_position(), row)?.at
            }
            _ => g.start.clone(),
        };
        self.db
            .write(Actor::Engine, |tx| {
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at, paused, paused_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, CASE WHEN ?5 IS NULL THEN NULL ELSE 'owner' END)",
                    params![item, v, ENGINEER, start, paused],
                )?;
                let id = tx.last_insert_rowid();
                tx.execute("DELETE FROM dispatch_queue WHERE item = ?1", [item])?;
                Ok(id)
            })
            .map_err(|e| e.to_string())
    }

    /// How many more works can hold a slot: the machines' slots less the works holding one, as the session manager
    /// reports them. A work holds no slot while it waits for the owner or for other works, unless the loop granted its
    /// step one.
    pub fn free_slots(&mut self) -> Result<i64, String> {
        Ok(self.slots_by_machine()?.values().sum())
    }

    /// Each machine's free slots, as the session manager reports them from the works holding one: a work of the
    /// architect holds none, and a work holds none while it waits, unless the loop granted its step one, which it holds
    /// until the loop reaps that step. A work paused or asked to stop whose step runs on holds its slot until the loop
    /// reaps that step too, since a pause takes effect when the round under way ends.
    pub fn slots_by_machine(&mut self) -> Result<BTreeMap<String, i64>, String> {
        let mut works: BTreeSet<i64> = self.active()?.into_iter().collect();
        works.extend(self.stepping());
        let mut holding = Vec::new();
        for id in works {
            let w = self.work(id)?;
            if w.graph == ENGINEER && (!self.waiting(&w)? || self.granted(id)) {
                holding.push(w.machine.clone());
            }
        }
        Ok(sessions::worktree::free_slots(
            &self.config.config.install.machines.hosts,
            &holding,
        ))
    }

    /// The works whose wait is over and that wait on, holding no slot, until their machine has a slot free, as the last
    /// step of each found and as each stands now: one since paused or ended, or whose machine has a slot free again,
    /// which the loop grants its next step, is not among them. With them, the paused works whose pause is asked to be
    /// lifted while their machine has none free. The owner's page shows them so, since a work whose answer came and
    /// that does not move, or whose resume was asked and that stays paused, would otherwise look stuck. It only reads:
    /// which works wait on for a slot changes only as the loop reaps their steps and grants them slots.
    pub fn waiting_for_slot(&mut self) -> Result<Vec<i64>, String> {
        let mut out = BTreeSet::new();
        let held = lock(&self.shared.held_for_slot).clone();
        for id in held {
            let w = self.work(id)?;
            if w.ended.is_none() && w.paused.is_none() && self.waiting(&w)? && !self.room_for(&w)? {
                out.insert(id);
            }
        }
        for id in db::resume_asked(self.db.conn()).map_err(|e| e.to_string())? {
            let w = self.work(id)?;
            if w.graph == ENGINEER && w.ended.is_none() && w.paused.is_some() && !self.room_for(&w)? {
                out.insert(id);
            }
        }
        Ok(out.into_iter().collect())
    }

    /// Whether a work that holds no slot may take one: its machine has a slot free, or, before it has a machine, any
    /// machine has.
    fn room_for(&mut self, w: &Work) -> Result<bool, String> {
        let free = self.slots_by_machine()?;
        Ok(match &w.machine {
            Some(m) => free.get(m).copied().unwrap_or(0) > 0,
            None => free.values().sum::<i64>() > 0,
        })
    }

    /// Whether the work stands waiting: at the owner's or the architect's cell with its question asked, or at a row
    /// that waits. It reads the cell where the work stands and its last round only, since every step asks it of every
    /// work for the slots.
    pub fn waiting(&mut self, w: &Work) -> Result<bool, String> {
        let waits_by: Vec<String> = {
            let flow = self.flow(w.flow_version)?;
            let g = flow
                .graph(&w.graph)
                .ok_or_else(|| format!("work {}: its flow has no graph {}", w.id, w.graph))?;
            let Some(cell) = g.cell(&w.pos.at) else {
                return Ok(false);
            };
            if matches!(cell.step.as_deref(), Some("ask_owner" | "ask_architect")) {
                return Ok(true);
            }
            cell.rows.iter().filter(|r| r.waits).map(|r| r.name.clone()).collect()
        };
        if waits_by.is_empty() {
            return Ok(false);
        }
        let last: Option<(Option<String>, Option<String>)> = self
            .db
            .conn()
            .query_row(
                "SELECT cell, transition FROM rounds WHERE work = ?1 ORDER BY n DESC LIMIT 1",
                [w.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        Ok(last.is_some_and(|(cell, transition)| {
            cell.as_deref() == Some(w.pos.at.as_str()) && transition.is_some_and(|t| waits_by.contains(&t))
        }))
    }

    /// What holds an item back from starting, by the name of the row of the cell an item waits at to start (`S0`)
    /// that says it, `ready` when nothing does: held by the owner or the architect; an item it depends on by a blocking
    /// dependency not merged; its project's new starts held; a change the architect's open batch collected naming it
    /// (`changing`); no slot free (`free`).
    pub(crate) fn waits_to_start(
        &self,
        item: i64,
        project: &str,
        changing: &BTreeSet<i64>,
        free: i64,
    ) -> Result<&'static str, String> {
        let conn = self.db.conn();
        let one = |e: db::rusqlite::Error| e.to_string();
        let held: i64 = conn
            .query_row(
                "SELECT count(*) FROM items WHERE id = ?1 AND held IS NOT NULL",
                [item],
                |r| r.get(0),
            )
            .map_err(one)?;
        let blocked: i64 = conn
            .query_row("SELECT count(*) FROM item_blocked_by WHERE item = ?1", [item], |r| {
                r.get(0)
            })
            .map_err(one)?;
        let project_held: i64 = conn
            .query_row(
                "SELECT count(*) FROM dispatch_holds WHERE project = ?1",
                [project],
                |r| r.get(0),
            )
            .map_err(one)?;
        Ok(if held > 0 {
            "held"
        } else if blocked > 0 {
            "blocked"
        } else if project_held > 0 {
            "project_held"
        } else if changing.contains(&item) {
            "in_batch"
        } else if free <= 0 {
            "no_slot"
        } else {
            "ready"
        })
    }

    /// The next item to start a work on, of the items waiting to start that cell `S0`'s rows let start (those
    /// [`Engine::waits_to_start`] finds nothing holding back): the first the owner queued, or else the first of the
    /// next project in turn after the project of the work started last, so that projects with items ready take the
    /// slots in turn, each in its architect's order. An item held or blocked is not among those waiting (the view
    /// `ready_items`).
    fn next_ready(&mut self) -> Option<i64> {
        let changing = architect::held_by_batches(self.db.conn()).ok()?;
        let free = self.free_slots().ok()?;
        let waiting: Vec<(i64, String, Option<f64>)> = {
            let mut st = self
                .db
                .conn()
                .prepare("SELECT item, project, position FROM ready_items")
                .ok()?;
            let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).ok()?;
            rows.filter_map(Result::ok).collect()
        };
        let mut why = Vec::new();
        for (item, project, _) in &waiting {
            why.push(self.waits_to_start(*item, project, &changing, free).ok()?);
        }
        // Each item takes the row of the cell it waits at; one whose row leads on may start.
        let flow = self.flow(self.flow_now).ok()?;
        let g = flow.graph(ENGINEER)?;
        let leads_on = |reason: &str| {
            let mut given = serde_json::Map::new();
            given.insert("start".into(), Value::String(reason.to_string()));
            match g.cell(&g.start) {
                Some(c) if c.inenums.contains_key("start") => g
                    .take(&g.start, &serde_json::json!({}), &given)
                    .is_ok_and(|r| r.to != g.start),
                _ => reason == "ready",
            }
        };
        let ready: Vec<(i64, String, Option<f64>)> = waiting
            .into_iter()
            .zip(why)
            .filter(|(_, reason)| leads_on(reason))
            .map(|(x, _)| x)
            .collect();
        if let Some((item, _, _)) = ready.iter().find(|r| r.2.is_some()) {
            return Some(*item);
        }
        let last: Option<String> = self
            .db
            .conn()
            .query_row(
                "SELECT i.project FROM works w JOIN items i ON i.id = w.item WHERE w.graph = 'engineer'
                 ORDER BY w.id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .ok();
        let projects: BTreeSet<&str> = ready.iter().map(|r| r.1.as_str()).collect();
        let next = last
            .as_deref()
            .and_then(|l| projects.iter().find(|p| **p > l))
            .or_else(|| projects.iter().next())?;
        ready.iter().find(|r| r.1 == *next).map(|r| r.0)
    }

    /// The projects of the configuration, each of which has its architect.
    fn projects(&self) -> Vec<String> {
        self.config.config.projects.keys().cloned().collect()
    }

    /// A work the owner took a round of is paused by the owner after it, unless it ended, is done or is paused already.
    fn paused_after_step(&mut self, id: i64) -> Result<(), String> {
        let w = self.work(id)?;
        if w.ended.is_some() || w.paused.is_some() || self.done(&w)? {
            return Ok(());
        }
        self.db
            .write(Actor::Owner, |tx| {
                tx.execute(
                    "UPDATE works SET paused = ?2, paused_by = 'owner' WHERE id = ?1 AND paused IS NULL",
                    params![id, db::PAUSED_BY_OWNER],
                )?;
                Ok(())
            })
            .map_err(|e| e.to_string())
    }

    /// Lifts, as the owner, the pause of each work the owner started (`releasing`) that is still paused, once it may
    /// hold a slot: a work on an item holds none while it is paused, so its pause is lifted only while its machine has
    /// a slot free; and of the architect work that runs of each project whose architect the owner started
    /// (`architects`), which holds none. A work the architect stopped is the owner's release of it, which the architect
    /// is told. Gives the works the owner's Start is done with, lifted or found not paused, and the projects whose
    /// architect's it is done with.
    pub fn release_started(
        &mut self,
        releasing: &BTreeSet<i64>,
        architects: &BTreeSet<String>,
    ) -> Result<(Vec<i64>, Vec<String>), String> {
        let mut done = Vec::new();
        for &id in releasing {
            let w = self.work(id)?;
            if w.paused.is_some() && w.ended.is_none() && w.graph == ENGINEER && !self.room_for(&w)? {
                continue;
            }
            if w.paused.is_some() && w.ended.is_none() {
                self.lift(id)?;
            }
            done.push(id);
        }
        let mut projects = Vec::new();
        for project in architects {
            if let Some(id) = architect::running(self, project)?
                && self.work(id)?.paused.is_some()
            {
                self.lift(id)?;
            }
            projects.push(project.clone());
        }
        Ok((done, projects))
    }

    /// Lifts a work's pause as the owner's Start does, saying so.
    fn lift(&mut self, id: i64) -> Result<(), String> {
        let told = self
            .db
            .write(Actor::Owner, |tx| db::lift_pause(tx, id))
            .map_err(|e| e.to_string())?;
        self.say(&owner_started(id, told));
        Ok(())
    }

    /// The rounds this context's connection started or changed since `mark`, in the order they started, with the cell
    /// of the last: those of this step, its rounds and those it cut off or ended of other works, and none another
    /// thread or the owner's page wrote meanwhile ([`Db::rounds_since`]).
    fn rounds_since(&self, mark: &db::RoundsMark) -> Result<(Vec<RoundId>, Option<String>), String> {
        let rows = self.db.rounds_since(*mark).map_err(|e| e.to_string())?;
        let cell = rows.last().and_then(|(_, _, c)| c.clone());
        Ok((rows.into_iter().map(|(w, n, _)| RoundId::new(w, n)).collect(), cell))
    }
}

/// What the engine says of a work whose pause the owner's Start or resume lifted, `told` whether the architect, which had
/// stopped it, is told.
fn owner_started(id: i64, told: bool) -> String {
    let told = if told {
        "; the architect, which had stopped it, is told"
    } else {
        ""
    };
    format!("work {id} goes on, as the owner started it{told}")
}

/// What one step came to: the work it took a step of, none when no work had a step to take; the item the loop started
/// that work on, when it started it for the step; the rounds the step's connection started or ended, and the cell of the
/// last of them; and the owner's round asked that it was, when it was one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stepped {
    pub work: Option<i64>,
    pub started_item: Option<i64>,
    pub rounds: Vec<RoundId>,
    pub cell: Option<String>,
    pub asked: Option<Ask>,
}

/// The flow built into this engine, compiled against the database it makes, when `config` fits it; otherwise every
/// problem the configuration has with the flow, one to a line.
pub fn compile_for(config: &config::Config) -> Result<Compiled, String> {
    let relations = db::relations(db::SCHEMA).map_err(|e| format!("the database definition: {e}"))?;
    let tables = db::index_of(db::SCHEMA).map_err(|e| format!("the database definition: {e}"))?;
    let compiled = flow::compile(flow::FLOW, &flow::Context { relations, tables }).map_err(|e| e.to_string())?;
    let problems = config.check(&compiled);
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    Ok(compiled)
}

/// The line whose permissions an agent's cell runs with: its own, or for a cell on no line, the line of its graph's
/// agents: the architect's in the architect's graph, which only reads, and the engineer's otherwise.
pub fn line_of(g: &CompiledGraph, cell: &CompiledCell) -> String {
    match &cell.line {
        Some(l) => l.clone(),
        None if g.name == ARCHITECT => ARCHITECT.to_string(),
        None => ENGINEER.to_string(),
    }
}

/// Whether an agent's cell takes the interjections said to its line: one of its inputs reads them. A cell that reads
/// none, as a new architect reading a policy blind, is handed none while it runs.
pub fn takes_interjections(cell: &CompiledCell) -> bool {
    cell.inputs.iter().any(|x| {
        x.from
            .iter()
            .any(|s| matches!(s, flow::format::Src::Db { relation, .. } if relation == "waiting_interjections"))
    })
}

/// The rows of a cell whose name is given.
pub fn row<'g>(cell: &'g CompiledCell, name: &str) -> Option<&'g flow::CompiledRow> {
    cell.rows.iter().find(|r| r.name == name)
}

/// What a return says about itself: the form it holds, by the field it sits in; none for the cell's own answer.
pub fn form_of(g: &CompiledGraph, cell: &CompiledCell, output: &Value) -> Option<String> {
    let inner = output.get(&g.within)?;
    cell.forms
        .iter()
        .filter_map(|f| f.field.as_ref())
        .find(|f| inner.get(f.as_str()).is_some())
        .cloned()
}
