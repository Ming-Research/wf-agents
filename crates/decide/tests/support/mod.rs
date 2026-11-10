//! A work in memory, for the tests of the walk. It keeps the records as an engine would, only adding to them, and
//! takes from each decision exactly what the decision gives: why a run starts and what it starts from.

use std::collections::BTreeMap;

use decide::{
    Abandoned, CallId, Called, Cell, CellId, Decision, Done, Finished, Flow, LetGo, Outcome, Outsider, Pause, Records,
    Returned, Round, RunId, Seq, Stale, Started, TestName, Tests, Waited, Who, Write, Written, decide,
};

/// The named tests, each holding unless the test of the walk set it otherwise.
#[derive(Default)]
pub struct Named(pub BTreeMap<TestName, Outcome>);

impl Tests for Named {
    fn test(&self, name: &TestName) -> Outcome {
        self.0.get(name).cloned().unwrap_or(Outcome::Holds)
    }
}

/// What a run writes of a block: a change that matters to the cells that read it, one that does not, or a value a
/// condition reads (which matters).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Says {
    Matters,
    DoesNotMatter,
    Value(&'static str),
}

pub struct World {
    pub flow: Flow,
    pub records: Records,
    pub named: Named,
    seq: u64,
    ids: u64,
}

/// The merge a work rests at when nothing else is to be done: it waits for the owner's word.
pub fn gate() -> Cell {
    let mut merge = Cell::new("merge");
    merge.requires = vec!["the merge may go".into()];
    merge
}

/// The owner's word at the merge, as the walk reports waiting for it.
pub fn merge_word() -> Waited {
    Waited::Precondition {
        test: "the merge may go".into(),
        who: Who::Owner,
    }
}

/// What a work whose last cell is [`gate`] decides when nothing else is to be done: it waits for the owner's word.
pub fn resting() -> Decision {
    Decision::Wait {
        at: "merge".into(),
        on: vec![merge_word()],
    }
}

/// A cell that reads these blocks and may write those.
pub fn cell(id: &str, inputs: &[&str], outputs: &[&str]) -> Cell {
    let mut c = Cell::new(id);
    c.inputs = inputs.iter().map(|b| (*b).into()).collect();
    c.outputs = outputs.iter().map(|b| (*b).into()).collect();
    c
}

impl World {
    pub fn new(cells: Vec<Cell>) -> Self {
        let flow = Flow::new(cells).expect("the cells make a flow");
        Self {
            flow,
            records: Records::default(),
            named: Named::default(),
            seq: 0,
            ids: 0,
        }
    }

    /// A work whose last cell is a merge that waits for the owner: when nothing else is to be done the work rests
    /// there, and the test can go on changing things.
    pub fn with_gate(mut cells: Vec<Cell>) -> Self {
        cells.push(gate());
        let mut w = Self::new(cells);
        w.set("the merge may go", Outcome::Waits { who: Who::Owner });
        w
    }

    /// Installs a changed flow over the same records.
    pub fn install(&mut self, cells: Vec<Cell>) {
        self.flow = Flow::new(cells).expect("the cells make a flow");
    }

    pub fn tick(&mut self) -> Seq {
        self.seq += 1;
        Seq(self.seq)
    }

    fn id(&mut self) -> u64 {
        self.ids += 1;
        self.ids
    }

    pub fn decide(&self) -> Decision {
        decide(&self.flow, &self.records, &self.named)
    }

    /// Sets what a named test comes to.
    pub fn set(&mut self, test: &str, outcome: Outcome) {
        self.named.0.insert(test.into(), outcome);
    }

    /// Starts the cell, which the walk must have said to run, with exactly what the decision gives, and begins its
    /// first round.
    pub fn start(&mut self, cell: &str) -> RunId {
        let Decision::Run {
            cell: next,
            because,
            basis,
        } = self.decide()
        else {
            panic!("the walk does not run {cell}: {:?}", self.decide())
        };
        assert_eq!(next.as_str(), cell, "the walk runs another cell");
        let run = RunId(self.id());
        let at = self.tick();
        self.records.started.push(Started {
            run,
            cell: next,
            at,
            because,
            basis,
        });
        let at = self.tick();
        self.records.rounds.push(Round { run, at });
        run
    }

    /// The run of the cell that has started and neither finished nor been abandoned.
    pub fn open(&self, cell: &str) -> RunId {
        let id: CellId = cell.into();
        let r = &self.records;
        (r.started.iter().rev())
            .find(|s| {
                s.cell == id
                    && !r.finished.iter().any(|f| f.run == s.run)
                    && !r.abandoned.iter().any(|a| a.run == s.run)
            })
            .expect("the cell has a run that neither finished nor was abandoned")
            .run
    }

    fn under_way(&self, run: RunId) -> bool {
        let began = self.records.rounds.iter().filter(|r| r.run == run).count();
        let ended = self.records.returned.iter().filter(|r| r.run == run).count();
        began > ended
    }

    /// The round of the cell's run that is under way ends, the run neither finishing nor resting at a call: the walk
    /// will say to go on with it.
    pub fn returns(&mut self, cell: &str) {
        let run = self.open(cell);
        assert!(self.under_way(run), "no round of {cell} is under way");
        let at = self.tick();
        self.records.returned.push(Returned { run, at });
    }

    /// The round under way of the cell's run ends at a call: for this person, or one the engine makes (`None`).
    pub fn calls(&mut self, cell: &str, on: Option<Who>) -> CallId {
        self.returns(cell);
        let run = self.open(cell);
        let call = CallId(self.id());
        let at = self.tick();
        self.records.called.push(Called { call, run, on, at });
        call
    }

    /// The call has its result.
    pub fn done(&mut self, call: CallId) {
        let at = self.tick();
        self.records.done.push(Done { call, at });
    }

    /// Begins the next round of the cell's run, which the walk must have said to go on with.
    pub fn go_on(&mut self, cell: &str) {
        let Decision::GoOn { cell: next, run } = self.decide() else {
            panic!("the walk does not go on with {cell}: {:?}", self.decide())
        };
        assert_eq!(next.as_str(), cell, "the walk goes on with another cell");
        let at = self.tick();
        self.records.rounds.push(Round { run, at });
    }

    /// The cell's run, whose round under way ends here, finishes with these writes; the place of the entry names the
    /// versions it wrote.
    pub fn finish(&mut self, cell: &str, writes: &[(&str, Says)]) -> Seq {
        let run = self.open(cell);
        if self.under_way(run) {
            let at = self.tick();
            self.records.returned.push(Returned { run, at });
        }
        let at = self.tick();
        let writes = (writes.iter())
            .map(|(block, says)| Write {
                block: (*block).into(),
                matters: *says != Says::DoesNotMatter,
                value: match says {
                    Says::Value(v) => Some((*v).to_string()),
                    _ => None,
                },
            })
            .collect();
        self.records.finished.push(Finished { run, at, writes });
        at
    }

    /// Starts and finishes the cell; the place of its finish names the versions it wrote.
    pub fn run(&mut self, cell: &str, writes: &[(&str, Says)]) -> Seq {
        self.start(cell);
        self.finish(cell, writes)
    }

    /// Abandons the run the walk says to abandon, and says why; any other decision is a fault of the test.
    pub fn abandon(&mut self) -> Stale {
        let Decision::Abandon { run, because, .. } = self.decide() else {
            panic!("nothing to abandon: {:?}", self.decide())
        };
        let at = self.tick();
        self.records.abandoned.push(Abandoned { run, at });
        because
    }

    /// A person writes the block; the place of the entry names the version.
    pub fn person_writes(&mut self, block: &str) -> Seq {
        self.outsider_writes(block, Outsider::Person, None)
    }

    /// A person sets the block to a value a condition reads.
    pub fn person_sets(&mut self, block: &str, value: &str) -> Seq {
        self.outsider_writes(block, Outsider::Person, Some(value.to_string()))
    }

    /// Something outside the work writes the block.
    pub fn outside_writes(&mut self, block: &str) -> Seq {
        self.outsider_writes(block, Outsider::Outside, None)
    }

    /// A person writes prose into the block, and the entry says the change does not matter.
    pub fn person_writes_not_mattering(&mut self, block: &str) -> Seq {
        let at = self.tick();
        self.records.written.push(Written {
            block: block.into(),
            by: Outsider::Person,
            matters: false,
            value: None,
            at,
        });
        at
    }

    /// Something outside the work writes the block, and the owner says the change does not matter to the works under
    /// way: a changed specification of a cell, for one.
    pub fn outside_writes_not_mattering(&mut self, block: &str) -> Seq {
        let at = self.tick();
        self.records.written.push(Written {
            block: block.into(),
            by: Outsider::Outside,
            matters: false,
            value: None,
            at,
        });
        at
    }

    fn outsider_writes(&mut self, block: &str, by: Outsider, value: Option<String>) -> Seq {
        let at = self.tick();
        self.records.written.push(Written {
            block: block.into(),
            by,
            matters: true,
            value,
            at,
        });
        at
    }

    /// A person writes the block and leaves its value as it was: a confirmation of what was recommended, for one.
    pub fn person_confirms(&mut self, block: &str, value: &str) -> Seq {
        let at = self.tick();
        self.records.written.push(Written {
            block: block.into(),
            by: Outsider::Person,
            matters: false,
            value: Some(value.to_string()),
            at,
        });
        at
    }

    /// The owner lets the cell go on after it reached its limit.
    pub fn let_go(&mut self, cell: &str) {
        let at = self.tick();
        self.records.let_go.push(LetGo { cell: cell.into(), at });
    }

    pub fn pause(&mut self) {
        let at = self.tick();
        self.records.pauses.push(Pause { paused: true, at });
    }

    pub fn resume(&mut self) {
        let at = self.tick();
        self.records.pauses.push(Pause { paused: false, at });
    }

    pub fn cancel(&mut self) {
        let at = self.tick();
        self.records.cancelled = Some(at);
    }
}
