//! A work in memory, for the tests of the walk. It keeps the records as an engine would, only adding to them, and
//! takes from each decision exactly what the decision gives: why a run starts, what it starts from, what it is given.

use std::collections::BTreeMap;

use decide::{
    Abandoned, CallId, Called, Cause, Cell, CellId, Decision, Done, Finished, Flow, LetGo, Lost, Outcome, Outsider,
    Pause, ProductId, Records, Refusal, RefusalId, Refuser, Returned, Round, RunId, Seq, Stale, Started, TestName,
    Tests, Who, Write, Written, current, decide,
};

/// The named tests, each holding unless the test of the walk set it otherwise.
#[derive(Default)]
pub struct Named(pub BTreeMap<TestName, Outcome>);

impl Tests for Named {
    fn test(&self, name: &TestName, _records: &Records) -> Outcome {
        self.0.get(name).cloned().unwrap_or(Outcome::Holds)
    }
}

/// Whether what a run writes matters to the cells that depend on it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Says {
    Matters,
    DoesNotMatter,
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

/// A cell that depends on these products and may write those.
pub fn cell(id: &str, follows: &[&str], writes: &[&str]) -> Cell {
    let mut c = Cell::new(id);
    c.follows = follows.iter().map(|p| (*p).into()).collect();
    c.writes = writes.iter().map(|p| (*p).into()).collect();
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

    /// Whether nothing is to be done but wait for the owner's word at the merge.
    pub fn rests(&self) -> bool {
        let word = decide::Waited::Precondition {
            test: "the merge may go".into(),
            who: Who::Owner,
        };
        self.decide()
            == Decision::Wait {
                at: "merge".into(),
                on: vec![word],
            }
    }

    fn tick(&mut self) -> Seq {
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

    /// The cell the walk says to run, when it says to run one.
    pub fn runs_next(&self) -> Option<String> {
        match self.decide() {
            Decision::Run { cell, .. } => Some(cell.0),
            _ => None,
        }
    }

    /// Why the walk says to run a cell; any other decision is a fault of the test.
    pub fn because(&self) -> Vec<Cause> {
        match self.decide() {
            Decision::Run { because, .. } => because,
            other => panic!("the walk runs nothing: {other:?}"),
        }
    }

    /// What the walk waits for; any other decision is a fault of the test.
    pub fn waits(&self) -> (String, Vec<decide::Waited>) {
        match self.decide() {
            Decision::Wait { at, on } => (at.0, on),
            other => panic!("the walk does not wait: {other:?}"),
        }
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
            given,
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
            given,
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

    /// A cell's record: its latest finished run.
    pub fn record(&self, cell: &str) -> RunId {
        let id: CellId = cell.into();
        let r = &self.records;
        (r.finished.iter().rev())
            .find(|f| r.started.iter().any(|s| s.run == f.run && s.cell == id))
            .expect("the cell has a record")
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

    /// The cell's run, whose round under way ends here, finishes with these answers and these writes.
    pub fn finish(&mut self, cell: &str, answers: &[(&str, &str)], writes: &[(&str, Says)]) {
        let run = self.open(cell);
        if self.under_way(run) {
            let at = self.tick();
            self.records.returned.push(Returned { run, at });
        }
        let at = self.tick();
        let answers = answers.iter().map(|(k, v)| ((*k).into(), (*v).to_string())).collect();
        let writes = (writes.iter())
            .map(|(product, says)| Write {
                product: (*product).into(),
                matters: *says == Says::Matters,
            })
            .collect();
        self.records.finished.push(Finished {
            run,
            at,
            answers,
            writes,
        });
    }

    /// Starts and finishes the cell.
    pub fn run(&mut self, cell: &str, answers: &[(&str, &str)], writes: &[(&str, Says)]) -> RunId {
        let run = self.start(cell);
        self.finish(cell, answers, writes);
        run
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

    /// The cell's record refuses the version of the product it started from.
    pub fn refuse(&mut self, cell: &str, product: &str) -> RefusalId {
        let run = self.record(cell);
        let id: ProductId = product.into();
        let started = self
            .records
            .started
            .iter()
            .find(|s| s.run == run)
            .expect("the run started");
        let version = (started.basis.get(&id).copied().flatten()).expect("the run started from a version of it");
        self.refusal(Refuser::Run(run), id, version)
    }

    /// The owner refuses the current version of the product.
    pub fn owner_refuses(&mut self, product: &str) -> RefusalId {
        let id: ProductId = product.into();
        let version = current(&self.flow, &self.records, &self.named, &id).expect("the product has a version");
        self.refusal(Refuser::Person(Who::Owner), id, version)
    }

    fn refusal(&mut self, by: Refuser, product: ProductId, version: Seq) -> RefusalId {
        let id = RefusalId(self.id());
        let at = self.tick();
        self.records.refusals.push(Refusal {
            id,
            by,
            product,
            version,
            at,
        });
        id
    }

    /// A person writes the product.
    pub fn person_writes(&mut self, product: &str) {
        self.outsider_writes(product, Outsider::Person);
    }

    /// Something outside the work writes the product.
    pub fn outside_writes(&mut self, product: &str) {
        self.outsider_writes(product, Outsider::Outside);
    }

    fn outsider_writes(&mut self, product: &str, by: Outsider) {
        let at = self.tick();
        self.records.written.push(Written {
            product: product.into(),
            by,
            at,
        });
    }

    /// The product is lost.
    pub fn lose(&mut self, product: &str) {
        let at = self.tick();
        self.records.lost.push(Lost {
            product: product.into(),
            at,
        });
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
