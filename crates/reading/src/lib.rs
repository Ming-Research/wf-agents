//! The histories' own reading of a work's records, written from the design tree and not from the walk: the versions of
//! each block, the meaning of each, what each cell consumed, what is new to it, what calls it is owed, which questions
//! hold the work and why a run would start (design document `data-and-context`). The properties are judged by this
//! reading, so that a fault in the walk's own reading is not repeated in the judging. It reads only the records and the
//! flow, with its named tests, never what the deciding code works out of them: it takes the deciding code's types and
//! calls none of its walk. It is read by every test that judges the deciding code by the records, the histories of
//! crate `decide`, those of the engineer's flow (crate `engine`, `tests/engineer`, which takes its file in) and the
//! exploration of the repository's own flow (crate `flow`, `tests/explored`), so it is one reading they share.

use std::collections::{BTreeMap, BTreeSet};

use decide::{
    BlockId, CallId, Called, Cause, Cell, CellId, Condition, Finished, Flow, Outsider, Questioned, Records, RunId, Seq,
    Started, TestKind, TestName, Write,
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
    /// The version at which what the block says last changed: this one, or an earlier one; none where the block has
    /// said nothing yet, as of a first version handed in as changing nothing.
    pub meaning: Option<Seq>,
    pub value: Option<String>,
    /// Whether it holds nothing.
    pub empty: bool,
}

impl Version {
    /// Whether this write changed what the block says.
    pub fn changed_meaning(&self) -> bool {
        self.meaning == Some(self.at)
    }
}

/// Why a run of a cell would start, as the entries say: a block that runs it is new to it, `was` being the version it
/// consumed and `worked` the version it last worked from; or the results of the calls it is owed are all in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reason {
    Changed {
        block: BlockId,
        was: Option<Seq>,
        worked: Option<Seq>,
    },
    Results {
        calls: Vec<CallId>,
    },
}

pub struct Reading<'a> {
    pub records: &'a Records,
    /// The blocks whose versions are entries added to a record, which replace none: the flow says which.
    pub entries: BTreeSet<BlockId>,
    /// Of each judge of the flow, each verdict it hands in with the dialogue it writes its points in, as the flow pairs
    /// them: (the judge, the verdict, the dialogue).
    pub rulings: Vec<(CellId, BlockId, BlockId)>,
    /// Every version of every block, in the order they were written.
    pub versions: BTreeMap<BlockId, Vec<Version>>,
    pub started: BTreeMap<RunId, &'a Started>,
    pub finished: BTreeMap<RunId, &'a Finished>,
}

impl<'a> Reading<'a> {
    /// The reading of these records. The meaning of each version follows decision meaning-told-by-the-writer: a
    /// version that has a value, or follows one that has, is a choice: it changes the meaning when the value differs
    /// from the one before, and only then, whoever writes it, the first version of a choice having its own. Otherwise
    /// a person's prose always changes the meaning; what the outside writes does unless it was approved as not
    /// mattering; a cell's write does when the cell says so, and when it says not, it is taken at its word only if the
    /// version it started from has the meaning of the version it writes over: a cell that did not see what it writes
    /// over is not taken at its word. A first version the outside or a cell hands in as changing nothing says nothing,
    /// and is new to no cell (design document `data-and-context`, 9). A judge's point the owner ruled on is new to no
    /// cell from the ruling on (39): the flow says which verdict is whose, and of which dialogue.
    pub fn new(flow: &Flow, records: &'a Records) -> Self {
        let started: BTreeMap<RunId, &Started> = records.started.iter().map(|s| (s.run, s)).collect();
        let finished: BTreeMap<RunId, &Finished> = records.finished.iter().map(|f| (f.run, f)).collect();
        struct Entry<'b> {
            at: Seq,
            block: &'b BlockId,
            by: Writer,
            matters: bool,
            value: Option<String>,
            empty: bool,
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
                empty: w.empty,
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
                    empty: w.empty,
                });
            }
        }
        entries.sort_by_key(|e| e.at);
        let mut versions: BTreeMap<BlockId, Vec<Version>> = BTreeMap::new();
        for e in entries {
            let of_block = versions.entry(e.block.clone()).or_default();
            let own = Some(e.at);
            // A write that holds nothing says nothing where no version is before it, where the one before holds nothing
            // too, and where the block adds entries, which replace none (design document `data-and-context`, 9).
            let nothing = e.empty && (flow.adds_entries(e.block) || of_block.last().is_none_or(|over| over.empty));
            let meaning = match of_block.last() {
                _ if nothing => of_block.last().and_then(|over| over.meaning),
                // Handed in as changing nothing, with no version before it, a version says nothing: a person's prose
                // and a choice excepted, whose meaning their writer does not tell.
                None if e.value.is_none() && e.by != Writer::Person && !e.matters => None,
                None => own,
                Some(over) if e.value.is_some() || over.value.is_some() => {
                    if e.value == over.value {
                        over.meaning
                    } else {
                        own
                    }
                }
                Some(over) => match &e.by {
                    Writer::Person => own,
                    Writer::Outside if e.matters => own,
                    Writer::Outside => over.meaning,
                    Writer::Cell(..) if e.matters => own,
                    Writer::Cell(_, run) => {
                        let from = started[run].basis.get(e.block).copied().flatten();
                        let seen = from
                            .and_then(|v| of_block.iter().find(|x| x.at == v))
                            .map(|x| x.meaning);
                        if seen == Some(over.meaning) { over.meaning } else { own }
                    }
                },
            };
            of_block.push(Version {
                at: e.at,
                by: e.by,
                meaning,
                value: e.value,
                empty: e.empty,
            });
        }
        let rulings = (flow.cells().iter())
            .flat_map(|c| (c.verdicts.iter()).map(|(verdict, of)| (c.id.clone(), verdict.clone(), of.dialogue.clone())))
            .collect();
        let entries = (flow.cells().iter())
            .flat_map(|c| c.outputs.iter().chain(&c.runs_on))
            .filter(|b| flow.adds_entries(b))
            .cloned()
            .collect();
        Self {
            records,
            entries,
            rulings,
            versions,
            started,
            finished,
        }
    }

    /// Whether this version of the block, a judge's point in its dialogue, was ruled on before this place: a person
    /// wrote the judge's verdict after the point (design document `data-and-context`, 39).
    pub fn ruled(&self, block: &BlockId, version: &Version, place: Seq) -> bool {
        (self.rulings.iter())
            .filter(|(_, _, dialogue)| dialogue == block)
            .any(|(judge, verdict, _)| {
                matches!(&version.by, Writer::Cell(c, _) if c == judge)
                    && (self.of(verdict).iter()).any(|w| w.by == Writer::Person && w.at > version.at && w.at < place)
            })
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

    /// Whether this write, handed in by the run, would change what the block says over its current version, by the
    /// reading's own rule of meaning ([`Reading::new`]): a choice where its value differs from the one before; prose
    /// that the run says matters, or says not where the version its round was last told of has not the meaning of the
    /// one it would write over; and a first version but one handed in as changing nothing, which says nothing (design
    /// document `data-and-context`, 9). A version put in to the round after it began is told of, as the one it started
    /// from is: a return sent back for writing over a version it was not told of then says what it says over that
    /// version.
    pub fn would_change(&self, run: RunId, write: &Write) -> bool {
        let over = self.current(&write.block);
        // A write that holds nothing says nothing where none is before it, where the one before holds nothing too, and
        // where the block adds entries.
        if write.empty && (self.entries.contains(&write.block) || over.is_none_or(|over| over.empty)) {
            return false;
        }
        let Some(over) = over else {
            return write.value.is_some() || !write.meaning_unchanged;
        };
        if write.value.is_some() || over.value.is_some() {
            return write.value != over.value;
        }
        if !write.meaning_unchanged {
            return true;
        }
        let seen = self
            .told(run, &write.block)
            .and_then(|v| self.of(&write.block).iter().find(|x| x.at == v))
            .map(|x| x.meaning);
        seen != Some(over.meaning)
    }

    /// The latest version of the block the run was told of before this place: the one it started from, or one put in
    /// to it.
    pub fn told_before(&self, run: RunId, block: &BlockId, place: Seq) -> Option<Seq> {
        let began = self.started[&run].basis.get(block).copied().flatten();
        let put_in = (self.records.put_in.iter())
            .filter(|p| p.run == run && &p.block == block && p.at < place)
            .map(|p| p.version);
        began.into_iter().chain(put_in).max()
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
    /// changing what the block says, not written by a run of the cell itself, and no judge's point ruled on.
    pub fn new_to(&self, cell: &CellId, block: &BlockId, place: Seq) -> Vec<&Version> {
        let was = self.consumed(cell, block, place);
        (self.of(block).iter())
            .filter(|v| v.at < place && Some(v.at) > was && v.changed_meaning())
            .filter(|v| !matches!(&v.by, Writer::Cell(c, _) if c == cell))
            .filter(|v| !self.ruled(block, v, place))
            .collect()
    }

    /// The version of the block the cell last worked from before this place: the latest of the one it consumed, one a
    /// run of it wrote, and one the owner ruled on. What the cell wrote is no change to it, and a point the owner ruled
    /// on is new to no cell (design document `data-and-context`, 10, 39), so each counts as seen.
    pub fn worked(&self, cell: &CellId, block: &BlockId, place: Seq) -> Option<Seq> {
        let own_or_ruled = (self.of(block).iter())
            .filter(|v| v.at < place)
            .filter(|v| matches!(&v.by, Writer::Cell(c, _) if c == cell) || self.ruled(block, v, place))
            .map(|v| v.at)
            .max();
        self.consumed(cell, block, place).max(own_or_ruled)
    }

    /// Whether, at this place, a point is new to the cell in a dialogue it holds as the maker while the dialogue is
    /// open (design document `data-and-context`, 36, 41): a version new to it that a run of the other side wrote, a
    /// cell of the flow for which the dialogue is none it makes. A judge's dialogue is open while the verdict the flow
    /// pairs with it stands at the value that does not pass; a doubt, which has no verdict, while its latest version
    /// is the other side's.
    pub fn point(&self, flow: &Flow, cell: &CellId, dialogue: &BlockId, place: Seq) -> bool {
        let makes = |c: &CellId| (flow.cells().iter()).any(|x| &x.id == c && x.dialogues.contains_key(dialogue));
        let other = |v: &Version| matches!(&v.by, Writer::Cell(c, _) if !makes(c));
        let point = self.new_to(cell, dialogue, place).into_iter().any(other);
        let verdict = (flow.cells().iter())
            .flat_map(|c| &c.verdicts)
            .find(|(_, of)| &of.dialogue == dialogue);
        let open = match verdict {
            Some((verdict, of)) => {
                self.current_before(verdict, place).and_then(|v| v.value.as_deref()) == Some(of.fails.as_str())
            }
            None => self.current_before(dialogue, place).is_some_and(other),
        };
        point && open
    }

    /// What is new to the cell at this place: each block that runs it with a version new to it, in the order the cell
    /// names them.
    pub fn reasons(&self, cell: &Cell, place: Seq) -> Vec<Reason> {
        (cell.runs_on.iter())
            .filter(|block| !self.new_to(&cell.id, block, place).is_empty())
            .map(|block| Reason::Changed {
                block: block.clone(),
                was: self.consumed(&cell.id, block, place),
                worked: self.worked(&cell.id, block, place),
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

    /// The questions that hold the work before this place, each by its name with the cell it holds: its latest version
    /// before the place asks, its cell waits for the answer, and no answer to that version was given before the place.
    /// A question is the cell's of its latest version, and an answer to an earlier version answers it no more (design
    /// document `data-and-context`, 30, 32).
    pub fn holding(&self, place: Seq) -> Vec<(&str, &CellId)> {
        let mut latest: BTreeMap<&str, &Questioned> = BTreeMap::new();
        for q in self.records.questions.iter().filter(|q| q.at < place) {
            let kept = latest.entry(q.name.as_str()).or_insert(q);
            if q.at > kept.at {
                *kept = q;
            }
        }
        let answered = |name: &str, q: &Questioned| {
            (self.records.answers.iter()).any(|a| a.question == name && a.version == q.at && a.at < place)
        };
        (latest.into_iter())
            .filter(|(name, q)| q.waits && !q.withdrawn && !answered(name, q))
            .map(|(name, q)| (name, &q.cell))
            .collect()
    }

    /// Whether a question of the cell holds the work at it before this place ([`Reading::holding`]).
    pub fn held(&self, cell: &CellId, place: Seq) -> bool {
        self.holding(place).iter().any(|(_, c)| *c == cell)
    }

    /// Why the cell would be entered at this place: what is new to it, and the results it is owed once all are in.
    /// While one has no result, or a question of its that it waits for holds it, only the owner turning to it (a block
    /// that interrupts: the owner's words or answer) enters it, by a version no run of the cell started before the
    /// place and not abandoned before it was told; once every result is in, a cell its question holds is entered for
    /// them with every version of the owner's turning to it new to it (decision a-question-that-waits-holds-its-cell).
    pub fn entering(&self, flow: &Flow, cell: &Cell, place: Seq) -> Vec<Reason> {
        let mut reasons = self.reasons(cell, place);
        let owed = self.owed(&cell.id, place);
        let open = owed.iter().any(|c| !self.done(*c, place));
        if open || self.held(&cell.id, place) {
            let results = !open && !owed.is_empty();
            let abandoned = |run: RunId| (self.records.abandoned.iter()).any(|a| a.run == run && a.at < place);
            let seen = |block: &BlockId| {
                (self.started.values())
                    .filter(|s| s.cell == cell.id && s.at < place && !abandoned(s.run))
                    .filter_map(|s| self.told_before(s.run, block, place))
                    .max()
            };
            // The latest version before the place that the cell did not write itself: a dialogue the cell answers in
            // holds its own answer after what it was told, which is never new to it.
            let others = |block: &BlockId| {
                (self.of(block).iter().rev())
                    .filter(|v| v.at < place)
                    .find(|v| !matches!(&v.by, Writer::Cell(c, _) if *c == cell.id))
                    .map(|v| v.at)
            };
            reasons.retain(|r| {
                matches!(r, Reason::Changed { block, .. }
                    if flow.interrupts(block) && (results || others(block) > seen(block)))
            });
            if results {
                reasons.push(Reason::Results { calls: owed });
            }
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

    /// Whether one key of the condition or of the preconditions of the cell at place `i` holds now: no block keeps it
    /// from holding, a block never written having its default where the flow gives one.
    pub fn key_holds(&self, flow: &Flow, key: &Condition, i: usize) -> bool {
        self.keeping(flow, key, i).is_empty()
    }

    /// Whether the named test holds now for the cell at place `i`: of the blocks it reads, all must have the value it
    /// tests for, or one, or none, a block never written having its default where the flow gives one; a test of what
    /// stands counts a block that holds the cell back as having it, and a test of content one whose current version
    /// holds anything.
    pub fn holds(&self, flow: &Flow, name: &TestName, i: usize) -> bool {
        self.keeping_test(flow, name, i).is_empty()
    }

    /// Whether the block, which a test of what stands reads for the value `is`, holds the cell at place `i` back, said
    /// from the records and what the rule is for, not from how the walk reads it: a judge's word holds the work while
    /// the judge applies, and a person's while it is the latest. A person's or the outside's write is the latest and
    /// has the value tested for; or a cell before place `i` that applies now has that value as its own last write of
    /// the block, and no person and nothing outside wrote the block since.
    fn refuses(&self, flow: &Flow, is: Option<&str>, block: &BlockId, i: usize) -> bool {
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
    }
}

/// Where a work stands, read from its records and its flow alone: whether something is still to happen by itself, or it
/// waits for someone, or it has come to a stop that nothing but a person or a source outside the work can end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rest {
    /// The work has ended: its last cell has a run that finished, or it was cancelled and nothing of it is open.
    Ended,
    /// A round is under way or a run goes on or is abandoned, or a cell has something new to run it and nothing holds
    /// it back but, at most, the pause, a slot or the work's turn: at this cell.
    Moving { at: CellId },
    /// Nothing runs, and a person, the owner, a source outside the work or the result of a call can bring something
    /// new: a question stands unanswered, a call's result is to come, a cell is at its limit, or a precondition waits
    /// for a block that a person or a source outside the work writes, or that a cell has said waits for one. It waits
    /// at this cell.
    Waiting { at: CellId },
    /// Nothing runs, nothing is to come from a call, and each block a precondition waits for is written only by cells,
    /// none of which will run: where the work stands, the first cell that applies whose preconditions do not hold, or,
    /// where there is none, its last cell, which nothing new runs; and the keys that do not hold there.
    Stalled { at: CellId, on: Vec<Condition> },
}

impl Reading<'_> {
    /// The cell's runs that finished before this place, started since the owner last let it go on, in the order they
    /// finished.
    pub fn finished_since_let_go(&self, cell: &CellId, place: Seq) -> Vec<&Started> {
        let let_go = (self.records.let_go.iter())
            .filter(|l| &l.cell == cell && l.at < place)
            .map(|l| l.at)
            .max();
        let mut runs: Vec<(Seq, &Started)> = (self.finished.values())
            .filter(|f| f.at < place)
            .map(|f| (f.at, self.started[&f.run]))
            .filter(|(_, s)| &s.cell == cell && let_go.is_none_or(|l| s.at > l))
            .collect();
        runs.sort_by_key(|(at, _)| *at);
        runs.into_iter().map(|(_, s)| s).collect()
    }

    /// How many of the cell's last finished runs before this place, since the owner last let it go on, were in a row
    /// ways round that dealt with something: a run one of whose reasons someone other than a cell after it wrote ends
    /// the row, and one that dealt with nothing or was entered for no block is passed over.
    pub fn ways_round_in_a_row(&self, flow: &Flow, i: usize, place: Seq) -> usize {
        let cell = &flow.cells()[i];
        let mut count = 0;
        for s in self.finished_since_let_go(&cell.id, place).into_iter().rev() {
            let forward = s.because.iter().any(|c| match c {
                Cause::NoRecord => true,
                Cause::Changed { behind, .. } => !behind,
                Cause::Results { .. } => false,
            });
            if forward {
                break;
            }
            let changed = s.because.iter().any(|c| matches!(c, Cause::Changed { .. }));
            let dealt = self.finished[&s.run].dealt_with.as_ref().is_none_or(|d| !d.is_empty());
            if changed && dealt {
                count += 1;
            }
        }
        count
    }

    /// How many of the cell's last finished runs before this place, since the owner last let it go on, each asked for
    /// calls and dealt with nothing, in a row.
    pub fn asked_in_a_row(&self, cell: &CellId, place: Seq) -> usize {
        (self.finished_since_let_go(cell, place).into_iter().rev())
            .take_while(|s| {
                let f = self.finished[&s.run];
                let ended = (self.records.returned.iter())
                    .filter(|e| e.run == s.run)
                    .map(|e| e.at)
                    .max();
                let asked = (self.records.called.iter())
                    .any(|c| c.run == s.run && c.at < f.at && ended.is_none_or(|e| c.at > e));
                f.dealt_with.as_ref().is_some_and(Vec::is_empty) && asked
            })
            .count()
    }

    /// Whether the cell at place `i`, entered for these reasons, is at a limit: its next run would be a way round after
    /// as many in a row as its limit, or would be for results after as many runs in a row that only asked as its limit
    /// of calls.
    pub fn at_limit(&self, flow: &Flow, i: usize, reasons: &[Reason]) -> bool {
        let now = Seq(u64::MAX);
        let cell = &flow.cells()[i];
        let ways =
            self.way_round(flow, i, reasons, now) && self.ways_round_in_a_row(flow, i, now) >= cell.limit as usize;
        let calls = reasons.iter().any(|r| matches!(r, Reason::Results { .. }))
            && self.asked_in_a_row(&cell.id, now) >= cell.calls as usize;
        ways || calls
    }

    /// Where the work stands ([`Rest`]), as the design tree says it. From the first cell in the order, a cell whose
    /// condition does not hold is passed, its questions holding nothing. Where a run of the cell is at rest, the work
    /// waits for its calls and its preconditions, and else the run goes on. Else a cell with a result to come or a
    /// question it waits for holds the work, the owner turning to it excepted; a precondition that does not hold holds
    /// the work at its cell, whether or not something new runs it; and a cell with something new runs, at its limit
    /// waiting for the owner. A work held at a precondition is stalled where each block it waits for is written by
    /// cells alone, and none of them said what it wrote waits for a source outside the work (decisions
    /// a-work-no-cell-can-move-is-stalled and what-a-run-wrote-may-wait-for-the-outside). A precondition that only a
    /// source outside the work can make hold, of a cell a run of which has finished, does not stop the walk at its
    /// cell: it holds back what the walk comes to after it, which waits where it stands, for whatever else it waits for
    /// besides (decision dependencies-hold-a-started-work-where-it-would-run).
    pub fn rest(&self, flow: &Flow) -> Rest {
        let now = Seq(u64::MAX);
        let cells = flow.cells();
        let records = self.records;
        let last = &cells[cells.len() - 1].id;
        let abandoned = |run: RunId| records.abandoned.iter().any(|a| a.run == run);
        let open = |s: &Started| !self.finished.contains_key(&s.run) && !abandoned(s.run);
        if self
            .started
            .values()
            .any(|s| &s.cell == last && self.finished.contains_key(&s.run))
        {
            return Rest::Ended;
        }
        if records.cancelled().is_some() {
            return match self.started.values().find(|s| open(s)) {
                Some(s) => Rest::Moving { at: s.cell.clone() },
                None => Rest::Ended,
            };
        }
        // A round under way, even of a run being stopped, ends before anything else begins.
        let under_way = |run: RunId| {
            let began = (records.rounds.iter()).filter(|r| r.run == run).map(|r| r.at).max();
            let ended = (records.returned.iter()).filter(|r| r.run == run).map(|r| r.at).max();
            began > ended
        };
        if let Some(s) = self.started.values().find(|s| under_way(s.run)) {
            return Rest::Moving { at: s.cell.clone() };
        }
        let at_rest = |cell: &CellId| self.started.values().find(|s| &s.cell == cell && open(s));
        // A run at rest of a cell that no longer applies is abandoned first.
        for s in self.started.values().filter(|s| open(s)) {
            if flow.place(&s.cell).is_none_or(|i| !self.applies(flow, i)) {
                return Rest::Moving { at: s.cell.clone() };
            }
        }
        let mut held = false;
        for (i, cell) in cells.iter().enumerate() {
            let waiting = || Rest::Waiting { at: cell.id.clone() };
            // A cell that is skipped holds nothing, its questions included.
            if !self.applies(flow, i) {
                continue;
            }
            // What a source outside the work holds back, of the cells passed, is waited for wherever the walk comes to.
            let (gate, holds) = self.gate(flow, i);
            held |= holds;
            let held = held;
            let stands = |rest: Rest| if held { waiting() } else { rest };
            // The calls with no result yet: a call for a person, the owner or the architect is waited for; one the
            // engine makes itself is not, the work moves on as the engine makes it. What the preconditions that do
            // not hold make of the work counts beside them: a person's word still to come is waited for.
            let calls = |open: Vec<&Called>| {
                if open.is_empty() {
                    None
                } else if open.iter().any(|c| c.on.is_some()) {
                    Some(waiting())
                } else {
                    Some(Rest::Moving { at: cell.id.clone() })
                }
            };
            let with_gate = |calls: Option<Rest>, gate: Option<Rest>| match (calls, gate) {
                (None, gate) => gate,
                (calls, None) => calls,
                (Some(calls), Some(gate)) => Some(if matches!(gate, Rest::Waiting { .. }) {
                    gate
                } else {
                    calls
                }),
            };
            let owed = self.owed(&cell.id, now);
            let owed_open = calls(
                (records.called.iter())
                    .filter(|c| owed.contains(&c.call) && !self.done(c.call, now))
                    .collect(),
            );
            if let Some(run) = at_rest(&cell.id) {
                let open = (records.called.iter())
                    .filter(|c| c.run == run.run && !self.done(c.call, now))
                    .collect();
                return stands(with_gate(calls(open), gate).unwrap_or(Rest::Moving { at: cell.id.clone() }));
            }
            let reasons = self.entering(flow, cell, now);
            if reasons.is_empty() {
                if self.held(&cell.id, now) {
                    return waiting();
                }
                if let Some(rest) = with_gate(owed_open, gate) {
                    return stands(rest);
                }
                continue;
            }
            if let Some(gate) = gate {
                return stands(with_gate(owed_open, Some(gate)).unwrap_or(Rest::Moving { at: cell.id.clone() }));
            }
            return if self.at_limit(flow, i, &reasons) || held {
                waiting()
            } else {
                Rest::Moving { at: cell.id.clone() }
            };
        }
        Rest::Stalled {
            at: last.clone(),
            on: Vec::new(),
        }
    }

    /// What the cell at place `i`'s preconditions that do not hold make of the work: none where they all hold;
    /// waiting, where a block that keeps one from holding is written by a person, the owner or a source outside the
    /// work, or each cell that writes it said that what it wrote waits for one; else stalled on those keys. Whether
    /// the cell has been passed, and a key of its preconditions that does not hold is one only a source outside the
    /// work can make hold: it is not of the first kind, the work not standing at the cell for it, and is said apart.
    fn gate(&self, flow: &Flow, i: usize) -> (Option<Rest>, bool) {
        let cell = &flow.cells()[i];
        let passed = self.finished.values().any(|f| self.started[&f.run].cell == cell.id);
        let mut stalled = Vec::new();
        let mut waiting = false;
        let mut held = false;
        for key in &cell.requires {
            let keeping = self.keeping(flow, key, i);
            if keeping.is_empty() {
                continue;
            }
            let only_outside = keeping.iter().all(|block| {
                flow.outsiders(block)
                    .is_some_and(|o| !o.people && !o.by_owner && o.outside.is_some())
                    && flow.writers(block).next().is_none()
            });
            if passed && only_outside {
                held = true;
                continue;
            }
            let mut writers: Vec<&CellId> = Vec::new();
            let mut outsider = false;
            for block in &keeping {
                if let Some(o) = flow.outsiders(block) {
                    outsider |= o.people || o.by_owner || o.outside.is_some();
                }
                writers.extend(flow.writers(block).map(|c| &c.id));
            }
            writers.sort();
            writers.dedup();
            let outside = !writers.is_empty() && writers.iter().all(|w| self.waits_for_outside(flow, w, &keeping));
            if outsider || outside {
                waiting = true;
            }
            stalled.push(key.clone());
        }
        if stalled.is_empty() {
            return (None, held);
        }
        if waiting {
            return (Some(Rest::Waiting { at: cell.id.clone() }), held);
        }
        (
            Some(Rest::Stalled {
                at: cell.id.clone(),
                on: stalled,
            }),
            held,
        )
    }

    /// The blocks that keep this key of the cell at place `i` from holding, by the value each has now: none where it
    /// holds. A block never written has its default where the flow gives one.
    fn keeping<'k>(&self, flow: &'k Flow, key: &'k Condition, i: usize) -> Vec<&'k BlockId> {
        match key {
            Condition::Is { block, value } => {
                let now = match self.current(block) {
                    Some(v) => v.value.as_deref(),
                    None => flow.default_of(block),
                };
                if now == Some(value.as_str()) {
                    Vec::new()
                } else {
                    vec![block]
                }
            }
            Condition::Test(name) => self.keeping_test(flow, name, i),
        }
    }

    /// The blocks the named test reads that keep it from holding, for the cell at place `i`, by the value each has now:
    /// of the blocks it reads, all must have the value it tests for, or one, or none; a test of what stands counts a
    /// block that holds the cell back ([`Reading::refuses`]) as having it, and a test of content one whose current
    /// version holds anything. A block never written has its default where the flow gives one, which no cell wrote and
    /// so stands, and holds no content.
    fn keeping_test<'k>(&self, flow: &'k Flow, name: &TestName, i: usize) -> Vec<&'k BlockId> {
        let test = &flow.tests()[name];
        let is = test.is.as_deref();
        let has = |block: &BlockId| match (test.kind, is) {
            (TestKind::AnyContent, _) => self.current(block).is_some_and(|v| !v.empty),
            (_, None) => false,
            (TestKind::NoneStanding, Some(_)) => {
                self.refuses(flow, is, block, i) || (self.current(block).is_none() && flow.default_of(block) == is)
            }
            (_, Some(is)) => match self.current(block) {
                Some(v) => v.value.as_deref() == Some(is),
                None => flow.default_of(block) == Some(is),
            },
        };
        let reads = test.reads.iter();
        match test.kind {
            TestKind::All => reads.filter(|b| !has(b)).collect(),
            TestKind::Any | TestKind::AnyContent if test.reads.iter().any(&has) => Vec::new(),
            TestKind::Any | TestKind::AnyContent => reads.collect(),
            TestKind::None | TestKind::NoneStanding => reads.filter(|b| has(b)).collect(),
        }
    }

    /// Whether the cell's latest finished run said that what it wrote waits for a new version of a block a source
    /// outside the work writes, none being new to the cell yet, and wrote one of these blocks.
    fn waits_for_outside(&self, flow: &Flow, cell: &CellId, keeping: &[&BlockId]) -> bool {
        let Some(latest) = (self.finished.values())
            .filter(|f| &self.started[&f.run].cell == cell)
            .max_by_key(|f| f.at)
        else {
            return false;
        };
        let Some(waits) = latest.waits_for.as_ref() else {
            return false;
        };
        flow.outsiders(waits).is_some_and(|o| o.outside.is_some())
            && self.new_to(cell, waits, Seq(u64::MAX)).is_empty()
            && latest.writes.iter().any(|w| keeping.contains(&&w.block))
    }
}
