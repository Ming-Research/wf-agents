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

/// What ran a round, as it is told last: each of these triggers.
fn ran_by(triggers: &[&str]) -> String {
    let each: String = triggers.iter().map(|t| format!("\n- `{t}`")).collect();
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
        format!("## `request`, version {request}\n\nGreet the user."),
        outputs.clone(),
        "Write the plan for the request.".to_string(),
        ran_by(&["request"]),
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
    let (plan, request) = (sim.version("plan"), sim.version("request"));
    let outputs = returns(&["- `code`: "]);
    // The build never worked from either: each is new to it, and the session holds it already.
    let expected = message(&[
        format!("Kept: `plan` {plan}"),
        "# Build".to_string(),
        format!("## `plan`, version {plan}, in this session already"),
        format!("## `request`, version {request}, in this session already"),
        outputs.clone(),
        "Build what the plan says.".to_string(),
        ran_by(&["plan"]),
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
    let (code, plan) = (sim.version("code"), sim.version("plan"));
    let outputs = returns(&[
        "- `opinions`: ",
        "- `verdict`, a choice, one of `yes`, `no` (hand it in every time): ",
    ]);
    let expected = message(&[
        "# Review".to_string(),
        format!("## `code`, version {code}\n\n{}", CODE.trim_end()),
        format!("## `plan`, version {plan}\n\nGreet by name."),
        outputs.clone(),
        "Review the code.".to_string(),
        ran_by(&["code"]),
    ]);
    assert_eq!(entered.text, expected);
    assert_eq!(entered.fills[..2], [whole("code", code), whole("plan", plan)]);
}

#[test]
fn a_cell_entered_again_is_sent_what_is_new_and_its_task_by_its_held_line() {
    let sim = reviewed();
    let round = sim.round().expect("the plan is entered again");
    let entered = sim.enter(&round, false);
    let (code, opinions) = (sim.version("code"), sim.version("opinions"));
    let expected = message(&[
        format!("Kept: `code` {code}"),
        "# Plan".to_string(),
        format!("## `opinions`, version {opinions}\n\nName the error."),
        "As given before in this session.".to_string(),
        ran_by(&["opinions"]),
    ]);
    assert_eq!(entered.text, expected);
    assert_eq!(entered.fills, [whole("opinions", opinions)]);
}

#[test]
fn a_datum_the_session_holds_that_is_new_to_the_cell_is_sent_its_held_line() {
    let mut sim = reviewed();
    let first = sim.version("plan");
    carry(&mut sim, &[("plan", "Greet by name, and say why.", None)]);
    let round = sim.round().expect("the build is entered again");
    let entered = sim.enter(&round, false);
    let plan = sim.version("plan");
    let expected = message(&[
        format!("Kept: `plan` {plan}"),
        "# Build".to_string(),
        format!("## `plan`, version {plan}, in this session already; last worked from {first}"),
        "As given before in this session.".to_string(),
        ran_by(&["plan"]),
    ]);
    assert_eq!(entered.text, expected);
    assert_eq!(entered.fills, []);
    assert_eq!(entered.situation.data["plan"], Stands::HoldsLatest);
}

#[test]
fn a_datum_given_latest_is_sent_whole_where_the_session_lacks_it_and_its_held_line_where_it_holds_it_new() {
    let mut sim = Sim::new();
    sim.writes("request", "Greet the user.");
    carry(&mut sim, &[("plan", "Greet by name.", None)]);
    carry(&mut sim, &[("code", CODE, None)]);
    let first = sim.version("request");
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
    let request = sim.version("request");
    assert_eq!(lacking.situation.data["request"], Stands::Lacks);
    assert_eq!(holding.situation.data["request"], Stands::HoldsLatest);
    let sent = |entered: &Entered| {
        (entered.sent.iter())
            .find(|s| s.block.as_deref() == Some("request"))
            .map(|s| as_written(&s.pieces))
    };
    assert_eq!(
        sent(&lacking),
        Some(format!("## `request`, version {request}\n\nGreet the user by name."))
    );
    assert_eq!(
        sent(&holding),
        Some(format!(
            "## `request`, version {request}, in this session already; last worked from {first}"
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
        Fill::Change {
            block: BlockId::from("code"),
            version: code,
            since: first,
        },
        whole("plan", plan),
        whole("notes", notes[0].0),
        whole("notes", notes[1].0),
    ];
    assert_eq!(entered.fills, fills);
    let text = &entered.text;
    let change = format!(
        "## `code`, what changed from version {} to version {}\n\n{diff}",
        first.0, code.0
    );
    assert!(text.contains(&change), "{text}");
    let notes = format!(
        "## `notes`, version {}\n\nfirst\n\n## `notes`, version {}\n\nsecond\n\n",
        notes[0].0, notes[1].0
    );
    assert!(text.contains(&notes), "{text}");
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
    assert_eq!(blocks, ["request", "opinions", "plan"]);
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
    let change = format!("## `tree`, what changed from version {first} to version {latest}\n\n{files}");
    assert!(entered.text.contains(&change), "{}", entered.text);
    let fill = Fill::Change {
        block: BlockId::from("tree"),
        version: Seq(latest),
        since: Seq(first),
    };
    assert!(entered.fills.contains(&fill), "{:?}", entered.fills);
    // Where the session lacks every version, the latest is sent as any datum's.
    let anew = sim.enter(&round, true);
    assert!(
        anew.text.contains(&format!("## `tree`, version {latest}\n\nc2 t2")),
        "{}",
        anew.text
    );
}

#[test]
fn a_datum_given_last_and_latest_whose_session_holds_its_latest_and_not_the_version_last_worked_from_is_sent_whole() {
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
    assert_eq!(entered.situation.data["code"], Stands::Lacks);
    let sent = (entered.sent.iter())
        .find(|s| s.block.as_deref() == Some("code"))
        .expect("the code is sent");
    assert_eq!(sent.form, context::Form::Whole);
    let text = format!("## `code`, version {code}\n\n{changed}");
    assert_eq!(context::text::as_written(&sent.pieces), text);
    assert!(entered.fills.contains(&whole("code", code)));
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
    // The review is not given the verdict: the version is given with why the return was sent back, and filled, the
    // version put into the round not being held by its session for that.
    let sent_back = format!(
        "## Sent back\n\n- `verdict` has version {over} since.\n\n## `verdict`, version {over}, with the value `yes`\n\n\
         yes\n\nReturn again."
    );
    assert!(entered.text.contains(&sent_back), "{}", entered.text);
    assert_eq!(entered.fills, [Fill::SentBack { round: 2 }, whole("verdict", over)]);
}

#[test]
fn a_setting_the_task_a_trigger_s_words_or_a_block_s_words_name_is_sent_with_its_value_beside_it() {
    let mut sim = Sim::new();
    let language = |key: &str| (key == "owner.language").then(|| "Chinese".to_string());
    sim.named = flow::system::Settings::of(&sim.words.project, language);
    let code = sim.flow.blocks.get_mut(&BlockId::from("code")).expect("the code");
    code.what = Some("The code, its comments in `owner.language`.".to_string());
    let build = (sim.flow.cells.iter_mut())
        .find(|c| c.id.as_str() == "build")
        .expect("the build");
    build.task = Some("Build what the plan says, in `owner.language`.".to_string());
    build.triggers[0].does = Some("Say why in `owner.language`.".to_string());
    sim.writes("request", "Greet the user.");
    carry(&mut sim, &[("plan", "Greet by name.", None)]);
    let round = sim.round().expect("the build is entered");
    let entered = sim.enter(&round, false);
    for line in [
        "- `code`: The code, its comments in `owner.language` (Chinese).",
        "Build what the plan says, in `owner.language` (Chinese).",
        "- `plan`: Say why in `owner.language` (Chinese).",
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
