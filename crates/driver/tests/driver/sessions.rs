//! When a line of a work begins a new session: the driver records it with why, from the work's records and the
//! installation's policy, and writes the block of the source `session` where a cell of the line reads it, so that the
//! cell runs again before the line's next round.

use decide::BlockId;
use driver::Policy;
use driver::{FirstWrite, NewItem, SESSION};
use engine::RanOn;
use engine::{Ask, Handed, Session, Why};
use flow::{Compiled, Has};
use work::sessions::of_round;

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
    };
    let has = Has {
        sources: [SESSION.to_string()].into(),
        acts: ["merge".to_string()].into(),
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

/// The policy where every round runs on this model, with a limit of 500 tokens.
fn policy(model: &str) -> Policy {
    let model = model.to_string();
    Policy {
        limit: 500,
        on: Box::new(move |_, _| Some(on(&model))),
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
}

impl S {
    fn new() -> S {
        let db = Db::new();
        let flow = flow();
        let (mut driver, _) = Driver::open(
            Box::new(db.open()),
            vec![flow.clone()],
            Some(policy("opus")),
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
        // The round's one try began on the model, as its carrier says.
        let ran = on(&self.model);
        self.on(Event::Ran { run, on: ran });
        let spent = Spent {
            input_tokens: 10,
            context_tokens: Some(context),
            ..Default::default()
        };
        self.on(Event::Returned { run, came, spent });
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
        self.ends(context, Came::HandsIn(vec![handed]));
    }

    fn asks(&mut self, context: u64) {
        let ask = Ask::Tool {
            name: "read".to_string(),
            input: "request".to_string(),
        };
        self.ends(context, Came::Asks(vec![ask]));
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
fn a_round_past_the_limit_has_the_cell_that_loads_the_session_run_again_before_the_line_goes_on() {
    let mut s = S::new();
    s.hands_in(100, true);
    let (build, _) = s.round();
    s.asks(600);
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
    // The loading cell runs before the build's next round, in the new session, which it begins.
    let (load, cell) = s.round();
    assert_eq!(cell, "load");
    assert_eq!(s.engineer(load, 1), (1, true));
    s.hands_in(100, false);
    let call = s.kept().records.called.last().expect("the build's call").call;
    s.on(Event::Done {
        call,
        result: "the request".to_string(),
    });
    assert_eq!(s.round(), (build, "build".to_string()));
    assert_eq!(
        s.engineer(build, 2),
        (1, false),
        "the build goes on in the session the load began"
    );
    assert_eq!(s.session_block().len(), 1, "nothing more is written");
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
    s.ends(900, Came::HandsIn(vec![wrong]));
    assert_eq!(s.round().0, build, "sent back");
    assert!(
        s.sessions().is_empty(),
        "the return is written again in the session that wrote it"
    );
    s.hands_in(900, true);
    assert!(matches!(s.sessions().last(), Some((line, Why::Limit { .. })) if line == "engineer"));
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
    let (driver, todos) = Driver::open(
        Box::new(s.db.open()),
        vec![s.flow.clone()],
        Some(policy("sonnet")),
        Time(s.clock),
    )
    .unwrap();
    s.driver = driver;
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
