//! The histories' own reading of a work's records as they were at a place, written from the entries of the design and
//! not from the walk: the versions of each block with their meaning, the current value of each, whether a cell's
//! condition holds, and each cell's record. The properties are judged by this reading, never by what the deciding code
//! works out, so that a fault of the walk's reading is not repeated in the judging.

use std::collections::BTreeMap;

use decide::{BlockId, CellId, Condition, Finished, Flow, Outsider, Records, RunId, Seq, Started, TestKind, TestName};

/// Who wrote a version of a block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Writer {
    Person,
    Outside,
    /// A run, as it finished.
    Run(RunId),
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

pub struct Reading<'a> {
    flow: &'a Flow,
    /// Every version of every block written before the place, in the order they were written.
    versions: BTreeMap<BlockId, Vec<Version>>,
    started: BTreeMap<RunId, &'a Started>,
    /// The runs that finished before the place, in the order they finished.
    finished: Vec<&'a Finished>,
}

impl<'a> Reading<'a> {
    /// The reading of the entries placed before `before`, or of all of them. The meaning of each version follows the
    /// design of data and context: the first version of a block has its own. A version that has a value, or follows
    /// one that has, is a choice: it changes the meaning when its value differs from the one before, and only then. A
    /// person's prose always changes the meaning; what the outside writes does when it matters; a run's write does
    /// when the run says so, and when it says not, it is taken at its word only if the version it started from has the
    /// meaning of the version it writes over.
    pub fn new(flow: &'a Flow, records: &'a Records, before: Option<Seq>) -> Self {
        let counts = |at: Seq| before.is_none_or(|b| at < b);
        let started: BTreeMap<RunId, &Started> = (records.started.iter())
            .filter(|s| counts(s.at))
            .map(|s| (s.run, s))
            .collect();
        let mut finished: Vec<&Finished> = records.finished.iter().filter(|f| counts(f.at)).collect();
        finished.sort_by_key(|f| f.at);
        struct Entry<'b> {
            at: Seq,
            block: &'b BlockId,
            by: Writer,
            matters: bool,
            value: Option<String>,
        }
        let mut entries: Vec<Entry> = Vec::new();
        for w in records.written.iter().filter(|w| counts(w.at)) {
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
        for f in &finished {
            for w in &f.writes {
                entries.push(Entry {
                    at: f.at,
                    block: &w.block,
                    by: Writer::Run(f.run),
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
                    _ if e.matters => e.at,
                    Writer::Outside => over.meaning,
                    Writer::Run(run) => {
                        let from = started[run].basis.get(e.block).copied().flatten();
                        let seen = (from.and_then(|v| of_block.iter().find(|x| x.at == v))).map(|x| x.meaning);
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
            flow,
            versions,
            started,
            finished,
        }
    }

    /// The versions of the block, in the order they were written.
    pub fn of(&self, block: &BlockId) -> &[Version] {
        self.versions.get(block).map_or(&[], Vec::as_slice)
    }

    /// The current version of the block: the latest written.
    pub fn current(&self, block: &BlockId) -> Option<&Version> {
        self.of(block).last()
    }

    /// The block's value now, where it has one.
    pub fn value(&self, block: &BlockId) -> Option<&str> {
        self.current(block).and_then(|v| v.value.as_deref())
    }

    /// The meaning of the version of the block written at this place.
    pub fn meaning_of(&self, block: &BlockId, version: Option<Seq>) -> Option<Seq> {
        let at = version?;
        self.of(block).iter().find(|v| v.at == at).map(|v| v.meaning)
    }

    /// The meaning the block has now.
    pub fn meaning_now(&self, block: &BlockId) -> Option<Seq> {
        self.current(block).map(|v| v.meaning)
    }

    /// The run's start.
    pub fn started(&self, run: RunId) -> &Started {
        self.started[&run]
    }

    /// The cell whose run wrote the version, where a run did.
    pub fn cell_of(&self, v: &Version) -> Option<&CellId> {
        match v.by {
            Writer::Run(run) => Some(&self.started[&run].cell),
            _ => None,
        }
    }

    /// The cell's record: its latest finished run.
    pub fn record(&self, cell: &CellId) -> Option<&Finished> {
        (self.finished.iter().rev())
            .find(|f| &self.started[&f.run].cell == cell)
            .copied()
    }

    /// The runs that finished, in the order they finished.
    pub fn finished(&self) -> &[&'a Finished] {
        &self.finished
    }

    /// Whether the condition of the cell at this place holds now: with none it always does.
    pub fn applies(&self, i: usize) -> bool {
        match &self.flow.cells()[i].applies {
            None => true,
            Some(Condition::Is { block, value }) => self.value(block) == Some(value.as_str()),
            Some(Condition::Test(name)) => self.holds(name),
        }
    }

    /// Whether the named test holds now, by the values of the blocks it reads.
    pub fn holds(&self, name: &TestName) -> bool {
        let test = &self.flow.tests()[name];
        let mut has = (test.reads.iter()).map(|block| {
            self.current(block).is_some_and(|v| {
                v.value.as_deref() == Some(test.is.as_str()) && (test.kind != TestKind::NoneStanding || self.stands(v))
            })
        });
        match test.kind {
            TestKind::All => has.all(|h| h),
            TestKind::Any => has.any(|h| h),
            TestKind::None | TestKind::NoneStanding => !has.any(|h| h),
        }
    }

    /// Whether the version stands: unless a run of a cell that is skipped now wrote it. What a person or the outside
    /// wrote always stands, and so does what a cell no longer in the flow wrote.
    pub fn stands(&self, v: &Version) -> bool {
        match self.cell_of(v).and_then(|c| self.flow.place(c)) {
            Some(j) => self.applies(j),
            None => true,
        }
    }
}
