//! What the driver's tests share: small flows, a store that fails where a test says, and a harness that plays
//! everything outside the driver and checks, after every call, what must always hold.
//!
//! What the harness checks after every call that the driver took:
//! - a store opened on the same file, apart from the driver's, holds every item and every work the driver shows,
//!   exactly (principle `state-from-records`);
//! - what the driver gives to be done, that store already says is to be done;
//! - no round and no call is given twice, and nothing answered is given again;
//! - what was given and not yet answered is exactly what the driver says is pending;
//! - no work has two rounds under way (property `one-round-of-a-work-at-a-time`);
//! - what each work does next, as the driver shows it, agrees with what is to be done for it: after a call no run is
//!   left to start or to abandon, a run goes on with its round held only while the work is paused, a work is busy
//!   with a run exactly when a round of that run is given to be done, and a work paused began no round after the
//!   pause but of a run for a note a person wrote, and none given again after it failed;
//! - the dependencies block of every work that has not ended says whether the items its item depends on have merged,
//!   and its text names those still waited for, saying which were cancelled; the block of a work that has ended was
//!   not written after it ended; no work whose dependencies have not all merged has started a run where its flow
//!   holds it for them; and a work never let on has started no run;
//! - no more rounds that take a slot are under way than the driver has slots, as the driver says; no work waits for a
//!   slot while one is free; and while the pause of everything holds, the owner's or the driver's own as it opened on
//!   works going on, no work began a round after it was set but of a run for a note a person wrote, and none given
//!   again after it failed; the driver holds the pause of everything the store holds.
//!
//! When the driver says it stopped, the harness checks that what it says was kept is what the store holds.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use decide::{BlockId, CallId, Cause, Decision, End, Holder, Maker, Outsider, Pauser, RunId, Seq, Waited, Who};
use driver::{DEPENDENCIES, Driver, Failed, Fault, FirstWrite, MERGED, NewItem, Todo, WAITING};
use engine::{Ask, Back, Came, Effect, Event, Given, Handed, Moment, Spent, Time, Work};
use flow::{Compiled, Has, Kind, Source, compile};
use rusqlite::Connection;
use rusqlite::types::Value;
use store::{EverythingPaused, Item, ItemChange, ItemNumber, Storage, Store, WorkHead, WorkNumber};
use tempfile::TempDir;

/// The time of the first thing a test does.
pub const START: u64 = 1_800_000_000_000;

/// What the engine has: the driver's own sources outside a work, and the act `merge`.
pub fn has() -> Has {
    Has {
        sources: [DEPENDENCIES.to_string(), driver::START.to_string()].into(),
        acts: ["merge".to_string(), "start".to_string()].into(),
        ..Default::default()
    }
}

/// A flow compiled from the text of its files.
pub fn compiled(flow: &str, cells: &[(&str, &str)]) -> Compiled {
    let source = Source {
        flow: flow.to_string(),
        cells: (cells.iter())
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
        ..Default::default()
    };
    compile(&source, &has()).expect("the definition compiles")
}

/// The block the driver writes as the owner starts a work, which runs the first cell of each flow.
const START_BLOCK: &str = r#"
        [blocks.start]
        outside = "start"
        writes = { text = "The owner started the work." }
"#;

/// The dependencies block, which the driver writes, and the named test the first cell of a flow waits on: that every
/// item the work's item depends on has merged.
const DEPENDENCIES_BLOCK: &str = r#"
        [blocks.deps]
        values = ["waiting", "merged"]
        outside = "dependencies"
        writes = { merged = "Every item this one depends on has merged.", waiting = "Waits for the items {items} to merge.", cancelled = "{item} (cancelled)", between = ", " }
        [tests.ready]
        says = "Every item this one depends on has merged"
        kind = "all"
        reads = ["deps"]
        is = "merged"
"#;

/// A writer that waits for the items its item depends on to have merged, and reads a request and the opinions on what
/// it wrote; a judge that writes opinions for it and a verdict, a choice; and the merge, the engine's own cell.
/// Returns may be sent back twice, a run may make three calls, and a cell may go round three times.
pub fn judged() -> Compiled {
    let flow = format!(
        r#"
        name = "judged"
        order = ["write", "judge", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.main]
        [lines.other]
        [blocks.request]
        people = true
        [blocks.opinions]
        people = true
        [blocks.text]
        [blocks.verdict]
        values = ["yes", "no"]
        people = true
        [blocks.merged]
        shown = true
        {START_BLOCK}
        {DEPENDENCIES_BLOCK}"#
    );
    let write = r#"
        title = "Write"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Write the text the request asks for."
        requires = { test = "ready" }
        inputs = ["start", "request", "opinions"]
        outputs = ["text"]
    "#;
    let judge = r#"
        title = "Judge"
        kind = "agent"
        effort = "high"
        line = "other"
        task = "Judge the text, and give a verdict."
        inputs = ["text"]
        outputs = ["opinions", "verdict"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        inputs = ["text", "verdict"]
        outputs = ["merged"]
    "#;
    compiled(&flow, &[("write", write), ("judge", judge), ("merge", merge)])
}

/// A draft that waits for the items its item depends on to have merged and is run by the owner's start and a request,
/// and the merge.
pub fn short() -> Compiled {
    let flow = format!(
        r#"
        name = "short"
        order = ["draft", "merge"]
        [limits]
        runs = 2
        returns = 2
        calls = 3
        [lines.main]
        [blocks.request]
        people = true
        [blocks.plan]
        [blocks.merged]
        shown = true
        {START_BLOCK}
        {DEPENDENCIES_BLOCK}"#
    );
    let draft = r#"
        title = "Draft"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Draft the plan from the request."
        requires = { test = "ready" }
        inputs = ["start", "request"]
        outputs = ["plan"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        inputs = ["plan"]
        outputs = ["merged"]
    "#;
    compiled(&flow, &[("draft", draft), ("merge", merge)])
}

/// A writer, run by the owner's start and a request, and the merge, with no dependencies block: a work on it does not
/// wait for other items.
pub fn plain() -> Compiled {
    let flow = format!(
        r#"
        name = "plain"
        order = ["write", "merge"]
        [limits]
        runs = 2
        returns = 2
        calls = 3
        [lines.main]
        [blocks.request]
        people = true
        [blocks.text]
        [blocks.merged]
        shown = true
        {START_BLOCK}"#
    );
    let write = r#"
        title = "Write"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Write the text the request asks for."
        inputs = ["start", "request"]
        outputs = ["text"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        inputs = ["text"]
        outputs = ["merged"]
    "#;
    compiled(&flow, &[("write", write), ("merge", merge)])
}

/// Every flow the driver is given.
pub fn flows() -> Vec<Compiled> {
    vec![judged(), short(), plain()]
}

/// The flow of a deliverable item as the repository defines it (`flow/deliver`), compiled against what the engine has
/// for it: the driver's source and the act `deliver`.
pub fn deliver() -> Compiled {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/deliver");
    let source = flow::read(&dir).expect("the deliver flow is read");
    let has = Has {
        sources: [DEPENDENCIES.to_string()].into(),
        acts: ["deliver".to_string()].into(),
        // The triggers beside the flows are every flow's, and one says what happened by the files of the tree.
        details: ["files".to_string()].into(),
        ..Default::default()
    };
    compile(&source, &has).expect("the deliver flow compiles")
}

/// A database file in a directory of its own, which goes with it.
pub struct Db {
    _dir: TempDir,
    pub path: PathBuf,
}

impl Db {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("a directory for the database");
        let path = dir.path().join("store.db");
        Self { _dir: dir, path }
    }

    /// A store on the file, opened anew.
    pub fn open(&self) -> Store {
        Store::open(&self.path).expect("the store opens")
    }
}

/// Every row of every table of the database, each table's rows in a fixed order: what the file holds.
pub fn dump(path: &Path) -> BTreeMap<String, Vec<Vec<String>>> {
    let db = Connection::open(path).expect("the database opens");
    let mut tables = db
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
        .expect("the tables are listed");
    let names: Vec<String> = (tables.query_map([], |row| row.get(0)).expect("the tables are listed"))
        .collect::<Result<_, _>>()
        .expect("the tables are listed");
    let mut out = BTreeMap::new();
    for name in names {
        let mut query = db.prepare(&format!("SELECT * FROM {name}")).expect("the table is read");
        let columns = query.column_count();
        let mut rows: Vec<Vec<String>> = (query.query_map([], |row| {
            (0..columns)
                .map(|i| row.get::<_, Value>(i).map(|v| format!("{v:?}")))
                .collect::<Result<Vec<_>, _>>()
        }))
        .expect("the table is read")
        .collect::<Result<_, _>>()
        .expect("the table is read");
        rows.sort();
        out.insert(name, rows);
    }
    out
}

/// A function of the store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Change,
    Items,
    Works,
    Load,
    Save,
    /// The read of the owner's pause of everything.
    Everything,
    /// The keeping of the owner's pause of everything, set or lifted.
    PauseEverything,
}

/// Where the store of the tests fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    /// It does not.
    Never,
    /// The `n`th call of this function, or of any where none is named, counting from one, fails without doing
    /// anything; and so does every call after it when `on` is set.
    Fail { op: Option<Op>, n: usize, on: bool },
    /// The `n`th call of any function is done, and the process stops right after it: every later call fails.
    DieAfter { n: usize },
}

/// What the store of the tests was asked, and what it is to do.
#[derive(Debug)]
pub struct Calls {
    pub plan: Plan,
    /// Each call since the plan was set, in order.
    pub ops: Vec<Op>,
    /// Whether a call failed as the plan said.
    pub failed: bool,
    /// Whether the process stopped, as the plan said.
    pub dead: bool,
}

/// The fault the store of the tests gives when it fails.
pub fn failure() -> store::Fault {
    store::Fault::Database("the store of the tests fails here".to_string())
}

/// A real store on a file, which fails where its plan says.
pub struct Failing {
    store: Store,
    calls: Arc<Mutex<Calls>>,
}

impl Failing {
    pub fn new(store: Store) -> (Self, Arc<Mutex<Calls>>) {
        let calls = Arc::new(Mutex::new(Calls {
            plan: Plan::Never,
            ops: Vec::new(),
            failed: false,
            dead: false,
        }));
        let store = Self {
            store,
            calls: calls.clone(),
        };
        (store, calls)
    }

    /// Notes the call, and says whether it fails, or is done.
    fn call(&self, op: Op) -> Result<(), store::Fault> {
        let mut calls = self.calls.lock().expect("the calls are noted");
        if calls.dead {
            return Err(failure());
        }
        calls.ops.push(op);
        let nth = |c: &Calls, op: Option<Op>| c.ops.iter().filter(|o| op.is_none_or(|op| **o == op)).count();
        match calls.plan {
            Plan::Never => Ok(()),
            Plan::Fail { op: of, n, on } => {
                let fails = of.is_none_or(|of| of == op) && {
                    let count = nth(&calls, of);
                    count == n || (on && count > n)
                };
                if fails {
                    calls.failed = true;
                    Err(failure())
                } else {
                    Ok(())
                }
            }
            Plan::DieAfter { n } => {
                if nth(&calls, None) == n {
                    calls.dead = true;
                }
                Ok(())
            }
        }
    }
}

impl Storage for Failing {
    fn add(
        &mut self,
        title: &str,
        level: &str,
        project: &str,
        depends_on: &[ItemNumber],
        flow: &str,
        at: Time,
        first: &Work,
    ) -> Result<(ItemNumber, WorkNumber), store::Fault> {
        self.call(Op::Add)?;
        self.store.add(title, level, project, depends_on, flow, at, first)
    }

    fn change(&mut self, item: ItemNumber, change: &ItemChange, at: Time) -> Result<(), store::Fault> {
        self.call(Op::Change)?;
        self.store.change(item, change, at)
    }

    fn items(&self) -> Result<Vec<Item>, store::Fault> {
        self.call(Op::Items)?;
        self.store.items()
    }

    fn works(&self) -> Result<Vec<WorkHead>, store::Fault> {
        self.call(Op::Works)?;
        self.store.works()
    }

    fn load(&self, work: WorkNumber) -> Result<Work, store::Fault> {
        self.call(Op::Load)?;
        self.store.load(work)
    }

    fn save(&mut self, work: WorkNumber, given: &Work) -> Result<(), store::Fault> {
        self.call(Op::Save)?;
        self.store.save(work, given)
    }

    fn everything_paused(&self) -> Result<Option<EverythingPaused>, store::Fault> {
        self.call(Op::Everything)?;
        self.store.everything_paused()
    }

    fn pause_everything(&mut self, paused: bool, by: Pauser, at: Time) -> Result<(), store::Fault> {
        self.call(Op::PauseEverything)?;
        self.store.pause_everything(paused, by, at)
    }
}

/// What opening the driver gives.
pub type Opened = Result<(Driver, Vec<Todo>), Fault>;

/// Opens a driver on the file, through the store of the tests, with every flow.
pub fn open_on(db: &Db, now: Time) -> (Opened, Arc<Mutex<Calls>>) {
    open_with_plan(db, Plan::Never, now)
}

/// Opens a driver on the file, through the store of the tests failing as the plan says from its first call.
pub fn open_with_plan(db: &Db, plan: Plan, now: Time) -> (Opened, Arc<Mutex<Calls>>) {
    open_on_flows(db, plan, flows(), now)
}

/// Opens a driver on the file with these flows, through the store of the tests failing as the plan says from its first
/// call, with as many slots as the tests that do not count them never fill ([`SLOTS`]).
pub fn open_on_flows(db: &Db, plan: Plan, flows: Vec<Compiled>, now: Time) -> (Opened, Arc<Mutex<Calls>>) {
    open_on_slots(db, plan, flows, SLOTS, now)
}

/// Opens a driver on the file with these flows and this many slots, through the store of the tests failing as the plan
/// says from its first call.
pub fn open_on_slots(
    db: &Db,
    plan: Plan,
    flows: Vec<Compiled>,
    slots: usize,
    now: Time,
) -> (Opened, Arc<Mutex<Calls>>) {
    let (store, calls) = Failing::new(db.open());
    calls.lock().expect("the calls are noted").plan = plan;
    (Driver::open(Box::new(store), flows, slots, None, now), calls)
}

/// The slots of the driver in the tests that do not count them: more than any of them has works.
pub const SLOTS: usize = 1000;

/// Everything the driver shows of itself, and everything the file holds: what a refusal must leave as it was.
#[derive(Debug, PartialEq)]
pub struct State {
    pub items: Vec<Item>,
    pub works: Vec<(WorkHead, Work)>,
    pub pending: Vec<Todo>,
    pub fault: Option<Fault>,
    pub file: BTreeMap<String, Vec<Vec<String>>>,
}

/// A key that orders what is to be done the same way however it came to be held.
pub fn key(todo: &Todo) -> String {
    format!("{todo:?}")
}

pub fn sorted(todos: &[Todo]) -> Vec<Todo> {
    let mut todos = todos.to_vec();
    todos.sort_by_key(key);
    todos
}

/// The project the tests' items belong to where a test names none.
pub const PROJECT: &str = "one";

/// An item to add with no first writes, of [`PROJECT`], paused: its work begins nothing until it is let on.
pub fn new_item(title: &str, depends_on: &[ItemNumber], flow: &str) -> NewItem {
    NewItem {
        title: title.to_string(),
        level: "1".to_string(),
        project: PROJECT.to_string(),
        depends_on: depends_on.to_vec(),
        flow: flow.to_string(),
        writes: Vec::new(),
    }
}

/// A first write by people.
pub fn first_write(block: &str, value: Option<&str>) -> FirstWrite {
    FirstWrite {
        block: block.into(),
        content: format!("{block}, as first written"),
        value: value.map(str::to_string),
    }
}

/// The driver on a store of its own, and everything outside it: a clock, and what the driver gave to be done and was
/// not yet answered.
pub struct H {
    pub db: Db,
    /// None only while the process is stopped, between dropping the driver and opening it again.
    pub driver: Option<Driver>,
    /// What the store of the current driver was asked.
    pub calls: Arc<Mutex<Calls>>,
    /// A store opened on the same file, through which the tests read what is kept.
    pub reader: Store,
    /// The driver's clock.
    pub clock: u64,
    /// How many rounds have ended, so that what each took is told apart.
    pub ended: u64,
    /// What the driver gave to be done and was not yet answered.
    pub outstanding: Vec<Todo>,
    /// Every round and call the driver ever gave.
    pub ever: BTreeSet<String>,
    /// Every round and call answered.
    pub answered: BTreeSet<String>,
    /// Every stop of a round and withdrawal of a call the driver ever gave.
    pub undone: BTreeSet<String>,
    /// The works that were ever let on.
    pub let_on: BTreeSet<WorkNumber>,
    /// Whether every call is followed by the checks; a long run of histories checks only where it says.
    pub checking: bool,
    /// The works a test made faulty behind the driver's back, whose decisions are not checked.
    pub faulty: BTreeSet<WorkNumber>,
    /// The flows the driver is opened with, each time.
    pub flows: Vec<Compiled>,
    /// The slots the driver is opened with, each time.
    pub slots: usize,
}

/// What the driver answered to a call that the store of the tests may have stopped.
pub enum Outcome<T> {
    /// It did what it was asked.
    Done(T),
    /// It was stopped, keeping what it was asked or not.
    Stopped { kept: bool },
}

impl H {
    /// The driver opened on an empty store, with every flow: it has nothing to do.
    pub fn new() -> Self {
        Self::with(flows())
    }

    /// The driver opened on an empty store, with these flows: it has nothing to do.
    pub fn with(flows: Vec<Compiled>) -> Self {
        Self::with_slots(flows, SLOTS)
    }

    /// The driver opened on an empty store, with these flows and this many slots: it has nothing to do.
    pub fn with_slots(flows: Vec<Compiled>, slots: usize) -> Self {
        let db = Db::new();
        let (opened, calls) = open_on_slots(&db, Plan::Never, flows.clone(), slots, Time(START));
        let (driver, todos) = opened.expect("the driver opens");
        assert_eq!(todos, vec![], "the driver has something to do on an empty store");
        let reader = db.open();
        Self {
            db,
            driver: Some(driver),
            calls,
            reader,
            clock: START,
            ended: 0,
            outstanding: Vec::new(),
            ever: BTreeSet::new(),
            answered: BTreeSet::new(),
            undone: BTreeSet::new(),
            let_on: BTreeSet::new(),
            checking: true,
            faulty: BTreeSet::new(),
            flows,
            slots,
        }
    }

    pub fn d(&self) -> &Driver {
        self.driver.as_ref().expect("the driver is open")
    }

    pub fn dm(&mut self) -> &mut Driver {
        self.driver.as_mut().expect("the driver is open")
    }

    /// The time of the next call; the clock moves a second on.
    pub fn now(&mut self) -> Time {
        self.clock += 1000;
        Time(self.clock)
    }

    /// From now on the store of the driver fails as the plan says, its calls counted from here.
    pub fn arm(&mut self, plan: Plan) {
        let mut calls = self.calls.lock().expect("the calls are noted");
        calls.plan = plan;
        calls.ops.clear();
    }

    /// The calls the store of the driver was asked since it was last armed.
    pub fn ops(&self) -> Vec<Op> {
        self.calls.lock().expect("the calls are noted").ops.clone()
    }

    /// Whether the process stopped, as the plan of its store said.
    pub fn dead(&self) -> bool {
        self.calls.lock().expect("the calls are noted").dead
    }

    /// Whether a call of the store failed, as its plan said.
    pub fn failed(&self) -> bool {
        self.calls.lock().expect("the calls are noted").failed
    }

    /// Adds an item with its work, which the driver must take, and checks what must hold.
    pub fn add(&mut self, new: NewItem) -> (ItemNumber, WorkNumber) {
        let now = self.now();
        match self.try_add(now, new) {
            Outcome::Done(numbers) => numbers,
            Outcome::Stopped { .. } => panic!("the driver stopped"),
        }
    }

    /// Adds an item, whose work is added paused, and has the owner let its work on at once.
    pub fn add_let_on(&mut self, new: NewItem) -> (ItemNumber, WorkNumber) {
        let (item, work) = self.add(new);
        self.let_on(work);
        (item, work)
    }

    /// Adds an item at this time, which the driver must take unless its store stops it.
    pub fn try_add(&mut self, now: Time, new: NewItem) -> Outcome<(ItemNumber, WorkNumber)> {
        let before = self.reader.items().expect("the items are read").len();
        match self.dm().add(now, new) {
            Ok((item, work, todos)) => {
                self.given(&todos);
                if self.checking {
                    self.check();
                }
                Outcome::Done((item, work))
            }
            Err(Failed::Stopped { fault, kept }) => {
                self.stopped(&fault);
                let after = self.reader.items().expect("the items are read").len();
                assert!(!kept, "an add said it was kept as it stopped");
                assert_eq!(after, before, "an add that was not kept left an item in the store");
                Outcome::Stopped { kept }
            }
            Err(other) => panic!("the add was refused: {other:?}"),
        }
    }

    /// Tells the driver what happened, which it must take, and gives what is to be done.
    pub fn on(&mut self, work: WorkNumber, event: Event) -> Vec<Todo> {
        let now = self.now();
        match self.try_on(now, work, event) {
            Outcome::Done(todos) => todos,
            Outcome::Stopped { .. } => panic!("the driver stopped"),
        }
    }

    /// Tells the driver what happened at this time, which it must take unless its store stops it.
    pub fn try_on(&mut self, now: Time, work: WorkNumber, event: Event) -> Outcome<Vec<Todo>> {
        let before = self.reader.load(work).ok();
        match self.dm().happened(now, work, event.clone()) {
            Ok(todos) => {
                if matches!(event, Event::Resumed) {
                    self.let_on.insert(work);
                }
                self.answered(work, &event);
                self.given(&todos);
                if self.checking {
                    self.check();
                }
                Outcome::Done(todos)
            }
            Err(Failed::Stopped { fault, kept }) => {
                self.stopped(&fault);
                // What the driver says was kept is what the store holds: the work grew in the store exactly when
                // the thing that happened was kept.
                let after = self.reader.load(work).ok();
                assert_eq!(
                    kept,
                    after != before,
                    "the driver said kept: {kept} of {event:?}, and the store says otherwise"
                );
                if kept {
                    if matches!(event, Event::Resumed) {
                        self.let_on.insert(work);
                    }
                    self.answered(work, &event);
                }
                Outcome::Stopped { kept }
            }
            Err(other) => panic!("what happened was refused: {other:?}"),
        }
    }

    /// Whoever plans the work changes an item at this time, which the driver takes, or refuses leaving everything as it
    /// was, unless its store stops it; a refusal gives nothing to be done.
    pub fn try_change(&mut self, now: Time, item: ItemNumber, change: ItemChange) -> Outcome<Vec<Todo>> {
        let before = self.state();
        let items = self.reader.items().expect("the items are read");
        match self.dm().change(now, item, change) {
            Ok((_, todos)) => {
                self.given(&todos);
                if self.checking {
                    self.check();
                }
                Outcome::Done(todos)
            }
            Err(Failed::Stopped { fault, kept }) => {
                self.stopped(&fault);
                let after = self.reader.items().expect("the items are read");
                assert_eq!(
                    kept,
                    after != items,
                    "the driver said kept: {kept} of a change, and the store says otherwise"
                );
                Outcome::Stopped { kept }
            }
            Err(Failed::Unchangeable { .. } | Failed::NoSuchItem(_)) => {
                assert_eq!(self.state(), before, "a refused change changed the driver or the store");
                Outcome::Done(Vec::new())
            }
            Err(other) => panic!("the change was refused for no reason a change has: {other:?}"),
        }
    }

    /// The owner sets the pause of everything, or lifts it, which the driver must take; and gives what is to be done.
    pub fn pause_everything(&mut self, paused: bool) -> Vec<Todo> {
        let now = self.now();
        match self.try_pause_everything(now, paused) {
            Outcome::Done(todos) => todos,
            Outcome::Stopped { .. } => panic!("the driver stopped"),
        }
    }

    /// The owner sets the pause of everything, or lifts it, at this time, which the driver must take unless its store
    /// stops it.
    pub fn try_pause_everything(&mut self, now: Time, paused: bool) -> Outcome<Vec<Todo>> {
        let before = self
            .reader
            .everything_paused()
            .expect("the pause of everything is read");
        match self.dm().pause_everything(now, paused) {
            Ok(todos) => {
                self.given(&todos);
                if self.checking {
                    self.check();
                }
                Outcome::Done(todos)
            }
            Err(Failed::Stopped { fault, kept }) => {
                self.stopped(&fault);
                let after = self
                    .reader
                    .everything_paused()
                    .expect("the pause of everything is read");
                assert_eq!(
                    kept,
                    after != before,
                    "the driver said kept: {kept} of the pause of everything, and the store says otherwise"
                );
                Outcome::Stopped { kept }
            }
            Err(other) => panic!("the pause of everything was refused: {other:?}"),
        }
    }

    /// The driver stopped with this fault: once stopped, it shows nothing and says why.
    fn stopped(&self, fault: &Fault) {
        let d = self.d();
        assert_eq!(d.fault(), Some(fault), "the driver does not say why it stopped");
        assert_eq!(d.pending(), vec![], "a stopped driver shows what is pending");
        assert_eq!(d.works().count(), 0, "a stopped driver shows works");
    }

    /// What the driver must refuse, which must leave the driver and the store exactly as they were; and why.
    pub fn refused(&mut self, act: impl FnOnce(&mut Driver, Time) -> Result<(), Failed>) -> Failed {
        let before = self.state();
        let now = self.now();
        let why = act(self.dm(), now).expect_err("the driver refuses");
        assert_eq!(self.state(), before, "a refusal changed the driver or the store");
        self.check();
        why
    }

    pub fn state(&self) -> State {
        let d = self.d();
        State {
            items: d.works().map(|k| k.item.clone()).collect(),
            works: d.works().map(|k| (k.head.clone(), k.work.clone())).collect(),
            pending: d.pending(),
            fault: d.fault().cloned(),
            file: dump(&self.db.path),
        }
    }

    /// The process stops and starts again, and the owner, having looked, lifts at once, at the time it opened, the
    /// pause of everything the driver made as it opened, where it made one ([`H::reopen`]): gives what opening gave,
    /// then what lifting it gave.
    pub fn restart(&mut self) -> Vec<Todo> {
        let mut todos = self.reopen();
        todos.extend(self.lift_the_starts_pause());
        todos
    }

    /// The owner lifts, at the time the driver opened, the pause of everything the driver made as it opened, where that
    /// pause holds; gives what lifting it gave.
    pub fn lift_the_starts_pause(&mut self) -> Vec<Todo> {
        if !self.d().everything_paused().is_some_and(|p| p.by == Pauser::Engine) {
            return Vec::new();
        }
        match self.try_pause_everything(Time(self.clock), false) {
            Outcome::Done(todos) => todos,
            Outcome::Stopped { .. } => panic!("the driver stopped"),
        }
    }

    /// The process stops and starts again: the driver is dropped and opened anew on a new store on the same file. It
    /// gives what was given and not yet answered, and nothing answered; opened after a stop at which nothing was
    /// left half done, it gives exactly what it said was pending. Opened while a work is going on and nothing pauses
    /// everything, it pauses everything itself, as it opens, and gives nothing more than what was pending: no work
    /// began a round after it was set, which the checks hold.
    pub fn reopen(&mut self) -> Vec<Todo> {
        let whole = self.d().fault().is_none() && !self.dead();
        let before = self.d().pending();
        let paused = self
            .reader
            .everything_paused()
            .expect("the pause of everything is read");
        let going_on = (self.reader.works().expect("the works are read").into_iter()).any(|head| {
            let work = self.reader.load(head.number).expect("the work loads");
            let flow = (self.flows.iter())
                .find(|f| f.name == head.flow)
                .expect("the work's flow");
            let decision = decide::decide(&flow.flow, &work.records);
            !matches!(decision, Decision::Ended(_) | Decision::Fault(_)) && !decide::paused(&work.records)
        });
        self.driver = None;
        let (opened, calls) = open_on_slots(&self.db, Plan::Never, self.flows.clone(), self.slots, Time(self.clock));
        let (driver, todos) = opened.expect("the driver opens again");
        self.driver = Some(driver);
        self.calls = calls;
        if whole {
            assert_eq!(
                todos, before,
                "opened again, the driver gives other than what was pending"
            );
        }
        // Where works were going on and nothing paused everything, the driver paused everything as it opened, and
        // nothing else; otherwise the pause of everything is as it was.
        let held = EverythingPaused {
            since: Time(self.clock),
            by: Pauser::Engine,
        };
        let expected = if paused.is_none() && going_on {
            Some(held)
        } else {
            paused
        };
        assert_eq!(
            self.d().everything_paused(),
            expected,
            "opened again, the driver holds another pause of everything than it is to"
        );
        for todo in &self.outstanding {
            // A round or a call given before may be to be stopped or withdrawn now, by what was kept as it stopped.
            let undone = todos.iter().any(|t| {
                t.work == todo.work
                    && match (&todo.effect, &t.effect) {
                        (Effect::Round { run, .. }, Effect::Stop { run: r }) => r == run,
                        (Effect::Call { call, .. }, Effect::Withdraw { call: c }) => c == call,
                        _ => false,
                    }
            });
            assert!(
                todos.contains(todo) || undone,
                "opened again, the driver no longer gives {todo:?}"
            );
        }
        for todo in &todos {
            assert!(
                !self.answered.contains(&key(todo)),
                "opened again, the driver gives what was answered: {todo:?}"
            );
            match todo.effect {
                Effect::Round { .. } | Effect::Call { .. } | Effect::Release { .. } => self.ever.insert(key(todo)),
                Effect::Stop { .. } | Effect::Withdraw { .. } => self.undone.insert(key(todo)),
            };
            assert!(
                self.kept_as_pending(todo),
                "given as the driver opened before the store holds that it is to be done: {todo:?}"
            );
        }
        self.outstanding = todos.clone();
        if self.checking {
            self.check();
        }
        todos
    }

    /// Whether the store holds that the todo is to be done: it is pending of the work as the store keeps it.
    fn kept_as_pending(&self, todo: &Todo) -> bool {
        let kept = self.reader.load(todo.work).expect("the work loads");
        let head = (self.reader.works().expect("the works are read").into_iter())
            .find(|head| head.number == todo.work)
            .expect("the work is kept");
        let flow = (self.flows.iter())
            .find(|f| f.name == head.flow)
            .expect("the work's flow");
        engine::pending(flow, &kept).contains(&todo.effect)
    }

    /// What the event answers is answered: the round it ends, or the call it gives the result of.
    fn answered(&mut self, work: WorkNumber, event: &Event) {
        let answers = |todo: &Todo| {
            todo.work == work
                && match (event, &todo.effect) {
                    (
                        Event::Returned { run, .. } | Event::Failed { run, .. },
                        Effect::Round { run: r, .. } | Effect::Stop { run: r },
                    ) => r == run,
                    (Event::Done { call, .. }, Effect::Call { call: c, .. } | Effect::Withdraw { call: c }) => {
                        c == call
                    }
                    (Event::Released | Event::NotReleased { .. }, Effect::Release { .. }) => true,
                    _ => false,
                }
        };
        let (done, left): (Vec<Todo>, Vec<Todo>) = self.outstanding.drain(..).partition(answers);
        self.outstanding = left;
        self.answered.extend(done.iter().map(key));
    }

    /// Takes what the driver gave to be done. A round or a call is given once; a stop replaces the round it stops, and
    /// a withdrawal the call it withdraws. Each is kept before it is given.
    fn given(&mut self, todos: &[Todo]) {
        for todo in todos {
            match &todo.effect {
                Effect::Round { .. } | Effect::Call { .. } | Effect::Release { .. } => {
                    assert!(self.ever.insert(key(todo)), "given twice: {todo:?}");
                }
                Effect::Stop { run } => {
                    self.undone.insert(key(todo));
                    let at = (self.outstanding.iter()).position(|t| {
                        t.work == todo.work && matches!(t.effect, Effect::Round { run: r, .. } if r == *run)
                    });
                    self.outstanding
                        .remove(at.expect("a round stopped was given and not yet answered"));
                }
                Effect::Withdraw { call } => {
                    self.undone.insert(key(todo));
                    let at = (self.outstanding.iter()).position(|t| {
                        t.work == todo.work && matches!(t.effect, Effect::Call { call: c, .. } if c == *call)
                    });
                    self.outstanding
                        .remove(at.expect("a call withdrawn was given and not yet answered"));
                }
            }
            assert!(
                self.kept_as_pending(todo),
                "given before the store holds that it is to be done: {todo:?}"
            );
            self.outstanding.push(todo.clone());
        }
    }

    /// What must hold after every call the driver took.
    pub fn check(&self) {
        let d = self.d();
        assert_eq!(
            self.reader
                .everything_paused()
                .expect("the pause of everything is read"),
            d.everything_paused(),
            "the store holds another pause of everything than the driver"
        );
        let heads: Vec<WorkHead> = d.works().map(|k| k.head.clone()).collect();
        assert_eq!(
            self.reader.works().expect("the works are read"),
            heads,
            "the store holds other works"
        );
        let items: Vec<Item> = d.works().map(|k| k.item.clone()).collect();
        assert_eq!(
            self.reader.items().expect("the items are read"),
            items,
            "the store holds other items"
        );
        // How the work of each item ended, if it has.
        let ends: BTreeMap<ItemNumber, Option<End>> = (d.works())
            .map(|k| {
                let end = match self.decision(k.head.number) {
                    Decision::Ended(end) => Some(end),
                    _ => None,
                };
                (k.head.item, end)
            })
            .collect();
        for kept in d.works() {
            let number = kept.head.number;
            let stored = self.reader.load(number).expect("the work loads");
            assert_eq!(
                &stored, kept.work,
                "the store holds otherwise than the driver of work {}",
                number.0
            );
            assert!(under_way(kept.work) <= 1, "work {} has two rounds under way", number.0);
            if let Some(paused) = d.everything_paused() {
                self.held_by_everything(number, kept.flow, kept.work, paused.since);
            }
            let shown = d.view(number).expect("the work is shown").decision;
            if !self.faulty.contains(&number) {
                self.agrees_with_what_is_pending(number, kept.flow, kept.work, &shown);
            }
            if !kept.work.records.started.is_empty() {
                assert!(
                    self.let_on.contains(&number),
                    "work {} ran though it was never let on",
                    number.0
                );
            }
            if let Some(value) = dependencies_value(kept.flow, kept.work) {
                let waited: BTreeSet<String> = (kept.item.depends_on.iter())
                    .filter_map(|on| match ends.get(on) {
                        Some(Some(End::Merged)) => None,
                        Some(Some(End::Cancelled)) => Some(format!("{}c", on.0)),
                        _ => Some(on.0.to_string()),
                    })
                    .collect();
                // The driver writes no block of a work that has ended: what it says is what it said then.
                if !matches!(shown, Decision::Ended(_)) && !self.faulty.contains(&number) {
                    let says = if waited.is_empty() { MERGED } else { WAITING };
                    assert_eq!(
                        value, says,
                        "the dependencies block of work {} does not agree",
                        number.0
                    );
                    let text = dependencies_text(kept.flow, kept.work).unwrap_or_default();
                    assert_eq!(
                        named(&text),
                        waited,
                        "the dependencies block of work {} does not say which items it waits for: {text}",
                        number.0
                    );
                }
                if let Some(end) = end_at(kept.flow, kept.work) {
                    let ours = dependencies_block(kept.flow);
                    let late = (kept.work.records.written.iter())
                        .any(|w| w.by == Outsider::Outside && Some(&w.block) == ours && w.at > end);
                    assert!(
                        !late,
                        "the dependencies block of work {} was written after it ended",
                        number.0
                    );
                }
                if !waited.is_empty() {
                    assert!(
                        kept.work.records.started.is_empty(),
                        "work {} ran before its dependencies merged",
                        number.0
                    );
                }
            }
        }
        assert_eq!(
            sorted(&d.pending()),
            sorted(&self.outstanding),
            "what is pending is not what was given and not yet answered"
        );
        // The rounds of agents given out and not yet answered, a round to be stopped among them until it has ended,
        // counted from what was given and not by the driver's own count; a work at fault takes none, as nothing more is
        // decided of it.
        let taking = (self.outstanding.iter())
            .filter(|todo| {
                let Some(kept) = d.work(todo.work) else {
                    return false;
                };
                let cell = match &todo.effect {
                    Effect::Round { cell, .. } => Some(cell.clone()),
                    Effect::Stop { run } => (kept.work.records.started.iter())
                        .find(|s| s.run == *run)
                        .map(|s| s.cell.clone()),
                    _ => None,
                };
                let at_fault = matches!(decide::decide(&kept.flow.flow, &kept.work.records), Decision::Fault(_));
                cell.is_some_and(|cell| !engine_cell(kept.flow, cell.as_str())) && !at_fault
            })
            .count();
        let slots = d.slots();
        assert_eq!(slots.taken, taking, "the driver says {} slots are taken", slots.taken);
        assert_eq!(
            slots.slots, self.slots,
            "the driver has other slots than it was opened with"
        );
        assert!(
            taking <= self.slots,
            "{taking} rounds take a slot, of {} slots",
            self.slots
        );
        if taking < self.slots {
            for kept in d.works() {
                assert_ne!(
                    d.held(kept.head.number),
                    Some(Holder::Slot),
                    "work {} waits for a slot while {} of {} are taken",
                    kept.head.number.0,
                    taking,
                    self.slots
                );
            }
        }
    }

    /// While the pause of everything holds, since this time, the work began no round after it but of a run for a note
    /// a person wrote ([`for_a_note`]), and none given again after it failed.
    fn held_by_everything(&self, number: WorkNumber, flow: &Compiled, work: &Work, since: Time) {
        for (i, round) in work.records.rounds.iter().enumerate() {
            if work.time_of(round.at).is_none_or(|time| time <= since) {
                continue;
            }
            let run = round.run;
            let nth = work.records.rounds[..=i].iter().filter(|r| r.run == run).count();
            let again = matches!(work.back.get(&(run, nth - 1)), Some(Back::Failed { .. }));
            assert!(
                for_a_note(flow, work, run) && !again,
                "work {} began round {nth} of {run:?} under the pause of everything",
                number.0
            );
        }
    }

    /// What the work does next agrees with what is to be done for it: after a call, no run is left to start and none
    /// left to be abandoned, and a run goes on with its round held only while the work is paused, unless a wait every
    /// work shares holds the rounds of the cell's agent; a work is busy with a
    /// run exactly when a round of that run is given to be done; and a work paused began no round after the pause but
    /// of a run for a note a person wrote ([`for_a_note`]), and none given again after it failed.
    fn agrees_with_what_is_pending(&self, number: WorkNumber, flow: &Compiled, work: &Work, shown: &Decision) {
        let paused = decide::paused(&work.records);
        let rounds: Vec<RunId> = (self.d().pending().into_iter())
            .filter(|t| t.work == number)
            .filter_map(|t| match t.effect {
                Effect::Round { run, .. } => Some(run),
                _ => None,
            })
            .collect();
        // The wait every work shares, where it holds rounds now, leaves the run of a cell whose rounds it holds to
        // start or go on: an agent's, and without the network the engine's own acts too.
        let agents = |cell: &decide::CellId| {
            let kind = (flow.cells.iter()).find(|spec| spec.id == *cell).map(|spec| spec.kind);
            let offline = self
                .d()
                .outage()
                .is_some_and(|o| o.failure == decide::run::Failure::Offline);
            self.d().holds(Time(self.clock)) && (kind == Some(flow::Kind::Agent) || offline)
        };
        match shown {
            Decision::Run { cell, .. } | Decision::GoOn { cell, .. } if agents(cell) => {}
            Decision::Run { .. } | Decision::Abandon { .. } | Decision::Fault(_) => {
                panic!("after a call work {} is left deciding {shown:?}", number.0)
            }
            Decision::GoOn { .. } => assert!(
                self.d().held(number).is_some(),
                "work {} goes on with no round given and nothing holding it: {shown:?}",
                number.0
            ),
            Decision::Paused => assert!(
                paused || self.d().everything_paused().is_some(),
                "work {} shows paused and is not",
                number.0
            ),
            Decision::Busy { run, .. } => {
                assert_eq!(
                    rounds,
                    [*run],
                    "work {} is busy with no round of its run given",
                    number.0
                );
            }
            Decision::Wait { .. } | Decision::Ended(_) | Decision::Idle => {}
        }
        if !matches!(shown, Decision::Busy { .. }) {
            assert_eq!(
                rounds,
                [],
                "work {} has a round given and is not busy: {shown:?}",
                number.0
            );
        }
        if paused {
            let since = (work.records.pauses.iter()).filter(|p| p.paused).map(|p| p.at).max();
            let resumed = (work.records.pauses.iter()).filter(|p| !p.paused).map(|p| p.at).max();
            if let Some(since) = since
                && resumed.is_none_or(|r| r < since)
            {
                for (i, round) in work.records.rounds.iter().enumerate() {
                    if round.at < since {
                        continue;
                    }
                    let run = round.run;
                    let nth = work.records.rounds[..=i].iter().filter(|r| r.run == run).count();
                    let again = failure_paused(work, run, nth - 1);
                    assert!(
                        for_a_note(flow, work, run) && !again,
                        "work {} began round {nth} of {run:?} after it was paused",
                        number.0
                    );
                }
            }
        }
    }

    /// The round of the work that is under way: its run, cell, number and what it was given.
    pub fn round(&self, work: WorkNumber) -> (RunId, String, usize, Given) {
        (self.d().pending().into_iter())
            .find_map(|todo| match todo.effect {
                Effect::Round {
                    run,
                    cell,
                    number,
                    given,
                } if todo.work == work => Some((run, cell.0, number, given)),
                _ => None,
            })
            .expect("a round of the work is under way")
    }

    /// The round of the work under way ends with what came back.
    pub fn returns(&mut self, work: WorkNumber, came: Came) -> Vec<Todo> {
        let (run, ..) = self.round(work);
        let spent = self.spent();
        self.on(work, Event::Returned { run, came, spent })
    }

    /// The round of the work under way hands in these blocks.
    pub fn hands_in(&mut self, work: WorkNumber, blocks: &[(&str, Option<&str>)]) -> Vec<Todo> {
        self.returns(work, hands_in(blocks))
    }

    /// What the next round to end took, told apart from every other.
    pub fn spent(&mut self) -> Spent {
        self.ended += 1;
        spent(self.ended)
    }

    /// The call has its result.
    pub fn answer(&mut self, work: WorkNumber, call: CallId) -> Vec<Todo> {
        let result = format!("the result of {}", call.0);
        self.on(work, Event::Done { call, result })
    }

    /// The outside source says this of [`PROJECT`], which the driver must take, and gives what is to be done.
    pub fn outside(&mut self, source: &str, content: &str) -> Vec<Todo> {
        self.outside_of(PROJECT, source, content)
    }

    /// The outside source says this of the project of this name, which the driver must take, and gives what is to be
    /// done.
    pub fn outside_of(&mut self, project: &str, source: &str, content: &str) -> Vec<Todo> {
        let now = self.now();
        let todos = (self.dm().outside(now, project, source, content)).expect("the driver takes what the outside says");
        self.given(&todos);
        if self.checking {
            self.check();
        }
        todos
    }

    /// The driver is told the time, which the harness's clock takes on to, and what a look at the network found,
    /// where one was taken; gives what is to be done.
    pub fn tick(&mut self, at: Time, reached: Option<bool>) -> Vec<Todo> {
        self.clock = self.clock.max(at.0);
        let todos = (self.dm().tick(at, reached)).expect("the driver takes the time");
        self.given(&todos);
        if self.checking {
            self.check();
        }
        todos
    }

    /// The work is let on.
    pub fn let_on(&mut self, work: WorkNumber) -> Vec<Todo> {
        self.on(work, Event::Resumed)
    }

    /// Whoever plans the work changes an item, which the driver must take; gives whether that changed it, and what is
    /// to be done.
    pub fn change(&mut self, item: ItemNumber, change: ItemChange) -> (bool, Vec<Todo>) {
        let now = self.now();
        let (changed, todos) = (self.dm().change(now, item, change)).expect("the change is taken");
        self.given(&todos);
        if self.checking {
            self.check();
        }
        (changed, todos)
    }

    /// What the work does next, as its view says.
    pub fn decision(&self, work: WorkNumber) -> Decision {
        self.d().view(work).expect("the work is there").decision
    }

    /// The value of the work's dependencies block.
    pub fn dependencies(&self, work: WorkNumber) -> Option<String> {
        let kept = self.d().work(work).expect("the work is there");
        dependencies_value(kept.flow, kept.work)
    }

    /// Takes the work through to its merge, each round handing in what the flow asks; gives what the call in which
    /// the merge finished gave.
    pub fn through(&mut self, work: WorkNumber) -> Vec<Todo> {
        // The work lets go of what it held as it merges, which is answered here: it says nothing of the merge.
        for _ in 0..20 {
            let (_, cell, ..) = self.round(work);
            let kept = self.d().work(work).expect("the work is there");
            let came = proper(kept.flow, &cell);
            let todos = self.returns(work, came);
            if self.decision(work) == Decision::Ended(End::Merged) {
                return self.released(todos);
            }
        }
        panic!("the work did not merge");
    }
}

impl H {
    /// The todos, each release among them answered: the work let go of what it held. Gives the rest, with what the
    /// answers gave.
    pub fn released(&mut self, todos: Vec<Todo>) -> Vec<Todo> {
        let (releases, mut rest): (Vec<Todo>, Vec<Todo>) =
            (todos.into_iter()).partition(|t| matches!(t.effect, Effect::Release { .. }));
        for release in releases {
            rest.extend(self.on(release.work, Event::Released));
        }
        rest
    }
}

/// Something done to the driver at a step of a history.
#[derive(Clone, Debug)]
pub enum Act {
    Add(NewItem),
    On(WorkNumber, Event),
    /// Whoever plans the work changes an item, which the driver may refuse.
    Change(ItemNumber, ItemChange),
    /// The owner sets the pause of everything, or lifts it.
    Everything(bool),
}

/// What the stops of a history came to.
#[derive(Default, Debug)]
pub struct Stops {
    /// Calls the driver said were kept as it stopped.
    pub kept: usize,
    /// Calls the driver said were not kept as it stopped.
    pub not_kept: usize,
    /// Stops of the process right after a call of the store.
    pub deaths: usize,
}

/// Does the act at this time, which the driver must take unless its store stops it. Where the driver stops, or the
/// process stops, it is opened anew, the owner lifting at once the pause of everything it made as it opened
/// ([`H::restart`]), and what it said was not kept is told again; gives what the driver gave where it did what it was
/// asked and the process went on.
pub fn perform(h: &mut H, now: Time, act: &Act, stops: &mut Stops) -> Option<Vec<Todo>> {
    h.clock = now.0;
    loop {
        let (stopped, todos) = match act {
            Act::Add(new) => match h.try_add(now, new.clone()) {
                Outcome::Done(_) => (None, Some(Vec::new())),
                Outcome::Stopped { kept } => (Some(kept), None),
            },
            Act::On(work, event) => match h.try_on(now, *work, event.clone()) {
                Outcome::Done(todos) => (None, Some(todos)),
                Outcome::Stopped { kept } => (Some(kept), None),
            },
            Act::Change(item, change) => match h.try_change(now, *item, change.clone()) {
                Outcome::Done(todos) => (None, Some(todos)),
                Outcome::Stopped { kept } => (Some(kept), None),
            },
            Act::Everything(paused) => match h.try_pause_everything(now, *paused) {
                Outcome::Done(todos) => (None, Some(todos)),
                Outcome::Stopped { kept } => (Some(kept), None),
            },
        };
        let dead = h.dead();
        if dead {
            stops.deaths += 1;
        }
        match stopped {
            None => {
                if dead {
                    h.restart();
                    h.check();
                    return None;
                }
                return todos;
            }
            Some(kept) => {
                if kept {
                    stops.kept += 1;
                } else {
                    stops.not_kept += 1;
                }
                h.restart();
                h.check();
                if kept {
                    return None;
                }
            }
        }
    }
}

/// The flow's dependencies block, which the driver writes, where it has one.
pub fn dependencies_block(flow: &Compiled) -> Option<&BlockId> {
    let (block, _) = (flow.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(DEPENDENCIES))?;
    Some(block)
}

/// The value the flow's dependencies block has now in the work, where the flow has one.
pub fn dependencies_value(flow: &Compiled, work: &Work) -> Option<String> {
    let block = dependencies_block(flow)?;
    let versions = decide::versions(&work.records);
    let value = versions.get(block).and_then(|v| v.last()).and_then(|v| v.value.clone());
    Some(value.unwrap_or_default())
}

/// The text of the flow's dependencies block now in the work, where it has one.
pub fn dependencies_text(flow: &Compiled, work: &Work) -> Option<String> {
    let block = dependencies_block(flow)?;
    let versions = decide::versions(&work.records);
    let at = versions.get(block)?.last()?.at;
    work.contents.get(&(block.clone(), at)).cloned()
}

/// The items a dependencies block's text names: each number, followed by `c` where the text says it was cancelled.
pub fn named(text: &str) -> BTreeSet<String> {
    let text = text.replace(" (cancelled)", "c");
    (text.split(|c: char| !c.is_ascii_alphanumeric()))
        .filter(|word| word.starts_with(|c: char| c.is_ascii_digit()))
        .map(str::to_string)
        .collect()
}

/// Where the work ended in the order of its records: as it was cancelled, or as its last cell finished.
pub fn end_at(flow: &Compiled, work: &Work) -> Option<Seq> {
    let r = &work.records;
    let last = flow.flow.cells().last().map(|c| c.id.clone());
    let merged = (r.finished.iter())
        .filter(|f| {
            r.started
                .iter()
                .any(|s| s.run == f.run && Some(&s.cell) == last.as_ref())
        })
        .map(|f| f.at)
        .min();
    [merged, r.cancelled].into_iter().flatten().min()
}

/// Whether the cell is the engine's own.
pub fn engine_cell(flow: &Compiled, cell: &str) -> bool {
    (flow.cells.iter()).any(|spec| spec.id.as_str() == cell && spec.kind == Kind::Engine)
}

/// What a round of this cell hands in when it does what the flow asks: each of the cell's outputs but the opinions,
/// a choice with its first value.
pub fn proper(flow: &Compiled, cell: &str) -> Came {
    let place = flow.flow.place(&cell.into()).expect("the cell is in the flow");
    let handed = (flow.flow.cells()[place].outputs.iter())
        .filter(|block| block.as_str() != "opinions")
        .map(|block| {
            let value = (flow.blocks.get(block))
                .and_then(|b| b.values.as_ref())
                .map(|values| values[0].clone());
            Handed {
                block: block.clone(),
                content: format!("the {block} by {cell}"),
                matters: true,
                value,
            }
        })
        .collect();
    Came::hands_in(handed)
}

/// What a round took, told apart by this number.
pub fn spent(n: u64) -> Spent {
    Spent {
        usd_micros: 7 * n,
        input_tokens: 11 * n,
        cached_tokens: 13 * n,
        output_tokens: 17 * n,
        ..Default::default()
    }
}

/// How many rounds of the work are under way.
pub fn under_way(work: &Work) -> usize {
    let r = &work.records;
    (r.started.iter())
        .filter(|s| {
            let began = r.rounds.iter().filter(|x| x.run == s.run).count();
            let ended = r.returned.iter().filter(|x| x.run == s.run).count();
            began > ended
        })
        .count()
}

/// Whether the run started for a note a person wrote: one of its reasons is a note that a person wrote since its cell
/// last ran (since the start of the run of its record, or with no record of its first run), after the version its
/// cell's record knew, and before the run started; for a cell with no record, a note the run started from. A cell no
/// run of which started before has not run. Read from the records and the flow's notes; it does not ask whether the
/// write changed the note's meaning, nor, with no record, whether the cell reads it, so it is a little wider than the
/// rule.
pub fn for_a_note(flow: &Compiled, work: &Work, run: RunId) -> bool {
    let r = &work.records;
    let Some(started) = r.started.iter().find(|s| s.run == run) else {
        return false;
    };
    // Since the cell last ran: since the start of its latest run that finished dealing with something, or with none of
    // its first run. A run that only asked for calls dealt with nothing.
    let of_cell = || (r.started.iter()).filter(|s| s.cell == started.cell && s.at < started.at);
    let record = (r.finished.iter())
        .filter(|f| f.at < started.at && f.dealt_with.as_ref().is_none_or(|d| !d.is_empty()))
        .filter_map(|f| of_cell().find(|s| s.run == f.run).map(|s| (f.at, s.at)))
        .max();
    let Some(since) = record.map(|(_, at)| at).or_else(|| of_cell().map(|s| s.at).min()) else {
        return false;
    };
    let by_person = |block: &decide::BlockId, was: Option<decide::Seq>| {
        flow.flow.is_note(block)
            && (r.written.iter()).any(|w| {
                w.by == Outsider::Person && w.block == *block && w.at > since && Some(w.at) > was && w.at < started.at
            })
    };
    started.because.iter().any(|c| match c {
        Cause::NoRecord => started.basis.keys().any(|block| by_person(block, None)),
        Cause::Changed { block, was, .. } => by_person(block, *was),
        Cause::Results { .. } => false,
    })
}

/// A write by a person or the outside, its content saying what was written.
pub fn wrote(block: &str, by: Outsider, value: Option<&str>) -> Event {
    Event::Wrote {
        block: block.into(),
        by,
        content: format!("{block}, as written by {by:?}"),
        matters: true,
        value: value.map(str::to_string),
    }
}

/// A block handed in, its content naming it.
pub fn handed(block: &str, value: Option<&str>) -> Handed {
    Handed {
        block: block.into(),
        content: format!("the {block}"),
        matters: true,
        value: value.map(str::to_string),
    }
}

/// What came back: these blocks handed in.
pub fn hands_in(blocks: &[(&str, Option<&str>)]) -> Came {
    Came::hands_in(blocks.iter().map(|(b, v)| handed(b, *v)).collect())
}

pub fn tool(name: &str) -> Ask {
    Ask::Tool {
        name: name.to_string(),
        input: format!("what {name} is run on"),
    }
}

pub fn question(who: Who) -> Ask {
    Ask::Person {
        who,
        question: "which way?".to_string(),
    }
}

/// What was given, each a round: its work, cell and number. Anything else given fails the test.
pub fn rounds(todos: &[Todo]) -> Vec<(WorkNumber, String, usize)> {
    (todos.iter())
        .map(|todo| match &todo.effect {
            Effect::Round { cell, number, .. } => (todo.work, cell.0.clone(), *number),
            other => panic!("not a round: {other:?}"),
        })
        .collect()
}

/// The work waits before this cell for the items its item depends on to have merged.
pub fn waits_for_dependencies(cell: &str) -> Decision {
    Decision::Wait {
        at: cell.into(),
        on: vec![Waited::Precondition {
            on: "ready".into(),
            by: vec![dependencies()],
        }],
    }
}

/// The source outside the work that writes whether the items a work depends on merged: the only one that can make a
/// test of them hold.
pub fn dependencies() -> Maker {
    Maker::Outside {
        source: DEPENDENCIES.to_string(),
    }
}

/// The place of the last write by a person or the outside.
pub fn last_written(h: &H, work: WorkNumber) -> Seq {
    let kept = h.d().work(work).expect("the work is there");
    kept.work.records.written.last().expect("a write").at
}

/// The block's id.
pub fn block(name: &str) -> BlockId {
    BlockId::from(name)
}

/// Whether the run's round of this number failed and the engine paused the work for it, which was not let on since:
/// the engine paused it after the round began, as for its failure or its going over the budget, and no letting on came
/// after the round's beginning.
pub fn failure_paused(work: &Work, run: RunId, number: usize) -> bool {
    if number == 0 || !matches!(work.back.get(&(run, number)), Some(Back::Failed { .. })) {
        return false;
    }
    let r = &work.records;
    let Some(began) = r.rounds.iter().filter(|e| e.run == run).map(|e| e.at).nth(number - 1) else {
        return false;
    };
    let paused = (r.pauses.iter()).any(|p| p.paused && p.by == decide::Pauser::Engine && p.at > began);
    let let_on = (r.pauses.iter()).any(|p| !p.paused && p.at > began);
    paused && !let_on
}

/// The work with its moments joined where one follows another at the same time: what was added to it, and when each
/// entry was added, however the calls that added them were cut. A stop puts the pause of everything between two
/// things a call would have added at once, and lifting it adds the second at the same time, as a moment of its own.
pub fn by_time(work: &Work) -> Work {
    let mut moments: Vec<Moment> = Vec::new();
    for moment in &work.moments {
        if moments.last().is_none_or(|last| last.at != moment.at) {
            moments.push(moment.clone());
        }
    }
    Work {
        moments,
        ..work.clone()
    }
}
