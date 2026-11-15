//! A script played on the trial flow: its items added, the acts of people and of the outside given as their states
//! come, the engine stopped and opened again where an act says, and each work judged against its path; and what a
//! script's items, acts and paths name that the flows lack, told in one pass.

use service::{Bounds, Judged, Played, Script, play};

use crate::{START, trial_dir, trial_flow};

/// The trial's script with these items.
fn script(items: &str) -> Script {
    let trial = std::fs::read_to_string(trial_dir().join("script.toml")).expect("the script is read");
    Script::read(&format!("{trial}\n{items}")).expect("the script reads")
}

/// Plays the script on the trial flow over a store of its own, within an hour of the clock and a thousand things
/// taken; or what is wrong with it.
async fn played(script: &Script) -> Result<Played, Vec<String>> {
    let dir = tempfile::tempdir().expect("a directory for the store");
    let bounds = Bounds {
        steps: 1000,
        ms: 3_600_000,
    };
    play(
        script,
        &[trial_flow()],
        &dir.path().join("store.sqlite"),
        "trial",
        START,
        bounds,
    )
    .await
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
const ANSWER: &str = r#"{ at = { waits = "plan", for = "person" }, do = "result", result = "Yes, by name." }"#;

/// The trial's way from the plan's question to the merge, as its script has it: the build asks the CI, the review
/// objects to the first build and is content with the second.
const MERGED: &str = r#"{ runs = "plan" }, { waits = "plan", for = "person" }, { runs = "plan" },
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
    let path = r#"{ runs = "plan" }, { waits = "plan", for = "person" }, { runs = "plan" }, { runs = "build" },
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
    let path = r#"{ runs = "plan" }, { waits = "plan", for = "person" }, { runs = "plan" }, { runs = "build" },
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
async fn a_played_item_with_no_path_is_refused() {
    let faults = played(&script(&item(ANSWER, "")))
        .await
        .expect_err("the play is refused");
    assert_eq!(
        faults,
        vec!["items[0].path: a played item says the path its work must take".to_string()]
    );
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
        ]
        path = [
            { under_way = "plan" },
            { waits = "merge", for = "precondition", key = "ready" },
            { waits = "plan", for = "limit", by = "plan" },
            { waits = "plan", for = "outside", block = "plan" },
            { waits = "plan", for = "result" },
            { waits = "plan", for = "turn" },
            { let_on = false },
            { runs = "plan", who = "owner" },
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
            "items[0].path[0]: { under_way = \"plan\" } is read from the engine's view of the work now, and so only a \
             path's end",
            "items[0].path[1]: ready is no key of the preconditions of merge",
            "items[0].path[2]: plan is no cell after plan, which sends a cell back",
            "items[0].path[3]: no source outside the work writes plan",
            "items[0].path[4]: the result of what a cell asked for is no step of a path: the runs of the cells that \
             give it are",
            "items[0].path[5]: { waits = \"plan\", for = \"turn\" } is read from the engine's view of the work now, \
             and so only a path's end",
            "items[0].path[5]: plan is no act of the engine, which alone waits for the turn",
            "items[0].path[6]: says let_on = false, which says nothing: give true",
            "items[0].path[7]: says who of a step that says runs",
            "items[0].path[8]: says nothing: a step says one of runs, under_way, sent_back, fails, waits, paused, \
             let_on, let_go, reopened, stalled, ends, everything",
            "items[1].title: a title says what the item is to deliver: give it words",
            "items[1].flow: no flow is named other",
        ]
    );
}
