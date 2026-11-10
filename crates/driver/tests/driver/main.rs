//! The driver, subject by subject: each test opens a store of its own in a directory of its own, builds small flows,
//! and plays the part of everything outside the driver, answering what it gives to be done by telling it what came of
//! that. What each test expects is taken from the crate's documentation and the entries of the design, never from its
//! code. After every call the harness checks what must always hold: the store, read through a connection of its own,
//! holds of every work what the driver holds; what is given to be done is kept before it is given; nothing answered is
//! given again; and what was given and not yet answered is exactly what the driver says is pending. The tests are kept
//! in modules by subject, so that a new test goes into the module of what it tests.

mod events;
mod items;
mod kept;
mod opening;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use decide::{BlockId, CallId, Decision, Outcome, Outsider, RunId, Seq, TestName, Waited, Who};
use driver::{Driver, Failed, Fault, Seen, Test, Tests, Todo, dependencies_merged};
use engine::{Ask, Call, Came, Effect, Event, Given, Handed, Spent, Time, Work};
use flow::{Compiled, Source, compile};
use rusqlite::Connection;
use rusqlite::types::Value;
use store::{Item, ItemNumber, Store, WorkHead, WorkNumber};
use tempfile::TempDir;

/// The time of the first thing a test does.
const START: u64 = 1_800_000_000_000;

/// Every named test the driver is given: the one the driver has, and those the tests below write, each reading a
/// block of the work as it is.
fn tests() -> Tests {
    BTreeMap::from([
        (TestName::from("dependencies_merged"), dependencies_merged as Test),
        (TestName::from("verdict_yes"), verdict_yes as Test),
        (TestName::from("plan_asks_more"), plan_asks_more as Test),
        (TestName::from("request_given"), request_given as Test),
    ])
}

/// The verdict's value is yes.
fn verdict_yes(seen: &Seen<'_>) -> Outcome {
    if seen.value(&BlockId::from("verdict")).as_deref() == Some("yes") {
        Outcome::Holds
    } else {
        Outcome::Waits { who: Who::Owner }
    }
}

/// The plan, as its current version says it, asks for more.
fn plan_asks_more(seen: &Seen<'_>) -> Outcome {
    if seen
        .content(&BlockId::from("plan"))
        .is_some_and(|plan| plan.contains("more"))
    {
        Outcome::Holds
    } else {
        Outcome::Waits { who: Who::Owner }
    }
}

/// A request was written.
fn request_given(seen: &Seen<'_>) -> Outcome {
    if seen.content(&BlockId::from("request")).is_some() {
        Outcome::Holds
    } else {
        Outcome::Waits { who: Who::Owner }
    }
}

/// A flow compiled from the text of its files, against every named test the driver is given.
fn compiled(flow: &str, cells: &[(&str, &str)]) -> Compiled {
    let source = Source {
        flow: flow.to_string(),
        cells: (cells.iter())
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
    };
    let known: BTreeSet<TestName> = tests().keys().cloned().collect();
    compile(&source, &known).expect("the definition compiles")
}

/// A writer that waits for the items its item depends on to have merged, and reads a request and the opinions on what
/// it wrote; a judge that writes opinions for it and a verdict, a choice; and the merge, the engine's own cell.
/// Returns may be sent back twice, a run may make three calls, and a cell may go round three times.
fn judged() -> Compiled {
    let flow = r#"
        name = "judged"
        order = ["write", "judge", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.main]
        [lines.other]
        [tests]
        dependencies_merged = "Every item this one depends on has merged"
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
    "#;
    let write = r#"
        title = "Write"
        kind = "agent"
        line = "main"
        requires = ["dependencies_merged"]
        inputs = ["request", "opinions"]
        outputs = ["text"]
    "#;
    let judge = r#"
        title = "Judge"
        kind = "agent"
        line = "other"
        inputs = ["text"]
        outputs = ["opinions", "verdict"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        inputs = ["text", "verdict"]
        outputs = ["merged"]
    "#;
    compiled(flow, &[("write", write), ("judge", judge), ("merge", merge)])
}

/// A draft that waits for the items its item depends on to have merged and reads a request, and the merge.
fn short() -> Compiled {
    let flow = r#"
        name = "short"
        order = ["draft", "merge"]
        [limits]
        runs = 2
        returns = 2
        calls = 3
        [lines.main]
        [tests]
        dependencies_merged = "Every item this one depends on has merged"
        [blocks.request]
        people = true
        [blocks.plan]
        [blocks.merged]
        shown = true
    "#;
    let draft = r#"
        title = "Draft"
        kind = "agent"
        line = "main"
        requires = ["dependencies_merged"]
        inputs = ["request"]
        outputs = ["plan"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        inputs = ["plan"]
        outputs = ["merged"]
    "#;
    compiled(flow, &[("draft", draft), ("merge", merge)])
}

/// A cell that chooses a verdict and writes a plan; a cell that runs only when a named test finds the verdict's value
/// yes; another that runs only when a named test finds that the plan's content asks for more; and the merge.
fn forked() -> Compiled {
    let flow = r#"
        name = "forked"
        order = ["choose", "extra", "more", "merge"]
        [limits]
        runs = 2
        returns = 2
        calls = 3
        [lines.main]
        [tests]
        verdict_yes = "The verdict is yes"
        plan_asks_more = "The plan asks for more"
        [blocks.request]
        people = true
        [blocks.verdict]
        values = ["yes", "no"]
        shown = true
        [blocks.plan]
        [blocks.notes]
        [blocks.merged]
        shown = true
    "#;
    let choose = r#"
        title = "Choose"
        kind = "agent"
        line = "main"
        inputs = ["request"]
        outputs = ["verdict", "plan"]
    "#;
    let extra = r#"
        title = "Extra"
        kind = "agent"
        line = "main"
        when = { test = "verdict_yes" }
        inputs = ["plan"]
        outputs = ["notes"]
    "#;
    let more = r#"
        title = "More"
        kind = "agent"
        line = "main"
        when = { test = "plan_asks_more" }
        inputs = ["plan"]
        outputs = ["notes"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        inputs = ["plan", "notes"]
        outputs = ["merged"]
    "#;
    compiled(
        flow,
        &[("choose", choose), ("extra", extra), ("more", more), ("merge", merge)],
    )
}

/// A writer that waits, before it runs, for a named test to find that a person wrote a request; and the merge.
fn held() -> Compiled {
    let flow = r#"
        name = "held"
        order = ["write", "merge"]
        [limits]
        runs = 2
        returns = 2
        calls = 3
        [lines.main]
        [tests]
        request_given = "A request was written"
        [blocks.request]
        people = true
        [blocks.text]
        [blocks.merged]
        shown = true
    "#;
    let write = r#"
        title = "Write"
        kind = "agent"
        line = "main"
        requires = ["request_given"]
        inputs = ["request"]
        outputs = ["text"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        inputs = ["text"]
        outputs = ["merged"]
    "#;
    compiled(flow, &[("write", write), ("merge", merge)])
}

/// Every flow the driver is given.
fn flows() -> Vec<Compiled> {
    vec![judged(), short(), forked(), held()]
}

/// A database file in a directory of its own, which goes with it.
struct Db {
    _dir: TempDir,
    path: PathBuf,
}

impl Db {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a directory for the database");
        let path = dir.path().join("store.db");
        Self { _dir: dir, path }
    }

    /// A store on the file, opened anew.
    fn open(&self) -> Store {
        Store::open(&self.path).expect("the store opens")
    }
}

/// Every row of every table of the database, each table's rows in a fixed order: what the file holds.
fn dump(path: &Path) -> BTreeMap<String, Vec<Vec<String>>> {
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

/// Everything the driver shows of itself, and everything the file holds: what a refusal must leave as it was.
#[derive(Debug, PartialEq)]
struct State {
    items: Vec<Item>,
    works: Vec<(WorkHead, Work)>,
    pending: Vec<Todo>,
    fault: Option<Fault>,
    file: BTreeMap<String, Vec<Vec<String>>>,
}

/// A key that orders what is to be done the same way however it came to be held.
fn key(todo: &Todo) -> String {
    format!("{todo:?}")
}

fn sorted(todos: &[Todo]) -> Vec<Todo> {
    let mut todos = todos.to_vec();
    todos.sort_by_key(key);
    todos
}

/// The driver on a store of its own, and everything outside it: a clock, and what the driver gave to be done and was
/// not yet answered.
struct H {
    db: Db,
    /// None only while the driver is stopped, between dropping it and opening it again.
    driver: Option<Driver>,
    /// A store opened on the same file, through which the tests read what is kept.
    reader: Store,
    /// The driver's clock: a second for each call.
    clock: u64,
    /// How many rounds have ended, so that what each took is told apart.
    ended: u64,
    /// What the driver gave to be done and was not yet answered.
    outstanding: Vec<Todo>,
    /// Every round and every call the driver ever gave, as it gave them.
    ever: BTreeSet<String>,
}

impl H {
    /// The driver opened on an empty store, with every flow and every named test: it has nothing to do.
    fn new() -> Self {
        let db = Db::new();
        let (driver, todos) = Driver::open(db.open(), flows(), tests(), Time(START)).expect("the driver opens");
        assert_eq!(todos, vec![], "the driver has something to do on an empty store");
        let reader = db.open();
        Self {
            db,
            driver: Some(driver),
            reader,
            clock: START,
            ended: 0,
            outstanding: Vec::new(),
            ever: BTreeSet::new(),
        }
    }

    fn d(&self) -> &Driver {
        self.driver.as_ref().expect("the driver is open")
    }

    fn dm(&mut self) -> &mut Driver {
        self.driver.as_mut().expect("the driver is open")
    }

    /// The time of the next call; the clock moves a second on.
    fn now(&mut self) -> Time {
        self.clock += 1000;
        Time(self.clock)
    }

    /// Adds an item, which the driver must take, and checks what must hold.
    fn item(&mut self, title: &str, depends_on: &[ItemNumber]) -> ItemNumber {
        let now = self.now();
        let number = self.dm().add_item(now, title, depends_on).expect("the item is added");
        self.check();
        number
    }

    /// Adds a work, which the driver must take, and gives its number with what is to be done.
    fn work(&mut self, item: ItemNumber, flow: &str, paused: bool) -> (WorkNumber, Vec<Todo>) {
        let now = self.now();
        let (number, todos) = self.dm().add_work(now, item, flow, paused).expect("the work is added");
        self.given(&todos);
        self.check();
        (number, todos)
    }

    /// Tells the driver what happened, which it must take, and gives what is to be done.
    fn on(&mut self, work: WorkNumber, event: Event) -> Vec<Todo> {
        let now = self.now();
        let answers = event.clone();
        let todos = self.dm().happened(now, work, event).expect("what happened is taken");
        self.answered(work, &answers);
        self.given(&todos);
        self.check();
        todos
    }

    /// What the driver must refuse, which must leave the driver and the store exactly as they were; and why.
    fn refused(&mut self, act: impl FnOnce(&mut Driver, Time) -> Result<(), Failed>) -> Failed {
        let before = self.state();
        let now = self.now();
        let why = act(self.dm(), now).expect_err("the driver refuses");
        assert_eq!(self.state(), before, "a refusal changed the driver or the store");
        self.check();
        why
    }

    fn state(&self) -> State {
        let d = self.d();
        State {
            items: d.items().cloned().collect(),
            works: d.works().map(|k| (k.head.clone(), k.work.clone())).collect(),
            pending: d.pending(),
            fault: d.fault().cloned(),
            file: dump(&self.db.path),
        }
    }

    /// The driver is dropped, as at a stop, and opened again on the same file. What it gives as it opens is exactly
    /// what it said was pending before the stop, and is what was given and not yet answered.
    fn reopen(&mut self) -> Vec<Todo> {
        let before = self.d().pending();
        self.driver = None;
        let (driver, todos) =
            Driver::open(self.db.open(), flows(), tests(), Time(self.clock)).expect("the driver opens again");
        self.driver = Some(driver);
        assert_eq!(
            todos, before,
            "opened again, the driver gives other than what was pending"
        );
        assert_eq!(
            sorted(&todos),
            sorted(&self.outstanding),
            "opened again, the driver gives other than what was given and not yet answered"
        );
        self.check();
        todos
    }

    /// What the event answers is answered: the round it ends, or the call it gives the result of.
    fn answered(&mut self, work: WorkNumber, event: &Event) {
        let answers = |todo: &Todo| {
            todo.work == work
                && match (event, &todo.effect) {
                    (Event::Returned { run, .. }, Effect::Round { run: r, .. } | Effect::Stop { run: r }) => r == run,
                    (Event::Done { call, .. }, Effect::Call { call: c, .. } | Effect::Withdraw { call: c }) => {
                        c == call
                    }
                    _ => false,
                }
        };
        self.outstanding.retain(|todo| !answers(todo));
    }

    /// Takes what the driver gave to be done. A round or a call is given once; a stop replaces the round it stops, and
    /// a withdrawal the call it withdraws. Each is kept before it is given: the store, read anew, says it is to be
    /// done.
    fn given(&mut self, todos: &[Todo]) {
        for todo in todos {
            match &todo.effect {
                Effect::Round { .. } | Effect::Call { .. } => {
                    assert!(self.ever.insert(key(todo)), "given twice: {todo:?}");
                }
                Effect::Stop { run } => {
                    let at = (self.outstanding.iter()).position(|t| {
                        t.work == todo.work && matches!(t.effect, Effect::Round { run: r, .. } if r == *run)
                    });
                    self.outstanding
                        .remove(at.expect("a round stopped was given and not yet answered"));
                }
                Effect::Withdraw { call } => {
                    let at = (self.outstanding.iter()).position(|t| {
                        t.work == todo.work && matches!(t.effect, Effect::Call { call: c, .. } if c == *call)
                    });
                    self.outstanding
                        .remove(at.expect("a call withdrawn was given and not yet answered"));
                }
            }
            let kept = self.reader.load(todo.work).expect("the work loads");
            assert!(
                engine::pending(&kept).contains(&todo.effect),
                "given before the store holds that it is to be done: {todo:?}"
            );
            self.outstanding.push(todo.clone());
        }
    }

    /// What must hold after every call: the store holds what the driver holds, what was given and not yet answered is
    /// what the driver says is pending, and no work has two rounds under way.
    fn check(&self) {
        let d = self.d();
        let heads: Vec<WorkHead> = d.works().map(|k| k.head.clone()).collect();
        assert_eq!(
            self.reader.works().expect("the works are read"),
            heads,
            "the store holds other works"
        );
        let items: Vec<Item> = d.items().cloned().collect();
        assert_eq!(
            self.reader.items().expect("the items are read"),
            items,
            "the store holds other items"
        );
        for kept in d.works() {
            let number = kept.head.number;
            let stored = self.reader.load(number).expect("the work loads");
            assert_eq!(
                &stored, kept.work,
                "the store holds otherwise than the driver of work {}",
                number.0
            );
            assert!(under_way(kept.work) <= 1, "work {} has two rounds under way", number.0);
        }
        assert_eq!(
            sorted(&d.pending()),
            sorted(&self.outstanding),
            "what is pending is not what was given and not yet answered"
        );
    }

    /// The round of the work that is under way: its run, cell, number and what it was given.
    fn round(&self, work: WorkNumber) -> (RunId, String, usize, Given) {
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
    fn returns(&mut self, work: WorkNumber, came: Came) -> Vec<Todo> {
        let (run, ..) = self.round(work);
        let spent = self.spent();
        self.on(work, Event::Returned { run, came, spent })
    }

    /// The round of the work under way hands in these blocks.
    fn hands_in(&mut self, work: WorkNumber, blocks: &[(&str, Option<&str>)]) -> Vec<Todo> {
        self.returns(work, hands_in(blocks))
    }

    /// What the next round to end took, told apart from every other.
    fn spent(&mut self) -> Spent {
        self.ended += 1;
        let n = self.ended;
        Spent {
            usd_micros: 7 * n,
            input_tokens: 11 * n,
            cached_tokens: 13 * n,
            output_tokens: 17 * n,
        }
    }

    /// The call has its result.
    fn answer(&mut self, work: WorkNumber, call: CallId) -> Vec<Todo> {
        let result = format!("the result of {}", call.0);
        self.on(work, Event::Done { call, result })
    }

    /// What the work does next, as its view says.
    fn decision(&self, work: WorkNumber) -> Decision {
        self.d().view(work).expect("the work is there").decision
    }

    /// The blocks a round of this cell of the work hands in when it does what the flow asks: each of the cell's
    /// outputs but the opinions, a choice with its first value.
    fn outputs(&self, work: WorkNumber, cell: &str) -> Vec<(String, Option<String>)> {
        let kept = self.d().work(work).expect("the work is there");
        let place = kept.flow.flow.place(&cell.into()).expect("the cell is in the flow");
        (kept.flow.flow.cells()[place].outputs.iter())
            .filter(|block| block.as_str() != "opinions")
            .map(|block| {
                let value = (kept.flow.blocks.get(block))
                    .and_then(|b| b.values.as_ref())
                    .map(|values| values[0].clone());
                (block.0.clone(), value)
            })
            .collect()
    }

    /// Takes the work through to its merge, each round handing in what the flow asks; gives what the call in which
    /// the merge finished gave.
    fn through(&mut self, work: WorkNumber) -> Vec<Todo> {
        for _ in 0..20 {
            let (_, cell, ..) = self.round(work);
            let blocks = self.outputs(work, &cell);
            let blocks: Vec<(&str, Option<&str>)> = (blocks.iter()).map(|(b, v)| (b.as_str(), v.as_deref())).collect();
            let todos = self.hands_in(work, &blocks);
            if self.decision(work) == Decision::Ended(decide::End::Merged) {
                return todos;
            }
        }
        panic!("the work did not merge");
    }
}

/// How many rounds of the work are under way.
fn under_way(work: &Work) -> usize {
    let r = &work.records;
    (r.started.iter())
        .filter(|s| {
            let began = r.rounds.iter().filter(|x| x.run == s.run).count();
            let ended = r.returned.iter().filter(|x| x.run == s.run).count();
            began > ended
        })
        .count()
}

/// A write by a person or the outside, its content saying what was written.
fn wrote(block: &str, by: Outsider, value: Option<&str>) -> Event {
    Event::Wrote {
        block: block.into(),
        by,
        content: format!("{block}, as written by {by:?}"),
        matters: true,
        value: value.map(str::to_string),
    }
}

/// A block handed in, its content naming it.
fn handed(block: &str, value: Option<&str>) -> Handed {
    Handed {
        block: block.into(),
        content: format!("the {block}"),
        matters: true,
        value: value.map(str::to_string),
    }
}

/// What came back: these blocks handed in.
fn hands_in(blocks: &[(&str, Option<&str>)]) -> Came {
    Came::HandsIn(blocks.iter().map(|(b, v)| handed(b, *v)).collect())
}

fn tool(name: &str) -> Ask {
    Ask::Tool {
        name: name.to_string(),
        input: format!("what {name} is run on"),
    }
}

fn question(who: Who) -> Ask {
    Ask::Person {
        who,
        question: "which way?".to_string(),
    }
}

/// What was given, each a round: its work, cell and number. Anything else given fails the test.
fn rounds(todos: &[Todo]) -> Vec<(WorkNumber, String, usize)> {
    (todos.iter())
        .map(|todo| match &todo.effect {
            Effect::Round { cell, number, .. } => (todo.work, cell.0.clone(), *number),
            other => panic!("not a round: {other:?}"),
        })
        .collect()
}

/// The work waits before this cell for the items its item depends on to have merged, which the owner can give.
fn waits_for_dependencies(cell: &str) -> Decision {
    Decision::Wait {
        at: cell.into(),
        on: vec![Waited::Precondition {
            test: "dependencies_merged".into(),
            who: Who::Owner,
        }],
    }
}

/// The place of the last write by a person or the outside.
fn last_written(h: &H, work: WorkNumber) -> Seq {
    let kept = h.d().work(work).expect("the work is there");
    kept.work.records.written.last().expect("a write").at
}
