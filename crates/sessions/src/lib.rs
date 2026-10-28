//! The session manager: everything about the agents' sessions, what each holds and costs, and the command line they
//! run in. The engine decides when (which cell runs next, when a work waits); the session manager decides in which
//! session a round runs ([`choose`]), what the round's message gives and what it only refers to ([`held`], [`give`],
//! [`diff`]), how the command line starts and runs ([`cli`]), what each call cost ([`cost`]), and keeps a waiting
//! session's cache warm ([`warm`]). It reads and writes only the sessions, their keep-warm pings and the rounds'
//! records, so that it can move out of the engine's process; for now they share one.

pub mod choose;
pub mod cli;
pub mod cost;
pub mod diff;
pub mod give;
pub mod held;
pub mod round;
pub mod warm;

use std::collections::BTreeMap;
use std::path::Path;

use flow::{CompiledCell, CompiledGraph};

pub use choose::{Plan, Start};
pub use held::Held;
pub use round::{Round, RoundId};

/// The session manager as the engine holds it: the command line its agents run in, and each CLI's version as it
/// reported it.
pub struct Manager {
    agents: Box<dyn cli::Agents>,
    versions: BTreeMap<String, String>,
}

impl Manager {
    pub fn new(agents: Box<dyn cli::Agents>) -> Manager {
        Manager {
            agents,
            versions: BTreeMap::new(),
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
