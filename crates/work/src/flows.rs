//! The versions of its flow a work ran on, as its records say them (principle `the-past-as-it-ran`).
//!
//! A work is added on the version of its flow installed then, and runs on it until it goes on with another installed:
//! from then on it runs on that one, and each cell its records named before reads as the cell of that version it is
//! now, through the table of earlier ids (principle `cell-numbering`). Each is an entry of the work, [`OnFlow`], which
//! says how the records before it read; the records keep each cell as it was recorded, and are read through every
//! entry after them, in turn ([`read`]).

use std::collections::BTreeMap;

use decide::{CellId, Records, Seq};
use serde::{Deserialize, Serialize};

use crate::{Moment, Time, Work};

/// From this place on, the work runs on this version of its flow: as it was added, at the place before every other,
/// and again each time it went on with another version installed, at the place after everything before. Of a work kept
/// before the versions were, the first is the version installed as the engine first started with them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnFlow {
    /// The place of the entry in the order of what happened to the work.
    pub at: Seq,
    /// The version: the digest of the files of its definition.
    pub version: String,
    /// Each cell the records before this place name that is another cell on this version, by the id they name it by,
    /// with the id it is read as from here on; none where no cell changed its id.
    pub ids: BTreeMap<CellId, CellId>,
}

impl OnFlow {
    /// The id an entry at this place that names this cell reads it by from this version on, where it is another.
    pub fn reads(&self, at: Seq, cell: &CellId) -> Option<&CellId> {
        if at < self.at { self.ids.get(cell) } else { None }
    }

    /// Reads the cells the records name before this place as this version names them.
    pub fn read(&self, records: &mut Records) {
        if self.ids.is_empty() {
            return;
        }
        let read = |at: Seq, cell: &mut CellId| {
            if let Some(id) = self.reads(at, cell) {
                *cell = id.clone();
            }
        };
        for started in &mut records.started {
            read(started.at, &mut started.cell);
        }
        for question in &mut records.questions {
            read(question.at, &mut question.cell);
        }
        for let_go in &mut records.let_go {
            read(let_go.at, &mut let_go.cell);
        }
        for written in &mut records.written {
            if let Some(cell) = &mut written.said_to {
                read(written.at, cell);
            }
        }
    }
}

/// Reads the cells the records name as the version they now run on names them: through each of `flows`, in the order
/// of their places, each reading what was recorded before it.
pub fn read(records: &mut Records, flows: &[OnFlow]) {
    for on in flows {
        on.read(records);
    }
}

/// The id a cell recorded at this place is read by on the version the last of `flows` names: through each of them, in
/// turn, as [`read`] reads the records.
pub fn read_cell(flows: &[OnFlow], at: Seq, cell: &CellId) -> CellId {
    let mut cell = cell.clone();
    for on in flows {
        if let Some(id) = on.reads(at, &cell) {
            cell = id.clone();
        }
    }
    cell
}

/// The version of its flow the work runs on: the latest it was put on; none for a work kept before the versions were.
pub fn version(work: &Work) -> Option<&str> {
    work.flows.last().map(|on| on.version.as_str())
}

/// Every cell the records name, as they read now.
pub fn cells(records: &Records) -> impl Iterator<Item = &CellId> {
    (records.started.iter().map(|s| &s.cell))
        .chain(records.questions.iter().map(|q| &q.cell))
        .chain(records.let_go.iter().map(|l| &l.cell))
        .chain(records.written.iter().filter_map(|w| w.said_to.as_ref()))
}

impl Work {
    /// Puts the work on this version of its flow at this time, from its next place on: each cell its records named
    /// before that is another cell on it, by `ids`, reads as that one from now on. Nothing else is added to the work
    /// before it is saved, so that what the store keeps of it before the entry is what it held already.
    pub fn onto(&mut self, now: Time, version: String, ids: BTreeMap<CellId, CellId>) {
        let at = self.tick();
        let on = OnFlow { at, version, ids };
        on.read(&mut self.records);
        self.flows.push(on);
        self.moments.push(Moment { from: at, at: now });
    }
}

#[cfg(test)]
mod tests {
    use decide::{LetGo, Questioned, RunId, Started};

    use super::*;

    fn started(at: u64, cell: &str) -> Started {
        Started {
            run: RunId(at),
            cell: cell.into(),
            at: Seq(at),
            because: Vec::new(),
            basis: BTreeMap::new(),
        }
    }

    fn ids(pairs: &[(&str, &str)]) -> BTreeMap<CellId, CellId> {
        pairs
            .iter()
            .map(|(a, b)| (CellId::from(*a), CellId::from(*b)))
            .collect()
    }

    // A run recorded before a work was put on a renumbered version reads as the cell it ran at, through every version
    // after it in turn; one recorded after reads as it was recorded, and so does a cell whose id stayed.
    #[test]
    fn a_cell_recorded_before_a_version_reads_as_that_versions_cell() {
        let mut work = Work::default();
        work.records.started = vec![started(1, "D2"), started(2, "D3"), started(3, "I1")];
        work.records.let_go.push(LetGo {
            cell: "D2".into(),
            at: Seq(4),
        });
        work.onto(Time(10), "b".into(), ids(&[("D2", "D3"), ("D3", "D4")]));
        assert_eq!(work.flows[0].at, Seq(5));
        work.records.started.push(started(6, "D2"));
        work.records.questions.push(Questioned {
            name: "q".into(),
            cell: "D3".into(),
            run: RunId(6),
            engine: false,
            waits: true,
            withdrawn: false,
            at: Seq(7),
        });
        work.onto(Time(20), "c".into(), ids(&[("D2", "D6"), ("D4", "D5")]));
        assert_eq!(work.flows[1].at, Seq(8));
        let cells: Vec<&str> = work.records.started.iter().map(|s| s.cell.as_str()).collect();
        assert_eq!(cells, ["D3", "D5", "I1", "D6"]);
        assert_eq!(work.records.let_go[0].cell.as_str(), "D3");
        assert_eq!(work.records.questions[0].cell.as_str(), "D3");
        // As recorded, read through each version in turn, the same.
        let mut recorded = Work::default();
        recorded.records.started = vec![started(1, "D2"), started(2, "D3"), started(3, "I1"), started(6, "D2")];
        recorded.records.let_go.push(LetGo {
            cell: "D2".into(),
            at: Seq(4),
        });
        read(&mut recorded.records, &work.flows);
        assert_eq!(recorded.records.started, work.records.started);
        assert_eq!(recorded.records.let_go, work.records.let_go);
        assert_eq!(version(&work), Some("c"));
    }
}
