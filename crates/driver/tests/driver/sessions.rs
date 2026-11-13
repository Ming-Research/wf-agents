//! When a line of a work begins a new session: the driver records it with why, from the work's records and the
//! installation's policy, and writes the block of the source `session` where a cell of the line reads it, so that the
//! cell runs again before the line's next round.

use std::sync::{Arc, Mutex};

use decide::BlockId;
use driver::Policy;
use driver::{FirstWrite, NewItem, SESSION};
use engine::RanOn;
use engine::{Ask, Handed, Session, Why};
use flow::{Compiled, Has};
use work::sessions::{moved, of_round};

use super::*;

/// The flow: `load` reads the session block and loads the engineer's session, `build` goes on in it, and `review` has
/// a line of its own.
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
        [blocks.request]
        people = true
        [blocks.session]
        outside = "session"
        [blocks.context]
        [blocks.code]
        [blocks.verdict]
        [blocks.merged]
        shown = true
    "#;
    let cell = |line: &str, inputs: &str, outputs: &str| {
        format!(
            "title = \"A cell\"\nkind = \"agent\"\nline = \"{line}\"\neffort = \"low\"\ntask = \"Do it.\"\ninputs = {inputs}\noutputs = {outputs}\n"
        )
    };
    let load = cell("engineer", r#"["request", "session"]"#, r#"["context"]"#);
    let build = cell("engineer", r#"["request", "context"]"#, r#"["code"]"#);
    let review = cell("review", r#"["code"]"#, r#"["verdict"]"#);
    let merge =
        "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\ninputs = [\"verdict\"]\noutputs = [\"merged\"]\n";
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

/// The policy where every round runs on this model, with a limit of 500 tokens.
fn policy(model: &str) -> Policy {
    let model = model.to_string();
    Policy {
        limit: 500,
        on: Box::new(move |_, _| Some(on(&model))),
    }
}

/// The account rounds run under, as the installation's settings say it each time the policy is asked: none for the
/// program's own default directory.
type Account = Arc<Mutex<Option<String>>>;

fn account(dir: Option<&str>) -> Account {
    Arc::new(Mutex::new(dir.map(str::to_string)))
}

/// The policy where every round runs on this model, under the account as it is when asked, with a limit of 500 tokens.
fn policy_under(model: &str, account: Account) -> Policy {
    let model = model.to_string();
    Policy {
        limit: 500,
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
}

impl S {
    fn new() -> S {
        S::with(policy("opus"), None)
    }

    /// A driver with the policy, whose rounds' tries begin under the account.
    fn with(policy: Policy, account: Option<&str>) -> S {
        let db = Db::new();
        let flow = flow();
        let (mut driver, _) = Driver::open(
            Box::new(db.open()),
            vec![flow.clone()],
            SLOTS,
            Some(policy),
            Time(START),
        )
        .unwrap();
        let new = NewItem {
            title: "Greet".to_string(),
            level: "1".to_string(),
            depends_on: Vec::new(),
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
        }
    }

    fn kept(&self) -> &engine::Work {
        self.driver.work(self.work).expect("the work").work
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
                Effect::Round { run, cell, .. } => Some((run, cell.0)),
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
        let block = BlockId::from("session");
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

#[test]
fn a_round_past_the_limit_has_the_cell_that_loads_the_session_run_again_once_the_lines_runs_have_ended() {
    let mut s = S::new();
    s.hands_in(100, true);
    let (build, _) = s.round();
    // The build asks for a tool past the limit: it waits on the result in the session that holds what it did.
    s.asks(600);
    assert!(s.sessions().is_empty(), "a cell of the line waits for its result");
    assert!(s.session_block().is_empty());
    let call = s.kept().records.called.last().expect("the build's call").call;
    s.on(Event::Done {
        call,
        result: "the request".to_string(),
    });
    // Entered again for the result, the build goes on in its session.
    let (again, cell) = s.round();
    assert_eq!(cell, "build");
    assert_ne!(again, build);
    assert_eq!(s.engineer(again, 1), (0, false), "the build goes on in its session");
    assert!(!moved(s.kept(), "engineer", again, 1));
    // Its run ends past the limit: the line begins a new session at its next round.
    s.hands_in(600, true);
    let limit = Why::Limit {
        context: 600,
        limit: 500,
    };
    assert_eq!(s.sessions().last(), Some(&("engineer".to_string(), limit)));
    let block = s.session_block();
    assert_eq!(block.len(), 1);
    assert!(
        block[0].contains("`engineer`") && block[0].contains("600"),
        "{}",
        block[0]
    );
    // The loading cell runs before the line goes on, in the new session, which it begins.
    let (load, cell) = s.round();
    assert_eq!(cell, "load");
    assert_eq!(s.engineer(load, 1), (1, true));
    s.hands_in(100, false);
    assert_eq!(s.session_block().len(), 1, "nothing more is written");
}

#[test]
fn a_run_waiting_on_a_tools_result_as_its_line_begins_a_new_session_is_told_what_it_was_doing() {
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
    // asked: it is told what a session that begins is told.
    let (again, cell) = s.round();
    assert_eq!(cell, "build");
    assert_ne!(again, build);
    assert_eq!(s.engineer(again, 1), (1, false));
    assert!(moved(s.kept(), "engineer", again, 1));
    s.hands_in(100, true);
    let (review, _) = s.round();
    assert!(!moved(s.kept(), "review", review, 1), "a run's first round");
}

#[test]
fn the_limit_ends_no_session_at_a_return_sent_back_nor_at_the_round_of_the_cell_that_loads_it() {
    let mut s = S::new();
    s.hands_in(900, true);
    assert!(s.sessions().is_empty(), "the load's own size");
    let (build, _) = s.round();
    let wrong = Handed {
        block: "verdict".into(),
        content: "not the build's".to_string(),
        matters: true,
        value: None,
    };
    s.ends(900, Came::hands_in(vec![wrong]));
    assert_eq!(s.round().0, build, "sent back");
    assert!(
        s.sessions().is_empty(),
        "the return is written again in the session that wrote it"
    );
    s.hands_in(900, true);
    assert!(matches!(s.sessions().last(), Some((line, Why::Limit { .. })) if line == "engineer"));
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
    // the new one, and is told what a session that begins is told.
    let again = s.round().0;
    assert_ne!(again, review);
    let line = |r: RunId| r == review || r == again;
    assert_eq!(of_round(s.kept(), "review", line, again, 1), (1, true));
    assert!(moved(s.kept(), "review", again, 1));
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
    // Given again, it does not go on in the session the review's round was flagged in: it begins the new one, and is
    // told what a session that begins is told.
    let line = |r: RunId| r == review || r == again;
    assert_eq!(of_round(s.kept(), "review", line, again, 1), (1, true));
    assert!(moved(s.kept(), "review", again, 1));
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
    assert!(moved(s.kept(), "engineer", again, 1));
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

#[test]
fn another_account_begins_the_new_session_before_a_context_past_the_limit_does() {
    let under = account(None);
    let mut s = S::with(policy_under("opus", under.clone()), None);
    s.hands_in(100, true);
    let (run, cell) = s.round();
    assert_eq!(cell, "build");
    // The build's try began under the account before; it is changed as the try runs, and the round ends past the
    // limit, its run ending: both would begin a new session, and the account says why.
    s.began();
    *under.lock().unwrap() = Some("second".to_string());
    let handed = Handed {
        block: "code".into(),
        content: "the code".to_string(),
        matters: true,
        value: None,
    };
    let spent = Spent {
        input_tokens: 10,
        context_tokens: Some(600),
        ..Default::default()
    };
    s.on(Event::Returned {
        run,
        came: Came::hands_in(vec![handed]),
        spent,
    });
    assert_eq!(s.sessions(), [("engineer".to_string(), Why::Account { was: None })]);
}
