//! The session manager: everything about the agents' sessions, what each holds and costs, and the command line they
//! run in. The engine decides when (which cell runs next, when a work waits); the session manager decides in which
//! session a round runs ([`choose`]), what the round's message gives and what it only refers to ([`held`], [`give`],
//! [`diff`]), whether a round starts the new session the owner asked for, how the command line starts and runs
//! ([`cli`]), what each call cost ([`cost`]), and keeps a waiting
//! session's cache warm ([`warm`]); it sets up a work's worktree on a machine with a free slot, says whether a head
//! handed in is the clean worktree's, and removes the worktree once the work ended ([`worktree`]). It reads and writes
//! only the sessions, their keep-warm pings, the rounds' records and where works run, so that it can move out of the
//! engine's process; for now they share one.

pub mod choose;
pub mod cli;
pub mod cost;
pub mod diff;
pub mod give;
pub mod held;
pub mod interject;
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

/// The session manager as the engine holds it: the command line its agents run in, each CLI's version as it reported
/// it, and the new sessions the owner asked for that have not started yet.
pub struct Manager {
    agents: Box<dyn cli::Agents>,
    versions: BTreeMap<String, String>,
    /// The works whose next agent's round starts a new session, as the owner asked with its Start or Step.
    asked: BTreeSet<i64>,
    /// Whether the architect's next round starts a new session for its wake, as the owner asked.
    asked_wake: bool,
}

impl Manager {
    pub fn new(agents: Box<dyn cli::Agents>) -> Manager {
        Manager {
            agents,
            versions: BTreeMap::new(),
            asked: BTreeSet::new(),
            asked_wake: false,
        }
    }

    /// Asks that a work's next agent's round start a new session for its cell's line, or with none, that the
    /// architect's next round start one for its wake: the owner chose a new session with Start or Step.
    pub fn ask_new(&mut self, work: Option<i64>) {
        match work {
            Some(w) => {
                self.asked.insert(w);
            }
            None => self.asked_wake = true,
        }
    }

    /// Whether the owner asked that a work's next agent's round start a new session; `wake` says the work is a wake of
    /// the architect, for which the owner's ask is the architect's.
    pub fn asked_new(&self, work: i64, wake: bool) -> bool {
        self.asked.contains(&work) || (wake && self.asked_wake)
    }

    /// Where a work's next round at a cell runs ([`choose::plan`]), unless the owner asked for a new session, which it
    /// then starts, recorded as the owner's; the session the line had going on ends as at any new session, and is
    /// otherwise left as it is.
    pub fn plan(
        &self,
        conn: &Connection,
        work: i64,
        wake: bool,
        cell: &CompiledCell,
        limit: u64,
        on: &str,
    ) -> Result<Plan, String> {
        if self.asked_new(work, wake) {
            return Ok(Plan::New(Start::Owner));
        }
        choose::plan(conn, work, cell, limit, on)
    }

    /// Withdraws the owner's ask of a new session for a work's next round, as what the round would be is shown with one
    /// and without.
    pub fn forget(&mut self, work: i64) {
        self.asked.remove(&work);
    }

    /// The new session the owner asked for started: the work's next rounds go on in it.
    pub fn met(&mut self, work: i64, wake: bool) {
        self.asked.remove(&work);
        if wake {
            self.asked_wake = false;
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
        assert_eq!(of("1"), of("8"));
        assert_eq!(of("5"), of("5a"));
        for cell in ["1", "5", "6", "12"] {
            assert!(of(cell).len() <= all.len(), "{cell}");
        }
        assert!(
            all.contains("`redirect`") && !of("12").contains("`redirect`"),
            "{}",
            of("12")
        );
        let cells: Vec<&str> = format_cells(g, g.cell("5a").unwrap())
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        assert!(
            cells.contains(&"5") && cells.contains(&"5b") && !cells.contains(&"4"),
            "{cells:?}"
        );
    }

    // The flow starts the engineer's new sessions at implementing and fixing.
    #[test]
    fn the_engineer_starts_new_sessions_at_implementing_and_fixing() {
        let f = compiled();
        let g = f.graph("engineer").unwrap();
        let starts: Vec<&str> = g
            .cells
            .iter()
            .filter(|c| c.new_session)
            .map(|c| c.id.as_str())
            .collect();
        assert_eq!(starts, ["4", "8"]);
    }
}
