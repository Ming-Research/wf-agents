//! What reaches the carriers of each project, and what a work whose item has no project the settings name is answered:
//! its round fails or ends with nothing, its call is answered in the words beside the flows, and, where its item has no
//! project at all, it is let go of.

use std::sync::{Arc, Mutex};

use decide::{CallId, End, RunId};
use engine::Event;
use flow::Compiled;
use service::{CallReply, Message, Release, ReleaseReply, Round, RoundReply, Rounds, ToolCall, Tools};
use store::{First, Item, ItemNumber, WorkNumber};
use tokio::sync::mpsc;
use work::{Given, Spent, Work};

use super::*;

const WORK: WorkNumber = WorkNumber(4);

/// What reaches the carriers of the project `one`: each thing given them, by what it is and the work's item.
#[derive(Default)]
struct Reached(Mutex<Vec<String>>);

impl Reached {
    fn note(&self, what: &str, item: &Item) {
        self.0.lock().unwrap().push(format!("{what} #{}", item.number.0));
    }
}

impl Rounds for Reached {
    fn give(&self, round: Round, _: RoundReply) {
        self.note("give", &round.item);
    }

    fn stop(&self, round: Round, _: RoundReply) {
        self.note("stop", &round.item);
    }

    fn release(&self, release: Release, _: ReleaseReply) {
        self.note("release", &release.item);
    }
}

impl Tools for Reached {
    fn call(&self, call: ToolCall, _: CallReply) {
        self.note("call", &call.item);
    }

    fn withdraw(&self, call: ToolCall, _: CallReply) {
        self.note("withdraw", &call.item);
    }
}

/// The words beside the flows of a call answered for a work whose item has no project these name.
fn said() -> NoProject {
    NoProject {
        unnamed: "Nothing was run: {project} is not named.".to_string(),
        none: "Nothing was run: no project.".to_string(),
    }
}

/// The carriers of the one project `one`, and what reaches them.
fn by_project<T: ?Sized>(reached: Arc<T>) -> ByProject<T> {
    ByProject {
        of: [("one".to_string(), reached)].into(),
        said: said(),
    }
}

/// The flow of the end-to-end tests, which has every act.
fn flow() -> Arc<Compiled> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/real/flow");
    let source = flow::read(&dir).unwrap();
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = flow::Has {
        sources: names(&["tree", "main"]),
        acts: names(&["start", "check", "merge"]),
        ..Default::default()
    };
    Arc::new(flow::compile(&source, &has).unwrap())
}

/// The item of this number, of this project or of none.
fn item(number: u64, project: Option<&str>) -> Item {
    let first = First {
        title: "Greet the user".to_string(),
        level: None,
        project: project.map(str::to_string),
        depends_on: Vec::new(),
        tags: Vec::new(),
    };
    Item::new(ItemNumber(number), work::Time(0), first, Vec::new())
}

fn round(item: Item) -> Round {
    let flow = flow();
    Round {
        work: WORK,
        cell: flow.cells[0].clone(),
        flow,
        run: RunId(1),
        number: 1,
        given: Given::Task,
        item,
        kept: Arc::new(Work::default()),
        lines: Default::default(),
    }
}

fn call(item: Item) -> ToolCall {
    ToolCall {
        work: WORK,
        flow: flow(),
        run: RunId(1),
        call: CallId(2),
        name: "run_tests".to_string(),
        input: String::new(),
        item,
        kept: Arc::new(Work::default()),
    }
}

fn release(item: Item) -> Release {
    Release {
        work: WORK,
        end: End::Cancelled,
        flow: flow(),
        item,
        kept: Arc::new(Work::default()),
    }
}

/// What was said down the channel, each event once.
fn said_down(mut from: mpsc::UnboundedReceiver<Message>) -> Vec<Event> {
    let mut events = Vec::new();
    while let Ok(message) = from.try_recv() {
        if let Message::Happened { work, event } = message {
            assert_eq!(work, WORK);
            events.push(event);
        }
    }
    events
}

// What the loop gives the works of a project's items reaches that project's carriers, and nothing of a work whose item
// has another project or none reaches them.
#[test]
fn what_the_loop_gives_reaches_the_carriers_of_the_works_project() {
    let reached = Arc::new(Reached::default());
    let rounds = by_project::<dyn Rounds>(reached.clone());
    let tools = by_project::<dyn Tools>(reached.clone());
    let (to, from) = mpsc::unbounded_channel();
    for project in [Some("one"), Some("two"), None] {
        let number = if project == Some("one") { 1 } else { 2 };
        rounds.give(
            round(item(number, project)),
            RoundReply::new(WORK, RunId(1), to.clone()),
        );
        rounds.stop(
            round(item(number, project)),
            RoundReply::new(WORK, RunId(1), to.clone()),
        );
        rounds.release(release(item(number, project)), ReleaseReply::new(WORK, to.clone()));
        tools.call(call(item(number, project)), CallReply::new(WORK, CallId(2), to.clone()));
        tools.withdraw(call(item(number, project)), CallReply::new(WORK, CallId(2), to.clone()));
    }
    assert_eq!(
        *reached.0.lock().unwrap(),
        ["give #1", "stop #1", "release #1", "call #1", "withdraw #1"]
    );
    drop(to);
    assert_eq!(said_down(from).len(), 10, "each of the others answered once");
}

// A round of a work whose item has a project the settings do not name, or none, fails, saying why; stopped, it ends with
// nothing handed in.
#[test]
fn a_round_of_an_item_of_no_project_named_fails_saying_why_and_stopped_ends_with_nothing() {
    let rounds = by_project::<dyn Rounds>(Arc::new(Reached::default()));
    for (project, why) in [
        (
            Some("two"),
            "the item #2 belongs to the project two, which the installation's settings do not name",
        ),
        (None, "the item #2 belongs to no project"),
    ] {
        let (to, from) = mpsc::unbounded_channel();
        rounds.give(round(item(2, project)), RoundReply::new(WORK, RunId(1), to.clone()));
        rounds.stop(round(item(2, project)), RoundReply::new(WORK, RunId(1), to));
        let events = said_down(from);
        assert_eq!(events.len(), 2, "{events:?}");
        let Event::Failed { why: said, spent, .. } = &events[0] else {
            panic!("the round failed: {events:?}");
        };
        assert!(said.starts_with(why), "{said}");
        assert_eq!(*spent, Spent::default());
        let Event::Returned { came, spent, .. } = &events[1] else {
            panic!("the stopped round ended: {events:?}");
        };
        assert_eq!((came, spent), (&engine::Came::hands_in(Vec::new()), &Spent::default()));
    }
}

// A work whose item belongs to no project is let go of as it ends, since it made nothing in any repository; one of a
// project the settings do not name is not, saying why, since that project's repository may hold what it made.
#[test]
fn a_work_of_no_project_is_let_go_of_and_one_of_a_project_not_named_says_why_it_is_not() {
    let rounds = by_project::<dyn Rounds>(Arc::new(Reached::default()));
    let (to, from) = mpsc::unbounded_channel();
    rounds.release(release(item(2, None)), ReleaseReply::new(WORK, to.clone()));
    rounds.release(release(item(3, Some("two"))), ReleaseReply::new(WORK, to));
    let events = said_down(from);
    assert_eq!(events.len(), 2, "{events:?}");
    assert_eq!(events[0], Event::Released);
    let Event::NotReleased { why } = &events[1] else {
        panic!("it is not let go of: {events:?}");
    };
    assert!(why.contains("the project two"), "{why}");
}

// A call of a work whose item has no project the settings name is answered in the words beside the flows, the
// project's name filled in; withdrawn, it has the result of a call withdrawn.
#[test]
fn a_call_of_an_item_of_no_project_named_is_answered_in_the_words_beside_the_flows() {
    let tools = by_project::<dyn Tools>(Arc::new(Reached::default()));
    let (to, from) = mpsc::unbounded_channel();
    tools.call(call(item(2, Some("two"))), CallReply::new(WORK, CallId(2), to.clone()));
    tools.call(call(item(3, None)), CallReply::new(WORK, CallId(2), to.clone()));
    tools.withdraw(call(item(3, None)), CallReply::new(WORK, CallId(2), to));
    let results: Vec<String> = (said_down(from).into_iter())
        .map(|event| match event {
            Event::Done { result, .. } => result,
            other => panic!("the call has a result: {other:?}"),
        })
        .collect();
    assert_eq!(
        results,
        [
            "Nothing was run: two is not named.",
            "Nothing was run: no project.",
            service::WITHDRAWN
        ]
    );
}

// The words beside the flows say both cases, and the project's name is the one place they fill.
#[test]
fn the_words_beside_the_flows_say_a_call_of_no_project_named() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    let beside = flow::agents::read(&root).expect("what is written beside the flows is read");
    let said = &beside.installation.no_project;
    assert!(said.unnamed.contains("{project}"), "{}", said.unnamed);
    assert!(!said.none.contains('{'), "{}", said.none);
}
