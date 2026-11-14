//! The store, subject by subject: each test opens a store of its own and says what it holds, exactly. Works are made
//! by driving the engine's loop, so that what is saved is what the engine produces. The tests are kept in modules by
//! subject, so that a new test goes into the module of what it tests.

mod changes;
mod diverged;
mod everything;
mod histories;
mod incremental;
mod items;
mod observations;
mod only_added;
mod opening;
mod round_trip;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use decide::{
    CallId, Called, Done, Finished, Outsider, Pauser, PutIn, Returned, Round, RunId, Seq, Started, Who, Write, Written,
};
use engine::{Ask, Came, Effect, Event, Given, Handed, Spent, Time, Work, handle, pending};
use flow::{Compiled, Has, Source, compile};
use rusqlite::Connection;
use rusqlite::types::Value;
use store::{Storage, Store, WorkNumber};
use tempfile::TempDir;
use work::{Fill, Filled};

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
        effort = "high"
        line = "main"
        task = "Write the text the request asks for."
        inputs = ["request", "opinions", "main"]
        outputs = ["text"]
    "#;
    let judge = r#"
        title = "Judge"
        kind = "agent"
        effort = "high"
        line = "other"
        task = "Judge the text, and give a verdict."
        inputs = ["text", "checks"]
        outputs = ["opinions", "verdict"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        requires = { test = "may_merge" }
        inputs = ["text", "verdict"]
        outputs = ["merged"]
    "#;
    let source = Source {
        flow: flow.to_string(),
        cells: [("write", write), ("judge", judge), ("merge", merge)]
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
        ..Default::default()
    };
    let has = Has {
        sources: ["main".to_string()].into(),
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    compile(&source, &has).expect("the definition compiles")
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
        .add(&first("the item", "2", &[]), "judged", Time(START), &Work::default())
        .expect("the item is added");
    work
}

/// An item as it is added: this title, at this level, of the project `one`, depending on these items, and carrying no
/// tag.
fn first(title: &str, level: &str, depends_on: &[store::ItemNumber]) -> store::First {
    store::First {
        title: title.to_string(),
        level: Some(level.to_string()),
        project: Some("one".to_string()),
        depends_on: depends_on.to_vec(),
        tags: Vec::new(),
    }
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
        (pending(&self.flow, &self.work).into_iter())
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

    /// The round under way fails, for this reason: the engine pauses the work with it.
    fn fail(&mut self, why: &str) -> Vec<Effect> {
        let (run, _) = self.under_way();
        let spent = self.spent();
        let why = why.to_string();
        self.on(Event::Failed {
            run,
            why,
            failure: decide::run::Failure::Other,
            resets: None,
            spent,
        })
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
            // Some rounds tell the context they ended with and how often they were compacted, and some do not.
            context_tokens: n.is_multiple_of(2).then_some(19 * n),
            compactions: u32::try_from(n % 3).unwrap_or_default(),
        }
    }

    /// Every call that is to be made has its result, and so those that are to be made after them, until none is.
    fn answer(&mut self) {
        for _ in 0..10 {
            let calls: Vec<CallId> = (pending(&self.flow, &self.work).into_iter())
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
        for e in pending(&self.flow, &self.work) {
            if let Effect::Withdraw { call } = e {
                let result = "withdrawn".to_string();
                self.on(Event::Done { call, result });
            }
        }
    }

    /// Every round that is to be stopped ends.
    fn stopped(&mut self) {
        for e in pending(&self.flow, &self.work) {
            if let Effect::Stop { run } = e {
                let spent = self.spent();
                let came = Came::asks(vec![]);
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
    Came::hands_in(blocks.iter().map(|(b, v)| handed(b, *v)).collect())
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
/// finished, sent back and abandoned, calls for a tool and a person, done and withdrawn, a question asked and the
/// owner's answer to it, a run's limit of returns let go by the owner, a pause and a resume, a round that failed and
/// the engine's pause for it, a cell let go, and a cancel with a run at rest at the owner's call.
fn history(d: &mut Driver) {
    d.wrote("request", Outsider::Person, None);
    // The outside writes while the writer's first round is under way: its return deals with the request it was told
    // of, and the writer runs again for what the outside wrote. The return asks the owner a question beside, which the
    // owner answers.
    d.wrote("main", Outsider::Outside, None);
    let mut came = hands_in(&[("text", None)]);
    came.said.questions = vec![engine::Question::Asked {
        name: "which".to_string(),
        waits: false,
        recommends: Some(1),
        ask: "Which way?".to_string(),
        options: vec![
            engine::Way {
                does: "the short way".to_string(),
                follows: "less to read".to_string(),
            },
            engine::Way {
                does: "the long way".to_string(),
                follows: "more to read".to_string(),
            },
        ],
        about: None,
    }];
    d.end(came);
    let version = d.work.records.questions.last().expect("the question is kept").at;
    d.on(Event::Answer {
        question: "which".to_string(),
        version,
        option: 2,
        why: Some("The long way reads better.".to_string()),
    });
    // Sent back twice, then the owner is called at the limit of returns and lets the run go on.
    for _ in 0..3 {
        d.end(wanting());
    }
    d.answer();
    // A return that asks ends the run; the writer is entered again once both calls have their results.
    d.end(Came::asks(vec![tool("a"), question(Who::Owner)]));
    // Two lines begin a new session.
    d.on(Event::Anew {
        line: "main".to_string(),
        why: engine::Why::Limit { context: 9, limit: 5 },
    });
    d.on(Event::Anew {
        line: "other".to_string(),
        why: engine::Why::Owner,
    });
    d.answer();
    // The round of the run given the results is tried twice, once at each effort, the second time with an account of
    // its own.
    let (run, _) = d.under_way();
    for (effort, account) in [(None, None), (Some("high"), Some("an account"))] {
        let on = engine::RanOn {
            level: "2".to_string(),
            model: "a model".to_string(),
            effort: effort.map(str::to_string),
            account: account.map(str::to_string),
        };
        d.on(Event::Ran { run, attempt: 1, on });
    }
    // It asks again, and is entered again for those results.
    d.end(Came::asks(vec![tool("b"), tool("c")]));
    d.answer();
    d.end(hands_in(&[("text", None)]));
    // The judge asks the architect during a pause: the call is made, and entering it again waits for the resume.
    d.on(Event::Paused {
        by: Pauser::Owner,
        why: None,
    });
    d.end(Came::asks(vec![question(Who::Architect)]));
    d.answer();
    d.on(Event::Resumed);
    // The judge's round given the architect's answer fails, and the engine pauses the work; let on, the round is given
    // again.
    d.fail("The agent's program could not start.");
    d.on(Event::Resumed);
    // The judge asks the owner, and the writer runs again for a new request while the judge waits for the answer.
    d.end(Came::asks(vec![question(Who::Owner)]));
    d.wrote("request", Outsider::Person, None);
    d.end(hands_in(&[("text", None)]));
    d.wrote("checks", Outsider::Outside, Some("pass"));
    d.wrote("verdict", Outsider::Person, Some("no"));
    d.on(Event::LetGo { cell: "write".into() });
    // Answered, the judge is entered again, and hands in until a run of it finishes, writing a choice; the writer then
    // runs for its opinions.
    d.answer();
    for _ in 0..5 {
        if d.under_way().1 != "judge" {
            break;
        }
        d.end(hands_in(&[("opinions", None), ("verdict", Some("yes"))]));
    }
    assert_eq!(d.under_way().1, "write", "the judge never finished");
    // The writer is sent back to its limit of returns, and the work is cancelled while its run rests at the owner's
    // call: the run is abandoned and the call withdrawn.
    for _ in 0..3 {
        d.end(wanting());
    }
    let effects = d.on(Event::Cancelled { why: None });
    assert!(
        effects.iter().any(|e| matches!(e, Effect::Withdraw { .. })),
        "no call was withdrawn at the cancel: {effects:?}"
    );
    d.stopped();
    d.withdraw();
    // Nothing of it is under way then: what it held is let go.
    d.on(Event::Released);
    assert!(d.work.records.released().is_some(), "what the work held was not let go");
}

/// The [`history`], and then the work reopened, cancelled again and letting go again: what no store before items were
/// reopened could hold.
fn reopened_after(d: &mut Driver) {
    history(d);
    d.on(Event::Reopened {
        why: "the item it was joined into was too big".into(),
    });
    assert!(
        d.work.records.released().is_none(),
        "a work reopened holds its place again"
    );
    d.on(Event::Cancelled {
        why: Some("joined after all".into()),
    });
    d.on(Event::Released);
    assert!(
        d.work.records.released().is_some(),
        "what the work held was not let go again"
    );
}

/// A work on the judged flow whose returns are of the form every cell's return takes (design document
/// `data-and-context`), which the engine does not make yet, built entry by entry as such a work can go:
///
/// - The owner writes the request, and the writer is entered: its first try begins, and its session is filled with
///   which work and item it is for, the task, and the request whole. While its round is under way the owner writes the
///   request again, and that version is put in to it. Its return deals with the request and hands in the text.
/// - The judge is entered for the text, filled likewise. Its return asks for a tool and deals with nothing: the call is
///   made as the return is taken, which ends the run, and the cell waits until the call has its result.
/// - The judge is entered again, as a new run, for the result: its session, which holds the rest, is filled with the
///   result alone. That try finds the session lost, and the next, in a new one, is filled from the top. Its return
///   deals with nothing that ran it and hands in its opinions and a verdict.
/// - The owner writes the request a third time, and the writer is entered for that and the opinions: its session holds
///   the request it was told of last, so the request is filled as what changed since, and the opinions whole. Its
///   return deals with both and hands its text in as changing nothing in meaning.
fn of_the_new_form() -> Work {
    let mut b = Building::default();
    let first = b.person_writes("request");
    let writer = b.enters(
        "write",
        &[
            ("request", Some(first)),
            ("opinions", None),
            ("main", None),
            ("text", None),
        ],
    );
    b.filled(
        writer,
        1,
        vec![b.for_work(), Fill::part("task", WRITE), whole("request", first)],
    );
    let second = b.person_writes("request");
    b.put_in(writer, "request", second);
    b.returns(writer);
    let text = b.finishes(writer, &[("text", false, None)], &["request"]);

    let judge = b.enters(
        "judge",
        &[
            ("text", Some(text)),
            ("checks", None),
            ("opinions", None),
            ("verdict", None),
        ],
    );
    b.filled(
        judge,
        1,
        vec![b.for_work(), Fill::part("task", JUDGE), whole("text", text)],
    );
    b.returns(judge);
    let call = b.asks(judge, "check");
    b.finishes(judge, &[], &[]);
    b.answered(call, "check");

    let judge = b.enters(
        "judge",
        &[
            ("text", Some(text)),
            ("checks", None),
            ("opinions", None),
            ("verdict", None),
        ],
    );
    b.w.given.insert((judge, 1), Given::Results(vec![call]));
    b.filled(judge, 1, vec![Fill::Results(vec![call])]);
    b.anew("other");
    let refill = vec![
        b.for_work(),
        Fill::part("task", JUDGE),
        whole("text", text),
        Fill::Results(vec![call]),
    ];
    b.filled_at(judge, 1, 2, refill);
    b.returns(judge);
    let opinions = b.finishes(
        judge,
        &[("opinions", false, None), ("verdict", false, Some("yes"))],
        &[],
    );

    let third = b.person_writes("request");
    let basis = [
        ("request", Some(third)),
        ("opinions", Some(opinions)),
        ("main", None),
        ("text", Some(text)),
    ];
    let writer = b.enters("write", &basis);
    let fills = vec![
        Fill::Change {
            block: "request".into(),
            version: third,
            since: second,
        },
        whole("opinions", opinions),
    ];
    b.filled(writer, 1, fills);
    b.returns(writer);
    b.finishes(writer, &[("text", true, None)], &["request", "opinions"]);
    b.w
}

/// The tasks of the judged flow's two agents.
const WRITE: &str = "Write the text the request asks for.";
const JUDGE: &str = "Judge the text, and give a verdict.";

/// A version of a block filled whole.
fn whole(block: &str, version: Seq) -> Fill {
    Fill::Whole {
        block: block.into(),
        version,
    }
}

/// A work built entry by entry, each at the next place.
#[derive(Default)]
struct Building {
    w: Work,
}

impl Building {
    fn person_writes(&mut self, block: &str) -> Seq {
        let at = self.w.tick();
        (self.w.contents).insert(
            (block.into(), at),
            format!("the {block}, as the owner wrote it at {}", at.0),
        );
        self.w.records.written.push(Written {
            block: block.into(),
            by: Outsider::Person,
            matters: true,
            value: None,
            at,
        });
        at
    }

    /// The cell is entered: a run starts from these versions and its first round begins, given its task.
    fn enters(&mut self, cell: &str, basis: &[(&str, Option<Seq>)]) -> RunId {
        let run = RunId(self.w.name());
        let at = self.w.tick();
        self.w.records.started.push(Started {
            run,
            cell: cell.into(),
            at,
            because: vec![],
            basis: basis.iter().map(|(b, v)| ((*b).into(), *v)).collect(),
        });
        let at = self.w.tick();
        self.w.records.rounds.push(Round { run, at });
        self.w.given.insert((run, 1), Given::Task);
        run
    }

    fn for_work(&self) -> Fill {
        Fill::part("work", "Work W1, on the item #1.")
    }

    /// The first try of the round is filled with these.
    fn filled(&mut self, run: RunId, round: usize, fills: Vec<Fill>) {
        self.filled_at(run, round, 1, fills);
    }

    /// The try of the round begins, on a level and model, and its session is filled with these.
    fn filled_at(&mut self, run: RunId, round: usize, attempt: usize, fills: Vec<Fill>) {
        let at = self.w.tick();
        self.w.ran.push(engine::Ran {
            at,
            run,
            round,
            attempt,
            on: engine::RanOn {
                level: "2".to_string(),
                model: "a model".to_string(),
                effort: None,
                account: None,
            },
        });
        let at = self.w.tick();
        self.w.filled.push(Filled {
            at,
            run,
            round,
            attempt,
            fills,
        });
    }

    /// The line begins a new session: the program no longer had the one it was in.
    fn anew(&mut self, line: &str) {
        let at = self.w.tick();
        self.w.sessions.push(engine::Session {
            at,
            line: line.to_string(),
            why: engine::Why::Lost,
        });
    }

    fn put_in(&mut self, run: RunId, block: &str, version: Seq) {
        let at = self.w.tick();
        self.w.records.put_in.push(PutIn {
            run,
            block: block.into(),
            version,
            at,
        });
    }

    fn returns(&mut self, run: RunId) {
        let at = self.w.tick();
        self.w.records.returned.push(Returned { run, at });
    }

    /// The run finishes, its return accepted: these blocks handed in, each with whether its meaning is unchanged and
    /// its value, and these dealt with. Gives the place that names the versions it wrote.
    fn finishes(&mut self, run: RunId, writes: &[(&str, bool, Option<&str>)], dealt_with: &[&str]) -> Seq {
        let at = self.w.tick();
        for (block, _, _) in writes {
            (self.w.contents).insert(((*block).into(), at), format!("the {block}, of run {}", run.0));
        }
        self.w.records.finished.push(Finished {
            run,
            at,
            writes: (writes.iter())
                .map(|(block, meaning_unchanged, value)| Write {
                    block: (*block).into(),
                    meaning_unchanged: *meaning_unchanged,
                    value: value.map(str::to_string),
                })
                .collect(),
            dealt_with: Some(dealt_with.iter().map(|b| (*b).into()).collect()),
        });
        at
    }

    /// The tool the run's return asks for is called, as the return is taken.
    fn asks(&mut self, run: RunId, tool: &str) -> CallId {
        let call = CallId(self.w.name());
        let at = self.w.tick();
        self.w.records.called.push(Called {
            call,
            run,
            on: None,
            at,
        });
        self.w.calls.insert(
            call,
            engine::Call::Asked(Ask::Tool {
                name: tool.to_string(),
                input: format!("what {tool} is run on"),
            }),
        );
        call
    }

    /// The call has its result, whole.
    fn answered(&mut self, call: CallId, tool: &str) {
        let at = self.w.tick();
        self.w.records.done.push(Done { call, at });
        self.w.results.insert(call, format!("the whole result of {tool}"));
    }
}
