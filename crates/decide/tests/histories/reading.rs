//! The histories' own reading of a work's records, written from the design tree and not from the walk: the
//! versions of each block, the meaning of each, what each cell consumed, what is new to it, what calls it is owed and
//! why a run would start (design document `data-and-context`). The properties are judged by this reading, so that a
//! fault in the walk's own reading is not repeated in the judging. It reads only the records and the flow, with its
//! named tests, never what the deciding code works out of them. The exploration of the repository's own flow (crate
//! `flow`, `tests/explored`) judges its properties by it too.

use std::collections::BTreeMap;

use decide::{
    BlockId, CallId, Cause, Cell, CellId, Condition, Finished, Flow, Outsider, Records, RunId, Seq, Started, TestKind,
    TestName,
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

/// Why a run of a cell would start, as the entries say: a block that runs it is new to it, `was` being the version it
/// consumed; or the results of the calls it is owed are all in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reason {
    Changed { block: BlockId, was: Option<Seq> },
    Results { calls: Vec<CallId> },
}

pub struct Reading<'a> {
    pub records: &'a Records,
    /// Every version of every block, in the order they were written.
    pub versions: BTreeMap<BlockId, Vec<Version>>,
    pub started: BTreeMap<RunId, &'a Started>,
    pub finished: BTreeMap<RunId, &'a Finished>,
}

impl<'a> Reading<'a> {
    /// The reading of these records. The meaning of each version follows decision meaning-told-by-the-writer: the first
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
                    matters: !w.meaning_unchanged,
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

    /// The version of the block that was current just before this place.
    pub fn current_before(&self, block: &BlockId, place: Seq) -> Option<&Version> {
        self.of(block).iter().rev().find(|v| v.at < place)
    }

    /// The block's current version.
    pub fn current(&self, block: &BlockId) -> Option<&Version> {
        self.of(block).last()
    }

    /// The latest version of the block the run was told of: the one it started from, or one put in to it.
    pub fn told(&self, run: RunId, block: &BlockId) -> Option<Seq> {
        let began = self.started[&run].basis.get(block).copied().flatten();
        let put_in = (self.records.put_in.iter())
            .filter(|p| p.run == run && &p.block == block)
            .map(|p| p.version);
        began.into_iter().chain(put_in).max()
    }

    /// Whether the run finished before this place saying it dealt with the block: its return named it, or, where it did
    /// not say, the run began from it.
    pub fn dealt_with(&self, run: RunId, block: &BlockId, place: Seq) -> bool {
        let Some(f) = self.finished.get(&run).filter(|f| f.at < place) else {
            return false;
        };
        match &f.dealt_with {
            Some(named) => named.contains(block),
            None => self.started[&run].basis.contains_key(block),
        }
    }

    /// The version of the block the cell consumed before this place: the latest one a run of it that finished dealing
    /// with the block was told of. Nothing else consumes: no run that failed, was sent back, stopped or abandoned.
    pub fn consumed(&self, cell: &CellId, block: &BlockId, place: Seq) -> Option<Seq> {
        (self.started.values())
            .filter(|s| &s.cell == cell && self.dealt_with(s.run, block, place))
            .filter_map(|s| self.told(s.run, block))
            .max()
    }

    /// The versions of the block written before this place that are new to the cell: after the one it consumed,
    /// changing what the block says, and not written by a run of the cell itself.
    pub fn new_to(&self, cell: &CellId, block: &BlockId, place: Seq) -> Vec<&Version> {
        let was = self.consumed(cell, block, place);
        (self.of(block).iter())
            .filter(|v| v.at < place && Some(v.at) > was && v.changed_meaning())
            .filter(|v| !matches!(&v.by, Writer::Cell(c, _) if c == cell))
            .collect()
    }

    /// What is new to the cell at this place: each block that runs it with a version new to it, in the order the cell
    /// names them.
    pub fn reasons(&self, cell: &Cell, place: Seq) -> Vec<Reason> {
        (cell.runs_on.iter())
            .filter(|block| !self.new_to(&cell.id, block, place).is_empty())
            .map(|block| Reason::Changed {
                block: block.clone(),
                was: self.consumed(&cell.id, block, place),
            })
            .collect()
    }

    /// The calls the cell is owed the results of before this place, in the order they were made: each one a run of it
    /// asked for as its return was taken, after its last round ended and before it finished, that no run of the cell
    /// that finished since was given.
    pub fn owed(&self, cell: &CellId, place: Seq) -> Vec<CallId> {
        let given: Vec<CallId> = (self.finished.values())
            .filter(|f| f.at < place && &self.started[&f.run].cell == cell)
            .flat_map(|f| &self.started[&f.run].because)
            .flat_map(|c| match c {
                Cause::Results { calls } => calls.clone(),
                _ => Vec::new(),
            })
            .collect();
        let mut owed: Vec<(Seq, CallId)> = (self.records.called.iter())
            .filter(|c| c.at < place && !given.contains(&c.call))
            .filter(|c| {
                let Some(f) = self.finished.get(&c.run).filter(|f| f.at < place) else {
                    return false;
                };
                let ended = (self.records.returned.iter())
                    .filter(|e| e.run == c.run)
                    .map(|e| e.at)
                    .max();
                &self.started[&c.run].cell == cell && c.at < f.at && ended.is_none_or(|e| c.at > e)
            })
            .map(|c| (c.at, c.call))
            .collect();
        owed.sort();
        owed.into_iter().map(|(_, call)| call).collect()
    }

    /// Whether the call has its result before this place.
    pub fn done(&self, call: CallId, place: Seq) -> bool {
        self.records.done.iter().any(|d| d.call == call && d.at < place)
    }

    /// Why the cell would be entered at this place: what is new to it, and the results it is owed once all are in.
    /// While one has no result, only the owner turning to it (a block that interrupts) since its last run began enters
    /// it (decision the-owner-turning-to-a-waiting-cell-enters-it-once).
    pub fn entering(&self, flow: &Flow, cell: &Cell, place: Seq) -> Vec<Reason> {
        let mut reasons = self.reasons(cell, place);
        let owed = self.owed(&cell.id, place);
        if owed.iter().any(|c| !self.done(*c, place)) {
            let last = (self.started.values())
                .filter(|s| s.cell == cell.id && s.at < place)
                .map(|s| s.at)
                .max();
            reasons.retain(|r| {
                matches!(r, Reason::Changed { block, .. }
                    if flow.interrupts(block)
                        && self.current_before(block, place).is_some_and(|v| Some(v.at) > last))
            });
        } else if !owed.is_empty() {
            reasons.push(Reason::Results { calls: owed });
        }
        reasons
    }

    /// Whether every version of the block new to the cell at place `i` of the flow, before `place`, was written by a
    /// cell after it in the order (decision every-way-round-has-a-limit). One a person, the outside or a cell before it
    /// wrote makes it no way round.
    pub fn behind(&self, flow: &Flow, i: usize, block: &BlockId, place: Seq) -> bool {
        let cell = &flow.cells()[i].id;
        let new = self.new_to(cell, block, place);
        !new.is_empty()
            && new.iter().all(|v| match &v.by {
                Writer::Cell(c, _) => flow.place(c).is_some_and(|j| j > i),
                Writer::Person | Writer::Outside => false,
            })
    }

    /// Whether a run that starts at this place for these reasons is a way round: something new runs it, and every new
    /// version was written by a cell after it.
    pub fn way_round(&self, flow: &Flow, i: usize, reasons: &[Reason], place: Seq) -> bool {
        let changed: Vec<&BlockId> = (reasons.iter())
            .filter_map(|r| match r {
                Reason::Changed { block, .. } => Some(block),
                Reason::Results { .. } => None,
            })
            .collect();
        !changed.is_empty() && changed.iter().all(|b| self.behind(flow, i, b, place))
    }

    /// Whether the condition of the cell at place `i` of the flow is met now: every key of it holds; with none it
    /// always is.
    pub fn applies(&self, flow: &Flow, i: usize) -> bool {
        (flow.cells()[i].applies.iter()).all(|key| self.key_holds(flow, key, i))
    }

    /// Whether one key of the condition or of the preconditions of the cell at place `i` holds now: its named test, or
    /// the block's value, which is its default where the flow gives one and it was never written.
    pub fn key_holds(&self, flow: &Flow, key: &Condition, i: usize) -> bool {
        match key {
            Condition::Test(name) => self.holds(flow, name, i),
            Condition::Is { block, value } => {
                let now = match self.current(block) {
                    Some(v) => v.value.as_deref(),
                    None => flow.default_of(block),
                };
                now == Some(value.as_str())
            }
        }
    }

    /// Whether the named test holds now for the cell at place `i`, by the current versions of the blocks it reads that
    /// have the value it tests for: all of them, one, none. A test of what stands holds unless it is refused, as
    /// [`Reading::refused`] says from the records.
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
            TestKind::NoneStanding => !self.refused(flow, name, i),
        }
    }

    /// Whether a test of what stands holds the cell at place `i` back, said from the records and what the rule is for,
    /// not from how the walk reads it: a judge's word holds the work while the judge applies, and a person's while it
    /// is the latest. For some block the test reads, a person's or the outside's write is the latest and has the value
    /// tested for; or a cell before place `i` that applies now has that value as its own last write of the block, and
    /// no person and nothing outside wrote the block since.
    pub fn refused(&self, flow: &Flow, name: &TestName, i: usize) -> bool {
        let test = &flow.tests()[name];
        let is = Some(test.is.as_str());
        test.reads.iter().any(|block| {
            let versions = self.of(block);
            let outsider = |v: &Version| matches!(v.by, Writer::Person | Writer::Outside);
            let by_outsider = versions.last().is_some_and(|v| outsider(v) && v.value.as_deref() == is);
            let by_judge = (flow.cells()[..i].iter().enumerate()).any(|(j, c)| {
                let own = versions
                    .iter()
                    .rev()
                    .find(|v| matches!(&v.by, Writer::Cell(cell, _) if *cell == c.id));
                own.is_some_and(|own| {
                    own.value.as_deref() == is
                        && !versions.iter().any(|v| v.at > own.at && outsider(v))
                        && self.applies(flow, j)
                })
            });
            by_outsider || by_judge
        })
    }
}
