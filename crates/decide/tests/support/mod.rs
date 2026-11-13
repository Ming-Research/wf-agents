//! A work in memory, for the tests of the walk. It keeps the records as an engine would, only adding to them, and
//! takes from each decision exactly what the decision gives: why a run starts and what it starts from.

use std::collections::BTreeMap;

use decide::{
    Abandoned, Abandoning, BlockId, CallId, Called, Cell, CellId, Condition, Decision, Done, Finished, Flow, LetGo,
    Maker, Outsider, Outsiders, Pause, Pauser, Records, Returned, Round, RunId, Seq, Started, Test, TestKind, TestName,
    Waited, Who, Write, Written, decide,
};

/// The owner's word of this name: a named test of the choice of the same name, which people write, that holds unless
/// the choice is no. A person writing the choice as no or yes makes it fail or hold.
pub fn word(name: &str) -> Test {
    Test {
        kind: TestKind::None,
        reads: vec![name.into()],
        is: "no".to_string(),
    }
}

/// The merge's word, [`GATE`]: a named test of the choice of the same name that holds only once a person wrote it as
/// yes.
pub fn gate_word() -> Test {
    Test {
        kind: TestKind::All,
        reads: vec![GATE.into()],
        is: "yes".to_string(),
    }
}

/// The name of the merge's precondition, and of the choice it reads.
pub const GATE: &str = "the merge may go";

/// The named tests for these cells: those given, and for each other test a cell names, the owner's word of that name
/// ([`word`]).
fn tests_for(cells: &[Cell], given: &BTreeMap<TestName, Test>) -> BTreeMap<TestName, Test> {
    let mut tests = given.clone();
    for c in cells {
        let condition = match &c.applies {
            Some(Condition::Test(t)) => Some(t),
            _ => None,
        };
        for t in condition.into_iter().chain(&c.requires) {
            tests.entry(t.clone()).or_insert_with(|| word(t.as_str()));
        }
    }
    tests
}

/// The flow of these cells, with these named tests and the owner's word for each other test a cell names, and these
/// writers of blocks besides the cells: people write each other block a named test reads, and the notes.
fn flow_of(
    cells: Vec<Cell>,
    tests: &BTreeMap<TestName, Test>,
    notes: &[BlockId],
    declared: &BTreeMap<BlockId, Outsiders>,
) -> Flow {
    let tests = tests_for(&cells, tests);
    let mut outsiders = declared.clone();
    for block in tests.values().flat_map(|t| &t.reads) {
        if !declared.contains_key(block) {
            outsiders.entry(block.clone()).or_default().people = true;
        }
    }
    for block in notes {
        let o = outsiders.entry(block.clone()).or_default();
        o.people = true;
        o.note = true;
    }
    Flow::new(cells, tests, outsiders).expect("the cells make a flow")
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
    /// The named tests given beside those of the owner's word, which [`World::install`] keeps.
    tests: BTreeMap<TestName, Test>,
    /// The blocks that are notes, which [`World::install`] keeps.
    notes: Vec<BlockId>,
    /// Who besides the cells writes these blocks, which [`World::install`] keeps. Every other block a named test reads
    /// people write, as the owner's word.
    outsiders: BTreeMap<BlockId, Outsiders>,
    seq: u64,
    ids: u64,
}

/// The merge a work rests at when nothing else is to be done: it waits for the owner's word, [`gate_word`].
pub fn gate() -> Cell {
    let mut merge = Cell::new("merge");
    merge.requires = vec![GATE.into()];
    merge
}

/// The owner's word at the merge, as the walk reports waiting for it.
pub fn merge_word() -> Waited {
    Waited::Precondition {
        test: GATE.into(),
        by: vec![Maker::Owner],
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
    /// A work on these cells, each test a cell names being the owner's word of that name ([`word`]), and the merge's
    /// word [`gate_word`].
    pub fn new(cells: Vec<Cell>) -> Self {
        Self::with_tests(cells, BTreeMap::new())
    }

    /// A work on these cells with these named tests, and the owner's word for each other test a cell names.
    pub fn with_tests(cells: Vec<Cell>, tests: BTreeMap<TestName, Test>) -> Self {
        let mut tests = tests;
        tests.entry(GATE.into()).or_insert_with(gate_word);
        let flow = flow_of(cells, &tests, &[], &BTreeMap::new());
        Self {
            flow,
            records: Records::default(),
            tests,
            notes: Vec::new(),
            outsiders: BTreeMap::new(),
            seq: 0,
            ids: 0,
        }
    }

    /// A work whose last cell is a merge that waits for the owner: when nothing else is to be done the work rests
    /// there, and the test can go on changing things.
    pub fn with_gate(mut cells: Vec<Cell>) -> Self {
        cells.push(gate());
        Self::new(cells)
    }

    /// Installs a changed flow over the same records, with the same named tests, notes and writers of blocks.
    pub fn install(&mut self, cells: Vec<Cell>) {
        self.flow = flow_of(cells, &self.tests, &self.notes, &self.outsiders);
    }

    /// Makes these blocks notes: what a person writes in them are their words to the cells that read them, which a
    /// paused work still answers.
    pub fn notes(&mut self, blocks: &[&str]) {
        self.notes = blocks.iter().map(|b| (*b).into()).collect();
        self.install(self.flow.cells().to_vec());
    }

    /// Makes this source outside the work the writer of the block, in place of people: as the items a work depends on
    /// write whether they merged.
    pub fn written_outside(&mut self, block: &str, source: &str) {
        let outsiders = Outsiders {
            outside: Some(source.to_string()),
            ..Outsiders::default()
        };
        self.outsiders.insert(block.into(), outsiders);
        self.install(self.flow.cells().to_vec());
    }

    /// Makes the cells that have the block among their outputs its only writers: people write it no longer.
    pub fn written_by_cells(&mut self, block: &str) {
        self.outsiders.insert(block.into(), Outsiders::default());
        self.install(self.flow.cells().to_vec());
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
        decide(&self.flow, &self.records)
    }

    /// The owner gives the word of this name ([`word`], [`gate_word`]) as yes or, `false`, as no: a person writes its
    /// choice.
    pub fn says(&mut self, word: &str, yes: bool) -> Seq {
        self.person_sets(word, if yes { "yes" } else { "no" })
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
                meaning_unchanged: *says == Says::DoesNotMatter,
                value: match says {
                    Says::Value(v) => Some((*v).to_string()),
                    _ => None,
                },
            })
            .collect();
        self.records.finished.push(Finished {
            run,
            at,
            writes,
            dealt_with: None,
        });
        at
    }

    /// Starts and finishes the cell; the place of its finish names the versions it wrote.
    pub fn run(&mut self, cell: &str, writes: &[(&str, Says)]) -> Seq {
        self.start(cell);
        self.finish(cell, writes)
    }

    /// Abandons the run the walk says to abandon, and says why; any other decision is a fault of the test.
    pub fn abandon(&mut self) -> Abandoning {
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

    /// Something outside the work sets the block to a value a condition reads.
    pub fn outside_sets(&mut self, block: &str, value: &str) -> Seq {
        self.outsider_writes(block, Outsider::Outside, Some(value.to_string()))
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

    /// The owner pauses the work.
    pub fn pause(&mut self) {
        let at = self.tick();
        self.records.pauses.push(Pause {
            paused: true,
            at,
            by: Pauser::Owner,
            why: None,
        });
    }

    /// The engine pauses the work, for this reason: as when a round failed, or the work spent more than it may.
    pub fn engine_pauses(&mut self, why: &str) {
        let at = self.tick();
        self.records.pauses.push(Pause {
            paused: true,
            at,
            by: Pauser::Engine,
            why: Some(why.to_string()),
        });
    }

    /// The round under way of the cell's run fails: it ends with nothing come back, and the engine pauses the work
    /// with the reason in the same step, as the engine's loop does. To the walk the run is then at rest.
    pub fn fails(&mut self, cell: &str, why: &str) {
        self.returns(cell);
        self.engine_pauses(why);
    }

    /// The owner lets the work on again.
    pub fn resume(&mut self) {
        let at = self.tick();
        self.records.pauses.push(Pause {
            paused: false,
            at,
            by: Pauser::Owner,
            why: None,
        });
    }

    pub fn cancel(&mut self) {
        let at = self.tick();
        self.records.cancelled = Some(at);
    }
}
