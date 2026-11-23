//! What a round of the small flow is sent as its cell is entered, worked through one work in the words of the flow
//! crate's tests: what the session lacks is filled, what it holds is not, and the line of a datum it holds that is new
//! to the cell says so.

mod support;

use context::text::as_written;
use context::{Entered, Main, Stands, Values};
use decide::Outsider;
use decide::run::Unmet;
use decide::{BlockId, Seq};
use engine::{Ask, Came, Event, Fill, Why};
use support::{Sim, carry, changed};

/// What ran a round, as it is told last: each datum it watches that is new to it, with the version the cell last
/// consumed where it consumed one, and the latest (design document `data-and-context`, 11).
fn ran_by(new: &[(&str, Option<u64>, u64)]) -> String {
    let each: String = (new.iter())
        .map(|(datum, last, latest)| match last {
            Some(last) => format!("\n- `{datum}`, new since version {last}, now version {latest}"),
            None => format!("\n- `{datum}`, version {latest}"),
        })
        .collect();
    format!("## Why you are run\n{each}\n\nName in `dealt_with` each of these you have dealt with.")
}

/// The message of these parts, as one text.
fn message(parts: &[String]) -> String {
    format!("{}\n", parts.join("\n\n"))
}

/// The text a cell is told it returns, of these lines.
fn returns(lines: &[&str]) -> String {
    format!("## What this cell returns\n\n{}", lines.join("\n"))
}

/// The digest of a part filled with this text.
fn part(name: &str, text: &str) -> Fill {
    Fill::part(name, text)
}

fn whole(block: &str, version: u64) -> Fill {
    Fill::Whole {
        block: BlockId::from(block),
        version: Seq(version),
    }
}

const CODE: &str = "fn greet() {\n    say(\"hello\");\n}\n\nfn say(what: &str) {\n    print(what);\n}\n\nfn main() {\n    greet();\n}\n";

/// The work as far as the review's first round: the plan, the build and the review each entered once.
fn reviewed() -> Sim {
    let mut sim = Sim::new();
    sim.writes("request", "Greet the user.");
    carry(&mut sim, &[("plan", "Greet by name.", None)]);
    carry(&mut sim, &[("code", CODE, None)]);
    carry(
        &mut sim,
        &[("opinions", "Name the error.", None), ("verdict", "no", Some("no"))],
    );
    sim
}

#[test]
fn a_first_round_is_sent_its_heading_each_datum_written_whole_what_it_returns_its_task_and_what_ran_it() {
    let mut sim = Sim::new();
    sim.writes("request", "Greet the user.");
    let round = sim.round().expect("the plan is entered");
    let entered = sim.enter(&round, false);
    let request = sim.version("request");
    let outputs = returns(&["- `plan`: "]);
    let expected = message(&[
        "# Plan".to_string(),
        "## `request`, version 1, written by the owner\n\nGreet the user.".to_string(),
        outputs.clone(),
        "Write the plan for the request.".to_string(),
        ran_by(&[("request", None, 1)]),
    ]);
    assert_eq!(entered.text, expected);
    assert_eq!(
        entered.fills,
        [
            whole("request", request),
            part("outputs", &outputs),
            part("task", "Write the plan for the request.")
        ]
    );
    // A session that holds nothing is sent the same.
    assert_eq!(sim.enter(&round, true), entered);
}

#[test]
fn a_cell_entered_in_the_session_that_handed_in_what_it_is_given_is_sent_a_line_for_each_with_its_version() {
    let mut sim = Sim::new();
    sim.writes("request", "Greet the user.");
    carry(&mut sim, &[("plan", "Greet by name.", None)]);
    let round = sim.round().expect("the build is entered");
    let entered = sim.enter(&round, false);
    let (plan, request) = (sim.number("plan"), sim.number("request"));
    let outputs = returns(&["- `code`: "]);
    // The build never worked from either: each is new to it, and the session holds it already.
    let expected = message(&[
        format!("Kept: `plan` {plan}"),
        "# Build".to_string(),
        format!("## `request`, version {request}, written by the owner, in this session already"),
        format!("## `plan`, version {plan}, written on the line `engineer`, in this session already"),
        outputs.clone(),
        "Build what the plan says.".to_string(),
        ran_by(&[("plan", None, plan)]),
    ]);
    assert_eq!(entered.text, expected);
    assert_eq!(
        entered.fills,
        [part("outputs", &outputs), part("task", "Build what the plan says.")]
    );
    assert_eq!(entered.situation.data["request"], Stands::HoldsNew);
}

#[test]
fn a_cell_of_another_line_is_sent_whole_what_its_session_lacks() {
    let mut sim = Sim::new();
    sim.writes("request", "Greet the user.");
    carry(&mut sim, &[("plan", "Greet by name.", None)]);
    carry(&mut sim, &[("code", CODE, None)]);
    let round = sim.round().expect("the review is entered");
    let entered = sim.enter(&round, false);
    let (code, plan) = (sim.number("code"), sim.number("plan"));
    let outputs = returns(&[
        "- `opinions`: ",
        "- `verdict`, a choice, one of `yes`, `no` (hand it in every time): ",
    ]);
    let expected = message(&[
        "# Review".to_string(),
        format!("## `plan`, version {plan}, written on the line `engineer`\n\nGreet by name."),
        format!(
            "## `code`, version {code}, written on the line `engineer`\n\n{}",
            CODE.trim_end()
        ),
        outputs.clone(),
        "Review the code.".to_string(),
        ran_by(&[("code", None, code)]),
    ]);
    assert_eq!(entered.text, expected);
    let (code, plan) = (sim.version("code"), sim.version("plan"));
    assert_eq!(entered.fills[..2], [whole("plan", plan), whole("code", code)]);
}

#[test]
fn a_cell_entered_again_is_sent_what_is_new_and_its_task_by_its_held_line() {
    let sim = reviewed();
    let round = sim.round().expect("the plan is entered again");
    let entered = sim.enter(&round, false);
    let (code, opinions) = (sim.number("code"), sim.number("opinions"));
    let expected = message(&[
        format!("Kept: `code` {code}"),
        "# Plan".to_string(),
        format!("## `opinions`, version {opinions}, written on the line `reviewer`\n\nName the error."),
        "As given before in this session.".to_string(),
        ran_by(&[("opinions", None, opinions)]),
    ]);
    assert_eq!(entered.text, expected);
    assert_eq!(entered.fills, [whole("opinions", sim.version("opinions"))]);
}

#[test]
fn a_datum_the_session_holds_that_is_new_to_the_cell_is_sent_its_held_line() {
    let mut sim = reviewed();
    let first = sim.number("plan");
    carry(&mut sim, &[("plan", "Greet by name, and say why.", None)]);
    let round = sim.round().expect("the build is entered again");
    let entered = sim.enter(&round, false);
    let plan = sim.number("plan");
    let expected = message(&[
        format!("Kept: `plan` {plan}"),
        "# Build".to_string(),
        format!(
            "## `plan`, version {plan}, written on the line `engineer`, in this session already; last worked from {first}"
        ),
        "As given before in this session.".to_string(),
        ran_by(&[("plan", Some(first), plan)]),
    ]);
    assert_eq!(entered.text, expected);
    assert_eq!(entered.fills, []);
    assert_eq!(entered.situation.data["plan"], Stands::HoldsLatest);
}

// A version is told by its number among the versions of its datum, never by its place in the work's records: the
// plan's second version, with the code, the opinions and the verdict written between its first and it, is version 2,
// and the first, which the build last worked from, version 1 (decision a-version-is-numbered-within-its-datum).
#[test]
fn a_datum_s_second_version_is_told_as_version_2_whatever_was_written_between_its_versions() {
    let mut sim = reviewed();
    carry(&mut sim, &[("plan", "Greet by name, and say why.", None)]);
    let places = sim.versions("plan");
    assert!(
        places[1].0 > places[0].0 + 2,
        "other data were written between: {places:?}"
    );
    let round = sim.round().expect("the build is entered again");
    let entered = sim.enter(&round, false);
    let text = &entered.text;
    assert!(text.starts_with("Kept: `plan` 2\n"), "{text}");
    let held = "## `plan`, version 2, written on the line `engineer`, in this session already; last worked from 1";
    assert!(text.contains(held), "{text}");
    assert!(
        text.contains("\n- `plan`, new since version 1, now version 2"),
        "{text}"
    );
}

#[test]
fn a_datum_given_latest_is_sent_whole_where_the_session_lacks_it_and_its_held_line_where_it_holds_it_new() {
    let mut sim = Sim::new();
    sim.writes("request", "Greet the user.");
    carry(&mut sim, &[("plan", "Greet by name.", None)]);
    carry(&mut sim, &[("code", CODE, None)]);
    let first = sim.number("request");
    // The build worked from the first request; the plan, run by the new one, takes it into the line's session, while
    // the review goes on beside it.
    sim.writes("request", "Greet the user by name.");
    let round = loop {
        let round = sim.round().expect("a round is given");
        match sim.cell(&round).id.as_str() {
            "build" => break round,
            "plan" => carry(&mut sim, &[("plan", "Greet by name, and say why.", None)]),
            _ => carry(
                &mut sim,
                &[("opinions", "Fine.", None), ("verdict", "yes", Some("yes"))],
            ),
        };
    };
    let (lacking, holding) = (sim.enter(&round, true), sim.enter(&round, false));
    let request = sim.number("request");
    assert_eq!(lacking.situation.data["request"], Stands::Lacks);
    assert_eq!(holding.situation.data["request"], Stands::HoldsLatest);
    let sent = |entered: &Entered| {
        (entered.sent.iter())
            .find(|s| s.block.as_deref() == Some("request"))
            .map(|s| as_written(&s.pieces))
    };
    assert_eq!(
        sent(&lacking),
        Some(format!(
            "## `request`, version {request}, written by the owner\n\nGreet the user by name."
        ))
    );
    assert_eq!(
        sent(&holding),
        Some(format!(
            "## `request`, version {request}, written by the owner, in this session already; last worked from {first}"
        ))
    );
    assert_ne!(lacking.text, holding.text);
}

#[test]
fn what_changed_since_the_version_the_session_holds_is_sent_as_a_change_that_rebuilds_the_latest() {
    let mut sim = reviewed();
    let first = Seq(sim.version("code"));
    carry(&mut sim, &[("plan", "Greet by name, and say why.", None)]);
    sim.writes("notes", "first");
    sim.writes("notes", "second");
    let changed = CODE.replace("print(what)", "print(what, \"!\")");
    carry(&mut sim, &[("code", &changed, None)]);
    let round = sim.round().expect("the review is entered again");
    let entered = sim.enter(&round, false);
    let (code, plan) = (Seq(sim.version("code")), sim.version("plan"));
    let notes = sim.versions("notes");
    let diff = entered.run.data[&BlockId::from("code")].diff.clone().expect("a change");
    assert_eq!(context::diff::apply(CODE, &diff).as_deref(), Some(changed.as_str()));
    let fills = [
        whole("plan", plan),
        whole("notes", notes[0].0),
        whole("notes", notes[1].0),
        Fill::Change {
            block: BlockId::from("code"),
            version: code,
            since: first,
        },
    ];
    assert_eq!(entered.fills, fills);
    // Each told by its number among its block's versions: the code's first and second, the notes' first and second.
    let text = &entered.text;
    let change =
        format!("## `code`, what changed from version 1 to version 2, written on the line `engineer`\n\n{diff}");
    assert!(text.contains(&change), "{text}");
    let notes = "## `notes`, version 1, written by the owner\n\nfirst\n\n## `notes`, version 2, written by the owner\n\nsecond\n\n";
    assert!(text.contains(notes), "{text}");
}

#[test]
fn a_session_reopened_is_filled_from_the_top_and_holds_no_return() {
    let mut sim = reviewed();
    sim.anew("engineer", Why::Owner);
    let round = sim.round().expect("the plan is entered again");
    let entered = sim.enter(&round, false);
    let blocks: Vec<&str> = (entered.fills.iter())
        .filter_map(|f| match f {
            Fill::Whole { block, .. } => Some(block.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(blocks, ["plan", "request", "opinions"]);
    assert!(
        entered
            .fills
            .contains(&Fill::part("task", "Write the plan for the request."))
    );
    assert!(!entered.text.starts_with("Kept"), "{}", entered.text);
    assert_eq!(sim.enter(&round, true), entered);
}

#[test]
fn a_session_compacted_during_a_round_holds_only_what_that_round_handed_in() {
    let mut sim = Sim::new();
    sim.writes("request", "Greet the user.");
    let round = sim.round().expect("the plan is entered");
    let entered = sim.enter(&round, false);
    sim.tries(&round, 1, entered.fills);
    let came = Came::hands_in(vec![engine::Handed {
        block: BlockId::from("plan"),
        content: "Greet by name.".to_string(),
        matters: true,
        value: None,
    }]);
    sim.returns(&round, came, 1);
    let round = sim.round().expect("the build is entered");
    let entered = sim.enter(&round, false);
    let request = sim.version("request");
    // The request filled before the compaction is filled again; the plan handed in after it is held.
    assert_eq!(entered.fills[0], whole("request", request));
    assert!(!entered.fills.contains(&whole("plan", sim.version("plan"))));
    assert!(entered.text.starts_with("Kept: `plan`"), "{}", entered.text);
}

#[test]
fn a_run_entered_for_the_results_of_its_calls_is_sent_them_once() {
    let mut sim = reviewed();
    carry(&mut sim, &[("plan", "Greet by name, and say why.", None)]);
    carry(&mut sim, &[("code", CODE, None)]);
    let round = sim.round().expect("the review is entered again");
    let entered = sim.enter(&round, false);
    sim.tries(&round, 1, entered.fills);
    let ask = Ask::Tool {
        name: "query".to_string(),
        input: "{\"sql\": \"select 1\"}".to_string(),
    };
    sim.returns(&round, Came::asks(vec![ask]), 0);
    let call = sim.calls()[0];
    sim.done(call, "1");
    let round = sim.round().expect("the review is entered for the result");
    let entered = sim.enter(&round, false);
    let results = "## Results\n\n### `query` on {\"sql\": \"select 1\"}\n\n1\n\nGo on.";
    assert!(entered.text.contains(results), "{}", entered.text);
    assert_eq!(entered.fills.last(), Some(&Fill::Results(vec![call])));
    // The code, which the run that asked dealt with nothing of, is new to the cell still, and held by the session.
    assert_eq!(entered.situation.data["code"], Stands::HoldsLatest);
}

#[test]
fn a_round_after_its_return_was_sent_back_is_sent_why() {
    let mut sim = Sim::new();
    sim.writes("request", "Greet the user.");
    carry(&mut sim, &[("plan", "Greet by name.", None)]);
    carry(&mut sim, &[("code", CODE, None)]);
    carry(&mut sim, &[("opinions", "Name the error.", None)]);
    let round = sim.round().expect("the review's next round");
    assert_eq!(round.number, 2);
    let entered = sim.enter(&round, false);
    assert!(
        entered
            .text
            .contains("## Sent back\n\n- `verdict` is missing.\n\nReturn again."),
        "{}",
        entered.text
    );
    assert_eq!(entered.fills, [Fill::SentBack { round: 2 }]);
    assert_eq!(
        entered.run.sent_back,
        [(
            None,
            vec![Unmet::NotHandedIn {
                block: "verdict".into()
            }]
        )]
    );
    // In a session that holds nothing, the run's task and data come again, with why.
    let anew = sim.enter(&round, true);
    assert!(anew.fills.contains(&Fill::part("task", "Review the code.")));
    assert!(anew.text.contains("## Sent back"), "{}", anew.text);
}

// A return the engine found leaving files the project protects changed with nothing to say why is sent back naming
// each, in the words of the files beside the flows.
#[test]
fn a_return_that_leaves_a_protected_file_changed_and_says_not_why_is_sent_back_naming_it() {
    let mut sim = Sim::new();
    sim.writes("request", "Greet the user.");
    carry(&mut sim, &[("plan", "Greet by name.", None)]);
    let round = sim.round().expect("the build is entered");
    let entered = sim.enter(&round, sim.begins(&round));
    sim.tries(&round, 1, entered.fills);
    let mut came = Came::hands_in(Vec::new());
    came.found = ["AGENTS.md", "flow/cell.toml"]
        .map(|path| Unmet::Unexplained { path: path.to_string() })
        .to_vec();
    sim.returns(&round, came, 0);
    let again = sim.round().expect("the build's round after it was sent back");
    assert_eq!((again.run, again.number), (round.run, 2));
    let entered = sim.enter(&again, false);
    let said = "## Sent back\n\n- `AGENTS.md` is protected: say why, or take it back.\n- `flow/cell.toml` is \
                protected: say why, or take it back.\n";
    assert!(entered.text.contains(said), "{}", entered.text);
}

// A return the engine found changing, in its run, files that are none of the project's tests, on a line that changes
// only them, is sent back naming each, in the words of the files beside the flows.
#[test]
fn a_return_whose_run_changed_a_file_that_is_no_test_on_a_line_of_tests_is_sent_back_naming_it() {
    let mut sim = Sim::new();
    sim.writes("request", "Greet the user.");
    carry(&mut sim, &[("plan", "Greet by name.", None)]);
    let round = sim.round().expect("the build is entered");
    let entered = sim.enter(&round, sim.begins(&round));
    sim.tries(&round, 1, entered.fills);
    let mut came = Came::hands_in(Vec::new());
    came.found = ["src/greeting.rs", "Cargo.lock"]
        .map(|path| Unmet::NotATest { path: path.to_string() })
        .to_vec();
    sim.returns(&round, came, 0);
    let again = sim.round().expect("the build's round after it was sent back");
    assert_eq!((again.run, again.number), (round.run, 2));
    let entered = sim.enter(&again, false);
    let said = "## Sent back\n\n- `src/greeting.rs` is no test: take it back.\n- `Cargo.lock` is no test: take it \
                back.\n";
    assert!(entered.text.contains(said), "{}", entered.text);
}

// A return the engine found handing in the change while a question marked `tree` has the owner's answer and the
// change leaves the design tree as main has it is sent back naming the question, in the words of the files beside the
// flows.
#[test]
fn a_return_that_hands_in_the_change_without_an_answer_marked_tree_in_the_tree_is_sent_back_naming_the_question() {
    let mut sim = Sim::new();
    sim.writes("request", "Greet the user.");
    carry(&mut sim, &[("plan", "Greet by name.", None)]);
    let round = sim.round().expect("the build is entered");
    let entered = sim.enter(&round, sim.begins(&round));
    sim.tries(&round, 1, entered.fills);
    let mut came = Came::hands_in(Vec::new());
    came.found = vec![Unmet::TreeUnchanged {
        name: "placement".to_string(),
    }];
    sim.returns(&round, came, 0);
    let again = sim.round().expect("the build's round after it was sent back");
    assert_eq!((again.run, again.number), (round.run, 2));
    let entered = sim.enter(&again, false);
    let said = "## Sent back\n\n- `placement` is not in the tree: write it.\n";
    assert!(entered.text.contains(said), "{}", entered.text);
}

#[test]
fn main_taken_in_is_told_to_a_cell_that_takes_it_in_in_one_line_in_the_words_of_its_part() {
    let mut sim = Sim::new();
    for cell in &mut sim.flow.cells {
        cell.takes_main = cell.id.as_str() == "plan";
    }
    sim.writes("request", "Greet the user.");
    let round = sim.round().expect("the plan is entered");
    let name = "main".to_string();
    let took_in = Main::TookIn {
        name: name.clone(),
        main: "beef".to_string(),
        merge: "c0ffee".to_string(),
    };
    let entered = sim.enter_with(&round, Some(took_in), false);
    let merged = "main at beef was merged into the work's branch as the commit c0ffee; `git diff --name-only \
                  c0ffee^1 c0ffee` shows what it changed.";
    assert!(
        entered.text.starts_with(&format!("# Plan\n\n{merged}\n\n## `request`")),
        "{}",
        entered.text
    );
    // A list is one line, its items apart as `between` says.
    let conflict = Main::Conflict {
        name: name.clone(),
        main: "beef".to_string(),
        files: vec!["a.rs".to_string(), "b.rs".to_string()],
    };
    let entered = sim.enter_with(&round, Some(conflict), false);
    let said = "main at beef was not merged into the work's branch: it conflicts in a.rs, b.rs.";
    assert!(entered.text.contains(&format!("\n{said}\n")), "{}", entered.text);
    let entered = sim.enter_with(&round, Some(Main::Failed { name }), false);
    assert!(
        entered.text.contains("\nmain was not merged into the work's branch.\n"),
        "{}",
        entered.text
    );
    assert!(
        !sim.enter(&round, false).text.contains("merged"),
        "nothing where the branch held main"
    );
}

#[test]
fn what_the_page_shows_for_the_situation_is_what_the_engine_sends_in_it_with_its_places_open() {
    let mut sim = reviewed();
    sim.writes("notes", "first");
    carry(&mut sim, &[("plan", "Greet by name, and say why.", None)]);
    let round = sim.round().expect("the build is entered again");
    let entered = sim.enter(&round, false);
    let cell = sim.cell(&round);
    let open = context::message(
        &sim.flow,
        &sim.words,
        cell,
        &entered.situation,
        Values::Slots(&sim.named),
    );
    let shape = |sent: &[context::Sent]| -> Vec<(String, Option<String>, context::Form)> {
        sent.iter().map(|s| (s.part.clone(), s.block.clone(), s.form)).collect()
    };
    assert_eq!(shape(&open), shape(&entered.sent));
    let taken = context::text::as_written(&open[0].pieces);
    assert_eq!(taken, "Kept: {taken}");
}

#[test]
fn the_tree_changed_since_the_session_read_it_is_sent_as_the_files_the_carrier_says_changed() {
    let mut sim = Sim::new();
    sim.writes("tree", "c1 t1");
    sim.writes("request", "Greet the user.");
    let round = sim.round().expect("the plan is entered");
    let plan = sim.enter(&round, false);
    let first = sim.version("tree");
    assert!(plan.fills.contains(&whole("tree", first)), "{:?}", plan.fills);
    // A session that has read none of the tree is told so (design document `data-and-context`, 23).
    let unread = "## `tree`, version 1, written by the owner: this session has read none of it\n\nc1 t1";
    assert!(plan.text.contains(unread), "{}", plan.text);
    assert_eq!(plan.situation.data["tree"], Stands::Unread);
    sim.tries(&round, 1, plan.fills);
    // The tree changes while the plan's round is under way, as where it takes main in, so that the build is entered on
    // its new version.
    sim.writes("tree", "c2 t2");
    sim.hands_in(&round, &[("plan", "Greet by name.", None)]);
    let round = sim.round().expect("the build is entered");
    let entered = sim.enter(&round, false);
    let latest = sim.version("tree");
    // The build worked from no version of the tree, and its session read the first: what changed since is the files the
    // carrier says, whatever their length.
    let files = changed(Seq(first), Seq(latest));
    let change = format!("## `tree`, what changed from version 1 to version 2, written by the owner\n\n{files}");
    assert!(entered.text.contains(&change), "{}", entered.text);
    let fill = Fill::Change {
        block: BlockId::from("tree"),
        version: Seq(latest),
        since: Seq(first),
    };
    assert!(entered.fills.contains(&fill), "{:?}", entered.fills);
    // Where the session lacks every version, the latest is sent whole, saying the session has read none of it.
    let anew = sim.enter(&round, true);
    let unread = "## `tree`, version 2, written by the owner: this session has read none of it\n\nc2 t2";
    assert!(anew.text.contains(unread), "{}", anew.text);
}

// A cell brings what it writes up to date, and so needs the latest version of each of its outputs, whoever wrote it:
// where its session lacks one, as a session begun anew does, it is filled in, as `latest` gives it, after the data the
// cell reads and watches (design document `data-and-context`, 14, 21).
#[test]
fn a_round_whose_session_lacks_the_latest_of_what_its_cell_writes_is_given_it_after_its_data() {
    let mut sim = reviewed();
    carry(&mut sim, &[("plan", "Greet by name, and say why.", None)]);
    carry(&mut sim, &[("code", CODE, None)]);
    sim.anew("reviewer", Why::Owner);
    let round = sim.round().expect("the review is entered again");
    assert_eq!(sim.cell(&round).id.as_str(), "review");
    let entered = sim.enter(&round, false);
    let data: Vec<Fill> = (entered.fills.iter())
        .filter(|f| matches!(f, Fill::Whole { .. }))
        .cloned()
        .collect();
    let (plan, code) = (sim.version("plan"), sim.version("code"));
    let (opinions, verdict) = (sim.version("opinions"), sim.version("verdict"));
    let expected = vec![
        whole("plan", plan),
        whole("code", code),
        whole("opinions", opinions),
        whole("verdict", verdict),
    ];
    assert_eq!(data, expected);
    let heading = format!(
        "## `opinions`, version {}, written on the line `reviewer`\n\nName the error.",
        sim.number("opinions")
    );
    assert!(entered.text.contains(&heading), "{}", entered.text);
    // Entered again in the session that holds them, it is sent none of them.
    let mut sim = reviewed();
    carry(&mut sim, &[("plan", "Greet by name, and say why.", None)]);
    carry(&mut sim, &[("code", CODE, None)]);
    let round = sim.round().expect("the review is entered again");
    let entered = sim.enter(&round, false);
    let outputs =
        |f: &Fill| matches!(f, Fill::Whole { block, .. } if ["opinions", "verdict"].contains(&block.as_str()));
    assert!(!entered.fills.iter().any(outputs), "{:?}", entered.fills);
    assert!(!entered.text.contains("## `opinions`"), "{}", entered.text);
}

#[test]
fn a_datum_given_last_and_latest_whose_session_holds_its_latest_and_not_the_version_last_worked_from_is_not_sent_again()
{
    let mut sim = reviewed();
    let first = sim.version("code");
    carry(&mut sim, &[("plan", "Greet by name, and say why.", None)]);
    let changed = CODE.replace("print(what)", "print(what, \"!\")");
    carry(&mut sim, &[("code", &changed, None)]);
    // The reviewer's line begins a new session, which the review's next round fills with the latest code; its return
    // is sent back, and the round after it runs in that session, which holds the latest and not the version the
    // review last worked from.
    sim.anew("reviewer", Why::Owner);
    carry(&mut sim, &[("opinions", "Name it.", None)]);
    let round = sim.round().expect("the review's round after it was sent back");
    let entered = sim.enter(&round, false);
    let code = sim.version("code");
    assert!(first < code);
    assert_eq!(entered.situation.data["code"], Stands::HoldsLatest);
    let sent = (entered.sent.iter())
        .find(|s| s.block.as_deref() == Some("code"))
        .expect("the code's line is sent");
    assert_eq!(sent.form, context::Form::Held);
    assert!(!entered.fills.contains(&whole("code", code)), "{:?}", entered.fills);
}

#[test]
fn a_return_that_would_write_over_a_version_it_never_saw_is_sent_back_with_that_version() {
    let mut sim = Sim::new();
    sim.writes("request", "Greet the user.");
    carry(&mut sim, &[("plan", "Greet by name.", None)]);
    carry(&mut sim, &[("code", CODE, None)]);
    let round = sim.round().expect("the review is entered");
    let entered = sim.enter(&round, false);
    sim.tries(&round, 1, entered.fills);
    // A person gives the verdict while the review's round is under way, and its return writes over it.
    sim.take(Event::Wrote {
        block: "verdict".into(),
        by: Outsider::Person,
        content: "yes".to_string(),
        matters: true,
        value: Some("yes".to_string()),
    });
    let over = sim.version("verdict");
    sim.hands_in(&round, &[("verdict", "no", Some("no"))]);
    let round = sim.round().expect("the review's round after it was sent back");
    assert_eq!(round.number, 2);
    let put_in = (sim.work.records.put_in.iter()).any(|p| p.run == round.run && p.version.0 == over);
    assert!(put_in, "the version is put into the round");
    let entered = sim.enter(&round, false);
    // The review writes the verdict and reads it not: the version is given as the latest of what it writes, its session
    // lacking it (the version put into the round not being held by its session for that), and why the return was sent
    // back says it, and gives it no second time.
    // The verdict's first version, told so in both places.
    let given = "## `verdict`, version 1, with the value `yes`, written by the owner\n\nyes";
    assert!(entered.text.contains(given), "{}", entered.text);
    let sent_back = "## Sent back\n\n- `verdict` has version 1 since.\n\nReturn again.";
    assert!(entered.text.contains(sent_back), "{}", entered.text);
    assert_eq!(entered.fills, [whole("verdict", over), Fill::SentBack { round: 2 }]);
}

#[test]
fn a_round_whose_program_said_it_took_its_message_in_holds_it_whatever_it_failed_of_and_one_that_said_nothing_does_not()
{
    use decide::run::Failure;
    // A crash before the program read its input fails in passing as one after it does: only what the stream said tells.
    for (failure, received) in [
        (Failure::Passing, true),
        (Failure::Passing, false),
        (Failure::UsageLimit, true),
        (Failure::UsageLimit, false),
    ] {
        let mut sim = Sim::new();
        sim.writes("request", "Greet the user.");
        let round = sim.round().expect("the plan is entered");
        let first = sim.enter(&round, sim.begins(&round));
        sim.tries(&round, 1, first.fills.clone());
        sim.fails_as(&round, failure, received);
        if failure == Failure::Passing {
            let wait = *sim.calls().last().expect("the wait before it is tried again");
            sim.done(wait, "over");
        }
        let again = sim.round().expect("the plan's round is tried again");
        assert_eq!((again.run, again.number), (round.run, 2));
        assert_eq!(sim.begins(&again), !received, "{failure:?}, {received}");
        let entered = sim.enter(&again, sim.begins(&again));
        let request = whole("request", sim.version("request"));
        assert_eq!(
            entered.fills.contains(&request),
            !received,
            "{failure:?}: {:?}",
            entered.fills
        );
    }
}

#[test]
fn a_version_that_says_what_an_earlier_one_said_is_new_and_sent_and_no_repeat() {
    let mut sim = reviewed();
    let held = sim.version("plan");
    // The plan changes and changes back; the review, whose session holds the first, is sent the third whole, though it
    // says what the first said.
    carry(&mut sim, &[("plan", "Greet by name, kindly.", None)]);
    carry(&mut sim, &[("code", "fn greet() {}\n", None)]);
    carry(
        &mut sim,
        &[("opinions", "Kinder.", None), ("verdict", "no", Some("no"))],
    );
    carry(&mut sim, &[("plan", "Greet by name.", None)]);
    // The owner says the same twice while the build runs: each is given.
    sim.writes("notes", "Keep it short.");
    sim.writes("notes", "Keep it short.");
    carry(&mut sim, &[("code", "fn greet(name: &str) {}\n", None)]);
    let round = sim.round().expect("the review is entered again");
    assert_eq!(round.cell.as_str(), "review");
    let latest = sim.version("plan");
    assert_eq!(sim.content("plan", Seq(latest)), sim.content("plan", Seq(held)));
    let entered = sim.enter(&round, sim.begins(&round));
    assert!(entered.fills.contains(&whole("plan", latest)), "{:?}", entered.fills);
    let notes = sim.versions("notes");
    assert_eq!(notes.len(), 2);
    for version in notes {
        assert!(
            entered.fills.contains(&whole("notes", version.0)),
            "{:?}",
            entered.fills
        );
    }
}

#[test]
fn a_setting_the_task_what_a_cell_does_for_a_datum_or_a_block_s_words_name_is_sent_with_its_value_beside_it() {
    let mut sim = Sim::new();
    let language = |key: &str| (key == "owner.language").then(|| "Chinese".to_string());
    sim.named = flow::system::Settings::of(None, &sim.words.installation, language);
    let code = sim.flow.blocks.get_mut(&BlockId::from("code")).expect("the code");
    code.what = Some("The code, its comments in `owner.language`.".to_string());
    let build = (sim.flow.cells.iter_mut())
        .find(|c| c.id.as_str() == "build")
        .expect("the build");
    build.task = Some("Build what the plan says, in `owner.language`.".to_string());
    build.watches[0].does = Some("Say why in `owner.language`.".to_string());
    sim.writes("request", "Greet the user.");
    carry(&mut sim, &[("plan", "Greet by name.", None)]);
    let round = sim.round().expect("the build is entered");
    let entered = sim.enter(&round, false);
    let plan = format!(
        "- `plan`, version {}: Say why in `owner.language` (Chinese).",
        sim.number("plan")
    );
    for line in [
        "- `code`: The code, its comments in `owner.language` (Chinese).",
        "Build what the plan says, in `owner.language` (Chinese).",
        plan.as_str(),
    ] {
        assert!(entered.text.contains(line), "{line} is not in:\n{}", entered.text);
    }
    // The page shows them as the files write them.
    let cell = sim.cell(&round);
    let defined = context::defined(&sim.flow, &sim.words, cell);
    let task = defined
        .iter()
        .find(|d| d.part == flow::agents::Part::Task)
        .expect("the task");
    assert_eq!(
        as_written(&task.pieces),
        "Build what the plan says, in `owner.language`."
    );
}

// What is read of the branch as a round takes main in is written from outside and put into that round: no round's
// (decision a-version-read-at-a-rounds-end-is-that-rounds), so that a round given it is told it by no line, as any
// other version the outside wrote.
#[test]
fn a_version_put_into_a_round_from_outside_is_told_by_no_line() {
    let mut sim = Sim::of(support::branched(), support::words());
    sim.writes("request", "Greet the user.");
    let round = sim.round().expect("the plan is entered");
    let entered = sim.enter(&round, false);
    sim.tries(&round, 1, entered.fills);
    sim.take(Event::WroteForRound {
        run: round.run,
        block: "branch".into(),
        content: "Keep it short.".to_string(),
    });
    let put = sim.version("branch");
    assert!(
        (sim.work.records.put_in.iter()).any(|p| p.run == round.run && p.version.0 == put),
        "the version is put into the round"
    );
    // The request, written again while the round is under way, runs the plan again once it ends; its next round, in a
    // session that holds nothing, is given the version whole, by no line.
    sim.writes("request", "Greet the user by name.");
    sim.hands_in(&round, &[("plan", "Greet by name.", None)]);
    let first = round.run;
    let round = sim.round().expect("the plan is entered again, for the request");
    assert_eq!(sim.cell(&round).id.as_str(), "plan");
    assert_ne!(round.run, first);
    let entered = sim.enter(&round, true);
    let given = "## `branch`, version 1\n\nKeep it short.";
    assert!(entered.text.contains(given), "{}", entered.text);
}

/// Whether the message fills the datum whole, at any version.
fn fills_whole(entered: &Entered, block: &str) -> bool {
    (entered.fills.iter()).any(|f| matches!(f, Fill::Whole { block: b, .. } if b.as_str() == block))
}

// A session that goes on through a move holds what it took in before it only under a name the move left as it was
// (decision a-datum-a-move-renamed-is-given-again-whole): a move here reads what was written as the request as the code,
// as a renumbering reads what T2 wrote as `t2_words` as `t4_words`, and leaves the plan as it was. The build reads the
// code, and its session holds the request, filled into the plan's round, and the plan, which the session handed in.
#[test]
fn a_datum_a_move_renamed_is_given_whole_once_and_held_after_and_one_it_left_as_it_was_is_still_held() {
    let mut sim = Sim::new();
    sim.writes("request", "Greet the user.");
    carry(&mut sim, &[("plan", "Greet by name.", None)]);
    let round = sim.round().expect("the build is entered");
    // Before the move, the engineer's session holds the plan it handed in and the request it was given.
    let before = sim.enter(&round, false);
    assert!(
        !fills_whole(&before, "plan") && !fills_whole(&before, "request"),
        "{}",
        before.text
    );
    sim.moves(&[("request", "code")]);
    assert!(sim.versions("request").is_empty(), "the request reads as the code");
    let code = sim.version("code");
    let entered = sim.enter(&round, false);
    assert!(entered.fills.contains(&whole("code", code)), "{}", entered.text);
    assert!(!fills_whole(&entered, "plan"), "{}", entered.text);
    // Given whole once, after the move, it is held: the round, tried again in the session after its program took its
    // message in, is not given it again.
    sim.tries(&round, 1, entered.fills.clone());
    sim.fails_as(&round, decide::run::Failure::UsageLimit, true);
    let again = sim.round().expect("the build's round is tried again");
    assert_eq!((again.run, again.number), (round.run, 2));
    assert!(!sim.begins(&again), "the session goes on");
    let entered = sim.enter(&again, false);
    assert!(
        !fills_whole(&entered, "code") && !fills_whole(&entered, "plan"),
        "{}",
        entered.text
    );
}

// A datum a move gives the name of another the session held, as a new T2's `t2_words` after T2 was renumbered T4, is
// given whole as it is written after the move, never as a change of what the session held under that name.
#[test]
fn a_datum_written_after_a_move_under_a_name_the_move_took_from_another_is_given_whole() {
    let mut sim = Sim::new();
    sim.writes("request", "Greet the user.");
    // The plan's round fills the request into the engineer's session; the review hands in its verdict alone, so that
    // nothing runs after it.
    carry(&mut sim, &[("plan", "Greet by name.", None)]);
    carry(&mut sim, &[("code", CODE, None)]);
    carry(&mut sim, &[("verdict", "yes", Some("yes"))]);
    // The move reads what was written as the request as the notes, and the name `request` is another datum's, which is
    // written after it and runs the plan again, in the session that holds the request as it was filled.
    sim.moves(&[("request", "notes"), ("notes", "request")]);
    sim.writes("request", "Greet the user by name.");
    let round = sim.round().expect("the plan is entered again");
    assert_eq!(sim.cell(&round).id.as_str(), "plan");
    let entered = sim.enter(&round, false);
    let latest = sim.version("request");
    assert_eq!(sim.versions("request"), [Seq(latest)]);
    assert!(entered.fills.contains(&whole("request", latest)), "{}", entered.text);
    let changed = |f: &Fill| matches!(f, Fill::Change { block, .. } if block.as_str() == "request");
    assert!(!entered.fills.iter().any(changed), "{}", entered.text);
}

/// A maker and its judge on one line, the judge's verdict read by the test the flow's last cell requires, so that the
/// judge's points are its dialogue with the maker, and the owner may rule in the judge's place.
fn judged() -> flow::Compiled {
    let flow = "name = \"judged\"\norder = [\"make\", \"judge\", \"close\"]\n\n[limits]\nruns = 1000\nreturns = 1000\n\
                calls = 1000\n\n[lines.one]\n\n[blocks.request]\npeople = true\n\n[blocks.work]\n\n[blocks.talk]\n\n\
                [blocks.ok]\nvalues = [\"yes\", \"no\"]\npeople = true\n\n[tests.settled]\n\
                says = \"The judge is content\"\nkind = \"none_standing\"\nreads = [\"ok\"]\nis = \"no\"\n";
    let cell = |task: &str, rest: &str| {
        format!("title = \"{task}\"\nkind = \"agent\"\neffort = \"high\"\nline = \"one\"\ntask = \"{task}\"\n{rest}")
    };
    let cells = [
        (
            "make",
            cell(
                "Make it.",
                "watches = [\"request\", \"talk\"]\noutputs = [\"work\", \"talk\"]\n\n[kinds]\nrequest = \"latest\"\ntalk = \"all\"\n",
            ),
        ),
        (
            "judge",
            cell(
                "Judge it.",
                "watches = [\"work\", \"talk\"]\noutputs = [\"talk\", \"ok\"]\n\n[kinds]\nwork = \"latest\"\ntalk = \"all\"\n",
            ),
        ),
        (
            "close",
            cell(
                "Close it.",
                "requires = { test = \"settled\" }\nwatches = [\"work\"]\noutputs = []\n\n[kinds]\nwork = \"latest\"\n",
            ),
        ),
    ];
    let source = flow::Source {
        flow: flow.to_string(),
        cells: cells.into_iter().map(|(id, text)| (id.to_string(), text)).collect(),
        ..Default::default()
    };
    flow::compile(&source, &flow::Has::default()).expect("the judged flow compiles")
}

// The owner's ruling in a judge's place (design document `data-and-context`, 39): the points the judge wrote before it
// are new to no cell, in what a round is given as in what runs it. The maker, run again by a new request after the
// owner ruled on the judge's second point, holds that point in its session and last worked from the first: the point
// is no change to it.
#[test]
fn a_judge_s_point_the_owner_ruled_on_is_held_as_no_change_to_its_maker() {
    let mut sim = Sim::of(judged(), support::words());
    let next = |sim: &mut Sim, cell: &str, blocks: &[(&str, &str, Option<&str>)]| {
        let round = sim.round().expect("a round is given");
        assert_eq!(sim.cell(&round).id.as_str(), cell);
        let entered = sim.enter(&round, sim.begins(&round));
        sim.tries(&round, 1, entered.fills);
        sim.hands_in(&round, blocks);
    };
    sim.writes("request", "Greet the user.");
    next(&mut sim, "make", &[("work", "Hello.", None)]);
    next(
        &mut sim,
        "judge",
        &[("ok", "", Some("no")), ("talk", "Name the user.", None)],
    );
    next(&mut sim, "make", &[("work", "Hello, Ann.", None)]);
    // The owner pauses the work while the judge judges again, so that its point runs nothing before the owner rules.
    let round = sim.round().expect("the judge is entered again");
    assert_eq!(sim.cell(&round).id.as_str(), "judge");
    let entered = sim.enter(&round, sim.begins(&round));
    sim.tries(&round, 1, entered.fills);
    sim.take(Event::Paused {
        by: decide::Pauser::Owner,
        why: None,
    });
    sim.hands_in(&round, &[("ok", "", Some("no")), ("talk", "Greet warmly.", None)]);
    sim.wrote("ok", "", Some("yes"), Outsider::Person);
    sim.writes("request", "Greet the user warmly.");
    sim.take(Event::Resumed);
    let round = loop {
        let round = sim.round().expect("a round is given");
        if sim.cell(&round).id.as_str() == "make" {
            break round;
        }
        carry(&mut sim, &[]);
    };
    let entered = sim.enter(&round, sim.begins(&round));
    assert_eq!(entered.situation.data["talk"], Stands::Holds, "{}", entered.text);
}
