//! The store, subject by subject: each test opens a store of its own and says what it holds, exactly. Works are made
//! by driving the engine's loop, so that what is saved is what the engine produces. The tests are kept in modules by
//! subject, so that a new test goes into the module of what it tests.

mod diverged;
mod histories;
mod incremental;
mod items;
mod only_added;
mod opening;
mod round_trip;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use decide::{CallId, Outsider, RunId, Who};
use engine::{Ask, Came, Effect, Event, Handed, Spent, Time, Work, handle, pending};
use flow::{Compiled, Source, compile};
use rusqlite::Connection;
use rusqlite::types::Value;
use store::{Storage, Store, WorkNumber};
use tempfile::TempDir;

/// A writer that reads a request, the outside's main and the opinions on what it wrote; a judge that reads what the
/// writer wrote and the outside's checks, a choice, and writes opinions for the writer and a verdict, a choice; and a
/// merge, the engine's own cell, that waits for the owner's word: a choice, `go`, that people write and a named test
/// reads. Returns may be sent back twice, a run may make three calls, and a cell may go round three times.
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
        [tests.may_merge]
        says = "The owner let the work merge"
        kind = "all"
        reads = ["go"]
        is = "yes"
        [blocks.go]
        values = ["yes", "no"]
        people = true
        [blocks.request]
        people = true
        [blocks.main]
        outside = "main"
        [blocks.checks]
        values = ["pass", "fail"]
        outside = "main"
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
        inputs = ["request", "opinions", "main"]
        outputs = ["text"]
    "#;
    let judge = r#"
        title = "Judge"
        kind = "agent"
        line = "other"
        inputs = ["text", "checks"]
        outputs = ["opinions", "verdict"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        requires = ["may_merge"]
        inputs = ["text", "verdict"]
        outputs = ["merged"]
    "#;
    let source = Source {
        flow: flow.to_string(),
        cells: [("write", write), ("judge", judge), ("merge", merge)]
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
    };
    compile(&source, &["main".to_string()].into()).expect("the definition compiles")
}

/// The time of the first thing a test does.
const START: u64 = 1_800_000_000_000;

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

/// The item and the work a store of the tests holds first, added at fixed times so that two stores can be compared.
fn first_work(store: &mut Store) -> WorkNumber {
    let (_, work) = store
        .add("the item", &[], "judged", Time(START), &Work::default())
        .expect("the item is added");
    work
}

/// Every row of every table of the database, each table's rows in a fixed order: what the file holds, however the
/// rows came to be written.
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

/// A work driven through the engine's loop.
struct Driver {
    flow: Compiled,
    work: Work,
    /// The driver's clock: a second for each thing it hands the engine.
    clock: u64,
    /// How many rounds have ended, so that what each took is told apart.
    ended: u64,
    /// Where the work is saved after each thing that happens, when it is.
    saving: Option<Saving>,
}

/// A work saved after each thing that happens to it, and loaded back each time from a store opened anew on the file.
struct Saving {
    db: Db,
    store: Store,
    work: WorkNumber,
}

impl Driver {
    /// A new work on the judged flow, kept in memory alone.
    fn new() -> Self {
        Self {
            flow: judged(),
            work: Work::default(),
            clock: START,
            ended: 0,
            saving: None,
        }
    }

    /// A new work on the judged flow, saved after each thing that happens to it.
    fn saved() -> Self {
        let db = Db::new();
        let mut store = db.open();
        let work = first_work(&mut store);
        Self {
            saving: Some(Saving { db, store, work }),
            ..Self::new()
        }
    }

    /// Takes the event, which the loop must take, and gives what is to be done. Where the work is saved, it is saved
    /// and must load back as it is.
    fn on(&mut self, event: Event) -> Vec<Effect> {
        self.clock += 1000;
        let now = Time(self.clock);
        let effects = handle(&self.flow, &mut self.work, now, event).expect("the event is taken");
        if let Some(s) = &mut self.saving {
            s.store.save(s.work, &self.work).expect("the work is saved");
            let loaded = s.db.open().load(s.work).expect("the work loads");
            assert_eq!(
                loaded, self.work,
                "the work loaded from a store opened anew is not the work saved"
            );
        }
        effects
    }

    /// The run whose round is under way, and its cell.
    fn under_way(&self) -> (RunId, String) {
        (pending(&self.work).into_iter())
            .find_map(|e| match e {
                Effect::Round { run, cell, .. } => Some((run, cell.0)),
                _ => None,
            })
            .expect("a round is under way")
    }

    /// The round under way ends with what came back.
    fn end(&mut self, came: Came) -> Vec<Effect> {
        let (run, _) = self.under_way();
        let spent = self.spent();
        self.on(Event::Returned { run, came, spent })
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

    /// Every call that is to be made has its result, and so those that are to be made after them, until none is.
    fn answer(&mut self) {
        for _ in 0..10 {
            let calls: Vec<CallId> = (pending(&self.work).into_iter())
                .filter_map(|e| match e {
                    Effect::Call { call, .. } => Some(call),
                    _ => None,
                })
                .collect();
            if calls.is_empty() {
                return;
            }
            for call in calls {
                let result = format!("the result of {}", call.0);
                self.on(Event::Done { call, result });
            }
        }
        panic!("calls are still to be made");
    }

    /// Every call that is to be withdrawn is said to have its result: that it was withdrawn.
    fn withdraw(&mut self) {
        for e in pending(&self.work) {
            if let Effect::Withdraw { call } = e {
                let result = "withdrawn".to_string();
                self.on(Event::Done { call, result });
            }
        }
    }

    /// Every round that is to be stopped ends.
    fn stopped(&mut self) {
        for e in pending(&self.work) {
            if let Effect::Stop { run } = e {
                let spent = self.spent();
                let came = Came::Asks(vec![]);
                self.on(Event::Returned { run, came, spent });
            }
        }
    }

    /// A person or the outside writes the block.
    fn wrote(&mut self, block: &str, by: Outsider, value: Option<&str>) -> Vec<Effect> {
        self.on(Event::Wrote {
            block: block.into(),
            by,
            content: format!("{block}, as written by {by:?} at {}", self.clock),
            matters: true,
            value: value.map(str::to_string),
        })
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

/// What the writer hands in that does not meet what the flow requires: the judge's verdict.
fn wanting() -> Came {
    hands_in(&[("verdict", Some("yes"))])
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

/// A history that leaves an entry of every kind: writes by a person and the outside with values and without, runs
/// finished, sent back and abandoned, calls for a tool and a person, done and withdrawn, both limits of a run let go
/// by the owner, a pause and a resume, a cell let go, and a cancel with a round under way.
fn history(d: &mut Driver) {
    d.wrote("request", Outsider::Person, None);
    // The outside writes while the writer's first round is under way: as the round ends its run is abandoned.
    d.wrote("main", Outsider::Outside, None);
    d.end(hands_in(&[("text", None)]));
    // Sent back twice, then the owner is called at the limit of returns and lets the run go on.
    for _ in 0..3 {
        d.end(wanting());
    }
    d.answer();
    d.end(Came::Asks(vec![tool("a"), question(Who::Owner)]));
    d.answer();
    // Two more calls would pass the limit of calls: the owner is called, lets the run go on, and they are made.
    d.end(Came::Asks(vec![tool("b"), tool("c")]));
    d.answer();
    d.end(hands_in(&[("text", None)]));
    // The judge asks the architect during a pause: the call is made, and its next round waits for the resume.
    d.on(Event::Paused);
    d.end(Came::Asks(vec![question(Who::Architect)]));
    d.answer();
    d.on(Event::Resumed);
    // The judge rests at a question while the writer runs again for a new request: what the writer writes then
    // abandons the judge's run, and its call is withdrawn.
    d.end(Came::Asks(vec![question(Who::Owner)]));
    d.wrote("request", Outsider::Person, None);
    d.end(hands_in(&[("text", None)]));
    d.withdraw();
    d.wrote("checks", Outsider::Outside, Some("pass"));
    d.wrote("verdict", Outsider::Person, Some("no"));
    d.on(Event::LetGo { cell: "write".into() });
    // The judge hands in until a run of it finishes, writing a choice; the writer then runs for its opinions.
    for _ in 0..5 {
        if d.under_way().1 != "judge" {
            break;
        }
        d.end(hands_in(&[("opinions", None), ("verdict", Some("yes"))]));
    }
    assert_eq!(d.under_way().1, "write", "the judge never finished");
    let effects = d.on(Event::Cancelled);
    assert!(
        effects.iter().any(|e| matches!(e, Effect::Stop { .. })),
        "no round was under way at the cancel: {effects:?}"
    );
    d.stopped();
}
