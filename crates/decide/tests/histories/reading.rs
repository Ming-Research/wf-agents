//! The histories' own reading of a work's records, written from the entries of the design and not from the walk: the
//! versions of each block, the meaning of each, what each cell's record knows and why a run would start. The
//! properties are judged by this reading, so that a fault in the walk's own reading is not repeated in the judging.
//! It reads only the records and the flow, with its named tests, never what the deciding code works out of them.

use std::collections::BTreeMap;

use decide::{
    BlockId, Cell, CellId, Condition, Finished, Flow, Outsider, Records, RunId, Seq, Started, TestKind, TestName,
};

/// Who wrote a version of a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Writer {
    Person,
    Outside,
    /// A run of this cell, as it finished.
    Cell(CellId, RunId),
}

/// A version of a block: one write of it.
#[derive(Clone, Debug)]
pub struct Version {
    /// The place of the entry that wrote it, which names it.
    pub at: Seq,
    pub by: Writer,
    /// The version at which what the block says last changed: this one, or an earlier one.
    pub meaning: Seq,
    pub value: Option<String>,
}

impl Version {
    /// Whether this write changed what the block says.
    pub fn changed_meaning(&self) -> bool {
        self.meaning == self.at
    }
}

/// Why a run of a cell would start, as the entries say: it has no record, or these inputs have another meaning than
/// the version its record knows (`was`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reason {
    NoRecord,
    Changed { block: BlockId, was: Option<Seq> },
}

pub struct Reading<'a> {
    pub records: &'a Records,
    /// Every version of every block, in the order they were written.
    pub versions: BTreeMap<BlockId, Vec<Version>>,
    pub started: BTreeMap<RunId, &'a Started>,
    pub finished: BTreeMap<RunId, &'a Finished>,
}

impl<'a> Reading<'a> {
    /// The reading of these records. The meaning of each version follows entry meaning-told-by-the-writer: the first
    /// version of a block has its own. A version that has a value, or follows one that has, is a choice: it changes the
    /// meaning when the value differs from the one before, and only then, whoever writes it. Otherwise a person's
    /// prose always changes the meaning; what the outside writes does unless it was approved as not mattering; a
    /// cell's write does when the cell says so, and when it says not, it is taken at its word only if the version it
    /// started from has the meaning of the version it writes over: a cell that did not see what it writes over is not
    /// taken at its word.
    pub fn new(records: &'a Records) -> Self {
        let started: BTreeMap<RunId, &Started> = records.started.iter().map(|s| (s.run, s)).collect();
        let finished: BTreeMap<RunId, &Finished> = records.finished.iter().map(|f| (f.run, f)).collect();
        struct Entry<'b> {
            at: Seq,
            block: &'b BlockId,
            by: Writer,
            matters: bool,
            value: Option<String>,
        }
        let mut entries: Vec<Entry> = Vec::new();
        for w in &records.written {
            entries.push(Entry {
                at: w.at,
                block: &w.block,
                by: match w.by {
                    Outsider::Person => Writer::Person,
                    Outsider::Outside => Writer::Outside,
                },
                matters: w.matters,
                value: w.value.clone(),
            });
        }
        for f in &records.finished {
            let cell = started[&f.run].cell.clone();
            for w in &f.writes {
                entries.push(Entry {
                    at: f.at,
                    block: &w.block,
                    by: Writer::Cell(cell.clone(), f.run),
                    matters: w.matters,
                    value: w.value.clone(),
                });
            }
        }
        entries.sort_by_key(|e| e.at);
        let mut versions: BTreeMap<BlockId, Vec<Version>> = BTreeMap::new();
        for e in entries {
            let of_block = versions.entry(e.block.clone()).or_default();
            let meaning = match of_block.last() {
                None => e.at,
                Some(over) if e.value.is_some() || over.value.is_some() => {
                    if e.value == over.value {
                        over.meaning
                    } else {
                        e.at
                    }
                }
                Some(over) => match &e.by {
                    Writer::Person => e.at,
                    Writer::Outside if e.matters => e.at,
                    Writer::Outside => over.meaning,
                    Writer::Cell(..) if e.matters => e.at,
                    Writer::Cell(_, run) => {
                        let from = started[run].basis.get(e.block).copied().flatten();
                        let seen = from
                            .and_then(|v| of_block.iter().find(|x| x.at == v))
                            .map(|x| x.meaning);
                        if seen == Some(over.meaning) { over.meaning } else { e.at }
                    }
                },
            };
            of_block.push(Version {
                at: e.at,
                by: e.by,
                meaning,
                value: e.value,
            });
        }
        Self {
            records,
            versions,
            started,
            finished,
        }
    }

    pub fn of(&self, block: &BlockId) -> &[Version] {
        self.versions.get(block).map_or(&[], Vec::as_slice)
    }

    /// The meaning of the version of the block written at this place; `None` for no version.
    pub fn meaning(&self, block: &BlockId, version: Option<Seq>) -> Option<Seq> {
        let at = version?;
        let v = self.of(block).iter().find(|v| v.at == at);
        Some(v.unwrap_or_else(|| panic!("{block} has no version at {at:?}")).meaning)
    }

    /// The version of the block that was current just before this place.
    pub fn current_before(&self, block: &BlockId, place: Seq) -> Option<&Version> {
        self.of(block).iter().rev().find(|v| v.at < place)
    }

    /// The block's current version.
    pub fn current(&self, block: &BlockId) -> Option<&Version> {
        self.of(block).last()
    }

    /// The versions of the block after `after` (all where it is `None`) up to and including `upto` that changed what
    /// it says.
    pub fn changes(&self, block: &BlockId, after: Option<Seq>, upto: Seq) -> Vec<&Version> {
        (self.of(block).iter())
            .filter(|v| after.is_none_or(|a| v.at > a) && v.at <= upto && v.changed_meaning())
            .collect()
    }

    /// The cell's record just before this place: its latest run that had finished.
    pub fn record(&self, cell: &CellId, place: Seq) -> Option<RunId> {
        (self.finished.values())
            .filter(|f| f.at < place && &self.started[&f.run].cell == cell)
            .max_by_key(|f| f.at)
            .map(|f| f.run)
    }

    /// The version of the block a finished run knows: the one it wrote, if it did, and otherwise the one it started
    /// from.
    pub fn known(&self, run: RunId, block: &BlockId) -> Option<Seq> {
        match self.finished.get(&run) {
            Some(f) if f.writes.iter().any(|w| &w.block == block) => Some(f.at),
            _ => self.started[&run].basis.get(block).copied().flatten(),
        }
    }

    /// Why the cell would start a run at this place (entry state-is-blocks): no record, or each input whose current
    /// meaning differs from the meaning of the version its record knows; empty when it needs no run.
    pub fn reasons(&self, cell: &Cell, place: Seq) -> Vec<Reason> {
        let Some(record) = self.record(&cell.id, place) else {
            return vec![Reason::NoRecord];
        };
        (cell.inputs.iter())
            .filter_map(|block| {
                let was = self.known(record, block);
                let now = self.current_before(block, place).map(|v| v.meaning);
                (self.meaning(block, was) != now).then(|| Reason::Changed {
                    block: block.clone(),
                    was,
                })
            })
            .collect()
    }

    /// Whether a change of the block from `was` to `now` is a way round for the cell at place `i` of the flow (entry
    /// every-way-round-has-a-limit): every change of its meaning between the two was written by a cell after it in the
    /// order. One made by a person, the outside, the cell itself or a cell before it makes it no way round.
    pub fn behind(&self, flow: &Flow, i: usize, block: &BlockId, was: Option<Seq>, now: Seq) -> bool {
        let changes = self.changes(block, was, now);
        !changes.is_empty()
            && changes.iter().all(|v| match &v.by {
                Writer::Cell(cell, _) => flow.place(cell).is_some_and(|j| j > i),
                Writer::Person | Writer::Outside => false,
            })
    }

    /// Whether the run that starts or started at this place for these reasons is a way round: it has reasons, and
    /// each is a change of meaning that only cells after it made.
    pub fn way_round(&self, flow: &Flow, i: usize, reasons: &[Reason], place: Seq) -> bool {
        !reasons.is_empty()
            && reasons.iter().all(|r| match r {
                Reason::NoRecord => false,
                Reason::Changed { block, was } => {
                    let now = self.current_before(block, place).map(|v| v.at);
                    now.is_some_and(|now| self.behind(flow, i, block, *was, now))
                }
            })
    }

    /// Whether the condition of the cell at place `i` of the flow is met now: with none it always is.
    pub fn applies(&self, flow: &Flow, i: usize) -> bool {
        match &flow.cells()[i].applies {
            None => true,
            Some(Condition::Test(name)) => self.holds(flow, name, i),
            Some(Condition::Is { block, value }) => {
                self.current(block).and_then(|v| v.value.as_deref()) == Some(value.as_str())
            }
        }
    }

    /// Whether the named test holds now for the cell at place `i`, by the current versions of the blocks it reads that
    /// have the value it tests for: all of them, one, none; or, for `none_standing`, none that stands. A version
    /// stands unless a cell wrote it that is skipped now; a cell is asked only when it
    /// stands before place `i`, as every writer of what a test reads does in a flow the compiler takes.
    pub fn holds(&self, flow: &Flow, name: &TestName, i: usize) -> bool {
        let test = &flow.tests()[name];
        let with_value: Vec<&Version> = (test.reads.iter())
            .filter_map(|b| self.current(b))
            .filter(|v| v.value.as_deref() == Some(test.is.as_str()))
            .collect();
        match test.kind {
            TestKind::All => with_value.len() == test.reads.len(),
            TestKind::Any => !with_value.is_empty(),
            TestKind::None => with_value.is_empty(),
            TestKind::NoneStanding => !with_value.iter().any(|v| self.stands(flow, v, i)),
        }
    }

    /// Whether the version stands for the cell at place `i`: unless it was written by a run of a cell before that place
    /// whose condition is not met now.
    pub fn stands(&self, flow: &Flow, v: &Version, i: usize) -> bool {
        match &v.by {
            Writer::Cell(cell, _) => match flow.place(cell) {
                Some(j) if j < i => self.applies(flow, j),
                _ => true,
            },
            Writer::Person | Writer::Outside => true,
        }
    }
}
