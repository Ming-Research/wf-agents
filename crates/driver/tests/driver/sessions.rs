//! When a line of a work begins a new session: the driver records it with why, from the work's records and the
//! installation's policy, and the work goes on where it stood: a new session runs no cell, and the next round of the
//! line is filled in it with all its cell's data.

use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, Mutex};

use driver::cache::{Cause, Miss};
use driver::{Budget, Policy};
use driver::{FirstWrite, Found, NewItem};
use engine::RanOn;
use engine::{Ask, Handed, Session, Why};
use flow::{Compiled, Has};
use work::Measures;
use work::sessions::of_round;

use super::*;

/// The flow: `load` begins the engineer's line, `build` goes on in its session unless a person stops it, and `review`
/// has a line of its own.
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
    let load = cell("engineer", r#"["request"]"#, r#"["context"]"#);
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
        files: beside(),
        ..Default::default()
    };
    let has = Has {
        sources: ["tree".to_string()].into(),
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the flow compiles")
}

/// The account the owner took in a test of keeping sessions warm, which a session is kept warm only under.
const ACCOUNT: &str = "work";

/// Two lines: the engineer's, which `load` begins and `build` goes on in, and the review's, which `look` begins and
/// `review` goes on in.
fn two_lines() -> Compiled {
    let flow = r#"
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
        [blocks.context]
        [blocks.code]
        [blocks.notes]
        [blocks.verdict]
        [blocks.merged]
        shown = true
    "#
    .to_string();
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
            ("load", cell("engineer", r#"["request"]"#, r#"["context"]"#)),
            ("build", cell("engineer", r#"["request", "context"]"#, r#"["code"]"#)),
            ("look", cell("review", r#"["code"]"#, r#"["notes"]"#)),
            ("review", cell("review", r#"["code", "notes"]"#, r#"["verdict"]"#)),
            ("merge", merge.to_string()),
        ]
        .into_iter()
        .map(|(id, text)| (id.to_string(), text))
        .collect(),
        files: beside(),
        ..Default::default()
    };
    let has = Has {
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the flow compiles")
}

/// Two lines that both come back to cells of theirs while the work stands at the engineer's: `load` and `build` are the
/// engineer's, `look`, which comes before `build`, the review's, and runs on the owner's remarks, so that it runs while
/// the build waits on a tool's result; `review` is the review's after `build`.
fn remarks() -> Compiled {
    let flow = r#"
        name = "sessions"
        order = ["load", "look", "build", "review", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.engineer]
        [lines.review]
        [blocks.request]
        people = true
        [blocks.remarks]
        people = true
        [blocks.context]
        [blocks.notes]
        [blocks.code]
        [blocks.verdict]
        [blocks.merged]
        shown = true
    "#
    .to_string();
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
            ("load", cell("engineer", r#"["request"]"#, r#"["context"]"#)),
            ("look", cell("review", r#"["remarks"]"#, r#"["notes"]"#)),
            (
                "build",
                cell("engineer", r#"["request", "context", "notes"]"#, r#"["code"]"#),
            ),
            ("review", cell("review", r#"["code"]"#, r#"["verdict"]"#)),
            ("merge", merge.to_string()),
        ]
        .into_iter()
        .map(|(id, text)| (id.to_string(), text))
        .collect(),
        files: beside(),
        ..Default::default()
    };
    let has = Has {
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
        directory: None,
        account: None,
    }
}

/// Opens the driver again on the store with this policy, and the owner lifts at once, at the time it opened, the pause
/// of everything it made as it opened on the work going on: gives what both gave.
fn reopened(s: &mut S, policy: Policy) -> Vec<Todo> {
    let now = Time(s.clock);
    let (mut driver, mut todos) = Driver::open(
        Box::new(s.db.open()),
        vec![s.flow.clone()],
        SLOTS,
        Some(policy),
        None,
        now,
    )
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
        model: Box::new(move |_, _| Some(model.clone())),
        changed: None,
        warm: None,
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
    /// How often the program compacted the session during each round of the work that ends.
    compactions: u32,
}

impl S {
    fn new() -> S {
        S::with(policy("opus"), None)
    }

    /// A driver with the policy, whose rounds' tries begin under the account.
    fn with(policy: Policy, account: Option<&str>) -> S {
        S::on_flow(flow(), policy, account)
    }

    /// A driver with the policy whose owner took the account [`ACCOUNT`], under which its rounds' tries begin: a
    /// session is kept warm only under the account in use.
    fn warmed(policy: Policy) -> S {
        S::warmed_on(flow(), policy)
    }

    /// A driver on the flow with the policy whose owner took the account [`ACCOUNT`], under which its rounds' tries
    /// begin.
    fn warmed_on(flow: Compiled, policy: Policy) -> S {
        let mut s = S::on_flow(flow, policy, Some(ACCOUNT));
        let now = Time(s.clock);
        s.driver.add_account(now, ACCOUNT).expect("the account is added");
        s.driver.take_account(now, ACCOUNT).expect("the account is taken");
        s
    }

    /// A driver on the flow with the policy, whose rounds' tries begin under the account.
    fn on_flow(flow: Compiled, policy: Policy, account: Option<&str>) -> S {
        let db = Db::new();
        let (mut driver, _) = Driver::open(
            Box::new(db.open()),
            vec![flow.clone()],
            SLOTS,
            Some(policy),
            None,
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
            continues: None,
            depends_on: Vec::new(),
            flow: "sessions".to_string(),
            writes: vec![FirstWrite {
                block: "request".into(),
                content: "Greet the user.".to_string(),
                value: None,
            }],
            why: None,
            waits_for: Vec::new(),
        };
        let (_, before, _, _) = driver.add(Time(START), earlier).unwrap();
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
                hindrance: None,
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
            continues: None,
            flow: "sessions".to_string(),
            writes: vec![FirstWrite {
                block: "request".into(),
                content: "Greet the user.".to_string(),
                value: None,
            }],
            why: None,
            waits_for: Vec::new(),
        };
        let (_, work, _, _) = driver.add(Time(START), new).unwrap();
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
            compactions: 0,
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
            continues: None,
            flow: "sessions".to_string(),
            writes: vec![FirstWrite {
                block: "request".into(),
                content: "Greet the user.".to_string(),
                value: None,
            }],
            why: None,
            waits_for: Vec::new(),
        };
        self.clock += 1000;
        let (_, work, _, _) = self.driver.add(Time(self.clock), new).expect("the work is added");
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
            uncached_tokens: 10,
            context_tokens: Some(context),
            usd_micros: self.usd,
            compactions: self.compactions,
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

    fn engineer(&self, run: RunId, number: usize) -> (u32, bool) {
        engineer_in(self.kept(), run, number)
    }
}

/// Which session of the engineer's line the round of this number of the run runs in, and whether it begins it, as the
/// work's records say.
fn engineer_in(work: &engine::Work, run: RunId, number: usize) -> (u32, bool) {
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

#[test]
fn a_line_goes_on_in_its_first_session_while_nothing_ends_it() {
    let mut s = S::new();
    s.hands_in(100, true);
    assert!(s.sessions().is_empty(), "no new session");
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
    // The line goes on in the session its load began, and the loading cell has run once.
    let (review, cell) = s.round();
    assert_eq!(cell, "review", "the engineer's line handed in; the review goes on");
    assert_ne!(review, build);
    assert_eq!(runs(&s, "load"), 1);
}

// A new session of a line runs no cell: the work goes on at the cell whose turn it is, which begins the new session,
// and the line's earlier cells do not run again (design document `data-and-context`, 7, 25). Runs in memory in
// milliseconds; the one test that a new session asked of each line runs again none of its cells.
#[test]
fn a_new_session_of_a_line_runs_no_cell_and_the_work_goes_on_where_it_stood() {
    let mut s = S::on_flow(two_lines(), policy("opus"), None);
    let asked = |line: &str| Event::Anew {
        line: line.to_string(),
        why: Why::Owner,
    };
    s.hands_in(100, true);
    assert_eq!(s.round().1, "build");
    // The owner asks the engineer's line for a new session while the build's round is under way: the line begins one,
    // and the work goes on to the review's line.
    s.began();
    s.on(asked("engineer"));
    s.hands_in(100, true);
    assert!(matches!(s.sessions().last(), Some((line, Why::Owner)) if line == "engineer"));
    assert_eq!(s.round().1, "look");
    s.hands_in(100, true);
    assert_eq!(s.round().1, "review");
    // The owner asks the review's line while its round is under way: it begins one, and the round that follows goes on
    // to the merge.
    s.began();
    s.on(asked("review"));
    s.hands_in(100, true);
    assert!(matches!(s.sessions().last(), Some((line, Why::Owner)) if line == "review"));
    assert_eq!(s.round().1, "merge");
    assert_eq!((runs(&s, "load"), runs(&s, "look"), runs(&s, "build")), (1, 1, 1));
}

#[test]
fn a_run_waiting_on_a_tools_result_as_its_line_begins_a_new_session_begins_it_once_entered_again() {
    let mut s = S::new();
    s.hands_in(100, true);
    let (build, _) = s.round();
    s.asks(100);
    // The engine opens again with another model while the build waits on its tool's result.
    reopened(&mut s, policy("sonnet"));
    s.model = "sonnet".to_string();
    assert!(matches!(s.sessions().last(), Some((line, Why::Model { .. })) if line == "engineer"));
    let call = s.kept().records.called.last().expect("the build's call").call;
    s.on(Event::Done {
        call,
        result: "the request".to_string(),
    });
    // Entered again for the result, the build begins the new session, which never saw the run that asked, and no
    // cell ran before it.
    let (again, cell) = s.round();
    assert_eq!(cell, "build");
    assert_ne!(again, build);
    assert_eq!(s.engineer(again, 1), (1, true));
    assert_eq!(runs(&s, "load"), 1);
}

// A run waiting on a tool's result as what a new session of its line would begin with comes to differ from what its
// session began with, as after a flow installed while it waits, does not hold the new session back: as for another
// model, the line begins it at once as the engine opens, and the run, entered again for the result, begins the new
// session. Runs in memory in milliseconds; the one test that this reason takes the timing of a change of model.
#[test]
fn a_run_waiting_on_a_tools_result_as_its_session_comes_to_differ_begins_the_new_session_once_entered_again() {
    let mut s = S::new();
    s.hands_in(100, true);
    let (build, _) = s.round();
    s.asks(250);
    reopened(&mut s, kept_against(|line, session| line == "engineer" && session == 0));
    let why = Why::Changed { context: Some(250) };
    assert_eq!(s.sessions().last(), Some(&("engineer".to_string(), why)));
    let call = s.kept().records.called.last().expect("the build's call").call;
    s.on(Event::Done {
        call,
        result: "the request".to_string(),
    });
    let (again, cell) = s.round();
    assert_eq!(cell, "build");
    assert_ne!(again, build);
    assert_eq!(s.engineer(again, 1), (1, true));
    assert_eq!(s.sessions().len(), 1);
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
        uncached_tokens: 10,
        context_tokens: Some(100),
        ..Default::default()
    };
    s.on(Event::Failed {
        run: review,
        why: "the safeguards flagged this message".to_string(),
        failure: decide::run::Failure::Flagged,
        resets: None,
        hindrance: None,
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
fn a_round_under_way_as_a_store_kept_before_sessions_were_recorded_opens_goes_on_in_the_session_it_began_in() {
    let mut s = S::new();
    s.hands_in(100, true);
    s.hands_in(100, true);
    let (review, _) = s.round();
    s.began();
    let spent = Spent {
        uncached_tokens: 10,
        context_tokens: Some(100),
        ..Default::default()
    };
    s.on(Event::Failed {
        run: review,
        why: "the safeguards flagged this message".to_string(),
        failure: decide::run::Failure::Flagged,
        resets: None,
        hindrance: None,
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
    // Given again, it goes on in the session it began in, as every round does: the session its line began as the engine
    // opened is the next round's.
    let line = |r: RunId| r == review || r == again;
    assert_eq!(of_round(s.kept(), "review", line, again, 1), (0, false));
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
    let rounds = todos.iter().any(|todo| matches!(&todo.effect, Effect::Round { .. }));
    assert!(!rounds, "a new session runs no cell while the build waits: {todos:?}");
    let call = s.kept().records.called.last().expect("the build's call").call;
    s.on(Event::Done {
        call,
        result: "the request".to_string(),
    });
    // The build's round runs on the new model, and nothing more begins anew.
    let (again, cell) = s.round();
    assert_eq!(cell, "build");
    assert_ne!(again, build);
    s.model = "sonnet".to_string();
    s.hands_in(100, true);
    assert_eq!(s.sessions().len(), 1);
}

/// The policy where every round runs on one model, whose carriers say, each time they are asked of a line and of the
/// session of it the line's next round runs in, what `changed` says: whether a new session of the line would begin with
/// another system prompt or form of return than that session began with.
fn kept_against(changed: impl Fn(&str, u32) -> bool + Send + 'static) -> Policy {
    Policy {
        changed: Some(Box::new(move |_, _, _, cell, session| {
            changed(cell.line.as_deref().unwrap_or_default(), session)
        })),
        ..policy("opus")
    }
}

// What a new session of the engineer's line would begin with comes to differ from what its session began with while a
// round of the line is under way, as a change of the project's files in the worktree makes it: the line begins a new
// session at once, recorded with the context of the session it leaves as its latest round that ended told it; the round
// under way goes on in the session it began in, no cell runs for the new session, and the new session, begun with what
// a new one begins with, begins no other. Runs in memory in milliseconds; the one test of the record of this reason.
#[test]
fn a_line_whose_session_began_otherwise_than_a_new_one_would_now_begins_one_recorded_with_the_context_it_left() {
    let changed = Arc::new(AtomicBool::new(false));
    let since = changed.clone();
    let policy = kept_against(move |line, session| line == "engineer" && session == 0 && since.load(SeqCst));
    let mut s = S::with(policy, None);
    s.hands_in(1234, true);
    let (build, cell) = s.round();
    assert_eq!(cell, "build");
    assert!(s.sessions().is_empty(), "nothing changed yet: {:?}", s.sessions());
    changed.store(true, SeqCst);
    s.began();
    let why = Why::Changed { context: Some(1234) };
    assert_eq!(s.sessions(), [("engineer".to_string(), why)]);
    assert_eq!(
        s.engineer(build, 1),
        (0, false),
        "the round under way goes on in its session"
    );
    s.hands_in(100, true);
    assert_eq!(s.round().1, "review", "no cell runs for the new session");
    assert_eq!(s.sessions().len(), 1, "the new session begins no other");
    assert_eq!((runs(&s, "load"), runs(&s, "build")), (1, 1));
}

// The carrier takes main in after the round has begun, which changes what a new session of the line would begin with, and
// the driver records the new session before the round's first try is recorded. The round goes on in the session it began
// in, as its carrier decided from the work as kept then, and the records say the same; the line's next round begins the
// new session. Runs in memory in milliseconds; the one test of a new session recorded between a round's beginning and its
// first try, which no other test records in that order.
#[test]
fn a_session_begun_while_a_round_takes_main_in_is_the_next_rounds() {
    let changed = Arc::new(AtomicBool::new(false));
    let since = changed.clone();
    let policy = kept_against(move |line, session| line == "engineer" && session == 0 && since.load(SeqCst));
    let mut s = S::with(policy, None);
    s.hands_in(1234, true);
    let (build, cell) = s.round();
    assert_eq!(cell, "build");
    // What the carrier is given as the round is handed out.
    let handed = s.kept().clone();
    changed.store(true, SeqCst);
    s.on(Event::WroteForRound {
        run: build,
        block: "tree".into(),
        content: "c1 t1".to_string(),
    });
    assert!(
        matches!(s.sessions().as_slice(), [(line, Why::Changed { .. })] if line == "engineer"),
        "{:?}",
        s.sessions()
    );
    // The round's first try is recorded after the new session, and the round asks for a tool.
    s.asks(100);
    assert_eq!(
        s.engineer(build, 1),
        (0, false),
        "the round went on in the session it began in"
    );
    assert_eq!(
        engineer_in(&handed, build, 1),
        s.engineer(build, 1),
        "the records say what the carrier decided as the round began"
    );
    // The build, entered again for the tool's result, begins the new session.
    let call = s.kept().records.called.last().expect("the build's call").call;
    s.on(Event::Done {
        call,
        result: "the request".to_string(),
    });
    let (again, cell) = s.round();
    assert_eq!(cell, "build");
    assert_eq!(s.engineer(again, 1), (1, true));
    assert_eq!(s.sessions().len(), 1, "the new session begins no other");
}

// A round flagged after its line began a new session while the round took main in ran in the session it began in, which
// that new session ended already, so the flag begins no other: the round is tried again in the one session recorded, which
// holds nothing of it. Runs in memory in milliseconds; the one test of a round that fails after a session was recorded
// between its beginning and its first try, where the flag's session would otherwise be counted from the first try.
#[test]
fn a_flagged_round_whose_line_began_a_new_session_as_it_took_main_in_is_tried_again_in_that_session() {
    let changed = Arc::new(AtomicBool::new(false));
    let since = changed.clone();
    let policy = kept_against(move |line, session| line == "engineer" && session == 0 && since.load(SeqCst));
    let mut s = S::with(policy, None);
    s.hands_in(1234, true);
    let (build, cell) = s.round();
    assert_eq!(cell, "build");
    changed.store(true, SeqCst);
    s.on(Event::WroteForRound {
        run: build,
        block: "tree".into(),
        content: "c1 t1".to_string(),
    });
    s.began();
    let spent = Spent {
        uncached_tokens: 10,
        context_tokens: Some(100),
        ..Default::default()
    };
    s.on(Event::Failed {
        run: build,
        why: "the safeguards flagged this message".to_string(),
        failure: decide::run::Failure::Flagged,
        resets: None,
        hindrance: None,
        spent,
    });
    assert!(
        matches!(s.sessions().as_slice(), [(line, Why::Changed { .. })] if line == "engineer"),
        "the flag begins no session beside the one recorded as main was taken in: {:?}",
        s.sessions()
    );
    // The round is given again, as its run's next, and begins that session.
    assert_eq!(s.round().0, build);
    let next = s.kept().records.rounds.iter().filter(|r| r.run == build).count();
    assert_eq!(next, 2);
    assert_eq!(s.engineer(build, 1), (0, false));
    assert_eq!(s.engineer(build, 2), (1, true));
}

// A line whose session began with what a new one would begin with now goes on in it: the carriers are asked of each
// line, of the session its next round runs in, and say no. Runs in memory in milliseconds; beside the test of a session
// that began otherwise, it shows that the new session comes from the carriers' answer, and not from their being asked.
#[test]
fn a_line_whose_session_began_with_what_a_new_one_would_begin_with_now_goes_on_in_it() {
    let asked: Arc<Mutex<Vec<(String, u32)>>> = Arc::default();
    let record = asked.clone();
    let policy = kept_against(move |line, session| {
        record.lock().expect("the asks").push((line.to_string(), session));
        false
    });
    let mut s = S::with(policy, None);
    // What the earlier work of the harness was asked is left out: only this work's rounds are looked at.
    asked.lock().expect("the asks").clear();
    s.hands_in(100, true);
    s.hands_in(100, true);
    assert_eq!(s.round().1, "review");
    assert!(s.sessions().is_empty(), "{:?}", s.sessions());
    let asked = asked.lock().expect("the asks");
    for line in ["engineer", "review"] {
        assert!(asked.contains(&(line.to_string(), 0)), "{line}: {asked:?}");
    }
}

// A line goes on in its session under another account: the session lives in the one directory of the agents' program,
// and the service takes a session made under another account (decision
// a-line-goes-on-in-its-session-under-another-account). Taken while a round waits, as the engine opens, and once the
// line goes on, the account begins no session.
#[test]
fn a_line_goes_on_in_its_session_after_another_account_is_taken() {
    let mut s = S::new();
    s.hands_in(100, true);
    s.asks(100);
    // The session was begun under no account; the owner adds an account and takes it while the build waits on
    // its tool's result.
    let now = Time(s.clock);
    s.driver.add_account(now, "second").expect("the account is added");
    s.driver.take_account(now, "second").expect("the account is taken");
    s.account = Some("second".to_string());
    assert!(s.sessions().is_empty(), "{:?}", s.sessions());
    // Opened again, the engine holds the account in use, and begins no session for it.
    reopened(&mut s, policy("opus"));
    assert_eq!(s.driver.accounts().in_use(), Some("second"));
    assert!(s.sessions().is_empty(), "{:?}", s.sessions());
    let call = s.kept().records.called.last().expect("the build's call").call;
    s.on(Event::Done {
        call,
        result: "the request".to_string(),
    });
    let (again, cell) = s.round();
    assert_eq!(cell, "build", "the build goes on, not the load");
    s.began();
    assert!(s.sessions().is_empty(), "{:?}", s.sessions());
    assert_eq!(s.engineer(again, 1), (0, false), "in the session its load began");
    assert_eq!(runs(&s, "load"), 1);
}

// A session is pinged only where its last call ran under the account in use, a ping running under that account; one
// whose last call ran under another waits unpinged, and is pinged again once its account is taken back.
#[test]
fn a_session_is_kept_warm_only_while_its_last_call_ran_under_the_account_in_use() {
    let mut s = S::with(warm("opus"), Some("first"));
    let now = Time(s.clock);
    for name in ["first", "second"] {
        s.driver.add_account(now, name).expect("the account is added");
    }
    s.driver.take_account(now, "first").expect("taken");
    s.hands_in(100, true);
    s.asks(100);
    assert_eq!(kept_warm(&s).len(), 1, "its last call ran under the account in use");
    s.clock += 1000;
    s.driver.take_account(Time(s.clock), "second").expect("taken");
    assert!(kept_warm(&s).is_empty(), "its last call ran under another account");
    let todos = s
        .driver
        .tick(Time(s.clock + 3_300_000), None, None)
        .expect("the driver takes the time");
    assert!(pings(&s, &todos).is_empty(), "{todos:?}");
    s.clock += 1000;
    s.driver.take_account(Time(s.clock), "first").expect("taken back");
    assert_eq!(kept_warm(&s).len(), 1, "taken back, its session is kept warm again");
}

// With no account in use, no session is pinged, a ping having no account to run under, and no ping wakes the driver;
// a session whose last call ran under an account not in use is not pinged either, until the owner takes that account.
// Milliseconds, nothing run: the one test that a ping is never handed out to fail for want of an account.
#[test]
fn no_session_is_kept_warm_while_no_account_is_in_use() {
    let mut s = S::with(warm("opus"), None);
    s.hands_in(100, true);
    s.asks(100);
    assert!(
        kept_warm(&s).is_empty(),
        "its last call ran under no account, and none is in use"
    );
    let todos = s
        .driver
        .tick(Time(s.clock + 3_300_000), None, None)
        .expect("the driver takes the time");
    assert!(pings(&s, &todos).is_empty(), "{todos:?}");
    assert_eq!(s.driver.wake(), None, "no ping wakes the driver");
    let mut s = S::with(warm("opus"), Some(ACCOUNT));
    s.hands_in(100, true);
    s.asks(100);
    assert!(
        kept_warm(&s).is_empty(),
        "its last call ran under an account not in use"
    );
    let now = Time(s.clock);
    s.driver.add_account(now, ACCOUNT).expect("the account is added");
    s.driver.take_account(now, ACCOUNT).expect("the account is taken");
    assert_eq!(kept_warm(&s).len(), 1, "taken, its session is kept warm");
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

/// Ends each round of the work, but the merge's, as the agents do, each handing in what matters. Each event is taken
/// as the round of its number, counting from none, is under way.
fn through(s: &mut S, events: &[(usize, Event)]) {
    let mut round = 0;
    loop {
        let (_, cell) = s.round();
        if cell == "merge" {
            return;
        }
        for (at, event) in events {
            if *at == round {
                s.on(event.clone());
            }
        }
        s.hands_in(100, true);
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
    assert_eq!(
        s.engineer(build, 1),
        (0, false),
        "the round goes on in the session it began in, under its id"
    );
    // When the round ends, the work goes on: the compaction runs no cell.
    s.hands_in(100, true);
    let (_, cell) = s.round();
    assert_eq!(cell, "review");
    assert_eq!((runs(&s, "load"), runs(&s, "build")), (1, 1));
}

#[test]
fn a_compaction_at_any_step_runs_no_cell() {
    for round in 0..4 {
        let mut s = S::new();
        through(&mut s, &[(round, compacted())]);
        let at = format!("a compaction as round {round} was under way");
        assert_eq!(runs(&s, "load"), 1, "{at}");
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
fn a_session_compacted_and_a_tree_changed_at_two_steps_run_no_cell() {
    for first in 0..4 {
        for second in 0..4 {
            let mut s = S::new();
            let events = [(first, compacted()), (second, tree_is("c1 t1"))];
            through(&mut s, &events);
            let at = format!("a compaction as round {first}, a new tree as round {second}");
            assert_eq!(runs(&s, "load"), 1, "{at}");
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

// A round that fails for a model the service does not have pauses its own work, and every other work that runs a
// round on that model, saying why; a work whose rounds run on another model goes on.
#[test]
fn a_model_the_service_does_not_have_pauses_every_work_whose_rounds_run_on_it() {
    // Level 1 runs on opus, which the service does not have; level 2 on sonnet.
    let by_level = Policy {
        samples: 1,
        changed: None,
        warm: None,
        model: Box::new(|item, _| {
            let model = if item.level.as_deref() == Some("2") {
                "sonnet"
            } else {
                "opus"
            };
            Some(model.to_string())
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
            continues: None,
            flow: "sessions".to_string(),
            writes: vec![FirstWrite {
                block: "request".into(),
                content: "Greet the user.".to_string(),
                value: None,
            }],
            why: None,
            waits_for: Vec::new(),
        };
        let (_, work, _, _) = s.driver.add(Time(s.clock), new).expect("added");
        s.driver.happened(Time(s.clock), work, Event::Resumed).expect("let on");
        work
    };
    let (same, other) = (add("1"), add("2"));
    s.on(Event::Failed {
        run,
        why: "There's an issue with the selected model".to_string(),
        failure: decide::run::Failure::NoModel,
        resets: None,
        hindrance: None,
        spent: Spent::default(),
    });
    let paused = |work: WorkNumber| decide::paused(&s.driver.work(work).expect("the work").work.records);
    assert!(paused(s.work), "the round's own work is paused");
    assert!(paused(same), "a work whose rounds run on the model is paused");
    assert!(!paused(other), "a work whose rounds run on another model goes on");
    let records = &s.driver.work(same).expect("the work").work.records;
    let why = records.pauses.last().and_then(|p| p.why.clone()).unwrap_or_default();
    assert!(
        why.contains("opus") && why.contains(&format!("work {}", s.work)),
        "{why}"
    );
    // A whole sentence from the files, with the service's own words after a colon, and the work's number written as
    // the engine writes one.
    assert!(why.starts_with("The service has no model "), "{why}");
    assert!(why.ends_with(": There's an issue with the selected model."), "{why}");
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
            hindrance: None,
            spent: Spent::default(),
        });
    };
    guarded(&mut s, build);
    assert_eq!(s.sessions(), [("engineer".to_string(), Why::Guarded)]);
    // The build is tried again, and begins the new session.
    let (again, _) = s.round();
    assert_eq!(again, build);
    assert_eq!(s.engineer(again, 2), (1, true));
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
            unmeasured_pings: 2,
            budget: None,
        }),
        ..policy(model)
    }
}

/// The policy of [`warm`] with this much of what a work may spend.
fn warm_within(model: &str, budget: Option<Budget>) -> Policy {
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
    driver::warm::kept(
        held.flow,
        s.work,
        held.work,
        held.item,
        policy,
        warm,
        s.driver.accounts(),
        Time(s.clock),
    )
}

/// A work with this policy whose engineer's session waits for a tool's result, and when its next ping is due, as a
/// policy that keeps sessions warm says: a work that is not kept warm waits all the same, and a test asks whether it
/// is pinged at that time.
fn waiting(policy: Policy) -> (S, u64) {
    let mut s = S::warmed(policy);
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
        None,
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
    let mut s = S::warmed(warm("opus"));
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

// How many pings a wait is worth is worked out from what the work's own calls on the session's model cost, as the
// program reported it, and from no name of a model: where the build's round and the first ping report a token read
// at a fortieth of an hour's write, the session is pinged past the two pings a wait is worth where nothing reports a
// cost. Fails where the worth comes from a table of models by their names, or where nothing reported counts as none.
#[test]
fn a_session_is_pinged_as_often_as_the_works_own_reports_of_its_model_make_worth() {
    let mut s = S::warmed(warm("opus"));
    // What a call cost, as the program reports it, on a model whose token sent fresh costs four millionths of a dollar
    // and whose read from the cache a fifth of one: a write kept an hour costs eight, forty reads.
    let reported = |fresh: u64, written: u64, read: u64, output: u64| Spent {
        usd_micros: 4 * (fresh + 2 * written + 5 * output) + read / 5,
        uncached_tokens: fresh + written,
        cache_written_tokens: Some(written),
        cached_tokens: read,
        output_tokens: output,
        context_tokens: Some(1_000),
        ..Spent::default()
    };
    s.hands_in(100, true);
    // The build's round reports what it cost, and waits on a tool's result.
    let (run, _) = s.round();
    let measures = Measures {
        calls: 3,
        ..Measures::default()
    };
    s.on(Event::Measured {
        run,
        attempt: 1,
        measures,
    });
    s.began();
    let ask = Ask::Tool {
        name: "read".to_string(),
        input: "request".to_string(),
    };
    let spent = reported(5_000, 50_000, 3_000_000, 20_000);
    s.on(Event::Returned {
        run,
        came: Came::asks(vec![ask]),
        spent,
    });
    let held = s.driver.work(s.work).expect("the work");
    assert_eq!(
        driver::prices::learned(held.flow, held.work, "opus"),
        None,
        "a round alone does not tell two prices"
    );
    for ping in 1..=3 {
        let at = woken(&s);
        let todos = s.driver.tick(Time(at), None, None).unwrap();
        assert_eq!(pings(&s, &todos), [("engineer".to_string(), 0)], "ping {ping}");
        s.clock = at + 5_000;
        let read = reported(3, 60, 400_000, 25);
        s.driver.happened(Time(s.clock), s.work, pinged(read, None)).unwrap();
    }
    let held = s.driver.work(s.work).expect("the work");
    let prices = driver::prices::learned(held.flow, held.work, "opus").expect("a ping and a round tell the prices");
    assert!((prices.input - 4.0).abs() < 1e-6, "{prices:?}");
    assert!((prices.read - 0.2).abs() < 1e-6, "{prices:?}");
    assert_eq!(prices.pings_worth(), 39);
    assert!(
        !kept_warm(&s).is_empty(),
        "three pings of the 39 the wait is worth were sent"
    );
}

#[test]
fn a_ping_that_failed_is_tried_again_a_minute_after_and_none_past_the_caches_life() {
    let mut s = S::warmed(warm("opus"));
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
    let accounts = s.driver.accounts();
    let late = driver::warm::due(
        held.flow,
        s.work,
        held.work,
        held.item,
        &warm("opus"),
        true,
        accounts,
        kept[0].life,
    );
    assert!(late.is_empty());
}

#[test]
fn a_session_a_ping_compacted_counts_as_reopened() {
    let mut s = S::warmed(warm("opus"));
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

// A line whose session began otherwise than a new one would begin now is not kept warm: its next round begins a new
// session, which holds nothing of this one. Runs in memory in milliseconds; the one test that keeping a session warm
// asks what it began with, as it asks what model it ran on.
#[test]
fn a_line_whose_session_began_otherwise_than_a_new_one_would_now_is_not_kept_warm() {
    let (s, _) = waiting(warm("opus"));
    assert_eq!(
        kept_warm(&s).len(),
        1,
        "its session is kept warm while it began with what a new one would"
    );
    let changed = Policy {
        changed: Some(Box::new(|_, _, _, _, _| true)),
        ..warm("opus")
    };
    assert!(kept_warm_on(&s, &changed).is_empty());
}

/// The policy of [`warm`] whose carriers' check of what a new session would begin with ([`Policy::changed`]) counts
/// each time it is asked, and answers as `changed` says.
fn counting(asked: &Arc<AtomicUsize>, changed: &Arc<AtomicBool>) -> Policy {
    let (asked, changed) = (asked.clone(), changed.clone());
    Policy {
        changed: Some(Box::new(move |_, _, _, _, _| {
            asked.fetch_add(1, SeqCst);
            changed.load(SeqCst)
        })),
        ..warm("opus")
    }
}

// A wake takes the next ping from the sessions each work keeps warm, worked out again only as the work grows: asked
// again while nothing changed, it goes through no work's sessions, and asks the carriers nothing, whose check reads the
// work's worktree. A ping that ends is the session's last call, and the wake moves to the next ping. Fails where a wake
// works the pings out of every work's sessions again, as it did with every work's sessions on each wake of the loop.
// Milliseconds.
#[test]
fn a_wake_goes_through_no_works_sessions_again_while_nothing_changed() {
    let asked = Arc::new(AtomicUsize::new(0));
    let (mut s, at) = waiting(counting(&asked, &Arc::new(AtomicBool::new(false))));
    let before = asked.load(SeqCst);
    for _ in 0..3 {
        assert_eq!(s.driver.wake().map(|wake| wake.at.0), Some(at));
    }
    assert_eq!(asked.load(SeqCst), before, "a wake asked the carriers again");
    let todos = s.driver.tick(Time(at), None, None).unwrap();
    assert_eq!(pings(&s, &todos), [("engineer".to_string(), 0)]);
    s.clock = at + 1_000;
    let read = Spent {
        cached_tokens: 100,
        ..Default::default()
    };
    s.driver.happened(Time(s.clock), s.work, pinged(read, None)).unwrap();
    let asked_then = asked.load(SeqCst);
    assert_eq!(
        woken(&s),
        s.clock + 3_300_000,
        "the next ping, after the one that ended"
    );
    assert_eq!(s.driver.wake().map(|wake| wake.at.0), Some(s.clock + 3_300_000));
    assert_eq!(asked.load(SeqCst), asked_then, "a wake asked the carriers again");
}

// An item put at a level whose model is not the one its line's session ran on writes nothing to the work, and its line
// is to begin a new session: the work keeps that session warm no more, as worked out afresh, and no ping wakes the
// driver or is given. Fails where what the work keeps is worked out again only as the work grows. Milliseconds.
#[test]
fn a_line_whose_item_was_put_at_another_model_is_kept_warm_no_more() {
    let by_level = || Policy {
        model: Box::new(|item: &store::Item, _: &flow::CellSpec| {
            Some(
                if item.level.as_deref() == Some("2") {
                    "sonnet"
                } else {
                    "opus"
                }
                .to_string(),
            )
        }),
        ..warm("opus")
    };
    let (mut s, at) = waiting(by_level());
    assert_eq!(s.driver.wake().map(|wake| wake.at.0), Some(at));
    let item = s.driver.work(s.work).expect("the work").item.number;
    let level = driver::Asked::Item(store::ItemChange::Level("2".into()));
    s.driver
        .change(Time(s.clock), None, vec![(item, level)])
        .expect("the level is changed");
    assert!(kept_warm_on(&s, &by_level()).is_empty(), "its session is to be left");
    assert_eq!(s.driver.wake(), None, "woken for a ping it will not give");
    let todos = s.driver.tick(Time(at), None, None).unwrap();
    assert!(pings(&s, &todos).is_empty(), "{todos:?}");
}

// The carriers read from the work's worktree whether a line begins a new session, which a round of another line can
// change before any record of the work does. A ping due by what the work kept is given only as the work stands when it
// is due: with the line to begin a new session, none is given, and the driver waits for it no more. Fails where a ping
// is given from what the work kept before the look at the time. Milliseconds.
#[test]
fn a_ping_is_given_as_the_line_stands_when_it_is_due() {
    let (asked, changed) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicBool::new(false)));
    let (mut s, at) = waiting(counting(&asked, &changed));
    changed.store(true, SeqCst);
    let todos = s.driver.tick(Time(at), None, None).unwrap();
    assert!(pings(&s, &todos).is_empty(), "{todos:?}");
    assert_eq!(s.driver.wake(), None, "woken for a ping it will not give");
}

/// What the program's own report of what it gave the model holds in the rounds of these tests unless a test changes it:
/// the digests of its tools and of its agents.
const REPORT: [(&str, &str); 2] = [("program_tools", "t1"), ("program_agents", "a1")];

/// The round under way says what its calls came to: its first call read this from the cache of a first prompt of this
/// many tokens, with these parts before the session's messages.
fn measured_as(s: &mut S, read: u64, context: u64, before: &[(&str, &str)]) {
    let (run, _) = s.round();
    let measures = Measures {
        calls: 1,
        first_read: Some(read),
        first_context: Some(context),
        before: (before.iter())
            .map(|(part, value)| (part.to_string(), value.to_string()))
            .collect(),
        ..Measures::default()
    };
    s.on(Event::Measured {
        run,
        attempt: 1,
        measures,
    });
}

/// The round under way says what its calls came to: its first call read this from the cache, on this model, the
/// program's report as in every round, and a first prompt of 2,000 tokens, which is more than the 1,000 the sessions of
/// these tests hold: a session gone on in is never shorter.
fn measured(s: &mut S, read: u64, model: &str) {
    measured_as(s, read, 2_000, &[("model", model), REPORT[0], REPORT[1]]);
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
    assert!(!miss.expected(), "nothing before the messages differed");
    assert!(miss.fault(), "both rounds hold the program's report: a fault");
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

// The cache is asked about when the round's first call was made, and not about when the round began, which is where its
// session is decided: a round that began within the cache's life and whose first try came after it, as when the carrier
// took main in for long before the program ran, wrote the session again by design. Runs in memory in milliseconds; the
// one test that sets the two times apart, which every other test of a miss leaves a second apart.
#[test]
fn a_round_whose_first_try_came_after_the_caches_life_is_no_miss_though_it_began_within_it() {
    let mut s = S::with(warm("opus"), None);
    measured(&mut s, 0, "a");
    s.hands_in(1_000, true);
    // The build begins as the load ends, in the session the load began, and reads a tenth of it.
    measured(&mut s, 100, "a");
    s.clock += 2 * 3_600_000;
    s.asks(1_000);
    let misses = s.driver.misses(s.work);
    assert!(misses.is_empty(), "{misses:?}");
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
    assert_eq!(misses[0].cause, Cause::Differs(vec!["model".to_string()]));
    assert!(misses[0].expected());
}

// A line goes on in its session under another account, whose cache holds nothing of it: the next round's miss is
// expected, the account being what comes before the session's messages and differs from the call before.
#[test]
fn a_miss_after_another_account_was_taken_names_the_account_as_what_differs() {
    let mut s = S::with(warm("opus"), None);
    measured_under(&mut s, 0, None);
    s.hands_in(1_000, true);
    let now = Time(s.clock);
    s.driver.add_account(now, "work").expect("the account is added");
    s.driver.take_account(now, "work").expect("the account is taken");
    s.account = Some("work".to_string());
    measured_under(&mut s, 0, Some("work"));
    s.asks(1_000);
    let misses = s.driver.misses(s.work);
    assert_eq!(misses.len(), 1, "{misses:?}");
    assert_eq!(misses[0].cause, Cause::Differs(vec!["account".to_string()]));
    assert!(misses[0].expected());
    assert!(s.sessions().is_empty(), "no session begins anew for it");
}

/// The round under way said what its first call read from the cache, on the model `a`, under the account where one was
/// in use, which is left out where none was.
fn measured_under(s: &mut S, read: u64, account: Option<&str>) {
    let mut before = vec![("model", "a"), REPORT[0], REPORT[1]];
    if let Some(account) = account {
        before.push(("account", account));
    }
    measured_as(s, read, 2_000, &before);
}

/// The session's first round hands in with 1,000 tokens in its context and its second goes on in it at once, saying
/// this of its first call and this of what the round spent, so that what the second round's miss is told by is the
/// first call's alone.
fn goes_on(first: &[(&str, &str)], then: &[(&str, &str)], read: u64, context: u64, compactions: u32) -> Vec<Miss> {
    let mut s = S::with(warm("opus"), None);
    measured_as(&mut s, 0, 2_000, first);
    s.hands_in(1_000, true);
    measured_as(&mut s, read, context, then);
    s.compactions = compactions;
    s.asks(1_000);
    s.driver.misses(s.work)
}

// The shape of work 35 run 36: the program compacted the session before the round's first call, which went on in the
// summary, with a first prompt far under what the session held, and read what no compaction changes. No miss, and no
// fault of wf-agents. Red before the change: the round was listed with nothing differing, a fault.
#[test]
fn a_round_that_compacted_the_session_before_its_first_call_is_no_miss() {
    let model = [("model", "a"), REPORT[0], REPORT[1]];
    assert!(goes_on(&model, &model, 100, 500, 1).is_empty());
}

// The same round with no compaction recorded is a miss whose cause the records do not say, not a fault of wf-agents and
// not left out.
#[test]
fn a_round_whose_first_prompt_is_shorter_than_what_was_held_with_no_compaction_recorded_is_a_miss_of_unknown_cause() {
    let model = [("model", "a"), REPORT[0], REPORT[1]];
    let misses = goes_on(&model, &model, 100, 500, 0);
    assert_eq!(misses.len(), 1, "{misses:?}");
    assert_eq!(misses[0].cause, Cause::Unknown);
    assert!(!misses[0].expected() && !misses[0].fault());
}

// A compaction later in the round leaves the first prompt as long as what was held, and the first call missed all the
// same: a miss, judged as before.
#[test]
fn a_round_that_compacted_but_whose_first_prompt_was_not_shorter_is_still_a_miss() {
    let model = [("model", "a"), REPORT[0], REPORT[1]];
    let misses = goes_on(&model, &model, 100, 1_000, 1);
    assert_eq!(misses.len(), 1, "{misses:?}");
    assert_eq!(misses[0].cause, Cause::Fault);
    assert_eq!(goes_on(&model, &model, 100, 2_000, 1).len(), 1);
}

// The shape of work 23 run 15: the engine's arguments the same, the effort changed, and the program's list of tools
// shorter by one tool. The cause is the tools, which the program said; not the effort.
#[test]
fn a_miss_after_the_programs_tool_list_changed_names_the_list_and_not_the_effort() {
    let first = [("model", "a"), ("effort", "high"), REPORT[0], REPORT[1]];
    let then = [("model", "a"), ("effort", "xhigh"), ("program_tools", "t2"), REPORT[1]];
    let misses = goes_on(&first, &then, 0, 2_000, 0);
    assert_eq!(misses.len(), 1, "{misses:?}");
    assert_eq!(misses[0].cause, Cause::Differs(vec!["program_tools".to_string()]));
}

// A change of effort alone, with both rounds holding the program's report, caused no miss in the records, and names
// none: where the cache did not read the session all the same, nothing before the messages differed.
#[test]
fn a_miss_after_a_change_of_effort_alone_names_no_effort() {
    let first = [("model", "a"), ("effort", "high"), REPORT[0], REPORT[1]];
    let then = [("model", "a"), ("effort", "xhigh"), REPORT[0], REPORT[1]];
    let misses = goes_on(&first, &then, 0, 2_000, 0);
    assert_eq!(misses.len(), 1, "{misses:?}");
    assert_eq!(misses[0].cause, Cause::Fault);
}

// A round kept before the program's report was kept says nothing of what the program gave the model, and a round with
// no measures at all says nothing of what came before its messages: a miss after either is of unknown cause, and the
// report is not named as what differs.
#[test]
fn a_miss_where_the_call_before_holds_no_report_of_the_program_is_of_unknown_cause() {
    let model = [("model", "a"), REPORT[0], REPORT[1]];
    let kept_earlier = [("model", "a"), ("effort", "high"), ("tools", "x")];
    let misses = goes_on(&kept_earlier, &model, 0, 2_000, 0);
    assert_eq!(misses.len(), 1, "{misses:?}");
    assert_eq!(misses[0].cause, Cause::Unknown);
    // No measures for the call before.
    let mut s = S::with(warm("opus"), None);
    s.hands_in(1_000, true);
    measured_as(&mut s, 0, 2_000, &model);
    s.asks(1_000);
    let misses = s.driver.misses(s.work);
    assert_eq!(misses.len(), 1, "{misses:?}");
    assert_eq!(misses[0].cause, Cause::Unknown);
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
    let mut s = S::warmed(warm("opus"));
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
fn only_the_lines_the_work_can_come_back_to_and_none_while_a_round_of_its_own_line_is_under_way_are_kept_warm() {
    let mut s = S::warmed(warm("opus"));
    s.hands_in(100, true);
    s.hands_in(100, true);
    // The review's round is under way: its own line is not pinged, and the engineer's, which nothing the review writes
    // runs, is not a line the work can come back to.
    let (_, cell) = s.round();
    assert_eq!(cell, "review");
    s.began();
    assert!(kept_warm(&s).is_empty(), "a ping beside a round of its line");
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

/// The owner's remarks, which run the review's `look` cell of [`remarks`].
fn remark(text: &str) -> Event {
    Event::Wrote {
        block: "remarks".into(),
        by: Outsider::Person,
        content: text.to_string(),
        matters: true,
        value: None,
    }
}

/// The lines of the sessions kept warm, in the order of the walk.
fn lines_kept(s: &S) -> Vec<String> {
    kept_warm(s).into_iter().map(|k| k.due.line).collect()
}

/// A work of [`remarks`] whose engineer's build waits on a tool's result, with when that round ended.
fn awaiting_a_tool() -> (S, u64) {
    let mut s = S::warmed_on(remarks(), warm("opus"));
    s.hands_in(100, true);
    assert_eq!(s.round().1, "build");
    s.asks(100);
    let ended = s.clock;
    (s, ended)
}

#[test]
fn a_session_is_kept_warm_by_its_own_last_call_while_a_round_of_another_line_is_under_way() {
    let (mut s, build_ended) = awaiting_a_tool();
    // The owner's remarks run the review's look while the build waits on its tool, and it ends: the review's session,
    // called last as the round ended, is kept warm from that call, beside the engineer's, kept from its own.
    s.on(remark("one"));
    assert_eq!(s.round().1, "look");
    s.hands_in(100, true);
    let look_ended = s.clock;
    let kept = kept_warm(&s);
    assert_eq!(lines_kept(&s), ["engineer", "review"]);
    assert_eq!(
        kept[0].at.0,
        build_ended + 3_300_000,
        "55 minutes after the engineer's own last call"
    );
    assert_eq!(
        kept[1].at.0,
        look_ended + 3_300_000,
        "55 minutes after the review's own last call"
    );
    // A second remark runs the look again: a round of the review's line is under way, which has a last call of its
    // own. Its session is not pinged while its program runs, and the engineer's is kept warm by its own last call all
    // the same, as if the round were none.
    s.on(remark("two"));
    assert_eq!(s.round().1, "look");
    s.began();
    let kept = kept_warm(&s);
    assert_eq!(
        lines_kept(&s),
        ["engineer"],
        "the review's own line has its round under way"
    );
    assert_eq!(kept[0].at.0, build_ended + 3_300_000);
}

#[test]
fn a_ping_is_given_while_a_round_of_another_line_is_under_way() {
    let (mut s, build_ended) = awaiting_a_tool();
    s.on(remark("a remark"));
    s.began();
    // The engineer's ping falls due while the review's look is under way: it is given, and the review's own line,
    // whose round is under way, is not.
    let at = build_ended + 3_300_000;
    let todos = s.driver.tick(Time(at), None, None).unwrap();
    assert_eq!(pings(&s, &todos), [("engineer".to_string(), 0)]);
}

#[test]
fn a_round_of_another_line_begins_while_a_session_is_pinged() {
    let (mut s, build_ended) = awaiting_a_tool();
    let at = build_ended + 3_300_000;
    let todos = s.driver.tick(Time(at), None, None).unwrap();
    assert_eq!(pings(&s, &todos), [("engineer".to_string(), 0)]);
    s.clock = at;
    // The owner's remarks run the review's look while the engineer's session is pinged: its round begins at once, on
    // a line whose session the ping is not in.
    let rounds = s.kept().records.rounds.len();
    s.on(remark("a remark"));
    assert_eq!(
        s.kept().records.rounds.len(),
        rounds + 1,
        "the review's round was held by the ping"
    );
    assert_eq!(s.round().1, "look");
    // The ping ends beside it, and the review's round goes on.
    s.clock += 1_000;
    let read = Spent {
        cached_tokens: 100,
        context_tokens: Some(120),
        ..Default::default()
    };
    s.driver.happened(Time(s.clock), s.work, pinged(read, None)).unwrap();
    assert_eq!(s.round().1, "look");
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
                waits_for: Vec::new(),
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

/// A budget of a dollar for a work, where each letting-on of one the engine paused for it allows fifty cents more.
const BUDGET: Budget = Budget {
    work: 1_000_000,
    let_on: 500_000,
};

#[test]
fn no_session_of_a_work_the_engine_paused_for_spending_more_than_it_may_is_pinged() {
    let budget = Some(BUDGET);
    let mut s = S::warmed(warm_within("opus", budget));
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
        waits_for: Vec::new(),
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
fn a_session_of_a_work_the_engine_paused_within_what_a_letting_on_allowed_is_pinged() {
    let budget = Some(BUDGET);
    let mut s = S::warmed(warm_within("opus", budget));
    s.usd = 600_000;
    s.hands_in(100, true);
    s.asks(100);
    let at = kept_warm_on(&s, &warm_within("opus", budget))
        .first()
        .expect("a session kept warm")
        .at
        .0;
    // Its two rounds spent $1.20, past the dollar of its budget: the engine paused it for that, and the owner let it
    // on, which allows it fifty cents more.
    s.on(Event::Paused {
        by: Pauser::Engine,
        why: Some("it spent $1.20 of the $1.00 a work may spend".to_string()),
        waits_for: Vec::new(),
    });
    s.on(Event::Resumed);
    // Paused by the engine again, for a round of it that failed, it is under the $1.50 it may now spend: its session
    // is kept warm, the pause being no refusal to let it spend more.
    s.on(Event::Paused {
        by: Pauser::Engine,
        why: Some("a round of it failed".to_string()),
        waits_for: Vec::new(),
    });
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
                waits_for: Vec::new(),
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
    let todos = s.driver.tick(Time(at), None, None).unwrap();
    assert!(pings(&s, &todos).is_empty());
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
        hindrance: None,
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
    let todos = (s.driver)
        .tick(Time(at + 1), Some((Some(ACCOUNT.to_string()), Found::Well)), None)
        .unwrap();
    assert_eq!(pings(&s, &todos), [("engineer".to_string(), 0)]);
}

#[test]
fn no_ping_of_a_line_the_work_passed_for_good_while_its_pause_holds_it_at_another_lines_cell() {
    let mut s = S::warmed_on(two_lines(), warm("opus"));
    s.hands_in(100, true);
    // The owner pauses the work while the build's round is under way: having handed in, the work stands paused at the
    // look, a cell of the review's line, and nothing the review's cells write runs a cell of the engineer's.
    s.on(Event::Paused {
        by: Pauser::Owner,
        why: None,
        waits_for: Vec::new(),
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
fn a_ping_that_compacted_the_session_while_the_work_rests_runs_no_cell() {
    let mut s = S::warmed(warm("opus"));
    // A person stops the build while the load runs: once the load has handed in, the work rests, its last round the
    // load's.
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
    // The compaction is recorded as the session reopened, and runs no cell: the work rests as it did.
    assert_eq!(s.sessions(), [("engineer".to_string(), Why::Compacted)]);
    let round = (s.driver.pending().iter()).any(|t| matches!(t.effect, Effect::Round { .. }) && t.work == s.work);
    assert!(!round, "a round is to be done for the compaction");
    assert_eq!(runs(&s, "load"), 1);
}

// A round that would begin while a ping of its line is under way begins once the ping has ended, after what the ping
// did to its session, so that it is filled as the session then stands: a compaction the ping made comes before it.
#[test]
fn a_round_due_while_its_lines_ping_is_under_way_begins_after_the_ping_and_what_it_did() {
    let mut s = S::warmed(warm("opus"));
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

/// What a ping that read the engineer's session from the cache took.
fn read_the_session() -> Spent {
    Spent {
        cached_tokens: 100,
        context_tokens: Some(120),
        ..Default::default()
    }
}

// A session is pinged by its own last call as often as a wait is worth (`Warm::unmeasured_pings`, nothing in these
// works reporting what its calls cost), whatever the work's other lines run. The engineer's session waits on a tool's
// call while the review's round stays under way through every ping: each ping is due 55 minutes after the session's
// own last call, which the ping before it is, and a third is not given; the same pings as
// `a_line_that_waits_has_its_session_pinged_before_the_cache_lets_it_go_as_often_as_a_wait_is_worth` gives with no
// round beside them.
#[test]
fn a_session_beside_a_round_of_another_line_is_pinged_as_often_as_a_wait_is_worth_and_no_oftener() {
    let (mut s, build_ended) = awaiting_a_tool();
    s.on(remark("a remark"));
    assert_eq!(s.round().1, "look");
    s.began();
    let mut at = build_ended + 3_300_000;
    for ping in 1..=2 {
        assert_eq!(lines_kept(&s), ["engineer"], "before ping {ping}");
        assert_eq!(
            woken(&s),
            at,
            "ping {ping} is due 55 minutes after the session's own last call"
        );
        let todos = s.driver.tick(Time(at), None, None).unwrap();
        assert_eq!(
            pings(&s, &todos),
            [("engineer".to_string(), 0)],
            "ping {ping}, and none of the review's line, whose round is under way"
        );
        assert_eq!(
            s.round().1,
            "look",
            "the review's round is under way through ping {ping}"
        );
        s.clock = at + 5_000;
        let ended = pinged(read_the_session(), None);
        s.driver.happened(Time(s.clock), s.work, ended).unwrap();
        at = s.clock + 3_300_000;
    }
    assert_eq!(s.kept().pinged.len(), 2);
    assert!(
        lines_kept(&s).is_empty(),
        "as many pings as the wait is worth were sent"
    );
    let todos = s.driver.tick(Time(at), None, None).unwrap();
    assert!(pings(&s, &todos).is_empty(), "a third ping");
    assert_eq!(s.round().1, "look", "the review's round is still under way");
}

// A compaction by a ping is taken as it is without a round of another line (`a_ping_that_compacted_the_session_while_
// the_work_rests_runs_no_cell`): the session is recorded as reopened, and runs no cell. Beside a round of another line
// the ping changes nothing of that round: no round of the work begins beside it, and once it has ended the work goes
// on where it stood.
#[test]
fn a_ping_that_compacted_its_session_beside_a_round_of_another_line_leaves_that_round_alone() {
    let (mut s, build_ended) = awaiting_a_tool();
    s.on(remark("a remark"));
    s.began();
    let at = build_ended + 3_300_000;
    let todos = s.driver.tick(Time(at), None, None).unwrap();
    assert_eq!(pings(&s, &todos).len(), 1);
    s.clock = at + 1_000;
    let rounds = s.kept().records.rounds.len();
    let compacted = Spent {
        compactions: 1,
        ..Default::default()
    };
    let todos = s
        .driver
        .happened(Time(s.clock), s.work, pinged(compacted, None))
        .unwrap();
    assert!(
        s.sessions().contains(&("engineer".to_string(), Why::Compacted)),
        "the compaction is recorded: {:?}",
        s.sessions()
    );
    assert_eq!(
        s.kept().records.rounds.len(),
        rounds,
        "a round began beside the review's"
    );
    assert!(
        !todos
            .iter()
            .any(|t| matches!(t.effect, Effect::Round { .. } | Effect::Stop { .. })),
        "the ping's end did something to the round beside it: {todos:?}"
    );
    assert_eq!(s.round().1, "look");
    assert_eq!(runs(&s, "load"), 1, "the compaction ran no cell");
}

// A cancelled work has all its runs abandoned and is decided as ended, though the round of another line that was under
// way when it was cancelled has not yet returned: with no ping held back by a round of the work under way, that
// the work has ended is what keeps its sessions from being pinged. The cancel comes while the review's round is under
// way and the engineer's session is due: no ping is given as the round is stopped, nor after.
#[test]
fn no_session_of_a_cancelled_work_is_pinged_though_another_lines_round_was_under_way() {
    let (mut s, build_ended) = awaiting_a_tool();
    s.on(remark("a remark"));
    s.began();
    s.on(Event::Cancelled { why: None });
    let at = build_ended + 3_300_000;
    assert!(lines_kept(&s).is_empty(), "a session of a cancelled work is kept warm");
    let todos = s.driver.tick(Time(at), None, None).unwrap();
    assert!(pings(&s, &todos).is_empty(), "a ping of a cancelled work");
}

// A stop of the engine holds no call and no keep-warm ping: the round under way as the stop is asked for, which asks
// for a tool, has its call given, and the session that waits for its result is pinged when its ping is due, as with no
// stop, so that the cache the engine's next start goes on from is kept; the engine may not end while the ping is under
// way. Fails where a stop holds either, or lets the engine end with a ping under way. Milliseconds; no other test gives
// a call or a ping under a stop.
#[test]
fn a_stop_holds_neither_a_call_nor_a_ping() {
    let mut s = S::warmed(warm("opus"));
    s.hands_in(100, true);
    s.driver.ask_to_stop(Time(s.clock), false).expect("the stop is kept");
    s.asks(100);
    let calls = (s.driver.pending().iter())
        .filter(|t| t.work == s.work && matches!(t.effect, Effect::Call { .. }))
        .count();
    assert_eq!(calls, 1, "the round's call is given under the stop");
    let at = woken(&s);
    let todos = s.driver.tick(Time(at), None, None).expect("the driver takes the time");
    assert_eq!(
        pings(&s, &todos),
        [("engineer".to_string(), 0)],
        "the waiting session is pinged under the stop"
    );
    assert!(!s.driver.may_end(), "a ping is under way");
    s.clock = at + 5_000;
    s.driver
        .happened(Time(s.clock), s.work, pinged(Spent::default(), None))
        .unwrap();
    assert!(s.driver.may_end(), "nothing is under way once the ping has ended");
}
