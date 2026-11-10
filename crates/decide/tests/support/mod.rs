//! A work in memory, for the tests of the walk: it keeps the records as an engine would, and does to them what each
//! decision asks.

use std::collections::BTreeMap;

use decide::{
    Built, Cell, CellId, Decision, Flow, LetGo, Outcome, ProductId, Records, Refusal, Refuser, Run, RunId, RunState,
    Seq, TestName, Tests, Version, Who, Writer, decide,
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
    seq: Seq,
    runs: RunId,
}

impl World {
    pub fn new(cells: Vec<Cell>) -> Self {
        Self {
            flow: Flow { cells },
            records: Records::default(),
            named: Named::default(),
            seq: 0,
            runs: 0,
        }
    }

    /// A work whose last cell is a merge that waits for the owner: when nothing else is to be done the work rests
    /// there, and the test can go on changing things.
    pub fn with_gate(mut cells: Vec<Cell>) -> Self {
        let mut merge = Cell::new("merge");
        merge.requires = vec!["the merge may go".into()];
        cells.push(merge);
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
        self.seq
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

    /// Sets what a named test comes to.
    pub fn set(&mut self, test: &str, outcome: Outcome) {
        self.named.0.insert(test.into(), outcome);
    }

    /// Starts the cell, which the walk must have said to run: the run is built on what stands now.
    pub fn start(&mut self, cell: &str) -> RunId {
        assert_eq!(
            self.runs_next().as_deref(),
            Some(cell),
            "the walk says {:?}",
            self.decide()
        );
        let id: CellId = cell.into();
        let c = self.flow.cell(&id).expect("the cell is in the flow").clone();
        let mut basis = BTreeMap::new();
        for p in c.follows.iter() {
            if let Some(v) = self.records.current(p) {
                basis.insert(
                    p.clone(),
                    Built {
                        content: v.content,
                        meaning: v.meaning,
                    },
                );
            }
        }
        self.runs += 1;
        let started = self.tick();
        self.records.runs.push(Run {
            id: self.runs,
            cell: id,
            started,
            state: RunState::Running,
            basis,
            answers: BTreeMap::new(),
        });
        self.runs
    }

    /// Writes a version of the product: its content changes, and its meaning when the write matters or is the first.
    fn write(&mut self, product: &str, by: Writer, write: Write) {
        let id: ProductId = product.into();
        let (content, meaning) = match self.records.current(&id) {
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

    /// Finishes the cell's running run with these answers and these writes.
    pub fn finish(&mut self, cell: &str, answers: &[(&str, &str)], writes: &[(&str, Write)]) {
        let id: CellId = cell.into();
        let run = self
            .records
            .runs
            .iter()
            .position(|r| r.cell == id && r.state == RunState::Running)
            .expect("the cell has a running run");
        let run_id = self.records.runs[run].id;
        for (product, write) in writes {
            self.write(product, Writer::Run(run_id), *write);
        }
        let at = self.tick();
        let r = &mut self.records.runs[run];
        r.state = RunState::Finished { at };
        r.answers = answers.iter().map(|(k, v)| ((*k).into(), (*v).to_string())).collect();
    }

    /// Starts and finishes the cell.
    pub fn run(&mut self, cell: &str, answers: &[(&str, &str)], writes: &[(&str, Write)]) {
        self.start(cell);
        self.finish(cell, answers, writes);
    }

    /// The cell's record refuses the product as it stands.
    pub fn refuse(&mut self, cell: &str, product: &str, why: &str) {
        let by = self.records.record(&cell.into()).expect("the cell has a record").id;
        self.refusal(Refuser::Run(by), product, why);
    }

    /// The owner refuses the product as it stands.
    pub fn owner_refuses(&mut self, product: &str, why: &str) {
        self.refusal(Refuser::Person(Who::Owner), product, why);
    }

    fn refusal(&mut self, by: Refuser, product: &str, why: &str) {
        let id: ProductId = product.into();
        let content = self.records.current(&id).expect("the product exists").content;
        let at = self.tick();
        self.records.refusals.push(Refusal {
            by,
            product: id,
            content,
            why: why.to_string(),
            at,
        });
    }

    /// A person writes the product, which always changes its meaning.
    pub fn person_writes(&mut self, product: &str) {
        self.write(product, Writer::Person, Write::Matters);
    }

    /// The product is lost: it stands as gone, its meaning kept.
    pub fn lose(&mut self, product: &str) {
        let id: ProductId = product.into();
        let v = self.records.current(&id).expect("the product exists").clone();
        let at = self.tick();
        self.records.versions.push(Version {
            content: v.content + 1,
            at,
            gone: true,
            ..v
        });
    }

    /// The cell's running run comes to wait at a call for this person.
    pub fn wait_for(&mut self, cell: &str, call: &str, who: Who) {
        let id: CellId = cell.into();
        let r = self
            .records
            .runs
            .iter_mut()
            .find(|r| r.cell == id && r.state == RunState::Running)
            .expect("the cell has a running run");
        r.state = RunState::Waiting {
            call: call.to_string(),
            who,
            answered: false,
        };
    }

    /// The person gives what the cell's waiting run waits for.
    pub fn answer(&mut self, cell: &str) {
        let id: CellId = cell.into();
        let r = self
            .records
            .runs
            .iter_mut()
            .find(|r| r.cell == id && matches!(r.state, RunState::Waiting { .. }))
            .expect("the cell has a waiting run");
        if let RunState::Waiting { answered, .. } = &mut r.state {
            *answered = true;
        }
    }

    /// Does what the decision to let a waiting run go on, or to abandon one, asks; any other decision is a fault of
    /// the test.
    pub fn obey(&mut self) {
        match self.decide() {
            Decision::GoOn { run, .. } => {
                self.records
                    .runs
                    .iter_mut()
                    .find(|r| r.id == run)
                    .expect("the run")
                    .state = RunState::Running;
            }
            Decision::Abandon { run, .. } => {
                let at = self.tick();
                self.records
                    .runs
                    .iter_mut()
                    .find(|r| r.id == run)
                    .expect("the run")
                    .state = RunState::Abandoned { at };
            }
            other => panic!("nothing to obey in {other:?}"),
        }
    }

    /// The owner lets the cell go on after it reached its limit.
    pub fn let_go(&mut self, cell: &str) {
        let at = self.tick();
        self.records.let_go.push(LetGo { cell: cell.into(), at });
    }
}

/// A cell that follows these products and writes those.
pub fn cell(id: &str, follows: &[&str], writes: &[&str]) -> Cell {
    let mut c = Cell::new(id);
    c.follows = follows.iter().map(|p| (*p).into()).collect();
    c.writes = writes.iter().map(|p| (*p).into()).collect();
    c
}
