//! What reaches the carriers of each project, and what a work whose item has no project the settings name, or a project
//! held for its declaration, is answered: its round fails or ends with nothing, its call is answered in the words
//! beside the flows or with why the project is held, and, where its item has no project at all, it is let go of.

use std::sync::{Arc, Mutex};

use decide::{CallId, End, RunId};
use engine::Event;
use flow::Compiled;
use service::{CallReply, Message, OfWork, Release, ReleaseReply, Round, RoundReply, Rounds, ToolCall, Tools};
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

    fn seen(&self, asked: OfWork) -> Pin<Box<dyn Future<Output = Result<Seen, String>> + Send + '_>> {
        self.note("seen", &asked.item);
        Box::pin(async { Ok(Seen::default()) })
    }

    fn changes(&self, asked: OfWork) -> Pin<Box<dyn Future<Output = Result<Option<Changes>, String>> + Send + '_>> {
        self.note("changes", &asked.item);
        Box::pin(async { Ok(None) })
    }

    fn diff(
        &self,
        asked: OfWork,
        _: String,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Diff>, String>> + Send + '_>> {
        self.note("diff", &asked.item);
        Box::pin(async { Ok(None) })
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

/// Why the project `held` is held.
const HELD: &str =
    "refs/heads/main:wf-agents.toml, the project's declaration on main where works branch from, cannot be read";

/// Words, unlike the files' own, for the result of a call withdrawn.
const WITHDRAWN: &str = "Dropped, no answer came.";

/// The carriers of the one project `one`, and what reaches them, and the project `held`, which is held.
fn by_project<T: ?Sized>(reached: Arc<T>) -> ByProject<T> {
    ByProject {
        of: [
            ("one".to_string(), Ok(reached)),
            ("held".to_string(), Err(HELD.to_string())),
        ]
        .into(),
        said: said(),
        withdrawn: WITHDRAWN.to_string(),
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
        continues: None,
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
        account: None,
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
        test_time: None,
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
            WITHDRAWN
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

/// The work of this item as the driver gives it to be looked at.
fn of_work(item: Item) -> OfWork {
    OfWork {
        work: WORK,
        item,
        merged: None,
        place: decide::Seq(1),
        under_way: false,
    }
}

// What a work changed is asked of the carrier of its item's project; where the item has a project the settings do not
// name, or none, there is nothing to say of it, and no carrier is asked. Well under a millisecond alone; no other test
// holds this to the carriers of more than one project.
#[tokio::test]
async fn what_a_work_changed_is_asked_of_its_projects_carrier_and_is_nothing_where_it_has_none() {
    let reached = Arc::new(Reached::default());
    let rounds = by_project::<dyn Rounds>(reached.clone());
    for project in [Some("one"), Some("two"), None] {
        let number = if project == Some("one") { 1 } else { 2 };
        let of = || of_work(item(number, project));
        assert!(rounds.seen(of()).await.is_ok());
        assert!(matches!(rounds.changes(of()).await, Ok(None)));
        assert!(matches!(rounds.diff(of(), "farewell.txt".to_string()).await, Ok(None)));
    }
    assert_eq!(*reached.0.lock().unwrap(), ["seen #1", "changes #1", "diff #1"]);
}

// A project held for its declaration carries nothing out: a round of its works fails with why, and stopped ends with
// nothing; a work of it is not let go of, saying why; a call of it is answered with why, and withdrawn has the result of
// a call withdrawn; and what the loop gives the project that is not held reaches it all the same.
#[test]
fn a_held_project_carries_nothing_out_and_says_why_its_works_stop() {
    let reached = Arc::new(Reached::default());
    let rounds = by_project::<dyn Rounds>(reached.clone());
    let tools = by_project::<dyn Tools>(reached.clone());
    let (to, from) = mpsc::unbounded_channel();
    rounds.give(
        round(item(5, Some("held"))),
        RoundReply::new(WORK, RunId(1), to.clone()),
    );
    rounds.stop(
        round(item(5, Some("held"))),
        RoundReply::new(WORK, RunId(1), to.clone()),
    );
    rounds.release(release(item(5, Some("held"))), ReleaseReply::new(WORK, to.clone()));
    tools.call(call(item(5, Some("held"))), CallReply::new(WORK, CallId(2), to.clone()));
    tools.withdraw(call(item(5, Some("held"))), CallReply::new(WORK, CallId(2), to.clone()));
    rounds.give(round(item(1, Some("one"))), RoundReply::new(WORK, RunId(1), to.clone()));
    drop(to);
    assert_eq!(
        *reached.0.lock().unwrap(),
        ["give #1"],
        "nothing reaches a project that is held"
    );
    let events = said_down(from);
    assert_eq!(events.len(), 5, "{events:?}");
    let Event::Failed { why, spent, .. } = &events[0] else {
        panic!("the round failed: {events:?}");
    };
    assert_eq!(
        *why,
        format!("the item #5 belongs to the project held, which is held: {HELD}")
    );
    assert_eq!(*spent, Spent::default());
    assert!(matches!(&events[1], Event::Returned { .. }), "{events:?}");
    let Event::NotReleased { why } = &events[2] else {
        panic!("it is not let go of: {events:?}");
    };
    assert!(why.contains(HELD), "{why}");
    let Event::Done { result, .. } = &events[3] else {
        panic!("the call has a result: {events:?}");
    };
    assert_eq!(
        *result,
        format!("the item #5 belongs to the project held, which is held: {HELD}")
    );
    assert!(
        matches!(&events[4], Event::Done { result, .. } if result == WITHDRAWN),
        "{events:?}"
    );
}

// What the page asks of a held project's work is answered with why, not with nothing: its worktree and its changes may
// be there. A work of no project, or of one the settings do not name, has none, and is answered with nothing.
#[tokio::test]
async fn the_page_is_told_why_a_held_projects_work_cannot_be_read_and_nothing_where_there_is_none() {
    let rounds = by_project::<dyn Rounds>(Arc::new(Reached::default()));
    let of = |project: Option<&str>| OfWork {
        work: WORK,
        item: item(5, project),
        merged: None,
        place: decide::Seq::default(),
        under_way: false,
    };
    let said = |why: Result<(), String>| why.expect_err("it is said why");
    let held = of(Some("held"));
    assert!(said(rounds.seen(held.clone()).await.map(|_| ())).contains(HELD));
    assert!(said(rounds.changes(held.clone()).await.map(|_| ())).contains(HELD));
    assert!(said(rounds.diff(held, "a.txt".to_string()).await.map(|_| ())).contains(HELD));
    for project in [None, Some("two")] {
        assert!(rounds.seen(of(project)).await.is_ok(), "{project:?}");
        assert_eq!(rounds.changes(of(project)).await, Ok(None), "{project:?}");
        assert_eq!(
            rounds.diff(of(project), "a.txt".to_string()).await,
            Ok(None),
            "{project:?}"
        );
    }
}

/// Carriers whose host says one pull request is open, by the project it is asked of.
struct Hosted;

impl Rounds for Hosted {
    fn give(&self, _: Round, _: RoundReply) {}

    fn stop(&self, _: Round, _: RoundReply) {}

    fn open_pulls(
        &self,
        project: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Vec<OpenPull>>, String>> + Send + '_>> {
        let title = project.to_string();
        Box::pin(async move {
            Ok(Some(vec![OpenPull {
                number: 7,
                title,
                url: "https://host.example/pull/7".to_string(),
                draft: false,
                updated: 0,
            }]))
        })
    }
}

// The pull requests open on a project's host are asked of that project's carriers alone; a project the settings do not
// name, or one held for its declaration, is said not to be asked, with why, which says nothing of there being none.
#[tokio::test]
async fn the_open_pull_requests_are_asked_of_the_named_projects_host_and_one_not_named_says_so() {
    let rounds = by_project::<dyn Rounds>(Arc::new(Hosted));
    let open = rounds
        .open_pulls("one")
        .await
        .expect("the host says")
        .expect("it has pull requests");
    assert_eq!((open[0].number, open[0].title.as_str()), (7, "one"));
    assert_eq!(
        rounds.open_pulls("two").await,
        Err("the installation's settings do not name the project two".to_string())
    );
    assert_eq!(
        rounds.open_pulls("held").await,
        Err(format!("the project held is held: {HELD}"))
    );
}
