//! The session manager: everything about the agents' sessions, what each holds and costs, and the command line they
//! run in. The engine decides when (which cell runs next, when a work waits); the session manager decides in which
//! session a round runs ([`choose`]), what the round's message gives and what it only refers to ([`held`], [`give`],
//! [`diff`]), whether a round starts the new session the owner asked for, how the command line starts and runs
//! ([`cli`]), what each call cost ([`cost`]), keeps a waiting session's cache warm ([`warm`]) and tells a round that
//! missed it ([`miss`]); it sets up a work's worktree on a machine with a free slot, says whether a head
//! handed in is the clean worktree's, removes the worktree once the work ended, and brings a project's architect's
//! worktree to the latest main before each of its rounds ([`worktree`]). It reads and writes
//! only the sessions, their keep-warm pings, the rounds' records and where works run, so that it can move out of the
//! engine's process; for now they share one.

pub mod choose;
pub mod cli;
pub mod cost;
pub mod diff;
pub mod give;
pub mod held;
pub mod interject;
pub mod miss;
pub mod round;
pub mod warm;
pub mod worktree;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use db::rusqlite::Connection;

use flow::{CompiledCell, CompiledGraph};

pub use choose::{Plan, Start};
pub use held::Held;
pub use round::{Round, RoundId};

/// Whose next round the owner asked to start a new session: a work's, or a project's architect's.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fresh {
    Work(i64),
    Architect(String),
}

/// The session manager as the engine holds it: the command line its agents run in, each CLI's version as it reported
/// it, and the new sessions the owner asked for that have not started yet.
pub struct Manager {
    agents: Box<dyn cli::Agents>,
    versions: BTreeMap<String, String>,
    /// The works whose next agent's round starts a new session, as the owner asked with its Start or Step.
    asked: BTreeSet<i64>,
    /// The projects whose architect's next round starts a new session for its work, as the owner asked.
    asked_architect: BTreeSet<String>,
}

impl Manager {
    pub fn new(agents: Box<dyn cli::Agents>) -> Manager {
        Manager {
            agents,
            versions: BTreeMap::new(),
            asked: BTreeSet::new(),
            asked_architect: BTreeSet::new(),
        }
    }

    /// Asks that a work's next agent's round start a new session for its cell's line, or that a project's architect's
    /// next round start one for its work: the owner chose a new session with Start or Step.
    pub fn ask_new(&mut self, fresh: Fresh) {
        match fresh {
            Fresh::Work(w) => {
                self.asked.insert(w);
            }
            Fresh::Architect(p) => {
                self.asked_architect.insert(p);
            }
        }
    }

    /// Whether the owner asked that a work's next agent's round start a new session; `architect` names the project of
    /// a work of its architect's graph, for which the owner's ask is that architect's.
    pub fn asked_new(&self, work: i64, architect: Option<&str>) -> bool {
        self.asked.contains(&work) || architect.is_some_and(|p| self.asked_architect.contains(p))
    }

    /// Where a work's next round at a cell runs ([`choose::plan`]), unless the owner asked for a new session, which it
    /// then starts, recorded as the owner's; the session the line had going on ends as at any new session, and is
    /// otherwise left as it is. `architect` names the project of a work of its architect's graph.
    pub fn plan(
        &self,
        conn: &Connection,
        work: i64,
        architect: Option<&str>,
        cell: &CompiledCell,
        limit: u64,
        on: &str,
    ) -> Result<Plan, String> {
        if self.asked_new(work, architect) {
            return Ok(Plan::New(Start::Owner));
        }
        choose::plan(conn, work, cell, limit, on)
    }

    /// The new session the owner asked for started: the work's next rounds go on in it.
    pub fn met(&mut self, work: i64, architect: Option<&str>) {
        self.asked.remove(&work);
        if let Some(p) = architect {
            self.asked_architect.remove(p);
        }
    }

    /// Runs a round; `activity` is told what the agent is doing while it runs.
    pub fn run(&self, r: &cli::Request, activity: &dyn Fn(&str)) -> cli::Reply {
        self.agents.run(r, activity)
    }

    /// The agents' command line, for what runs it besides a round.
    pub fn agents(&self) -> &dyn cli::Agents {
        self.agents.as_ref()
    }

    /// A CLI's version as it reports it, asked the first time only, with its configuration directory; unknown for a
    /// CLI the configuration does not name.
    pub fn version(&mut self, data: &Path, config: &config::Config, name: &str) -> String {
        if let Some(v) = self.versions.get(name) {
            return v.clone();
        }
        let Some(c) = config.install.agents.clis.get(name) else {
            return "unknown".to_string();
        };
        let (home, _) = cli::home(data, c, name);
        let v = self.agents.version(c, Some(&home));
        self.versions.insert(name.to_string(), v.clone());
        v
    }
}

/// The agent's cells whose forms and tools `format:common` gives a session of a cell's line: every agent's cell on the
/// line, so that it is the same for every round of the line and given once in a session; for a cell on no line, whose
/// session is its own, the cell alone.
pub fn format_cells<'g>(g: &'g CompiledGraph, cell: &CompiledCell) -> Vec<&'g CompiledCell> {
    g.cells
        .iter()
        .filter(|c| c.kind == "agent")
        .filter(|c| match &cell.line {
            Some(l) => c.line.as_ref() == Some(l),
            None => c.id == cell.id,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compiled() -> flow::Compiled {
        let relations = db::relations(db::SCHEMA).unwrap();
        let tables = db::index_of(db::SCHEMA).unwrap();
        flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap()
    }

    // Each line's sessions are given only the forms and tools some cell of the line has, the same for every cell of
    // the line: the advisor, which hands in no redirect, is not told of it.
    #[test]
    fn each_line_is_given_only_its_own_common_forms() {
        let f = compiled();
        let g = f.graph("engineer").unwrap();
        let all = flow::formats::common_format(g, &[]);
        let of = |cell: &str| flow::formats::common_format_of(g, &format_cells(g, g.cell(cell).unwrap()), &[]);
        assert_eq!(of("R1"), of("T6"));
        assert_eq!(of("T3"), of("T4"));
        for cell in ["R1", "T3", "T1", "A1"] {
            assert!(of(cell).len() <= all.len(), "{cell}");
        }
        assert!(
            all.contains("`redirect`") && !of("A1").contains("`redirect`"),
            "{}",
            of("A1")
        );
        let cells: Vec<&str> = format_cells(g, g.cell("T4").unwrap())
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        assert!(
            cells.contains(&"T3") && cells.contains(&"T5") && !cells.contains(&"I1"),
            "{cells:?}"
        );
    }

    // The flow starts the engineer's new sessions at researching, implementing and fixing.
    #[test]
    fn the_engineer_starts_new_sessions_at_researching_implementing_and_fixing() {
        let f = compiled();
        let g = f.graph("engineer").unwrap();
        let starts: Vec<&str> = g
            .cells
            .iter()
            .filter(|c| c.new_session)
            .map(|c| c.id.as_str())
            .collect();
        assert_eq!(starts, ["R1", "I1", "T6"]);
    }
}
