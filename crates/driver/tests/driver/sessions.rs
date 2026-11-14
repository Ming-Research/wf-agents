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
        writes = { text = "A new session of the line `{line}`: {why}.", limit = "its context was {context} tokens as its last round ended, past the limit of {limit}", model = "its rounds now run on another model than {was}, which it was begun on", account = "its rounds now run under another account than it was begun under", owner = "the owner asked for one", flagged = "the service's safety check flagged its last round", lost = "the agents' program no longer had the session it was in", compacted = "The session of the line `{line}` was compacted: it holds only a summary of what it read." }
        [blocks.tree]
        outside = "tree"
        shown = true
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
    let build = cell("engineer", r#"["request", "context"]"#, r#"["code"]"#) + "when = { test = \"going\" }\n";
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
        sources: [SESSION.to_string(), "tree".to_string()].into(),
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
fn a_cell_skipped_after_it_asked_for_calls_holds_its_lines_session_no_longer() {
    let mut s = S::new();
    s.hands_in(100, true);
    // The build asks for a tool past the limit: the line goes on in its session while the build waits.
    s.asks(600);
    assert!(s.sessions().is_empty(), "a cell of the line waits for its result");
    // A person stops the build: skipped now, it waits for nothing, and the limit ends the line's session.
    s.on(Event::Wrote {
        block: "stop".into(),
        by: Outsider::Person,
        content: "Stop.".to_string(),
        matters: true,
        value: Some("yes".to_string()),
    });
    let limit = Why::Limit {
        context: 600,
        limit: 500,
    };
    assert_eq!(s.sessions().last(), Some(&("engineer".to_string(), limit)));
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
        limit: 500,
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
