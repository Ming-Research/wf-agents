//! A script played on the trial flow, and on flows whose cells read main: its items added, the acts of people, of
//! whoever plans the work and of the outside given as their states come, the engine stopped and opened again where an
//! act says, main moved once for every item that moves it, and each work judged against its path; and what a script's
//! items, acts and paths name that the flows lack, told in one pass.

use flow::Compiled;
use service::{Bounds, Judged, Played, Script, play};
use store::{Storage, Store, WorkNumber};

use crate::{START, trial_dir, trial_flow, words};

/// The trial's script with these items.
fn script(items: &str) -> Script {
    let trial = std::fs::read_to_string(trial_dir().join("scripts/trial.toml")).expect("the script is read");
    Script::read(&format!("{trial}\n{items}")).expect("the script reads")
}

/// Plays the script on the trial flow over a store of its own, within an hour of the clock and a thousand things
/// taken; or what is wrong with it.
async fn played(script: &Script) -> Result<Played, Vec<String>> {
    played_on(script, trial_flow()).await.0
}

/// Plays the script on this flow over a store of its own, within an hour of the clock and a thousand things taken,
/// and gives what came of it, or what is wrong with it, with the store, which the directory holds as `store.sqlite`.
async fn played_on(script: &Script, flow: Compiled) -> (Result<Played, Vec<String>>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a directory for the store");
    let bounds = Bounds {
        steps: 1000,
        ms: 3_600_000,
    };
    let path = dir.path().join("store.sqlite");
    let played = play(script, &[flow], &words(), &path, "trial", START, bounds).await;
    (played, dir)
}

/// The store a play left in this directory.
fn store(dir: &tempfile::TempDir) -> Store {
    Store::open(dir.path().join("store.sqlite")).expect("the store opens")
}

/// A writer of the text a request asks for, which reads main as well, and a merge, the engine's own cell.
fn moving() -> Compiled {
    let flow = r#"
        name = "moving"
        order = ["write", "merge"]
        [limits]
        again = 2
        [lines.main]
        [blocks.request]
        people = true
        [blocks.main]
        outside = "main"
        [blocks.text]
        [blocks.merged]
        shown = true
    "#;
    let write = r#"
        title = "Write"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Write the text the request asks for."
        inputs = ["request", "main"]
        outputs = ["text"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        inputs = ["text"]
        outputs = ["merged"]
    "#;
    let source = flow::Source {
        flow: flow.to_string(),
        cells: [("write", write), ("merge", merge)]
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
        ..Default::default()
    };
    let has = flow::Has {
        sources: ["main".to_string()].into(),
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the definition compiles")
}

/// Two items of the moving flow, each moving main at the state its act names, with these paths.
fn moving_items(first: (&str, &str), second: (&str, &str)) -> Script {
    let item = |title: &str, (at, path): (&str, &str)| {
        format!(
            r#"
            [[items]]
            title = "{title}"
            flow = "moving"
            writes = [{{ block = "request", content = "{title}, please." }}]
            acts = [{{ at = {at}, do = "main", content = "The head of main." }}]
            path = [{path}]
            "#
        )
    };
    let text = format!("{}{}", item("Greet", first), item("Part", second));
    Script::read(&text).expect("the script reads")
}

/// How many versions of main the work of this number was written.
fn mains(store: &Store, work: u64) -> usize {
    let kept = store.load(WorkNumber(work)).expect("the store has the work");
    (kept.records.written.iter())
        .filter(|w| w.block.as_str() == "main")
        .count()
}

/// The item every play here adds, with these acts and this path.
fn item(acts: &str, path: &str) -> String {
    format!(
        r#"
        [[items]]
        title = "Greet the user"
        flow = "trial"
        writes = [{{ block = "request", content = "Greet the user, please." }}]
        acts = [{acts}]
        path = [{path}]
        "#
    )
}

/// The owner answers the plan's question once the plan waits for it.
const ANSWER: &str =
    r#"{ at = { waits = "plan", for = "answer", question = "named" }, do = "answer", question = "named", option = 1 }"#;

/// The trial's way from the plan's question to the merge, as its script has it: the build asks the CI, the review
/// objects to the first build and is content with the second.
const MERGED: &str = r#"{ runs = "plan" }, { waits = "plan", for = "answer", question = "named" }, { runs = "plan" },
    { runs = "build" }, { waits = "build", for = "tool" }, { runs = "build" },
    { runs = "review" }, { runs = "build" }, { runs = "review" }, { runs = "merge" }, { ends = "merged" }"#;

#[tokio::test(start_paused = true)]
async fn an_answer_while_the_plan_waits_for_it_takes_the_work_to_its_merge() {
    let played = played(&script(&item(ANSWER, MERGED)))
        .await
        .expect("the script is played");
    assert!(played.took(), "{played}");
}

#[tokio::test(start_paused = true)]
async fn a_pause_while_the_builds_round_is_under_way_holds_the_work_until_it_is_let_on() {
    let acts = format!(
        r#"{ANSWER}, {{ at = {{ under_way = "build" }}, do = "pause" }},
        {{ at = {{ waits = "build", for = "tool" }}, do = "resume" }}"#
    );
    let path = r#"{ runs = "plan" }, { waits = "plan", for = "answer" }, { runs = "plan" }, { runs = "build" },
        { paused = "owner" }, { waits = "build", for = "tool" }, { let_on = true }, { runs = "build" },
        { runs = "review" }, { runs = "build" }, { runs = "review" }, { runs = "merge" }, { ends = "merged" }"#;
    let played = played(&script(&item(&acts, path))).await.expect("the script is played");
    assert!(played.took(), "{played}");
}

#[tokio::test(start_paused = true)]
async fn the_engine_stopped_while_the_reviews_round_is_under_way_gives_the_round_again_on_the_same_path() {
    let acts = format!(
        r#"{ANSWER}, {{ at = {{ under_way = "review" }}, do = "restart" }},
        {{ at = {{ under_way = "review" }}, do = "resume_everything" }}"#
    );
    let played = played(&script(&item(&acts, MERGED)))
        .await
        .expect("the script is played");
    assert!(played.took(), "{played}");
}

#[tokio::test(start_paused = true)]
async fn a_cancel_while_a_round_is_under_way_ends_the_work() {
    let acts = format!(r#"{ANSWER}, {{ at = {{ under_way = "build" }}, do = "cancel", why = "Not needed." }}"#);
    let path = r#"{ runs = "plan" }, { waits = "plan", for = "answer" }, { runs = "plan" }, { runs = "build" },
        { ends = "cancelled" }"#;
    let played = played(&script(&item(&acts, path))).await.expect("the script is played");
    assert!(played.took(), "{played}");
}

#[tokio::test(start_paused = true)]
async fn an_act_whose_state_never_comes_is_named_as_not_given_at_the_bound() {
    let acts = format!(r#"{ANSWER}, {{ at = {{ under_way = "review", run = 3 }}, do = "pause" }}"#);
    let played = played(&script(&item(&acts, MERGED)))
        .await
        .expect("the script is played");
    assert!(!played.took());
    assert_eq!(played.items[0].judged, Judged::Took, "{played}");
    assert_eq!(played.items[0].unreached, vec![2]);
    assert_eq!(
        played.to_string(),
        "items[0], W1: took its path; its act 2 was not given: its state never came\n"
    );
}

#[tokio::test(start_paused = true)]
async fn a_path_left_is_named_at_its_first_step_off_it() {
    let wrong = MERGED.replacen(
        r#"{ runs = "review" }, { runs = "build" }"#,
        r#"{ runs = "review" }, { runs = "merge" }"#,
        1,
    );
    let played = played(&script(&item(ANSWER, &wrong)))
        .await
        .expect("the script is played");
    assert_eq!(
        played.items[0].judged,
        Judged::Left {
            step: 8,
            said: Some(r#"{ runs = "merge" }"#.to_string()),
            records: r#"{ runs = "build" }"#.to_string(),
        },
        "{played}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_play_starts_its_clock_at_the_time_the_script_names_as_its_start() {
    // The trial's script with a start a day after the tests' own: the work is added then, and not at the play's start.
    let start = START.0 + 86_400_000;
    let text = format!(
        "start = {start}\n{}",
        std::fs::read_to_string(trial_dir().join("scripts/trial.toml")).unwrap()
    );
    let script = Script::read(&format!("{text}\n{}", item(ANSWER, MERGED))).expect("the script reads");
    let (played, dir) = played_on(&script, trial_flow()).await;
    assert!(played.expect("the script is played").took());
    let kept = store(&dir).load(WorkNumber(1)).expect("the store has the work");
    assert_eq!(kept.moments.first().map(|m| m.at), Some(work::Time(start)));
}

#[tokio::test(start_paused = true)]
async fn a_played_item_with_no_path_is_refused() {
    let faults = played(&script(&item(ANSWER, "")))
        .await
        .expect_err("the play is refused");
    assert_eq!(
        faults,
        vec!["items[0].path: a played item says the path its work must take".to_string()]
    );
}

#[tokio::test(start_paused = true)]
async fn main_moves_once_for_every_item_that_moves_it_once_each_stands_where_it_does() {
    // Both writers' rounds are under way at once: main moves once, into each work, and each writes again for it.
    let at = r#"{ under_way = "write", run = 1 }"#;
    let path = r#"{ runs = "write" }, { runs = "write" }, { runs = "merge" }, { ends = "merged" }"#;
    let (played, dir) = played_on(&moving_items((at, path), (at, path)), moving()).await;
    let played = played.expect("the script is played");
    assert!(played.took(), "{played}");
    let store = store(&dir);
    assert_eq!((mains(&store, 1), mains(&store, 2)), (1, 1));
}

#[tokio::test(start_paused = true)]
async fn main_does_not_move_while_an_item_that_moves_it_stands_elsewhere() {
    // The second item's writer never runs a third time: main moves for neither, and each act is named not given.
    let first = (
        r#"{ under_way = "write", run = 1 }"#,
        r#"{ runs = "write" }, { runs = "merge" }, { ends = "merged" }"#,
    );
    let second = (r#"{ under_way = "write", run = 3 }"#, first.1);
    let (played, dir) = played_on(&moving_items(first, second), moving()).await;
    let played = played.expect("the script is played");
    let unreached: Vec<(&Judged, &[usize])> = (played.items.iter())
        .map(|item| (&item.judged, item.unreached.as_slice()))
        .collect();
    assert_eq!(
        unreached,
        vec![(&Judged::Took, &[1][..]), (&Judged::Took, &[1][..])],
        "{played}"
    );
    let store = store(&dir);
    assert_eq!((mains(&store, 1), mains(&store, 2)), (0, 0));
}

/// A writer of the text a request asks for, the engine's own check of it, which reads main, and the merge, which waits
/// for the check to say yes.
fn checking() -> Compiled {
    let flow = r#"
        name = "checking"
        order = ["write", "check", "merge"]
        [limits]
        again = 2
        [lines.main]
        [blocks.request]
        people = true
        [blocks.main]
        outside = "main"
        [blocks.text]
        [blocks.checked]
        values = ["yes", "no"]
        [blocks.merged]
        shown = true
    "#;
    let write = r#"
        title = "Write"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Write the text the request asks for."
        inputs = ["request"]
        outputs = ["text"]
    "#;
    let check = r#"
        title = "Check"
        kind = "engine"
        act = "check"
        inputs = ["text", "main"]
        outputs = ["checked"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        requires = { checked = "yes" }
        inputs = ["checked"]
        outputs = ["merged"]
    "#;
    let source = flow::Source {
        flow: flow.to_string(),
        cells: [("write", write), ("check", check), ("merge", merge)]
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
        ..Default::default()
    };
    let has = flow::Has {
        sources: ["main".to_string()].into(),
        acts: ["check".to_string(), "merge".to_string()].into(),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the definition compiles")
}

#[tokio::test(start_paused = true)]
async fn a_check_given_again_after_main_moved_takes_in_the_main_it_is_given_again_with() {
    // The check's first round fails in passing, and main moves while the engine waits to try it again: the round given
    // again takes in the main that moved, as the real check tells its round, and the check does not run again for it.
    let script = Script::read(
        r#"
        [[flows.checking.rounds]]
        cell = "check"
        fails = "The check was cut off."
        failure = "passing"

        [[items]]
        title = "Greet"
        flow = "checking"
        writes = [{ block = "request", content = "Greet, please." }]
        acts = [{ at = { waits = "check", for = "retry" }, do = "main", content = "The head of main, moved." }]
        path = [
            { runs = "write" },
            { runs = "check" },
            { fails = "check", failure = "passing" },
            { waits = "check", for = "retry" },
            { runs = "merge" },
            { ends = "merged" },
        ]
        "#,
    )
    .expect("the script reads");
    let (played, dir) = played_on(&script, checking()).await;
    let played = played.expect("the script is played");
    assert!(played.took(), "{played}");
    assert_eq!(
        mains(&store(&dir), 1),
        2,
        "the main made up for the first round, and the main that moved"
    );
}

#[tokio::test(start_paused = true)]
async fn the_planners_changes_are_made_as_their_states_come_and_one_refused_is_named() {
    // The second item, before it is let on, comes to depend on the first, so that its plan waits for the first to
    // merge; the first is renamed, and cannot be cancelled while the second depends on it.
    let first = format!(
        r#"
        [[items]]
        title = "Greet the user"
        flow = "trial"
        writes = [{{ block = "request", content = "Greet the user, please." }}]
        acts = [
            {ANSWER},
            {{ at = {{ waits = "build", for = "tool" }}, do = "title", title = "Greet the user by name", why = "Named." }},
            {{ at = {{ waits = "build", for = "tool" }}, do = "cancel", why = "Not needed." }},
        ]
        path = [{MERGED}]
        "#
    );
    let second = format!(
        r#"
        [[items]]
        title = "Say goodbye"
        flow = "trial"
        held = true
        writes = [{{ block = "request", content = "Say goodbye, please." }}]
        acts = [
            {{ at = {{ paused = "owner" }}, do = "depends_on", item = 1, why = "It greets first." }},
            {{ at = {{ paused = "owner" }}, do = "resume" }},
            {{ at = {{ waits = "plan", for = "precondition" }}, do = "tag", tag = "after", why = "It waits." }},
            {ANSWER},
        ]
        path = [{{ waits = "plan", for = "precondition", key = "ready" }}, {{ let_on = true }}, {MERGED}]
        "#
    );
    let (played, dir) = played_on(&script(&format!("{first}{second}")), trial_flow()).await;
    let played = played.expect("the script is played");
    assert_eq!(
        (played.items.iter())
            .map(|item| (&item.judged, item.unreached.len()))
            .collect::<Vec<_>>(),
        vec![(&Judged::Took, 0), (&Judged::Took, 0)],
        "{played}"
    );
    let refused: Vec<usize> = played.items[0].refused.iter().map(|(act, _)| *act).collect();
    assert_eq!((refused, played.items[1].refused.len()), (vec![3], 0), "{played}");
    assert!(!played.took());
    let items = store(&dir).items().expect("the store has the items");
    assert_eq!(items[0].title, "Greet the user by name");
    assert_eq!(items[1].tags, vec!["after".to_string()]);
}

#[test]
fn a_script_is_told_in_one_pass_everything_its_items_acts_and_paths_name_that_the_flows_lack() {
    let script = script(
        r#"
        [[items]]
        title = "Greet the user"
        flow = "trial"
        depends_on = [1]
        writes = [{ block = "verdict", content = "Yes." }, { block = "nothing", content = "Nothing." }]
        acts = [
            { at = { runs = "plan" }, do = "pause" },
            { at = { waits = "blid", for = "tool" }, do = "let_go", cell = "reviw" },
            { at = { waits = "plan", for = "answer", question = "naming" }, do = "write", block = "request" },
            { at = { under_way = "plan", waits = "plan", for = "tool" }, do = "say", words = "Hello." },
            { at = { ends = "merged" }, do = "depends_on", item = 1, why = "Itself." },
            { at = { ends = "merged" }, do = "main", content = "A head." },
            { at = { ends = "merged" }, do = "resume", why = "Now." },
            { at = { waits = "merge", for = "slot" }, do = "pause" },
            { at = { waits = "build", for = "result" }, do = "write", block = "request", content = "Hi.", value = "x" },
        ]
        path = [
            { under_way = "plan" },
            { waits = "merge", for = "precondition", key = "ready" },
            { waits = "plan", for = "limit", by = "plan" },
            { waits = "plan", for = "outside", block = "request" },
            { waits = "plan", for = "result" },
            { waits = "plan", for = "turn" },
            { let_on = false },
            { runs = "plan", key = "ready" },
            {},
            { ends = "merged" },
        ]

        [[items]]
        title = " "
        flow = "other"
        "#,
    );
    assert_eq!(
        script.check(&[trial_flow()]),
        vec![
            "items[0].depends_on: 1 is no place of an item before it, counted from 1",
            "items[0].writes[0]: people do not write the block verdict",
            "items[0].writes[0]: the block verdict is a choice, written with one of its values: yes, no",
            "items[0].writes[1]: the flow trial has no block nothing",
            "items[0].acts[0].at: { runs = \"plan\" } is no state of a work",
            "items[0].acts[1].at: the flow trial has no cell blid",
            "items[0].acts[1]: the flow trial has no cell reviw",
            "items[0].acts[2].at: no round of the script asks a question named naming",
            "items[0].acts[2]: an act that does write gives content",
            "items[0].acts[3].at: says under_way and waits: a step says one thing",
            "items[0].acts[3]: words are said to a line or about a question, one of them",
            "items[0].acts[4]: 1 is no place of another of the script's items, counted from 1",
            "items[0].acts[5]: the flow trial has no block of main",
            "items[0].acts[6]: an act that does resume takes no why",
            "items[0].acts[7].at: merge takes no slot, as an act of the engine's takes none",
            "items[0].acts[8].at: build asks for nothing in a record",
            "items[0].acts[8]: the block request is no choice, which x would be",
            "items[0].path[0]: { under_way = \"plan\" } is read from the engine's view of the work now, and so only a \
             path's end",
            "items[0].path[1]: ready is no key of the preconditions of merge",
            "items[0].path[2]: plan is no cell after plan, which sends a cell back",
            "items[0].path[3]: no cell writes request, which a run waits for the outside by",
            "items[0].path[4]: the result of what a cell asked for is no step of a path: the runs of the cells that \
             give it are",
            "items[0].path[4]: plan asks for nothing in a record",
            "items[0].path[5]: { waits = \"plan\", for = \"turn\" } is read from the engine's view of the work now, \
             and so only a path's end",
            "items[0].path[5]: plan is no act of the engine, which alone waits for the turn",
            "items[0].path[6]: says let_on = false, which says nothing: give true",
            "items[0].path[7]: says key of a step that says runs",
            "items[0].path[8]: says nothing: a step says one of runs, under_way, sent_back, fails, waits, paused, \
             let_on, let_go, reopened, stalled, ends, everything",
            "items[1].title: a title says what the item is to deliver: give it words",
            "items[1].flow: no flow is named other",
        ]
    );
}
