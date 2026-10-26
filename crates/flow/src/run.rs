//! Where a work stands in a compiled graph, and where a row takes it: the moves the compiler walks every state of,
//! made one at a time on the cells' ids.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::compiled::{CompiledGraph, CompiledRow};

/// Where a work stands.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// The cell the work is at.
    pub at: String,
    /// The cell the work left for the asking part; `@stopped` points at it.
    pub stopped: Option<String>,
    /// The cells waiting for an answer, innermost last; `@caller` points at the last.
    pub callers: Vec<String>,
    /// The cell that brought the work here; `@from` points at it.
    pub from: Option<String>,
    /// The values rows recorded.
    pub recorded: BTreeMap<String, String>,
}

impl Position {
    /// The cell a pointer points at here.
    pub fn pointer(&self, p: &str) -> Option<&str> {
        match p {
            "@stopped" => self.stopped.as_deref(),
            "@caller" => self.callers.last().map(String::as_str),
            "@from" => self.from.as_deref(),
            _ => None,
        }
    }
}

impl CompiledGraph {
    /// Where a work starts.
    pub fn start_position(&self) -> Position {
        Position {
            at: self.start.clone(),
            ..Position::default()
        }
    }

    fn in_region(&self, id: &str) -> bool {
        self.asking.iter().any(|a| a == id)
    }

    fn is_agent(&self, id: &str) -> bool {
        self.cell(id).is_some_and(|c| c.kind == "agent")
    }

    /// A cell that answers whoever called it: one with a row back to its caller or asker.
    pub fn is_called(&self, id: &str) -> bool {
        self.cell(id)
            .is_some_and(|c| c.rows.iter().any(|r| r.to == "@caller" || r.to == "@asker"))
    }

    /// Where `row`, taken at `p`, takes the work. The compiler has walked every state and found no row that goes
    /// nowhere, so an error here means the work's position was not one the flow can reach.
    pub fn step(&self, p: &Position, row: &CompiledRow) -> Result<Position, String> {
        let mut recorded = p.recorded.clone();
        recorded.extend(row.sets.iter().map(|(k, v)| (k.clone(), v.clone())));
        let m = p.at.as_str();
        let to = row.to.as_str();
        if to == "@self" {
            return Ok(Position { recorded, ..p.clone() });
        }
        let to_caller = p.callers.last().is_some_and(|c| c == to) && self.is_agent(to);
        if to == "@caller" || to == "@asker" || to_caller {
            let Some((last, rest)) = p.callers.split_last() else {
                return Err(format!("{m} \"{}\" answers, but no cell waits for an answer", row.name));
            };
            let mut t = last.clone();
            if to == "@asker"
                && let Some(rc) = &self.receive
                && !self.in_region(&t)
                && self.is_agent(&t)
                && self.cell(&t).and_then(|c| c.line.as_ref()) == self.cell(rc).and_then(|c| c.line.as_ref())
            {
                t = rc.clone();
            }
            let stopped = if self.in_region(&t) || !rest.is_empty() {
                p.stopped.clone()
            } else {
                None
            };
            return Ok(Position {
                at: t,
                stopped,
                callers: rest.to_vec(),
                from: Some(m.to_string()),
                recorded,
            });
        }
        let t = if to == "@stopped" {
            p.stopped
                .clone()
                .ok_or_else(|| format!("{m} \"{}\" goes back, but the work stopped nowhere", row.name))?
        } else {
            to.to_string()
        };
        if self.cell(&t).is_none() {
            return Err(format!("{m} \"{}\" goes to no such cell: {t}", row.name));
        }
        if self.is_called(&t) {
            if t == m || (p.callers.contains(&t) && self.is_agent(&t)) {
                return Err(format!(
                    "{m} \"{}\" calls {t}, which already waits for an answer",
                    row.name
                ));
            }
            let stopped = if self.in_region(m) || !p.callers.is_empty() {
                p.stopped.clone()
            } else if self.in_region(&t) {
                Some(m.to_string())
            } else {
                None
            };
            let mut callers = p.callers.clone();
            callers.push(m.to_string());
            return Ok(Position {
                at: t,
                stopped,
                callers,
                from: Some(m.to_string()),
                recorded,
            });
        }
        if !p.callers.is_empty() {
            return Err(format!(
                "{m} \"{}\" leaves while {} waits for an answer",
                row.name,
                p.callers.join(", ")
            ));
        }
        let stopped = if !self.in_region(&t) {
            None
        } else if self.in_region(m) {
            p.stopped.clone()
        } else {
            Some(m.to_string())
        };
        Ok(Position {
            at: t,
            stopped,
            callers: Vec::new(),
            from: Some(m.to_string()),
            recorded,
        })
    }
}
