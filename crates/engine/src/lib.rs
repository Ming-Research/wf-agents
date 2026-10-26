//! The engine: takes each work through the compiled flow, one cell at a time. At an agent's cell it runs a round of
//! the agent; at its own cells and the tool cells it does the step itself; at the owner's it waits for the owner's
//! answer. Each return is checked against the cell's schema and checks, takes exactly one row, and the round, the
//! work's new position and what the row does in the database are written in one transaction. What must happen
//! outside the database is put in the outbox in that transaction and carried out after it, so a crash loses none.
//!
//! In this version the engine and the session manager are one process on one machine, the owner answers for the
//! architect, and the engine takes one work one step at a time: a long round of one work holds up the others.

pub mod agent;
pub mod git;
pub mod hub;

mod actions;
mod checks;
mod inputs;
mod prompt;
mod steps;
mod tools;
mod values;
mod work;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use db::rusqlite::params;
use db::{Actor, Db};
use flow::{Compiled, CompiledCell, CompiledGraph};
use serde_json::Value;

pub use checks::CARD_RULING;
pub use work::{Item, Round, Work, json_col, query_json};

use agent::Agents;
use hub::GitHub;

/// The graph works on items run.
pub const ENGINEER: &str = "engineer";

/// How many times in a row a cell's return may be sent back before the engine pauses the work for the owner.
pub const MAX_REJECTED: usize = 3;

/// What one step of a work came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Advance {
    /// A round or step ended: the work moved on, or its return was sent back to be done again.
    Moved,
    /// The work waits: for the owner, for other works, or for its turn.
    Waits(String),
    /// The work is done, was ended, or is paused.
    Stopped,
}

pub struct Engine {
    pub db: Db,
    /// The data directory: the configuration, the database and the worktrees.
    pub data: PathBuf,
    pub config: config::Committed,
    pub agents: Box<dyn Agents>,
    pub github: Box<dyn GitHub>,
    flows: BTreeMap<i64, Compiled>,
    /// The flow version new works start on: the one built into this engine.
    pub flow_now: i64,
    /// Where the engine says what it does and what went wrong.
    pub say: Box<dyn Fn(&str)>,
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
        let path = config::database(data);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let mut db = Db::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let relations = db::relations(db::SCHEMA).map_err(|e| e.to_string())?;
        let compiled = flow::compile(flow::FLOW, &flow::Context { relations }).map_err(|e| e.to_string())?;
        let problems = config.config.check(&compiled);
        if !problems.is_empty() {
            return Err(problems.join("\n"));
        }
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
        Ok(Engine {
            db,
            data: data.to_path_buf(),
            config,
            agents,
            github,
            flows,
            flow_now,
            say: Box::new(|s| eprintln!("{s}")),
        })
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

    /// The graph a work runs on, as an owned copy, so the engine can be borrowed while it is in use.
    pub fn graph_of(&mut self, w: &Work) -> Result<CompiledGraph, String> {
        let f = self.flow(w.flow_version)?;
        f.graph(&w.graph)
            .cloned()
            .ok_or_else(|| format!("work {}: its flow has no graph {}", w.id, w.graph))
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

    /// Takes a work one step further.
    pub fn advance(&mut self, id: i64) -> Result<Advance, String> {
        let w = self.work(id)?;
        if w.ended.is_some() || w.paused.is_some() || self.done(&w)? {
            return Ok(Advance::Stopped);
        }
        let g = self.graph_of(&w)?;
        let cell = g
            .cell(&w.pos.at)
            .cloned()
            .ok_or_else(|| format!("work {id} is at no such cell: {}", w.pos.at))?;
        let result = if cell.kind == "agent" {
            steps::agent(self, &w, &g, &cell)
        } else {
            steps::engine(self, &w, &g, &cell)
        };
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

    pub fn pause(&mut self, id: i64, why: &str) -> Result<(), String> {
        self.db
            .write(Actor::Engine, |tx| {
                tx.execute("UPDATE works SET paused = ?2 WHERE id = ?1", params![id, why])?;
                Ok(())
            })
            .map_err(|e| e.to_string())
    }

    /// Starts a work on the item: at the start cell of the engineer's graph, on the flow built into the engine.
    pub fn create_work(&mut self, item: i64) -> Result<i64, String> {
        let v = self.flow_now;
        let start = self
            .flow(v)?
            .graph(ENGINEER)
            .map(|g| g.start.clone())
            .ok_or("the flow has no engineer graph")?;
        self.db
            .write(Actor::Engine, |tx| {
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, ?3, ?4)",
                    params![item, v, ENGINEER, start],
                )?;
                Ok(tx.last_insert_rowid())
            })
            .map_err(|e| e.to_string())
    }

    /// How many more works can hold a slot: the machines' slots less the works holding one. A work holds no slot
    /// while it waits for the owner or for other works.
    pub fn free_slots(&mut self) -> Result<i64, String> {
        let total: i64 = self
            .config
            .config
            .install
            .machines
            .hosts
            .values()
            .map(|h| i64::from(h.slots))
            .sum();
        let mut holding = 0;
        for id in self.active()? {
            let w = self.work(id)?;
            if !self.waiting(&w)? {
                holding += 1;
            }
        }
        Ok(total - holding)
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

    /// One pass of the main loop: reads the configuration again, carries out the outbox, starts works on ready
    /// items while slots are free, and takes each active work one step. Gives whether anything moved.
    pub fn tick(&mut self) -> Result<bool, String> {
        self.reload_config();
        let mut moved = actions::run_outbox(self)?;
        while self.free_slots()? > 0 {
            let next: Option<i64> = self
                .db
                .conn()
                .query_row("SELECT item FROM ready_items LIMIT 1", [], |r| r.get(0))
                .ok();
            let Some(item) = next else { break };
            let id = self.create_work(item)?;
            (self.say)(&format!("work {id} started on item {item}"));
            moved = true;
            if self.advance(id)? != Advance::Moved {
                break;
            }
        }
        for id in self.active()? {
            if self.advance(id)? == Advance::Moved {
                moved = true;
            }
        }
        Ok(moved)
    }

    /// Runs the main loop until `stop` says to.
    pub fn run(&mut self, stop: &dyn Fn() -> bool, idle: std::time::Duration) -> Result<(), String> {
        while !stop() {
            if !self.tick()? {
                std::thread::sleep(idle);
            }
        }
        Ok(())
    }
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
