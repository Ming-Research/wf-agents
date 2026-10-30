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
//! In this version the engine and the session manager are one process on one machine, and the engine takes one work
//! one step at a time: a long round of one work holds up the others.

pub mod blockers;
pub mod database;
pub mod ending;
pub mod git;
pub mod hub;
pub mod limits;

mod actions;
mod arch_changes;
mod architect;
mod checks;
mod failed;
mod faults;
mod inputs;
pub mod message;
mod reminders;
mod steps;
mod tools;
mod unparsed;
mod values;
mod versions;
mod warm;
mod work;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use db::rusqlite::{OptionalExtension, params};
use db::{Actor, Db};
use flow::{Compiled, CompiledCell, CompiledGraph};
use serde_json::Value;

/// The command line agents run in, which the session manager starts.
pub use sessions::cli as agent;

pub use checks::DESIGN_CARD_RULING;
pub use failed::{Halt, utc};
pub use steps::{cell_request, next_message};
pub use warm::GlobalPause;
pub use work::{Item, Round, RoundId, Work, json_col, query_json};

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
    /// The work is done, was ended or taken over, or is paused.
    Stopped,
}

/// Where the engine tells what the agent of a running round is doing: the round's work, its number in the work, and
/// what it does.
pub type Activity = dyn Fn(i64, i64, &str);

pub struct Engine {
    pub db: Db,
    /// The data directory: the configuration, the database and the worktrees.
    pub data: PathBuf,
    pub config: config::Committed,
    /// The session manager: the agents' sessions and the command line they run in.
    pub sessions: sessions::Manager,
    pub github: Box<dyn GitHub>,
    flows: BTreeMap<i64, Compiled>,
    /// The flow version new works start on: the one built into this engine.
    pub flow_now: i64,
    /// What the agent of a running round is doing, by its work and round, as its command line reports it.
    pub activity: Box<Activity>,
    /// Where the engine says which work it is taking a step of, and none once that step has ended: the owner's page
    /// shows that work's round as running, and what stops once it ends.
    pub busy: Box<dyn Fn(Option<i64>)>,
    /// Where the engine says what it does and what went wrong.
    pub say: Box<dyn Fn(&str)>,
    /// While a failed run's cause stops every work alike (a usage limit, no network), why and until when: no new
    /// round starts until then.
    pub halt: Option<Halt>,
    /// The owner's global pause while it holds, as the loop that runs the engine hands it each turn: no work takes a
    /// step but those the owner started, and no new work starts; none while works go on. The architects that judge
    /// what runs and what starts are told it with the slots.
    pub global_pause: Option<GlobalPause>,
    /// The kind of failed run the owner was last told stops every work, until a round runs to a return again: the
    /// owner is told once.
    told: Option<String>,
    /// How long a run that failed in passing waits before it runs again the first time; twice as long each time
    /// after, up to five minutes.
    pub retry_wait: Duration,
    /// The works taken over, each with the new work that took its item over, until the engine's loop has read them
    /// ([`Engine::replaced`]): the owner's Start of a work goes on in the work that replaced it.
    replaced: Vec<(i64, i64)>,
}

impl Engine {
    /// An engine on a data directory, with its committed configuration; the flow built into it is stored as a flow
    /// version if it is new.
    pub fn new(
        data: &Path,
        config: config::Committed,
        agents: Box<dyn Agents>,
        github: Box<dyn GitHub>,
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
        let mut flows = BTreeMap::new();
        flows.insert(flow_now, compiled);
        let mut e = Engine {
            db,
            data: data.to_path_buf(),
            config,
            sessions: sessions::Manager::new(agents),
            github,
            flows,
            flow_now,
            say: Box::new(|s| eprintln!("{s}")),
            activity: Box::new(|_, _, _| {}),
            busy: Box::new(|_| {}),
            halt: None,
            global_pause: None,
            told: None,
            retry_wait: Duration::from_secs(10),
            replaced: Vec::new(),
        };
        let names: Vec<String> = e.config.config.install.agents.clis.keys().cloned().collect();
        for name in names {
            e.cli_version(&name);
        }
        Ok(e)
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
            flows.insert(flow_now, compiled);
        }
        Ok(Engine {
            db,
            data: data.to_path_buf(),
            config,
            sessions: sessions::Manager::new(agents),
            github,
            flows,
            flow_now,
            say: Box::new(|s| eprintln!("{s}")),
            activity: Box::new(|_, _, _| {}),
            busy: Box::new(|_| {}),
            halt: None,
            global_pause: None,
            told: None,
            retry_wait: Duration::from_secs(10),
            replaced: Vec::new(),
        })
    }

    /// Brings works on an older flow version to the version the engine runs, which the engine does when each next goes
    /// on, before its step: a work that can move moves ([`Engine::move_up`]), and the rest are taken over
    /// ([`Engine::take_over`]).
    fn bring_up(&mut self, works: &[Work]) -> Result<Brought, String> {
        let mut over = Vec::new();
        for w in works {
            match self.cannot_move(w)? {
                None => self.move_up(w)?,
                Some(cannot) => over.push((w.clone(), self.moved_on(&cannot))),
            }
        }
        Ok(Brought {
            over: over.iter().map(|(w, _)| w.id).collect(),
            started: self.take_over(&over)?,
        })
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
        (self.say)(&format!(
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
    /// cut off, its design cards are replaced, its asks of the owner need no answer, its questions to the architect are
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
            (self.say)(&format!("work {id} stopped: {why}"));
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
                    (self.say)(&format!("work {id}: its worktree {} stays: {x}", dir.display()));
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
                (self.say)(&format!("work {new} started on item {item}, after work {id}"));
                self.replaced.push((id, new));
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
            (self.say)(&format!(
                "item {item} waits for a free slot at the head of the queue, to take over from work {id}"
            ));
        }
        Ok(started)
    }

    /// Starts a work over, as the owner or its project's architect asked ([`db::start_over`]), by taking it over as a
    /// work on an older flow version is taken over ([`Engine::take_over`]), saying who started it over and, for the
    /// architect, why. A work that can no longer be started over, as one that ended since it was asked, is left as it is.
    fn start_over(&mut self, id: i64, by: &str, why: Option<&str>) -> Result<(), String> {
        let w = self.work(id)?;
        if w.ended.is_some() || w.graph != ENGINEER || self.done(&w)? {
            (self.say)(&format!("work {id} is not started over: it is over"));
            return Ok(());
        }
        let who = if by == db::Actor::Architect.name() {
            "the architect"
        } else {
            "the owner"
        };
        let why = match why.map(|w| w.trim().trim_end_matches(['.', '\u{3002}'])) {
            Some(why) => format!("{who} started it over, and a new work on it takes the item over: {why}"),
            None => format!("{who} started it over, and a new work on it takes the item over"),
        };
        self.take_over(&[(w, why)])?;
        Ok(())
    }

    /// Carries out the starts over asked, whatever the global state, as the engine's loop does at each turn: the owner
    /// asks one on the page while every work may be paused, and nothing else of the outbox is carried out then. Gives
    /// whether any was.
    pub fn start_overs(&mut self) -> Result<bool, String> {
        actions::run_start_overs(self)
    }

    /// The works taken over since the engine's loop last read them, each with the new work that took its item over.
    pub fn replaced(&mut self) -> Vec<(i64, i64)> {
        std::mem::take(&mut self.replaced)
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
    /// stays paused after the round.
    fn take_in_hand(&mut self, id: i64) -> Result<(), String> {
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

    /// The configuration directory a CLI runs agents with, as the configuration names it: the login account the
    /// agents' calls are made under, which holds their sessions. [`Engine::cli_home`] makes it.
    pub fn cli_dir(&self, name: &str) -> Option<PathBuf> {
        let cli = self.config.config.install.agents.clis.get(name)?;
        Some(cli.home(&self.data, name))
    }

    /// The configuration directory a CLI runs agents with, made when it is not there yet, and kept out of the data
    /// directory's history even when the owner made it first by logging in there.
    pub fn cli_home(&self, name: &str) -> Option<PathBuf> {
        let cli = self.config.config.install.agents.clis.get(name)?;
        let (dir, failed) = agent::home(&self.data, cli, name);
        if let Some(x) = failed {
            (self.say)(&x);
        }
        Some(dir)
    }

    /// A CLI's version as it reports it, read the first time it is asked for: when the engine starts.
    pub fn cli_version(&mut self, name: &str) -> String {
        self.sessions.version(&self.data, &self.config.config, name)
    }

    /// Keeps warm the sessions of the works that wait: the session manager sends a ping to each session whose work's
    /// next round will go on in it, 55 minutes after its last call, and records it; gives how many it sent at `now`.
    /// While the owner's global pause holds, `pause` says whether it keeps them warm and which works run on despite it.
    pub fn keep_warm(&mut self, now: i64, pause: Option<&GlobalPause>) -> Result<usize, String> {
        warm::keep_warm(self, now, pause)
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
    pub fn halted(&mut self) -> bool {
        match &self.halt {
            Some(h) if db::now() < h.until => true,
            Some(h) => {
                (self.say)(&format!("the wait for {} is over: works go on", h.kind));
                self.halt = None;
                false
            }
            None => false,
        }
    }

    /// A round ran to a return: whatever stopped every work has passed.
    fn returned(&mut self) {
        self.told = None;
    }

    /// The compiled flow a work runs on, read from the database the first time.
    pub fn flow(&mut self, version: i64) -> Result<&Compiled, String> {
        if !self.flows.contains_key(&version) {
            let text: String = self
                .db
                .conn()
                .query_row("SELECT compiled FROM flow_versions WHERE id = ?1", [version], |r| {
                    r.get(0)
                })
                .map_err(|e| format!("flow version {version}: {e}"))?;
            let c: Compiled = serde_json::from_str(&text).map_err(|e| format!("flow version {version}: {e}"))?;
            self.flows.insert(version, c);
        }
        Ok(&self.flows[&version])
    }

    /// The graph a work runs on, as its project's agents are asked it: without the fields of every return whose need
    /// the project does not meet, such as flow feedback the project's configuration turned off. An owned copy, so the
    /// engine can be borrowed while it is in use.
    pub fn graph_of(&mut self, w: &Work) -> Result<CompiledGraph, String> {
        let g = self
            .flow(w.flow_version)?
            .graph(&w.graph)
            .cloned()
            .ok_or_else(|| format!("work {}: its flow has no graph {}", w.id, w.graph))?;
        self.as_asked(w, g)
    }

    /// A graph as the agents of a work's project are asked it ([`CompiledGraph::as_asked`]).
    fn as_asked(&self, w: &Work, g: CompiledGraph) -> Result<CompiledGraph, String> {
        let named: Option<String> = match (&w.project, w.item) {
            (Some(p), _) => Some(p.clone()),
            (None, Some(item)) => self
                .db
                .conn()
                .query_row("SELECT project FROM items WHERE id = ?1", [item], |r| r.get(0))
                .optional()
                .map_err(|e| format!("item {item}: {e}"))?,
            (None, None) => None,
        };
        let project = named.and_then(|p| self.config.config.projects.get(&p));
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

    /// Reads the committed configuration again; a broken one is not used, and the last good one stays.
    pub fn reload_config(&mut self) {
        match config::committed(&self.data) {
            Ok(c) if c.commit != self.config.commit => {
                let flow = self.flows.get(&self.flow_now).cloned();
                let problems = flow.map(|f| c.config.check(&f)).unwrap_or_default();
                if problems.is_empty() {
                    (self.say)(&format!("configuration {} in use", c.commit));
                    self.config = c;
                } else {
                    (self.say)(&format!("configuration {} not used: {}", c.commit, problems.join("; ")));
                }
            }
            Ok(_) => {}
            Err(e) => (self.say)(&format!("configuration not read, the last good one stays: {e}")),
        }
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

    /// Takes a work one step further; a work on an older flow version moves first, or is taken over instead.
    pub fn advance(&mut self, id: i64) -> Result<Advance, String> {
        self.take_step(id, false)
    }

    /// Takes a work one step further: a paused work only when the owner asked for its round.
    fn take_step(&mut self, id: i64, asked: bool) -> Result<Advance, String> {
        let mut w = self.work(id)?;
        if w.ended.is_some() || (w.paused.is_some() && !asked) || self.done(&w)? {
            return Ok(Advance::Stopped);
        }
        // A work asked to be started over is started over instead of going on.
        if db::start_over_asked(self.db.conn(), id).map_err(|e| e.to_string())? {
            actions::run_start_overs(self)?;
            return Ok(Advance::Stopped);
        }
        if w.flow_version != self.flow_now {
            match self.cannot_move(&w)? {
                Some(cannot) => {
                    let why = self.moved_on(&cannot);
                    self.take_over(&[(w, why)])?;
                    return Ok(Advance::Stopped);
                }
                None => {
                    self.move_up(&w)?;
                    w = self.work(id)?;
                }
            }
        }
        let g = self.graph_of(&w)?;
        let cell = g
            .cell(&w.pos.at)
            .cloned()
            .ok_or_else(|| format!("work {id} is at no such cell: {}", w.pos.at))?;
        // A work that waits, which it does at no agent's cell, holds no slot: it goes on from its wait, once what it
        // waits for came, only while its machine has a slot free, unless the owner asked for its round.
        let room = cell.kind == "agent" || asked || w.graph != ENGINEER || !self.waiting(&w)? || self.room_for(&w)?;
        (self.busy)(Some(id));
        let result = if cell.kind == "agent" {
            steps::agent(self, &w, &g, &cell)
        } else {
            steps::engine(self, &w, &g, &cell, room)
        };
        (self.busy)(None);
        match result {
            Ok(a) => Ok(a),
            Err(e) => {
                // A step that cannot be taken is the engine's or the configuration's fault, not the work's: the work
                // is paused with the reason, for the owner to see.
                let why = format!("cell {}: {e}", cell.id);
                (self.say)(&format!("work {id} paused: {why}"));
                self.pause(id, &why)?;
                Ok(Advance::Stopped)
            }
        }
    }

    /// Pauses a work whose step cannot be taken: the round the step left open is closed with the reason, so it shows
    /// as what it was and not as running, and the owner is told once. A question waiting for its answer stays open.
    pub fn pause(&mut self, id: i64, why: &str) -> Result<(), String> {
        let w = self.work(id)?;
        let g = self.graph_of(&w)?;
        let open = work::open_round(self.db.conn(), id)?.filter(|r| {
            !r.cell
                .as_deref()
                .and_then(|c| g.cell(c))
                .is_some_and(|c| matches!(c.step.as_deref(), Some("ask_owner" | "ask_architect")))
        });
        let text = match self.subject(&w) {
            Ok(Some(item)) => format!("Work {id} on item {} ({}) is paused: {why}", item.id, item.title),
            Ok(None) => format!("Work {id} ({}) is paused: {why}", w.of_architect()),
            Err(_) => format!("Work {id} is paused: {why}"),
        };
        self.db
            .write(Actor::Engine, |tx| {
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
        let g = self.flow(v)?.graph(ENGINEER).ok_or("the flow has no engineer graph")?;
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
    /// reports them. A work holds no slot while it waits for the owner or for other works.
    pub fn free_slots(&mut self) -> Result<i64, String> {
        Ok(self.slots_by_machine()?.values().sum())
    }

    /// Each machine's free slots, as the session manager reports them from the works holding one: a work of the
    /// architect holds none, and a work holds none while it waits.
    pub fn slots_by_machine(&mut self) -> Result<BTreeMap<String, i64>, String> {
        let mut holding = Vec::new();
        for id in self.active()? {
            let w = self.work(id)?;
            if w.graph == ENGINEER && !self.waiting(&w)? {
                holding.push(w.machine.clone());
            }
        }
        Ok(sessions::worktree::free_slots(
            &self.config.config.install.machines.hosts,
            &holding,
        ))
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
    /// that waits.
    pub fn waiting(&mut self, w: &Work) -> Result<bool, String> {
        let g = self.graph_of(w)?;
        let Some(cell) = g.cell(&w.pos.at) else {
            return Ok(false);
        };
        if matches!(cell.step.as_deref(), Some("ask_owner" | "ask_architect")) {
            return Ok(true);
        }
        let last = work::rounds(self.db.conn(), w.id)?.pop();
        Ok(last.is_some_and(|r| {
            r.cell.as_deref() == Some(cell.id.as_str())
                && r.transition
                    .as_ref()
                    .is_some_and(|t| cell.rows.iter().any(|x| &x.name == t && x.waits))
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
        let g = self.flow(self.flow_now).ok()?.graph(ENGINEER)?;
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

    /// A work of a project's architect to start now: once what the engine raises itself is queued, the rows of the cell
    /// a project's architect waits at to start (`S0`), taken for each project none of whose architect works runs; the
    /// first such project's whose row leads on. Gives the work started.
    fn architect_work_due(&mut self) -> Result<Option<i64>, String> {
        architect::queue_due(self)?;
        for project in self.projects() {
            if architect::running(self, &project)?.is_some() {
                continue;
            }
            if let Some(id) = architect::start(self, &project, false)? {
                (self.say)(&format!("work {id}: a work of the architect of {project} starts"));
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    /// The owner's step of a project's architect: the next round of the work of it that runs, or, when none runs and
    /// the cell it waits at to start (`S0`) leads on, a matter being due, a new work started paused by the owner, and
    /// its first round. Nothing runs when the architect has nothing to start a work for.
    pub fn step_architect(&mut self, project: &str) -> Result<Stepped, String> {
        self.reload_config();
        actions::run_outbox(self)?;
        if self.halted() {
            return Ok(Stepped::default());
        }
        if let Some(id) = architect::running(self, project)? {
            return self.step_work(id);
        }
        architect::queue_due(self)?;
        let mark = self.rounds_mark()?;
        let Some(id) = architect::start(self, project, true)? else {
            return Ok(Stepped::default());
        };
        (self.say)(&format!(
            "work {id}: a work of the architect of {project} starts, as the owner steps it"
        ));
        self.take_step(id, true)?;
        actions::run_outbox(self)?;
        let (rounds, cell) = self.rounds_since(&mark)?;
        Ok(Stepped {
            work: Some(id),
            started_item: None,
            rounds,
            cell,
        })
    }

    /// One pass over every work: reads the configuration again, carries out the outbox, moves or takes over the works
    /// that go on on an older flow version, starts a work of the architect when a matter is due, takes, while a slot is
    /// free, a step of each work that waits holding no slot, so that one whose wait is over takes the slot first, starts
    /// works on ready items while slots are free, and takes each other active work one step. Gives whether anything
    /// moved.
    pub fn tick(&mut self) -> Result<bool, String> {
        self.reload_config();
        let mut moved = actions::run_outbox(self)?;
        if self.halted() {
            return Ok(moved);
        }
        let old = self.outdated()?;
        if !old.is_empty() {
            self.bring_up(&old)?;
            moved = true;
        }
        if self.architect_work_due()?.is_some() {
            moved = true;
        }
        // The works that wait, holding no slot, take the free slots as what they wait for comes, before new works start.
        let mut stepped = BTreeSet::new();
        if self.free_slots()? > 0 {
            for id in self.waiting_works()? {
                if self.advance(id)? == Advance::Moved {
                    moved = true;
                }
                stepped.insert(id);
            }
        }
        while self.free_slots()? > 0 {
            let Some(item) = self.next_ready() else { break };
            let id = self.create_work(item)?;
            (self.say)(&format!("work {id} started on item {item}"));
            moved = true;
            if self.advance(id)? != Advance::Moved {
                break;
            }
        }
        for id in self.active()? {
            if self.halted() {
                break;
            }
            if !stepped.contains(&id) && self.advance(id)? == Advance::Moved {
                moved = true;
            }
        }
        Ok(moved)
    }

    /// The works on items that wait, holding no slot, in the order of their ids: once what one waits for came, it goes
    /// on before a new work starts on a ready item, when its machine has a slot free.
    fn waiting_works(&mut self) -> Result<Vec<i64>, String> {
        let mut out = Vec::new();
        for id in self.active()? {
            let w = self.work(id)?;
            if w.graph == ENGINEER && self.waiting(&w)? {
                out.push(id);
            }
        }
        Ok(out)
    }

    /// Takes exactly one step of the engine as a whole, the smallest pass, with which the engine's loop takes the
    /// works that go on by themselves round by round, reading between two steps what the owner asked: one round of
    /// one work, which is an agent's round, a step of the engine's own, a run of a tool cell, or starting a work. It
    /// first carries out the outbox; then, when works that go on are on an older flow version, it moves those that can
    /// move and takes the others over, and when it took one over, takes the first step of the first new work, when one
    /// started; else, when no work of the architect runs and a
    /// matter is due for it, it starts one and takes its first step; else, when a slot is free, it takes the step of the
    /// first work that waits, holding no slot, that has one, as one whose wait is over and goes on into the free slot;
    /// else, when a slot is free and an item is
    /// ready, it starts a work on that item and takes the work's first step; otherwise it takes the next step of the
    /// first active work that has one, in the order of the works' ids, beginning after `after` and going round to the
    /// lowest, so that every work gets its turn as in a pass. The outbox the step filled is carried out after it.
    pub fn step(&mut self, after: Option<i64>) -> Result<Stepped, String> {
        self.reload_config();
        actions::run_outbox(self)?;
        if self.halted() {
            return Ok(Stepped::default());
        }
        let mark = self.rounds_mark()?;
        let old = self.outdated()?;
        let brought = self.bring_up(&old)?;
        if let Some(first) = brought.over.first().copied() {
            let new = brought.started.first().copied();
            let mut started_item = None;
            if let Some(n) = new {
                started_item = self.work(n)?.item;
                self.advance(n)?;
            }
            actions::run_outbox(self)?;
            let (rounds, cell) = self.rounds_since(&mark)?;
            return Ok(Stepped {
                work: Some(new.unwrap_or(first)),
                started_item,
                rounds,
                cell,
            });
        }
        if let Some(id) = self.architect_work_due()? {
            self.advance(id)?;
            actions::run_outbox(self)?;
            let (rounds, cell) = self.rounds_since(&mark)?;
            return Ok(Stepped {
                work: Some(id),
                started_item: None,
                rounds,
                cell,
            });
        }
        // A work that waits, holding no slot, goes on when what it waits for came before a new work takes the slot.
        if self.free_slots()? > 0 {
            for id in self.waiting_works()? {
                self.advance(id)?;
                let (rounds, cell) = self.rounds_since(&mark)?;
                if !rounds.is_empty() {
                    actions::run_outbox(self)?;
                    return Ok(Stepped {
                        work: Some(id),
                        started_item: None,
                        rounds,
                        cell,
                    });
                }
            }
        }
        if self.free_slots()? > 0
            && let Some(item) = self.next_ready()
        {
            let id = self.create_work(item)?;
            (self.say)(&format!("work {id} started on item {item}"));
            self.advance(id)?;
            actions::run_outbox(self)?;
            let (rounds, cell) = self.rounds_since(&mark)?;
            return Ok(Stepped {
                work: Some(id),
                started_item: Some(item),
                rounds,
                cell,
            });
        }
        let (later, earlier): (Vec<i64>, Vec<i64>) = self
            .active()?
            .into_iter()
            .partition(|id| after.is_some_and(|a| *id > a));
        for id in later.into_iter().chain(earlier) {
            self.advance(id)?;
            let (rounds, cell) = self.rounds_since(&mark)?;
            if !rounds.is_empty() {
                actions::run_outbox(self)?;
                return Ok(Stepped {
                    work: Some(id),
                    started_item: None,
                    rounds,
                    cell,
                });
            }
        }
        Ok(Stepped::default())
    }

    /// The next round of one work, which the owner asked for whatever the global state: the outbox first, then that
    /// work one step, then the outbox that step filled. After its step the work is paused by the owner, in the owner's
    /// hands, until the owner starts it. A work on an older flow version moves first; one that cannot move is taken
    /// over instead, and the step is the first of the new work, paused by the owner after it the same way; when no
    /// slot is free for it, its item waits at the head of the queue and no round runs.
    pub fn step_work(&mut self, work: i64) -> Result<Stepped, String> {
        self.reload_config();
        actions::run_outbox(self)?;
        if self.halted() {
            return Ok(Stepped::default());
        }
        let mark = self.rounds_mark()?;
        self.take_in_hand(work)?;
        let w = self.work(work)?;
        let mut stepped = work;
        let mut started_item = None;
        let outdated = w.ended.is_none() && w.flow_version != self.flow_now && !self.done(&w)?;
        let cannot = if outdated { self.cannot_move(&w)? } else { None };
        if let Some(cannot) = cannot {
            let item = w.item;
            let why = self.moved_on(&cannot);
            if let Some(new) = self.take_over(&[(w, why)])?.first().copied() {
                self.take_step(new, true)?;
                stepped = new;
                started_item = item;
            }
        } else {
            self.take_step(work, true)?;
        }
        self.paused_after_step(stepped)?;
        actions::run_outbox(self)?;
        let (rounds, cell) = self.rounds_since(&mark)?;
        Ok(Stepped {
            work: Some(stepped),
            started_item,
            rounds,
            cell,
        })
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

    /// While every work is paused, one step of the works the owner started (`started`), and of the architects of the
    /// projects the owner started them for (`architects`): when no architect work of such a project runs and the cell
    /// it waits at to start (`S0`) leads on, a matter being due, one starts and takes its first step; otherwise the
    /// next step of the first of those works, and of the architect works that run, that has one, in the order of the
    /// works' ids beginning after `after`, as [`Engine::step`] goes round the works that go on. A started work paused
    /// by itself takes no step, and no new work starts on a ready item.
    pub fn step_started(
        &mut self,
        after: Option<i64>,
        started: &BTreeSet<i64>,
        architects: &BTreeSet<String>,
    ) -> Result<Stepped, String> {
        self.reload_config();
        actions::run_outbox(self)?;
        if self.halted() {
            return Ok(Stepped::default());
        }
        let mark = self.rounds_mark()?;
        architect::queue_due(self)?;
        for project in architects {
            if architect::running(self, project)?.is_some() {
                continue;
            }
            let Some(id) = architect::start(self, project, false)? else {
                continue;
            };
            (self.say)(&format!(
                "work {id}: a work of the architect of {project} starts, as the owner started it"
            ));
            self.advance(id)?;
            actions::run_outbox(self)?;
            let (rounds, cell) = self.rounds_since(&mark)?;
            return Ok(Stepped {
                work: Some(id),
                started_item: None,
                rounds,
                cell,
            });
        }
        let mut architect_works = BTreeSet::new();
        for project in architects {
            architect_works.extend(architect::running(self, project)?);
        }
        let (later, earlier): (Vec<i64>, Vec<i64>) = self
            .active()?
            .into_iter()
            .filter(|id| started.contains(id) || architect_works.contains(id))
            .partition(|id| after.is_some_and(|a| *id > a));
        for id in later.into_iter().chain(earlier) {
            self.advance(id)?;
            let (rounds, cell) = self.rounds_since(&mark)?;
            if !rounds.is_empty() {
                actions::run_outbox(self)?;
                return Ok(Stepped {
                    work: Some(id),
                    started_item: None,
                    rounds,
                    cell,
                });
            }
        }
        Ok(Stepped::default())
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
        (self.say)(&format!(
            "work {id} goes on, as the owner started it{}",
            if told {
                "; the architect, which had stopped it, is told"
            } else {
                ""
            }
        ));
        Ok(())
    }

    /// Each work's newest round, and the rounds not ended, as JSON lists of `[work, n]`.
    fn rounds_mark(&self) -> Result<(String, String), String> {
        let conn = self.db.conn();
        let newest: String = conn
            .query_row(
                "SELECT coalesce(json_group_array(json_array(work, last)), '[]')
                 FROM (SELECT work, max(n) AS last FROM rounds GROUP BY work)",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let open: String = conn
            .query_row(
                "SELECT coalesce(json_group_array(json_array(work, n)), '[]') FROM rounds WHERE ended_at IS NULL",
                [],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok((newest, open))
    }

    /// The rounds started since `mark`, and those open then that have ended since, in the order they started, with the
    /// cell of the last.
    fn rounds_since(&self, mark: &(String, String)) -> Result<(Vec<RoundId>, Option<String>), String> {
        let mut st = self
            .db
            .conn()
            .prepare(
                "SELECT r.work, r.n, r.cell FROM rounds r
                 WHERE r.n > coalesce((SELECT value ->> 1 FROM json_each(?1) WHERE value ->> 0 = r.work), 0)
                    OR (r.ended_at IS NOT NULL
                        AND EXISTS (SELECT 1 FROM json_each(?2) WHERE value ->> 0 = r.work AND value ->> 1 = r.n))
                 ORDER BY r.started_at, r.work, r.n",
            )
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map(params![mark.0, mark.1], |r| {
                Ok((RoundId::new(r.get(0)?, r.get(1)?), r.get::<_, Option<String>>(2)?))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let cell = rows.last().and_then(|(_, c)| c.clone());
        Ok((rows.into_iter().map(|(id, _)| id).collect(), cell))
    }
}

/// What one step came to: the work it took a step of, none when no work had a step to take; the item it started that
/// work on, when it started it; the rounds it started or ended, and the cell of the last of them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stepped {
    pub work: Option<i64>,
    pub started_item: Option<i64>,
    pub rounds: Vec<RoundId>,
    pub cell: Option<String>,
}

/// What bringing works on an older flow version to the version the engine runs came to: the works taken over, and the
/// new works started on their items.
#[derive(Debug, Clone, Default)]
struct Brought {
    over: Vec<i64>,
    started: Vec<i64>,
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
