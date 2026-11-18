//! When a line of a work begins a new session: the driver records it with why, from the work's records and the
//! installation's policy, and writes the block of the source `session` that a cell of the line reads, so that the cell
//! runs again before the line's next round; where each line has a block of its own, a new session of one line writes
//! its block alone and runs again no cell of another line.

use std::sync::{Arc, Mutex};

use decide::BlockId;
use driver::Policy;
use driver::{FirstWrite, Found, NewItem, SESSION};
use engine::RanOn;
use engine::{Ask, Handed, Session, Why};
use flow::{Compiled, Has};
use work::Measures;
use work::sessions::of_round;

use super::*;

/// The flow: `load` reads the session block and loads the engineer's session, `build` goes on in it unless a person
/// stops it, and `review` has a line of its own.
fn flow() -> Compiled {
    let flow = r#"
        name = "sessions"
        order = ["load", "build", "review", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.engineer]
        [lines.review]
        [tests.going]
        says = "Nobody stopped the build"
        kind = "none"
        reads = ["stop"]
        is = "yes"
        [blocks.request]
        people = true
        [blocks.stop]
        values = ["yes", "no"]
        people = true
        shown = true
        [blocks.session]
        outside = "session"
        writes = { text = "A new session of the line `{line}`: {why}.", limit = "its context was {context} tokens as its last round ended, past the limit of {limit} for its line", model = "its rounds now run on another model than {was}, which it was begun on", account = "its rounds now run under another account than it was begun under", owner = "the owner asked for one", flagged = "the service's safety check flagged its last round", guarded = "the run guard stopped its last round", too_long = "its last round's prompt was too long", lost = "the agents' program no longer had the session it was in", compacted = "The session of the line `{line}` was compacted: it holds only a summary of what it read." }
        [blocks.tree]
        outside = "tree"
        shown = true
        [blocks.context]
        [blocks.code]
        [blocks.verdict]
        [blocks.merged]
        shown = true
    "#;
    let cell = |line: &str, watches: &str, outputs: &str| {
        // Each datum it watches given as it last worked from it and as it is now.
        let kinds: Vec<String> = (watches.split('"').skip(1).step_by(2))
            .map(|datum| format!("{datum} = \"last_and_latest\""))
            .collect();
        format!(
            "title = \"A cell\"\nkind = \"agent\"\nline = \"{line}\"\neffort = \"low\"\ntask = \"Do it.\"\nwatches = {watches}\noutputs = {outputs}\nkinds = {{ {} }}\n",
            kinds.join(", ")
        )
    };
    let load = cell("engineer", r#"["request", "session"]"#, r#"["context"]"#);
    let build = cell("engineer", r#"["request", "context"]"#, r#"["code"]"#) + "when = { test = \"going\" }\n";
    let review = cell("review", r#"["code"]"#, r#"["verdict"]"#);
    let merge =
        "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nwatches = [\"verdict\"]\noutputs = [\"merged\"]\n";
    let source = flow::Source {
        flow: flow.to_string(),
        cells: [
            ("load", load),
            ("build", build),
            ("review", review),
            ("merge", merge.to_string()),
        ]
        .into_iter()
        .map(|(id, text)| (id.to_string(), text))
        .collect(),
        ..Default::default()
    };
    let has = Has {
        sources: [SESSION.to_string(), "tree".to_string()].into(),
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the flow compiles")
}

/// The words the driver writes into a session block, as the flow's own datum gives them.
const SESSION_WRITES: &str = r#"writes = { text = "A new session of the line `{line}`: {why}.", limit = "its context was {context} tokens as its last round ended, past the limit of {limit}", model = "its rounds now run on another model than {was}, which it was begun on", account = "its rounds now run under another account than it was begun under", owner = "the owner asked for one", flagged = "the service's safety check flagged its last round", lost = "the agents' program no longer had the session it was in", compacted = "The session of the line `{line}` was compacted: it holds only a summary of what it read." }"#;

/// Two lines, each loading its session by a cell that reads a session block of its own: `load` the engineer's, which
/// `build` goes on in, and `look` the review's, which `review` goes on in.
fn two_lines() -> Compiled {
    let flow = format!(
        r#"
        name = "sessions"
        order = ["load", "build", "look", "review", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.engineer]
        [lines.review]
        [blocks.request]
        people = true
        [blocks.engineer_session]
        outside = "session"
        {SESSION_WRITES}
        [blocks.review_session]
        outside = "session"
        {SESSION_WRITES}
        [blocks.context]
        [blocks.code]
        [blocks.notes]
        [blocks.verdict]
        [blocks.merged]
        shown = true
    "#
    );
    let cell = |line: &str, watches: &str, outputs: &str| {
        // Each datum it watches given as it last worked from it and as it is now.
        let kinds: Vec<String> = (watches.split('"').skip(1).step_by(2))
            .map(|datum| format!("{datum} = \"last_and_latest\""))
            .collect();
        format!(
            "title = \"A cell\"\nkind = \"agent\"\nline = \"{line}\"\neffort = \"low\"\ntask = \"Do it.\"\nwatches = {watches}\noutputs = {outputs}\nkinds = {{ {} }}\n",
            kinds.join(", ")
        )
    };
    let merge =
        "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nwatches = [\"verdict\"]\noutputs = [\"merged\"]\n";
    let source = flow::Source {
        flow,
        cells: [
            (
                "load",
                cell("engineer", r#"["request", "engineer_session"]"#, r#"["context"]"#),
            ),
            ("build", cell("engineer", r#"["request", "context"]"#, r#"["code"]"#)),
            ("look", cell("review", r#"["code", "review_session"]"#, r#"["notes"]"#)),
            ("review", cell("review", r#"["code", "notes"]"#, r#"["verdict"]"#)),
            ("merge", merge.to_string()),
        ]
        .into_iter()
        .map(|(id, text)| (id.to_string(), text))
        .collect(),
        ..Default::default()
    };
    let has = Has {
        sources: [SESSION.to_string()].into(),
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the flow compiles")
}

/// What a round runs on where the installation's model is this one.
fn on(model: &str) -> RanOn {
    RanOn {
        level: "1".to_string(),
        model: model.to_string(),
        effort: None,
        account: None,
    }
}

/// Opens the driver again on the store with this policy, and the owner lifts at once, at the time it opened, the pause
/// of everything it made as it opened on the work going on: gives what both gave.
fn reopened(s: &mut S, policy: Policy) -> Vec<Todo> {
    let now = Time(s.clock);
    let (mut driver, mut todos) = Driver::open(Box::new(s.db.open()), vec![s.flow.clone()], SLOTS, Some(policy), now)
        .expect("the driver opens again");
    if driver
        .everything_paused()
        .is_some_and(|paused| paused.by == Pauser::Engine)
    {
        todos.extend(driver.pause_everything(now, false).expect("the pause is lifted"));
    }
    s.driver = driver;
    todos
}

/// The policy where every round runs on this model, a history of one value giving a line, and no session kept warm.
fn policy(model: &str) -> Policy {
    let model = model.to_string();
    Policy {
        samples: 1,
        on: Box::new(move |_, _| Some(on(&model))),
        warm: None,
    }
}

/// The account rounds run under, as the installation's settings say it each time the policy is asked: none for the
/// program's own default directory.
type Account = Arc<Mutex<Option<String>>>;

fn account(dir: Option<&str>) -> Account {
    Arc::new(Mutex::new(dir.map(str::to_string)))
}

/// The policy where every round runs on this model, under the account as it is when asked, a history of one value
/// giving a line.
fn policy_under(model: &str, account: Account) -> Policy {
    let model = model.to_string();
    Policy {
        samples: 1,
        warm: None,
        on: Box::new(move |_, _| {
            Some(RanOn {
                account: account.lock().unwrap().clone(),
                ..on(&model)
            })
        }),
    }
}

/// A driver on its own store, with the flow and the policy, and a work on the flow let on.
struct S {
    db: Db,
    flow: Compiled,
    driver: Driver,
    work: WorkNumber,
    clock: u64,
    /// The model the rounds run on.
    model: String,
    /// The account the rounds' tries begin under.
    account: Option<String>,
    /// What each round of the work that ends spends, in millionths of a dollar.
    usd: u64,
}

impl S {
    fn new() -> S {
        S::with(policy("opus"), None)
    }

    /// A driver with the policy, whose rounds' tries begin under the account.
    fn with(policy: Policy, account: Option<&str>) -> S {
        S::on_flow(flow(), policy, account)
    }

    /// A driver on the flow with the policy, whose rounds' tries begin under the account.
    fn on_flow(flow: Compiled, policy: Policy, account: Option<&str>) -> S {
        let db = Db::new();
        let (mut driver, _) = Driver::open(
            Box::new(db.open()),
            vec![flow.clone()],
            SLOTS,
            Some(policy),
            Time(START),
        )
        .unwrap();
        // A work before, whose one round, of the line `engineer`, said what its calls came to and failed with a
        // context of 500 tokens, pausing it: the line's history, whose one value gives a line of 500 tokens. It
        // took no tokens, so that its session is not kept warm. The rounds of the work the tests run
        // say nothing of their calls, and so add nothing to it.
        let earlier = NewItem {
            title: "Before".to_string(),
            level: "1".to_string(),
            project: PROJECT.to_string(),
            tags: Vec::new(),
            depends_on: Vec::new(),
            flow: "sessions".to_string(),
            writes: vec![FirstWrite {
                block: "request".into(),
                content: "Greet the user.".to_string(),
                value: None,
            }],
        };
        let (_, before, _) = driver.add(Time(START), earlier).unwrap();
        let todos = driver.happened(Time(START), before, Event::Resumed).unwrap();
        let run = (todos.iter())
            .find_map(|t| match &t.effect {
                Effect::Round { run, .. } if t.work == before => Some(*run),
                _ => None,
            })
            .expect("the earlier work's first round");
        let measures = Measures {
            calls: 1,
            ..Measures::default()
        };
        let events = [
            Event::Ran {
                run,
                attempt: 1,
                on: on("opus"),
            },
            Event::Measured {
                run,
                attempt: 1,
                measures,
            },
            Event::Failed {
                run,
                why: "the agent could not go on".to_string(),
                failure: decide::run::Failure::Other,
                resets: None,
                spent: Spent {
                    context_tokens: Some(500),
                    ..Default::default()
                },
            },
        ];
        for event in events {
            driver.happened(Time(START), before, event).unwrap();
        }
        let new = NewItem {
            title: "Greet".to_string(),
            level: "1".to_string(),
            project: PROJECT.to_string(),
            depends_on: Vec::new(),
            tags: Vec::new(),
            flow: "sessions".to_string(),
            writes: vec![FirstWrite {
                block: "request".into(),
                content: "Greet the user.".to_string(),
                value: None,
            }],
        };
        let (_, work, _) = driver.add(Time(START), new).unwrap();
        driver.happened(Time(START), work, Event::Resumed).unwrap();
        S {
            db,
            flow,
            driver,
            work,
            clock: START,
            model: "opus".to_string(),
            account: account.map(str::to_string),
            usd: 0,
        }
    }

    fn kept(&self) -> &engine::Work {
        self.driver.work(self.work).expect("the work").work
    }

    /// Another work of the flow, let on, with its first round under way: the work and that round's run.
    fn another(&mut self) -> (WorkNumber, RunId) {
        let new = NewItem {
            title: "Another".to_string(),
            level: "1".to_string(),
            project: PROJECT.to_string(),
            depends_on: Vec::new(),
            tags: Vec::new(),
            flow: "sessions".to_string(),
            writes: vec![FirstWrite {
                block: "request".into(),
                content: "Greet the user.".to_string(),
                value: None,
            }],
        };
        self.clock += 1000;
        let (_, work, _) = self.driver.add(Time(self.clock), new).expect("the work is added");
        let todos = (self.driver)
            .happened(Time(self.clock), work, Event::Resumed)
            .expect("it is let on");
        let run = (todos.iter())
            .find_map(|todo| match &todo.effect {
                Effect::Round { run, .. } if todo.work == work => Some(*run),
                _ => None,
            })
            .expect("its first round");
        (work, run)
    }

    fn on(&mut self, event: Event) {
        self.clock += 1000;
        self.driver
            .happened(Time(self.clock), self.work, event)
            .expect("the driver takes it");
    }

    /// The round under way: its run and its cell.
    fn round(&self) -> (RunId, String) {
        (self.driver.pending().into_iter())
            .find_map(|todo| match todo.effect {
                Effect::Round { run, cell, .. } if todo.work == self.work => Some((run, cell.0)),
                _ => None,
            })
            .expect("a round is under way")
    }

    /// The round under way ends: it hands in what its cell writes, or asks for a tool where `asks`, having ended with
    /// this context.
    fn ends(&mut self, context: u64, came: Came) {
        let (run, _) = self.round();
        // The round's one try began on the model, under the account, as its carrier says.
        self.began();
        let spent = Spent {
            input_tokens: 10,
            context_tokens: Some(context),
            usd_micros: self.usd,
            ..Default::default()
        };
        self.on(Event::Returned { run, came, spent });
    }

    /// The one try of the round under way begins on the model, under the account, as its carrier says.
    fn began(&mut self) {
        let (run, _) = self.round();
        let ran = RanOn {
            account: self.account.clone(),
            ..on(&self.model)
        };
        self.on(Event::Ran {
            run,
            attempt: 1,
            on: ran,
        });
    }

    fn hands_in(&mut self, context: u64, matters: bool) {
        let (_, cell) = self.round();
        let block = match cell.as_str() {
            "load" => "context",
            "build" => "code",
            "look" => "notes",
            _ => "verdict",
        };
        let handed = Handed {
            block: block.into(),
            content: format!("the {block}"),
            matters,
            value: None,
        };
        self.ends(context, Came::hands_in(vec![handed]));
    }

    fn asks(&mut self, context: u64) {
        let ask = Ask::Tool {
            name: "read".to_string(),
            input: "request".to_string(),
        };
        self.ends(context, Came::asks(vec![ask]));
    }

    fn sessions(&self) -> Vec<(String, Why)> {
        (self.kept().sessions.iter())
            .map(|s: &Session| (s.line.clone(), s.why.clone()))
            .collect()
    }

    /// The versions of the session block, as their content.
    fn session_block(&self) -> Vec<String> {
        self.versions_of("session")
    }

    /// The versions of the block, as their content.
    fn versions_of(&self, block: &str) -> Vec<String> {
        let block = BlockId::from(block);
        (self.kept().records.written.iter())
            .filter(|w| w.block == block)
            .map(|w| self.kept().contents[&(block.clone(), w.at)].clone())
            .collect()
    }

    fn engineer(&self, run: RunId, number: usize) -> (u32, bool) {
        let work = self.kept();
        let on_line = |r: RunId| {
            let cell = work
                .records
                .started
                .iter()
                .find(|s| s.run == r)
                .map(|s| s.cell.as_str());
            matches!(cell, Some("load" | "build"))
        };
        of_round(work, "engineer", on_line, run, number)
    }
}

#[test]
fn a_line_goes_on_in_its_first_session_while_nothing_ends_it() {
    let mut s = S::new();
    s.hands_in(100, true);
    assert!(s.sessions().is_empty(), "no new session");
    assert!(s.session_block().is_empty(), "a first session writes no block");
    let (build, cell) = s.round();
    assert_eq!(cell, "build", "the line goes on");
    assert_eq!(s.engineer(build, 1), (0, false), "in the session the load began");
}

// However much context a round of a line ends with, the line goes on in its session: the agents' program compacts it
// at the installation's number, which every round's command line gives it, and no size of a session has the engine
// begin a new one (decision `a-session-is-compacted-at-the-installations-number`).
#[test]
fn a_round_ending_with_more_context_than_the_installations_number_has_its_line_go_on_in_its_session() {
    let mut s = S::new();
    s.hands_in(100, true);
    let (build, _) = s.round();
    // The build's run ends with far more context than any number an installation sets.
    s.hands_in(10_000_000, true);
    assert!(s.sessions().is_empty(), "no new session for the context it ended with");
    assert!(s.session_block().is_empty(), "the loading cell is not run again");
    // The line goes on in the session its load began, and the loading cell has run once.
    let (review, cell) = s.round();
    assert_eq!(cell, "review", "the engineer's line handed in; the review goes on");
    assert_ne!(review, build);
    assert_eq!(runs(&s, "load"), 1);
}

#[test]
fn a_new_session_of_one_line_writes_its_own_block_alone_and_runs_again_no_cell_of_another_line() {
    let mut s = S::on_flow(two_lines(), policy("opus"), None);
    let asked = |line: &str| Event::Anew {
        line: line.to_string(),
        why: Why::Owner,
    };
    s.hands_in(100, true);
    assert_eq!(s.round().1, "build");
    // The owner asks the engineer's line for a new session while the build's round is under way: the line begins one
    // as the round ends, and its own block is written.
    s.began();
    s.on(asked("engineer"));
    s.hands_in(100, true);
    assert!(matches!(s.sessions().last(), Some((line, Why::Owner)) if line == "engineer"));
    assert_eq!(s.versions_of("engineer_session").len(), 1);
    assert!(
        s.versions_of("review_session").is_empty(),
        "the review's line began none"
    );
    // Its loading cell runs again before anything else; the review's does not, and runs once, for the code.
    assert_eq!(s.round().1, "load");
    s.hands_in(100, false);
    assert_eq!(s.round().1, "look");
    s.hands_in(100, true);
    assert_eq!(s.round().1, "review");
    // The owner asks the review's line while its round is under way: it begins one, which writes its own block and
    // runs its loading cell alone.
    s.began();
    s.on(asked("review"));
    s.hands_in(100, true);
    assert!(matches!(s.sessions().last(), Some((line, Why::Owner)) if line == "review"));
    assert_eq!(s.versions_of("review_session").len(), 1);
    assert_eq!(
        s.versions_of("engineer_session").len(),
        1,
        "the engineer's line began no other"
    );
    assert_eq!(s.round().1, "look");
    assert_eq!((runs(&s, "load"), runs(&s, "look")), (2, 2));
}

#[test]
fn the_engineers_flow_gives_each_line_that_loads_its_session_a_block_of_its_own() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
    let source = flow::read(&dir).expect("the engineer's flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = Has {
        sources: names(&[
            "answer",
            "asked",
            "words",
            driver::DEPENDENCIES,
            "main",
            SESSION,
            "start",
            "head",
            "tree",
        ]),
        acts: names(&["start", "check", "merge"]),
        tools: names(&["query", "run_tests"]),
        ..Default::default()
    };
    let flow = flow::compile(&source, &has).expect("the engineer's flow compiles");
    // Each reader as its id and its line.
    let reader = |cell: &decide::CellId| {
        let line = (flow.cells.iter()).find(|c| &c.id == cell).and_then(|c| c.line.clone());
        format!("{cell} on {}", line.unwrap_or_default())
    };
    let read: Vec<(String, Vec<String>)> = (flow.blocks.iter())
        .filter(|(_, b)| b.writers.outside.as_deref() == Some(SESSION))
        .map(|(block, b)| (block.to_string(), b.readers.cells.iter().map(reader).collect()))
        .collect();
    assert_eq!(
        read,
        [("engineer_session".to_string(), vec!["R0 on engineer".to_string()])]
    );
}

#[test]
fn a_run_waiting_on_a_tools_result_as_its_line_begins_a_new_session_goes_on_in_the_session_the_load_began() {
    let mut s = S::new();
    s.hands_in(100, true);
    let (build, _) = s.round();
    s.asks(100);
    // The engine opens again with another model while the build waits on its tool's result.
    reopened(&mut s, policy("sonnet"));
    s.model = "sonnet".to_string();
    assert!(matches!(s.sessions().last(), Some((line, Why::Model { .. })) if line == "engineer"));
    let (load, cell) = s.round();
    assert_eq!(cell, "load");
    assert_eq!(s.engineer(load, 1), (1, true));
    s.hands_in(100, false);
    let call = s.kept().records.called.last().expect("the build's call").call;
    s.on(Event::Done {
        call,
        result: "the request".to_string(),
    });
    // Entered again for the result, the build goes on in the session the load began, which never saw the run that
    // asked.
    let (again, cell) = s.round();
    assert_eq!(cell, "build");
    assert_ne!(again, build);
    assert_eq!(s.engineer(again, 1), (1, false));
}

#[test]
fn a_flagged_round_of_a_store_kept_before_sessions_were_recorded_has_its_line_begin_a_new_session_as_it_opens() {
    let mut s = S::new();
    s.hands_in(100, true);
    s.hands_in(100, true);
    let (review, cell) = s.round();
    assert_eq!(cell, "review");
    s.began();
    let spent = Spent {
        input_tokens: 10,
        context_tokens: Some(100),
        ..Default::default()
    };
    s.on(Event::Failed {
        run: review,
        why: "the safeguards flagged this message".to_string(),
        failure: decide::run::Failure::Flagged,
        resets: None,
        spent,
    });
    assert_eq!(s.sessions(), [("review".to_string(), Why::Flagged)]);
    // The review is tried again in the new session, and waits there on a tool's result.
    assert_eq!(s.round().0, review);
    s.asks(100);
    let reopen = |s: &mut S| {
        reopened(s, policy("opus"));
    };
    reopen(&mut s);
    assert_eq!(
        s.sessions().len(),
        1,
        "a flagged round a recorded session follows begins no other"
    );
    // As a store kept before sessions were recorded holds it: the session the review went on in after its flagged
    // round was counted from the failures alone, and neither it nor what the rounds ran on is recorded.
    let db = rusqlite::Connection::open(&s.db.path).unwrap();
    db.execute_batch("DELETE FROM sessions; DELETE FROM ran;").unwrap();
    drop(db);
    reopen(&mut s);
    assert_eq!(s.sessions(), [("review".to_string(), Why::Flagged)]);
    let call = s.kept().records.called.last().expect("the review's call").call;
    s.on(Event::Done {
        call,
        result: "the code".to_string(),
    });
    // The review, entered again for the result, does not go on in the session its round was flagged in: it begins
    // the new one.
    let again = s.round().0;
    assert_ne!(again, review);
    let line = |r: RunId| r == review || r == again;
    assert_eq!(of_round(s.kept(), "review", line, again, 1), (1, true));
}

#[test]
fn a_round_under_way_as_a_store_kept_before_sessions_were_recorded_opens_runs_in_the_session_its_flag_begins() {
    let mut s = S::new();
    s.hands_in(100, true);
    s.hands_in(100, true);
    let (review, _) = s.round();
    s.began();
    let spent = Spent {
        input_tokens: 10,
        context_tokens: Some(100),
        ..Default::default()
    };
    s.on(Event::Failed {
        run: review,
        why: "the safeguards flagged this message".to_string(),
        failure: decide::run::Failure::Flagged,
        resets: None,
        spent,
    });
    // The review is tried again in the session that followed its flag, asks for a tool there, and its next round has
    // begun and not ended as the engine stops.
    s.asks(100);
    let call = s.kept().records.called.last().expect("the review's call").call;
    s.on(Event::Done {
        call,
        result: "the code".to_string(),
    });
    let again = s.round().0;
    assert_ne!(again, review);
    // As a store kept before sessions were recorded holds it, and the engine opens on it. (Its round's try is not
    // recorded here: the record would be the work's last entry, and deleting it would give the opening's moment the
    // place of the moment that record was taken at, which no store holds.)
    let db = rusqlite::Connection::open(&s.db.path).unwrap();
    db.execute_batch("DELETE FROM sessions; DELETE FROM ran;").unwrap();
    drop(db);
    let todos = reopened(&mut s, policy("opus"));
    let given = todos
        .iter()
        .any(|todo| matches!(&todo.effect, Effect::Round { run, number: 1, .. } if *run == again));
    assert!(given, "the round under way is given again: {todos:?}");
    assert_eq!(s.sessions(), [("review".to_string(), Why::Flagged)]);
    // Given again, it does not go on in the session the review's round was flagged in: it begins the new one.
    let line = |r: RunId| r == review || r == again;
    assert_eq!(of_round(s.kept(), "review", line, again, 1), (1, true));
}

#[test]
fn a_line_with_no_cell_that_reads_the_block_begins_its_new_session_with_no_write() {
    let mut s = S::new();
    s.hands_in(100, true);
    s.hands_in(100, true);
    assert_eq!(s.round().1, "review");
    s.on(Event::Anew {
        line: "review".to_string(),
        why: Why::Owner,
    });
    let kept = s.kept().sessions.last().expect("the owner's").clone();
    assert_eq!((kept.line.as_str(), &kept.why), ("review", &Why::Owner));
    assert!(s.session_block().is_empty());
    s.on(Event::Anew {
        line: "engineer".to_string(),
        why: Why::Lost,
    });
    assert_eq!(s.session_block().len(), 1, "the engineer's session is loaded again");
}

#[test]
fn a_line_whose_rounds_now_run_on_another_model_begins_a_new_session_as_the_engine_opens() {
    let mut s = S::new();
    s.hands_in(100, true);
    let (build, _) = s.round();
    s.asks(100);
    let todos = reopened(&mut s, policy("sonnet"));
    let was = Why::Model {
        was: "opus".to_string(),
    };
    assert_eq!(s.sessions().last(), Some(&("engineer".to_string(), was)));
    assert_eq!(s.session_block().len(), 1);
    let loads = todos
        .iter()
        .any(|todo| matches!(&todo.effect, Effect::Round { cell, .. } if cell.as_str() == "load"));
    assert!(loads, "the load runs again before the build goes on: {todos:?}");
    let _ = build;
    // The load's round runs on the new model, and nothing more begins anew.
    s.model = "sonnet".to_string();
    s.hands_in(100, false);
    assert_eq!(s.sessions().len(), 1);
}

#[test]
fn a_line_whose_rounds_now_run_under_another_account_begins_a_new_session_as_the_engine_opens() {
    // The session was begun under the program's own default directory, and the engine opens under a directory set.
    let mut s = S::new();
    s.hands_in(100, true);
    let (build, _) = s.round();
    s.asks(100);
    let todos = reopened(&mut s, policy_under("opus", account(Some("second"))));
    s.account = Some("second".to_string());
    assert_eq!(
        s.sessions(),
        [("engineer".to_string(), Why::Account { was: None })],
        "the review's line, which has begun no session, begins none"
    );
    assert_eq!(s.session_block().len(), 1);
    let loads = todos
        .iter()
        .any(|todo| matches!(&todo.effect, Effect::Round { cell, .. } if cell.as_str() == "load"));
    assert!(loads, "the load runs again before the build goes on: {todos:?}");
    // The load's round runs under the new account, and nothing more begins anew.
    s.hands_in(100, false);
    let call = s.kept().records.called.last().expect("the build's call").call;
    s.on(Event::Done {
        call,
        result: "the request".to_string(),
    });
    let (again, cell) = s.round();
    assert_eq!(cell, "build");
    assert_ne!(again, build);
    assert_eq!(s.sessions().len(), 1);
}

#[test]
fn a_line_whose_rounds_now_run_under_another_account_begins_a_new_session_at_the_next_event() {
    let under = account(Some("first"));
    let mut s = S::with(policy_under("opus", under.clone()), Some("first"));
    s.hands_in(100, true);
    s.asks(100);
    assert!(s.sessions().is_empty());
    // The account is changed while the build waits on its tool's result; the result is the next thing to happen.
    *under.lock().unwrap() = Some("second".to_string());
    s.account = Some("second".to_string());
    let call = s.kept().records.called.last().expect("the build's call").call;
    s.on(Event::Done {
        call,
        result: "the request".to_string(),
    });
    let was = Why::Account {
        was: Some("first".to_string()),
    };
    assert_eq!(s.sessions(), [("engineer".to_string(), was)]);
    assert_eq!(s.session_block().len(), 1);
}

/// The version of the tree a carrier says the work's branch has now.
fn tree_is(content: &str) -> Event {
    Event::Wrote {
        block: "tree".into(),
        by: Outsider::Outside,
        content: content.to_string(),
        matters: true,
        value: None,
    }
}

/// The agents' program compacted the line's session while a round of it was under way.
fn compacted() -> Event {
    Event::Anew {
        line: "engineer".to_string(),
        why: Why::Compacted,
    }
}

/// How many runs of the cell the work has started.
fn runs(s: &S, cell: &str) -> usize {
    (s.kept().records.started.iter())
        .filter(|started| started.cell.as_str() == cell)
        .count()
}

/// Ends each round of the work, but the merge's, as the agents do: the loading cell hands in a context that matters
/// the first time and changes nothing of what it means after, the others what matters. Each event is taken as the
/// round of its number, counting from none, is under way. Gives how many of the events were taken that have the
/// loading cell run again, which a tree does not.
fn through(s: &mut S, events: &[(usize, Event)]) -> usize {
    let (mut round, mut taken, mut loaded) = (0, 0, false);
    loop {
        let (_, cell) = s.round();
        if cell == "merge" {
            return taken;
        }
        for (at, event) in events {
            if *at == round {
                s.on(event.clone());
                // The loading cell's own compaction has it run no more: it would load again without end.
                taken += usize::from(matches!(event, Event::Anew { .. }) && cell != "load");
            }
        }
        let matters = cell != "load" || !loaded;
        loaded |= cell == "load";
        s.hands_in(100, matters);
        round += 1;
        assert!(round < 20, "the work does not come to its merge");
    }
}

#[test]
fn a_compaction_within_a_round_counts_as_the_session_reopened_and_goes_on_under_its_id() {
    let mut s = S::new();
    s.hands_in(100, true);
    let (build, cell) = s.round();
    assert_eq!(cell, "build");
    s.on(compacted());
    assert_eq!(s.sessions(), [("engineer".to_string(), Why::Compacted)]);
    let block = s.session_block();
    assert_eq!(block.len(), 1, "a new version of the session block");
    assert!(
        block[0].contains("`engineer`") && block[0].contains("compacted"),
        "{}",
        block[0]
    );
    assert_eq!(
        s.engineer(build, 1),
        (0, false),
        "the round goes on in the session it began in, under its id"
    );
    // The round did not end and consumed nothing: when it ends, the cell that loads the session runs first, and
    // what it finds unchanged runs nothing after it.
    s.hands_in(100, true);
    let (_, cell) = s.round();
    assert_eq!(cell, "load");
    s.hands_in(100, false);
    let (_, cell) = s.round();
    assert_eq!(cell, "review");
    assert_eq!(runs(&s, "build"), 1);
}

#[test]
fn a_compaction_at_any_step_has_the_cell_that_loads_the_session_run_again_and_nothing_after_it() {
    for round in 0..4 {
        let mut s = S::new();
        let taken = through(&mut s, &[(round, compacted())]);
        let at = format!("a compaction as round {round} was under way");
        assert_eq!(runs(&s, "load"), 1 + taken, "{at}");
        assert_eq!(runs(&s, "build"), 1, "{at}");
        assert_eq!(runs(&s, "review"), 1, "{at}");
    }
}

#[test]
fn a_new_version_of_the_tree_at_any_step_runs_no_cell() {
    for round in 0..3 {
        let mut s = S::new();
        through(&mut s, &[(round, tree_is("c1 t1"))]);
        let at = format!("a new tree as round {round} was under way");
        assert_eq!(runs(&s, "load"), 1, "{at}");
        assert_eq!(runs(&s, "build"), 1, "{at}");
        assert_eq!(runs(&s, "review"), 1, "{at}");
        assert_eq!(
            s.kept()
                .records
                .written
                .iter()
                .filter(|w| w.block.as_str() == "tree")
                .count(),
            1
        );
    }
}

#[test]
fn a_session_compacted_and_a_tree_changed_at_two_steps_have_the_cell_run_again_for_the_compaction_alone() {
    for first in 0..4 {
        for second in 0..4 {
            let mut s = S::new();
            let events = [(first, compacted()), (second, tree_is("c1 t1"))];
            let taken = through(&mut s, &events);
            let at = format!("a compaction as round {first}, a new tree as round {second}");
            assert_eq!(runs(&s, "load"), 1 + taken, "{at}");
            assert_eq!(runs(&s, "build"), 1, "{at}");
            assert_eq!(runs(&s, "review"), 1, "{at}");
        }
    }
}

#[test]
fn a_version_written_for_the_round_is_put_into_it_and_runs_no_cell() {
    let mut s = S::new();
    s.hands_in(100, true);
    let (build, cell) = s.round();
    assert_eq!(cell, "build");
    // The branch took main in as the run started: the tree is the merge's, and the round is told of it.
    s.on(Event::WroteForRound {
        run: build,
        block: "tree".into(),
        content: "c1 t1".to_string(),
    });
    let kept = s.kept();
    let version = (kept.records.written.iter().rev())
        .find(|w| w.block.as_str() == "tree")
        .expect("the version is written")
        .at;
    let put = (kept.records.put_in.iter()).filter(|p| p.run == build);
    assert_eq!(
        put.map(|p| (p.block.as_str(), p.version)).collect::<Vec<_>>(),
        [("tree", version)]
    );
    s.hands_in(100, true);
    let (_, cell) = s.round();
    assert_eq!(cell, "review", "nothing runs again for it");
    assert_eq!((runs(&s, "load"), runs(&s, "build")), (1, 1));
}

#[test]
fn a_compaction_within_a_round_of_the_cell_that_loads_the_session_has_it_run_no_more() {
    let mut s = S::new();
    let (_, cell) = s.round();
    assert_eq!(cell, "load");
    s.on(compacted());
    assert_eq!(s.sessions(), [("engineer".to_string(), Why::Compacted)]);
    assert!(s.session_block().is_empty(), "nothing is written for it");
    s.hands_in(100, true);
    let (_, cell) = s.round();
    assert_eq!(cell, "build", "the loading cell does not run again");
    // A later compaction of another cell's round is written, though the session block was not written for this one.
    s.on(compacted());
    assert_eq!(s.session_block().len(), 1);
}

// A round that fails for a model the service does not have pauses its own work, and every other work that runs a
// round on that model, saying why; a work whose rounds run on another model goes on.
#[test]
fn a_model_the_service_does_not_have_pauses_every_work_whose_rounds_run_on_it() {
    // Level 1 runs on opus, which the service does not have; level 2 on sonnet.
    let by_level = Policy {
        samples: 1,
        warm: None,
        on: Box::new(|item, _| {
            let model = if item.level.as_deref() == Some("2") {
                "sonnet"
            } else {
                "opus"
            };
            Some(RanOn {
                level: item.level.clone().unwrap_or_default(),
                ..on(model)
            })
        }),
    };
    let mut s = S::with(by_level, None);
    // The work's first round tries on opus, before the others are added.
    s.began();
    let (run, _) = s.round();
    let mut add = |level: &str| {
        let new = NewItem {
            title: format!("Greet at {level}"),
            level: level.to_string(),
            project: PROJECT.to_string(),
            depends_on: Vec::new(),
            tags: Vec::new(),
            flow: "sessions".to_string(),
            writes: vec![FirstWrite {
                block: "request".into(),
                content: "Greet the user.".to_string(),
                value: None,
            }],
        };
        let (_, work, _) = s.driver.add(Time(s.clock), new).expect("added");
        s.driver.happened(Time(s.clock), work, Event::Resumed).expect("let on");
        work
    };
    let (same, other) = (add("1"), add("2"));
    s.on(Event::Failed {
        run,
        why: "There's an issue with the selected model".to_string(),
        failure: decide::run::Failure::NoModel,
        resets: None,
        spent: Spent::default(),
    });
    let paused = |work: WorkNumber| decide::paused(&s.driver.work(work).expect("the work").work.records);
    assert!(paused(s.work), "the round's own work is paused");
    assert!(paused(same), "a work whose rounds run on the model is paused");
    assert!(!paused(other), "a work whose rounds run on another model goes on");
    let records = &s.driver.work(same).expect("the work").work.records;
    let why = records.pauses.last().and_then(|p| p.why.clone()).unwrap_or_default();
    assert!(
        why.contains("opus") && why.contains(&format!("work {}", s.work.0)),
        "{why}"
    );
}

#[test]
fn a_round_the_run_guard_stopped_is_tried_again_in_a_new_session_and_a_second_pauses_the_work() {
    let mut s = S::new();
    s.hands_in(100, true);
    let (build, cell) = s.round();
    assert_eq!(cell, "build");
    let guarded = |s: &mut S, run| {
        s.began();
        s.on(Event::Failed {
            run,
            why: "the run guard stopped the round".to_string(),
            failure: decide::run::Failure::Guarded,
            resets: None,
            spent: Spent::default(),
        });
    };
    guarded(&mut s, build);
    assert_eq!(s.sessions(), [("engineer".to_string(), Why::Guarded)]);
    // The loading cell runs first in the new session, and the build is tried again after it, in that session.
    let (load, cell) = s.round();
    assert_eq!(cell, "load");
    assert_eq!(s.engineer(load, 1), (1, true));
    s.hands_in(100, false);
    let (again, _) = s.round();
    assert_eq!(again, build);
    assert!(!decide::paused(&s.kept().records));
    guarded(&mut s, build);
    assert!(
        decide::paused(&s.kept().records),
        "stopped again, the work waits for the owner"
    );
}

/// The policy where every round runs on this model, and a line that waits has its session pinged 55 minutes after its
/// last call, of an hour the cache keeps it, at most twice in a wait; sessions are kept warm, and a work may spend what
/// it likes.
fn warm(model: &str) -> Policy {
    Policy {
        warm: Some(driver::Warm {
            life_ms: 3_600_000,
            after_ms: 3_300_000,
            most: [(model.to_string(), 2)].into(),
            budget: None,
        }),
        ..policy(model)
    }
}

/// The policy of [`warm`] with this budget of a work, in millionths of a dollar.
fn warm_within(model: &str, budget: Option<u64>) -> Policy {
    let mut policy = warm(model);
    (policy.warm.as_mut().expect("the sessions are kept warm")).budget = budget;
    policy
}

/// The pings of the work the driver gives in these todos.
fn pings(s: &S, todos: &[Todo]) -> Vec<(String, u32)> {
    (todos.iter())
        .filter(|t| t.work == s.work)
        .filter_map(|t| match &t.effect {
            Effect::Ping { line, session } => Some((line.clone(), *session)),
            _ => None,
        })
        .collect()
}

/// When the work's next ping is due, which is when the driver is to be told the time next.
fn woken(s: &S) -> u64 {
    let at = kept_warm(s).first().expect("a session kept warm").at.0;
    let wake = s.driver.wake().expect("the driver waits for a time").at.0;
    assert_eq!(wake, at, "the driver wakes as the work's next ping is due");
    at
}

/// A ping of the session of the line `engineer` ended, having taken this, or failed.
fn pinged(spent: Spent, failed: Option<&str>) -> Event {
    Event::Pinged {
        line: "engineer".to_string(),
        session: 0,
        spent,
        failed: failed.map(str::to_string),
    }
}

/// The sessions of the work's lines that are kept warm, as the policy and the driver's own state keep them.
fn kept_warm(s: &S) -> Vec<driver::warm::Kept> {
    kept_warm_on(s, &warm("opus"))
}

/// The sessions of the work's lines that are kept warm, as this policy and the driver's own state keep them.
fn kept_warm_on(s: &S, policy: &Policy) -> Vec<driver::warm::Kept> {
    let held = s.driver.work(s.work).expect("the work");
    let warm = s.driver.keeping_warm();
    driver::warm::kept(held.flow, held.work, held.item, policy, warm, Time(s.clock))
}

/// A work with this policy whose engineer's session waits for a tool's result, and when its next ping is due, as a
/// policy that keeps sessions warm says: a work that is not kept warm waits all the same, and a test asks whether it
/// is pinged at that time.
fn waiting(policy: Policy) -> (S, u64) {
    let mut s = S::with(policy, None);
    s.hands_in(100, true);
    s.asks(100);
    let at = kept_warm(&s).first().expect("a session kept warm").at.0;
    (s, at)
}

/// The driver is opened again on the store, with this policy, while the work goes on: it holds everything itself,
/// until the owner has seen how things stand.
fn opened_again(s: &mut S, policy: Policy) {
    let (driver, _) = Driver::open(
        Box::new(s.db.open()),
        vec![s.flow.clone()],
        SLOTS,
        Some(policy),
        Time(s.clock),
    )
    .expect("the driver opens again");
    s.driver = driver;
    assert!(
        s.driver
            .everything_paused()
            .is_some_and(|paused| paused.by == Pauser::Engine),
        "the engine holds everything as it opened"
    );
}

#[test]
fn a_line_that_waits_has_its_session_pinged_before_the_cache_lets_it_go_as_often_as_a_wait_is_worth() {
    let mut s = S::with(warm("opus"), None);
    s.hands_in(100, true);
    // The build waits on a tool's result: its line's session waits, called last as the build's round ended.
    s.asks(100);
    let last = s.clock;
    let at = woken(&s);
    assert_eq!(at, last + 3_300_000, "55 minutes after the session's last call");
    assert!(
        pings(&s, &s.driver.pending()).is_empty(),
        "a ping is none of the work's pending acts"
    );
    let todos = s.driver.tick(Time(at), None, None).expect("the driver takes the time");
    assert_eq!(
        pings(&s, &todos),
        [("engineer".to_string(), 0)],
        "the line's session alone, not the review's"
    );
    // Under way, it is not given again.
    let again = s.driver.tick(Time(at + 1), None, None).unwrap();
    assert!(pings(&s, &again).is_empty(), "given twice");
    // Ended, it is the session's last call: the next is due 55 minutes after it.
    s.clock = at + 5_000;
    let read = Spent {
        cached_tokens: 100,
        context_tokens: Some(120),
        ..Default::default()
    };
    s.driver.happened(Time(s.clock), s.work, pinged(read, None)).unwrap();
    assert_eq!(s.kept().pinged.len(), 1);
    let next = woken(&s);
    assert_eq!(next, s.clock + 3_300_000);
    let todos = s.driver.tick(Time(next), None, None).unwrap();
    assert_eq!(pings(&s, &todos).len(), 1);
    s.clock = next + 2_000;
    s.driver.happened(Time(s.clock), s.work, pinged(read, None)).unwrap();
    // Two pings were sent in the wait, as many as it is worth: the line's session is pinged no more.
    assert!(kept_warm(&s).is_empty(), "no more pings in this wait");
}

#[test]
fn a_ping_that_failed_is_tried_again_a_minute_after_and_none_past_the_caches_life() {
    let mut s = S::with(warm("opus"), None);
    s.hands_in(100, true);
    s.asks(100);
    let at = woken(&s);
    s.driver.tick(Time(at), None, None).unwrap();
    s.clock = at + 1_000;
    let failed = pinged(Spent::default(), Some("no network"));
    s.driver.happened(Time(s.clock), s.work, failed).unwrap();
    let kept = kept_warm(&s);
    assert_eq!(kept.len(), 1);
    assert_eq!(
        kept[0].at.0,
        s.clock + driver::warm::RETRY_MS,
        "a minute after the failure"
    );
    // Past the cache's life no ping is due: it would write the session whole again, for nothing.
    let held = s.driver.work(s.work).unwrap();
    let late = driver::warm::due(held.flow, held.work, held.item, &warm("opus"), true, kept[0].life);
    assert!(late.is_empty());
}

#[test]
fn a_session_a_ping_compacted_counts_as_reopened() {
    let mut s = S::with(warm("opus"), None);
    s.hands_in(100, true);
    s.asks(100);
    let at = woken(&s);
    s.driver.tick(Time(at), None, None).unwrap();
    s.clock = at + 1_000;
    let compacted = Spent {
        compactions: 1,
        ..Default::default()
    };
    s.driver
        .happened(Time(s.clock), s.work, pinged(compacted, None))
        .unwrap();
    assert_eq!(s.sessions(), [("engineer".to_string(), Why::Compacted)]);
}

/// The round under way says what its calls came to: its first call read this from the cache, on this model.
fn measured(s: &mut S, read: u64, model: &str) {
    let (run, _) = s.round();
    let measures = Measures {
        calls: 1,
        first_read: Some(read),
        first_context: Some(read + 10),
        before: [("model".to_string(), model.to_string())].into(),
        ..Measures::default()
    };
    s.on(Event::Measured {
        run,
        attempt: 1,
        measures,
    });
}

#[test]
fn a_round_that_went_on_in_its_session_within_the_caches_life_and_read_less_than_half_of_it_missed_the_cache() {
    let mut s = S::with(warm("opus"), None);
    measured(&mut s, 0, "a");
    s.hands_in(1_000, true);
    // The build goes on in the session the load began, at once, and reads a tenth of it: a miss nothing explains.
    let (build, _) = s.round();
    measured(&mut s, 100, "a");
    s.asks(1_000);
    let misses = s.driver.misses(s.work);
    assert_eq!(misses.len(), 1, "{misses:?}");
    let miss = &misses[0];
    assert_eq!((miss.run, miss.round, miss.read, miss.held), (build, 1, 100, 1_000));
    assert_eq!(miss.written(), 900);
    assert!(!miss.expected(), "nothing before the messages differed: a fault");
    // After the cache's life, its next round writes the session again by design: no miss.
    s.clock += 2 * 3_600_000;
    let call = s.kept().records.called.last().expect("the build's call").call;
    s.on(Event::Done {
        call,
        result: "the request".to_string(),
    });
    measured(&mut s, 0, "a");
    s.hands_in(1_000, true);
    assert_eq!(
        s.driver.misses(s.work).len(),
        1,
        "a round after the cache's life is no miss"
    );
}

#[test]
fn a_miss_after_what_comes_before_the_messages_changed_is_expected_and_a_hit_is_no_miss() {
    let mut s = S::with(warm("opus"), None);
    measured(&mut s, 0, "a");
    s.hands_in(1_000, true);
    // Read whole from the cache: no miss.
    measured(&mut s, 990, "a");
    s.asks(1_000);
    assert!(s.driver.misses(s.work).is_empty());
    let call = s.kept().records.called.last().expect("the build's call").call;
    s.on(Event::Done {
        call,
        result: "the request".to_string(),
    });
    // On another model, the session is written again by design.
    measured(&mut s, 0, "b");
    s.hands_in(1_000, true);
    let misses = s.driver.misses(s.work);
    assert_eq!(misses.len(), 1);
    assert_eq!(misses[0].differs, ["model"]);
    assert!(misses[0].expected());
}

#[test]
fn a_session_is_pinged_while_the_engine_holds_everything_as_it_opened() {
    let (mut s, at) = waiting(warm("opus"));
    // The engine opens again on the work going on and holds everything until the owner has seen how things stand: the
    // hold is on the work going on, and the session's cache is what the work has already paid for.
    opened_again(&mut s, warm("opus"));
    assert_eq!(
        s.driver.wake().map(|wake| wake.at.0),
        Some(at),
        "the driver waits for the ping's time"
    );
    let todos = s.driver.tick(Time(at), None, None).unwrap();
    assert_eq!(pings(&s, &todos), [("engineer".to_string(), 0)]);
}

#[test]
fn a_session_whose_cache_let_it_go_unpinged_wakes_the_driver_no_more() {
    let mut s = S::with(warm("opus"), None);
    s.hands_in(100, true);
    s.asks(100);
    let life = kept_warm(&s)[0].life.0;
    // The engine was told no time while the cache kept the session: past its life, no ping is due, and the driver is
    // woken for none, however often it is told the time.
    for now in [life, life + 1, life + 2] {
        let todos = s.driver.tick(Time(now), None, None).unwrap();
        assert!(pings(&s, &todos).is_empty(), "a ping past the cache's life");
        assert_eq!(s.driver.wake(), None, "woken at {now} for a ping it will not give");
    }
}

#[test]
fn only_the_lines_the_work_can_come_back_to_and_none_while_a_round_of_it_is_under_way_are_kept_warm() {
    let mut s = S::with(warm("opus"), None);
    s.hands_in(100, true);
    s.hands_in(100, true);
    // The review's round is under way: nothing of the work is pinged, on any line.
    let (_, cell) = s.round();
    assert_eq!(cell, "review");
    s.began();
    assert!(kept_warm(&s).is_empty(), "a ping beside a round of its work");
    // The review waits on a tool's result: its line comes back, and the engineer's, which nothing the review writes
    // runs, does not.
    let ask = Ask::Tool {
        name: "read".to_string(),
        input: "code".to_string(),
    };
    s.ends(100, Came::asks(vec![ask]));
    let lines: Vec<String> = kept_warm(&s).into_iter().map(|k| k.due.line).collect();
    assert_eq!(lines, ["review"]);
}

#[test]
fn a_session_is_pinged_while_the_owners_pause_holds_everything_or_the_work_alone() {
    for everything in [true, false] {
        let (mut s, at) = waiting(warm("opus"));
        if everything {
            s.driver
                .pause_everything(Time(s.clock), true)
                .expect("everything is paused");
        } else {
            s.on(Event::Paused {
                by: Pauser::Owner,
                why: None,
            });
        }
        let todos = s.driver.tick(Time(at), None, None).unwrap();
        assert_eq!(
            pings(&s, &todos),
            [("engineer".to_string(), 0)],
            "everything paused: {everything}"
        );
    }
}

#[test]
fn no_session_of_a_work_the_engine_paused_for_spending_more_than_it_may_is_pinged() {
    let budget = Some(1_000_000);
    let mut s = S::with(warm_within("opus", budget), None);
    s.usd = 600_000;
    s.hands_in(100, true);
    s.asks(100);
    let at = kept_warm_on(&s, &warm_within("opus", budget))
        .first()
        .expect("a session kept warm")
        .at
        .0;
    // Its two rounds spent $1.20 of the $1.00 a work may spend, and the engine paused it for that: a ping counts in
    // the work's spend, and the budget is the refusal to let the work spend more.
    s.on(Event::Paused {
        by: Pauser::Engine,
        why: Some("it spent $1.20 of the $1.00 a work may spend".to_string()),
    });
    let todos = s.driver.tick(Time(at), None, None).unwrap();
    assert!(pings(&s, &todos).is_empty(), "a ping of a work past its budget");
    assert!(s.driver.wake().is_none(), "nothing to wake for while it stands so");
    // Let on by the owner, who alone lets a work on, it is kept warm as any other.
    s.on(Event::Resumed);
    let todos = s.driver.tick(Time(at), None, None).unwrap();
    assert_eq!(pings(&s, &todos), [("engineer".to_string(), 0)]);
}

#[test]
fn no_session_is_pinged_at_all_where_the_owner_switched_keeping_them_warm_off() {
    // Under each hold a waiting session is otherwise pinged under: none, the engine's as it opened, and the owner's
    // over everything and over the work alone.
    for hold in ["none", "the engine's as it opened", "everything", "the work"] {
        let (mut s, at) = waiting(warm("opus"));
        s.driver
            .keep_warm(Time(s.clock), false)
            .expect("the owner switches it off");
        match hold {
            "the engine's as it opened" => opened_again(&mut s, warm("opus")),
            "everything" => {
                s.driver
                    .pause_everything(Time(s.clock), true)
                    .expect("everything is paused");
            }
            "the work" => s.on(Event::Paused {
                by: Pauser::Owner,
                why: None,
            }),
            _ => {}
        }
        assert!(!s.driver.keeping_warm(), "{hold}: the switch holds");
        let todos = s.driver.tick(Time(at), None, None).unwrap();
        assert!(pings(&s, &todos).is_empty(), "pinged while {hold} holds it");
        assert_eq!(
            s.driver.wake(),
            None,
            "woken for a ping it will not give while {hold} holds it"
        );
    }
}

#[test]
fn the_switch_of_keeping_sessions_warm_is_kept_and_a_ping_comes_once_it_is_switched_on_again() {
    let (mut s, at) = waiting(warm("opus"));
    assert!(s.driver.keeping_warm(), "kept warm until the owner switches it off");
    s.driver
        .keep_warm(Time(s.clock), false)
        .expect("the owner switches it off");
    // Opened again on the store, the engine keeps none warm: the switch is its own state, not a setting.
    opened_again(&mut s, warm("opus"));
    assert!(!s.driver.keeping_warm());
    assert!(s.driver.wake().is_none(), "nothing to wake for");
    assert!(pings(&s, &s.driver.tick(Time(at), None, None).unwrap()).is_empty());
    // Switched on again, the driver asks for the ping that is due and gives it at that time.
    s.driver
        .keep_warm(Time(s.clock), true)
        .expect("the owner switches it on");
    assert!(s.driver.keeping_warm());
    let wake = s.driver.wake().expect("the driver waits for a time");
    let todos = s.driver.tick(wake.at, None, None).unwrap();
    assert_eq!(pings(&s, &todos), [("engineer".to_string(), 0)]);
}

#[test]
fn the_wait_every_work_shares_stops_every_ping_until_a_check_finds_the_outside_well() {
    let (mut s, at) = waiting(warm("opus"));
    // Another work's round fails for a cause outside every work: no agent's round of any work begins until a check
    // finds the outside well, and a ping, which runs with the command line a round runs with, would fail as they do.
    let (other, run) = s.another();
    let failed = Event::Failed {
        run,
        why: "the network cannot be reached".to_string(),
        failure: decide::run::Failure::Offline,
        resets: None,
        spent: Spent::default(),
    };
    s.driver
        .happened(Time(s.clock), other, failed)
        .expect("the driver takes it");
    assert!(s.driver.outage().is_some_and(|outage| outage.holds()));
    assert!(!kept_warm(&s).is_empty(), "the session is kept warm all the same");
    let todos = s.driver.tick(Time(at), None, None).unwrap();
    assert!(pings(&s, &todos).is_empty(), "a ping while the agents' rounds wait");
    // A check finds the outside well: the wait is over, and the ping is given.
    let todos = s.driver.tick(Time(at + 1), Some(Found::Well), None).unwrap();
    assert_eq!(pings(&s, &todos), [("engineer".to_string(), 0)]);
}

#[test]
fn no_ping_of_a_line_the_work_passed_for_good_while_its_pause_holds_it_at_another_lines_cell() {
    let mut s = S::on_flow(two_lines(), warm("opus"), None);
    s.hands_in(100, true);
    // The owner pauses the work while the build's round is under way: having handed in, the work stands paused at the
    // look, a cell of the review's line, and nothing the review's cells write runs a cell of the engineer's.
    s.on(Event::Paused {
        by: Pauser::Owner,
        why: None,
    });
    s.hands_in(100, true);
    let shown = s.driver.view(s.work).expect("the work is shown").decision;
    assert!(
        matches!(&shown, Decision::Paused { cell } if cell.as_str() == "look"),
        "{shown:?}"
    );
    assert!(
        kept_warm(&s).is_empty(),
        "the engineer's line, which the work has passed for good, is pinged"
    );
}

#[test]
fn a_ping_that_compacted_the_session_after_the_loading_cells_round_has_that_cell_run_again() {
    let mut s = S::with(warm("opus"), None);
    // A person stops the build while the load runs: once the load has handed in, the work rests, its last round the
    // loading cell's.
    s.on(Event::Wrote {
        block: "stop".into(),
        by: Outsider::Person,
        content: "Stop.".to_string(),
        matters: true,
        value: Some("yes".to_string()),
    });
    s.hands_in(100, true);
    let at = woken(&s);
    s.driver.tick(Time(at), None, None).unwrap();
    s.clock = at + 1_000;
    let compacted = Spent {
        compactions: 1,
        ..Default::default()
    };
    s.driver
        .happened(Time(s.clock), s.work, pinged(compacted, None))
        .unwrap();
    // The compaction came in no round of the loading cell's: the session it loads holds a summary, and it runs again.
    assert_eq!(s.sessions(), [("engineer".to_string(), Why::Compacted)]);
    assert_eq!(s.session_block().len(), 1);
    let (_, cell) = s.round();
    assert_eq!(cell, "load");
}

// A round that would begin while a ping of its work is under way begins once the ping has ended, after what the ping
// did to its session, so that it is filled as the session then stands: a compaction the ping made comes before it.
#[test]
fn a_round_due_while_its_works_ping_is_under_way_begins_after_the_ping_and_what_it_did() {
    let mut s = S::with(warm("opus"), None);
    s.hands_in(100, true);
    s.asks(100);
    let at = woken(&s);
    s.driver.tick(Time(at), None, None).unwrap();
    s.clock = at;
    // The tool's result comes while the ping is under way: no round begins beside it.
    let rounds = s.kept().records.rounds.len();
    let call = s.kept().records.called.last().expect("the build's call").call;
    s.on(Event::Done {
        call,
        result: "the request".to_string(),
    });
    assert_eq!(s.kept().records.rounds.len(), rounds, "a round began beside the ping");
    assert!(
        !(s.driver.pending().iter()).any(|t| matches!(t.effect, Effect::Round { .. })),
        "a round is to be done beside the ping"
    );
    // The ping ends, having compacted the session: the compaction is recorded, and the round begins after it.
    s.clock += 1_000;
    let compacted = Spent {
        compactions: 1,
        ..Default::default()
    };
    s.driver
        .happened(Time(s.clock), s.work, pinged(compacted, None))
        .unwrap();
    let kept = s.kept();
    let compaction = (kept.sessions.iter())
        .find(|session| session.why == Why::Compacted)
        .expect("the ping's compaction")
        .at;
    assert_eq!(
        kept.records.rounds.len(),
        rounds + 1,
        "the round begins once the ping has ended"
    );
    let round = kept.records.rounds.last().expect("the round");
    assert!(
        round.at > compaction,
        "the round began before the compaction it is filled after"
    );
}

// A round that goes on in its session is given beside its message the return before it, which the session has held
// only since; a round that begins its session holds none.
#[test]
fn a_round_that_goes_on_in_its_session_has_the_return_before_it_and_one_that_begins_it_none() {
    let mut s = S::new();
    let (load, _) = s.round();
    s.hands_in(100, true);
    let (build, cell) = s.round();
    assert_eq!(cell, "build");
    let held = s.driver.work(s.work).expect("the work");
    let back = driver::history::last_return(held.flow, held.work, "engineer", build, 1);
    let handed = match back {
        Some(work::Back::Returned { handed, .. }) => handed.iter().map(|h| h.content.as_str()).collect::<Vec<_>>(),
        other => panic!("the load's return: {other:?}"),
    };
    assert_eq!(handed, ["the context"]);
    let first = driver::history::last_return(held.flow, held.work, "engineer", load, 1);
    assert_eq!(first, None, "the load began the session");
}
