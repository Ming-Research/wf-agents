//! A work in memory, for the tests of the walk: it keeps the records as an engine would, only adding to them, and does
//! to them what each decision asks.

use std::collections::BTreeMap;

use decide::{
    Abandoned, Answered, Asked, CallId, Cause, Cell, CellId, Decision, Delivered, Finished, Flow, LetGo, Outcome,
    Pause, ProductId, Records, Refusal, RefusalId, Refuser, RunId, Seq, Started, TestName, Tests, Version, Who, Writer,
    decide,
};

/// The named tests, each holding unless the test of the walk set it otherwise.
#[derive(Default)]
pub struct Named(pub BTreeMap<TestName, Outcome>);

impl Tests for Named {
    fn test(&self, name: &TestName, _records: &Records) -> Outcome {
        self.0.get(name).cloned().unwrap_or(Outcome::Holds)
    }
}

/// Whether a write matters to the cells after it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Write {
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
        w.set(
            "the merge may go",
            Outcome::Waits {
                what: "the owner's word".to_string(),
                who: Who::Owner,
            },
        );
        w
    }

    /// Whether nothing is to be done but wait at the merge.
    pub fn rests(&self) -> bool {
        matches!(self.decide(), Decision::Wait { at, .. } if at.as_str() == "merge")
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

    /// Why the walk says to run a cell, when it says to run one.
    pub fn because(&self) -> Vec<Cause> {
        match self.decide() {
            Decision::Run { because, .. } => because,
            other => panic!("the walk runs nothing: {other:?}"),
        }
    }

    /// Sets what a named test comes to.
    pub fn set(&mut self, test: &str, outcome: Outcome) {
        self.named.0.insert(test.into(), outcome);
    }

    /// Starts the cell, which the walk must have said to run, built on what the walk says and given what it says.
    pub fn start(&mut self, cell: &str) -> RunId {
        let Decision::Run {
            cell: next,
            basis,
            given,
            ..
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
            basis,
            given,
        });
        run
    }

    /// The run of the cell that has started and neither finished nor been abandoned.
    fn open(&self, cell: &str) -> RunId {
        let id: CellId = cell.into();
        self.records
            .started
            .iter()
            .rev()
            .find(|s| {
                s.cell == id
                    && !self.records.finished.iter().any(|f| f.run == s.run)
                    && !self.records.abandoned.iter().any(|a| a.run == s.run)
            })
            .expect("the cell has a run under way")
            .run
    }

    /// A cell's record: its latest finished run.
    pub fn record(&self, cell: &str) -> RunId {
        let id: CellId = cell.into();
        self.records
            .finished
            .iter()
            .rev()
            .find(|f| self.records.started.iter().any(|s| s.run == f.run && s.cell == id))
            .expect("the cell has a record")
            .run
    }

    /// Writes a version of the product: its content changes, and its meaning when the write matters or is the first.
    fn write(&mut self, product: &str, by: Writer, write: Write) {
        let id: ProductId = product.into();
        let last = self.records.versions.iter().rev().find(|v| v.product == id);
        let (content, meaning) = match last {
            Some(v) => (v.content + 1, v.meaning + u32::from(write == Write::Matters)),
            None => (1, 1),
        };
        let at = self.tick();
        self.records.versions.push(Version {
            product: id,
            content,
            meaning,
            by,
            at,
            gone: false,
        });
    }

    /// The cell's run, still under way, writes the product.
    pub fn writes(&mut self, cell: &str, product: &str, write: Write) {
        let run = self.open(cell);
        self.write(product, Writer::Run(run), write);
    }

    /// Finishes the cell's run with these answers and these writes.
    pub fn finish(&mut self, cell: &str, answers: &[(&str, &str)], writes: &[(&str, Write)]) {
        let run = self.open(cell);
        for (product, write) in writes {
            self.write(product, Writer::Run(run), *write);
        }
        let at = self.tick();
        let answers = answers.iter().map(|(k, v)| ((*k).into(), (*v).to_string())).collect();
        self.records.finished.push(Finished { run, at, answers });
    }

    /// Starts and finishes the cell.
    pub fn run(&mut self, cell: &str, answers: &[(&str, &str)], writes: &[(&str, Write)]) {
        self.start(cell);
        self.finish(cell, answers, writes);
    }

    /// The cell's record refuses the product as it stands.
    pub fn refuse(&mut self, cell: &str, product: &str, why: &str) -> RefusalId {
        let by = Refuser::Run(self.record(cell));
        self.refusal(by, product, why)
    }

    /// The owner refuses the product as it stands.
    pub fn owner_refuses(&mut self, product: &str, why: &str) -> RefusalId {
        self.refusal(Refuser::Person(Who::Owner), product, why)
    }

    fn refusal(&mut self, by: Refuser, product: &str, why: &str) -> RefusalId {
        let product: ProductId = product.into();
        let content = self
            .records
            .versions
            .iter()
            .rev()
            .find(|v| v.product == product)
            .expect("the product exists")
            .content;
        let id = RefusalId(self.id());
        let at = self.tick();
        self.records.refusals.push(Refusal {
            id,
            by,
            product,
            content,
            why: why.to_string(),
            at,
        });
        id
    }

    /// A person writes the product, which always changes its meaning.
    pub fn person_writes(&mut self, product: &str) {
        self.write(product, Writer::Person, Write::Matters);
    }

    /// The product is lost: a mark of the loss is added, its meaning kept.
    pub fn lose(&mut self, product: &str) {
        let id: ProductId = product.into();
        let v = self
            .records
            .versions
            .iter()
            .rev()
            .find(|v| v.product == id)
            .expect("the product exists")
            .clone();
        let at = self.tick();
        self.records.versions.push(Version {
            content: v.content + 1,
            at,
            gone: true,
            ..v
        });
    }

    /// The cell's run comes to wait at a call for this person.
    pub fn wait_for(&mut self, cell: &str, what: &str, who: Who) -> CallId {
        let run = self.open(cell);
        let call = CallId(self.id());
        let at = self.tick();
        self.records.asked.push(Asked {
            call,
            run,
            what: what.to_string(),
            who,
            at,
        });
        call
    }

    /// The person gives what the call waits for.
    pub fn answer(&mut self, call: CallId) {
        let at = self.tick();
        self.records.answered.push(Answered { call, at });
    }

    /// Abandons the run the walk says to abandon; any other decision is a fault of the test.
    pub fn abandon(&mut self) {
        let Decision::Abandon { run, .. } = self.decide() else {
            panic!("nothing to abandon: {:?}", self.decide())
        };
        let at = self.tick();
        self.records.abandoned.push(Abandoned { run, at });
    }

    /// Puts into the running run's session the refusals the walk says to deliver, as the engine would.
    pub fn deliver(&mut self) -> Vec<RefusalId> {
        let Decision::Busy { run, deliver, .. } = self.decide() else {
            panic!("no cell runs: {:?}", self.decide())
        };
        for refusal in &deliver {
            let at = self.tick();
            self.records.delivered.push(Delivered {
                run,
                refusal: *refusal,
                at,
            });
        }
        deliver
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

    pub fn cancel(&mut self) {
        let at = self.tick();
        self.records.cancelled = Some(at);
    }
}

/// A cell that depends on these products and writes those.
pub fn cell(id: &str, follows: &[&str], writes: &[&str]) -> Cell {
    let mut c = Cell::new(id);
    c.follows = follows.iter().map(|p| (*p).into()).collect();
    c.writes = writes.iter().map(|p| (*p).into()).collect();
    c
}
