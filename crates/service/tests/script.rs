//! The script the test carriers follow: how it is read, checked against the flows, and what a round returns by it; and
//! what the carriers that follow it write and answer.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use decide::run::{Failure, Unmet};
use decide::{BlockId, CallId, Called, Decision, End, Outsider, RunId, Seq, Started, Written};
use driver::Found;
use engine::{Came, Effect, Event, handle};
use flow::{CellSpec, Compiled};
use service::script::{Ends, INPUT_TOKENS, OUTPUT_TOKENS, ROUND_MS, Read, TOOL_MS, USD_MICROS};
use service::{CallReply, Message, Outside, Round, RoundReply, Rounds, Script, Scripted, ToolCall, Tools};
use store::{First, Item, ItemNumber, WorkNumber};
use tokio::sync::mpsc;
use work::{Ask, Call, Handed, Question, Spent, Time, Tool, Way, Work};

/// The directory beside the test flows, which holds the trial flow (`trial`) and what is written beside the flows.
fn beside() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../flow/tests/laid_out")
}

/// What the trial flow is compiled against: the driver's source outside a work, the engine's as the owner answers and
/// as the owner speaks, the act `merge` and the tool `query`.
fn trial_has() -> flow::Has {
    flow::Has {
        sources: BTreeSet::from([
            driver::DEPENDENCIES.to_string(),
            flow::ANSWER.to_string(),
            flow::WORDS.to_string(),
        ]),
        acts: BTreeSet::from(["merge".to_string()]),
        tools: BTreeSet::from(["query".to_string()]),
        ..Default::default()
    }
}

fn trial() -> Compiled {
    let source = flow::read(&beside().join("trial")).unwrap();
    flow::compile(&source, &trial_has()).unwrap()
}

/// What is written beside the test flows, in whose words the carriers read a round's questions.
fn words() -> flow::agents::Agents {
    flow::agents::read(&beside()).unwrap()
}

/// The engineer's flow as the repository defines it, compiled against what the engine has.
fn engineer() -> Compiled {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
    let source = flow::read(&dir).unwrap();
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = flow::Has {
        sources: names(&[
            "answer",
            "asked",
            "dependencies",
            "main",
            "session",
            "start",
            "head",
            "tree",
            "words",
        ]),
        acts: names(&["start", "check", "merge", "run_tests"]),
        tools: names(&["query"]),
        details: names(&["files"]),
        ..Default::default()
    };
    flow::compile(&source, &has).unwrap()
}

fn cell<'a>(flow: &'a Compiled, id: &str) -> &'a CellSpec {
    flow.cells.iter().find(|cell| cell.id.as_str() == id).unwrap()
}

fn handed(block: &str, content: &str, value: Option<&str>) -> Handed {
    Handed {
        block: block.into(),
        content: content.to_string(),
        matters: true,
        value: value.map(str::to_string),
    }
}

#[test]
fn a_script_is_told_everything_it_names_that_the_flow_lacks() {
    let script = Script::read(
        r#"
        [flows.other]

        [flows.trial.values]
        plan = "x"
        verdict = "maybe"

        [[flows.trial.rounds]]
        cell = "bild"

        [[flows.trial.rounds]]
        cell = "build"
        run = 0

        [[flows.trial.rounds]]
        cell = "review"
        hands_in = [{ block = "kode" }]
        asks = [{ tool = "ci" }]

        [[flows.trial.rounds]]
        cell = "merge"
        asks = [{ tool = "ci" }]
        took = { usd_micros = 5 }

        [[flows.trial.rounds]]
        cell = "review"
        hands_in = []

        [[flows.trial.rounds]]
        cell = "build"
        run = 2
        turn = 3
        at = "https://github.example/actions/runs/1"
        "#,
    )
    .unwrap();
    assert_eq!(
        script.check(&[trial()], &words()),
        [
            "flows.other: no flow is named other",
            "flows.trial.values.plan: the block plan is no choice",
            "flows.trial.values.verdict: maybe is not one of the values of verdict",
            "flows.trial.rounds[0]: the flow trial has no cell bild",
            "flows.trial.rounds[1]: works, runs and rounds are counted from 1",
            "flows.trial.rounds[2]: a round either hands in or asks, not both",
            "flows.trial.rounds[2].hands_in[0]: the flow trial has no block kode",
            "flows.trial.rounds[2].asks[0]: the cell review may not ask for the tool ci",
            "flows.trial.rounds[3]: an act of the engine asks for nothing",
            "flows.trial.rounds[3]: an act of the engine takes no cost and no tokens",
            "flows.trial.rounds[4]: says again what flows.trial.rounds[2] says",
            "flows.trial.rounds[5]: only an act of the engine waits for the turn or runs as a CI run",
        ]
    );
}

#[test]
fn a_round_hands_in_beside_its_blocks_what_its_entry_says_it_read() {
    let flow = engineer();
    let script = Script::read(
        r#"
        [[flows.engineer.rounds]]
        cell = "I1"
        hands_in = [{ block = "code", content = "The fix." }]
        reads = [{ block = "head", content = "4b8e2d1" }]
        "#,
    )
    .unwrap();
    let came = Came {
        read: vec![handed("head", "4b8e2d1", None)],
        ..Came::hands_in(vec![handed("code", "The fix.", None)])
    };
    assert_eq!(
        script.round(&flow, cell(&flow, "I1"), 1, 1, 1, &BTreeMap::new()).ends,
        Ends::Came(came)
    );
}

#[test]
fn a_round_the_script_says_nothing_of_reads_its_head_where_it_hands_in_what_an_act_describes_the_work_by() {
    let flow = engineer();
    let Ends::Came(implemented) = Script::default()
        .round(&flow, cell(&flow, "I1"), 1, 2, 1, &BTreeMap::new())
        .ends
    else {
        panic!("the round hands in");
    };
    let head = "The head as I1 committed it in its run 2, round 1.";
    assert_eq!(implemented.read, [handed("head", head, None)]);
    // The review only reads, and the research hands in no code.
    for of in ["T1", "R1"] {
        let Ends::Came(came) = Script::default()
            .round(&flow, cell(&flow, of), 1, 1, 1, &BTreeMap::new())
            .ends
        else {
            panic!("the round hands in");
        };
        assert!(came.read.is_empty(), "{of}: {:?}", came.read);
    }
}

#[test]
fn a_script_with_a_field_it_does_not_know_is_not_read() {
    assert!(Script::read("[took]\nseconds = 3\n").is_err());
    assert!(
        Script::read("[[flows.trial.rounds]]\ncell = \"plan\"\nasks = [{ tool = \"ci\", who = \"owner\" }]\n").is_err()
    );
    // A question to a person among what a round asks for is no tool to run: questions are asked as data.
    let asked = "[[flows.trial.rounds]]\ncell = \"plan\"\nasks = [{ question = \"Which name?\", who = \"owner\" }]\n";
    let refused = Script::read(asked).expect_err("a question to a person is refused");
    assert!(refused.contains("unknown field `question`"), "{refused}");
    // The engine's question is named as the words beside the flows name it, which a script does not say.
    let named = "[[flows.trial.rounds]]\ncell = \"build\"\n\n\
                 [[flows.trial.rounds.engine]]\nname = \"x\"\nwithdrawn = \"No.\"\n";
    assert!(Script::read(named).is_err());
}

#[test]
fn a_round_the_script_says_nothing_of_hands_in_the_outputs_no_earlier_cell_reads() {
    let flow = trial();
    let planned = Script::default().round(&flow, cell(&flow, "review"), 1, 2, 3, &BTreeMap::new());
    // The opinions, which the build before it reads, are a way back, and are not written.
    let verdict = "The verdict as review wrote it in its run 2, round 3.";
    assert_eq!(
        planned.ends,
        Ends::Came(Came::hands_in(vec![handed("verdict", verdict, Some("yes"))]))
    );
    assert_eq!(planned.ms, ROUND_MS);
    assert_eq!(
        planned.spent,
        Spent {
            usd_micros: USD_MICROS,
            input_tokens: INPUT_TOKENS,
            cached_tokens: 0,
            output_tokens: OUTPUT_TOKENS,
            ..Default::default()
        }
    );
}

/// The trial flow with its review bound to write its opinions every time.
fn trial_reviewing_every_time() -> Compiled {
    let mut source = flow::read(&beside().join("trial")).unwrap();
    let review = source.cells.get_mut("review").unwrap();
    *review = review.replacen(
        "outputs = [\"opinions\", \"verdict\"]",
        "outputs = [\"opinions\", \"verdict\"]\nmust_write = [\"opinions\"]",
        1,
    );
    flow::compile(&source, &trial_has()).unwrap()
}

#[test]
fn a_round_the_script_says_nothing_of_hands_in_too_every_block_its_cell_must_write() {
    let flow = trial_reviewing_every_time();
    // The opinions are a way back, and are written all the same: the review must write them every time.
    let words = |block: &str, run| format!("The {block} as review wrote it in its run {run}, round 1.");
    assert_eq!(
        Script::default()
            .round(&flow, cell(&flow, "review"), 1, 1, 1, &BTreeMap::new())
            .ends,
        Ends::Came(Came::hands_in(vec![
            handed("opinions", &words("opinions", 1), None),
            handed("verdict", &words("verdict", 1), Some("yes")),
        ]))
    );
    // Once they have a version as the run starts, they are written as a change that does not matter.
    let basis = BTreeMap::from([
        (BlockId::from("opinions"), Some(Seq(7))),
        ("verdict".into(), Some(Seq(7))),
    ]);
    assert_eq!(
        Script::default()
            .round(&flow, cell(&flow, "review"), 1, 2, 1, &basis)
            .ends,
        Ends::Came(Came::hands_in(vec![
            Handed {
                matters: false,
                ..handed("opinions", &words("opinions", 2), None)
            },
            handed("verdict", &words("verdict", 2), Some("yes")),
        ]))
    );
}

#[test]
fn an_unscripted_trial_whose_review_must_write_its_opinions_reaches_the_merge() {
    let flow = trial_reviewing_every_time();
    let mut work = Work::default();
    let mut clock = 0;
    let mut take = |work: &mut Work, event: Event| {
        clock += 1000;
        handle(&flow, work, Time(clock), event).expect("the loop takes it")
    };
    let wrote = |block: &str, by, value: Option<&str>| Event::Wrote {
        block: block.into(),
        by,
        content: format!("The {block}."),
        matters: true,
        value: value.map(str::to_string),
    };
    // The plan waits for the items its item depends on, and runs once they have merged.
    take(&mut work, wrote("request", Outsider::Person, None));
    let mut effects = take(&mut work, wrote("dependencies", Outsider::Outside, Some("merged")));
    for _ in 0..20 {
        let Some((run, cell, number)) = effects.iter().find_map(|effect| match effect {
            Effect::Round { run, cell, number, .. } => Some((*run, cell.clone(), *number)),
            _ => None,
        }) else {
            break;
        };
        let started = work.records.started.iter();
        let nth = started.clone().filter(|s| s.cell == cell && s.run <= run).count();
        let basis = started
            .clone()
            .find(|s| s.run == run)
            .expect("the run started")
            .basis
            .clone();
        let planned = Script::default().round(&flow, crate::cell(&flow, cell.as_str()), 1, nth, number, &basis);
        let Ends::Came(came) = planned.ends else {
            panic!("an unscripted round fails nothing");
        };
        let returned = Event::Returned {
            run,
            came,
            spent: planned.spent,
        };
        effects = take(&mut work, returned);
    }
    assert_eq!(decide::decide(&flow.flow, &work.records), Decision::Ended(End::Merged));
}

#[test]
fn a_choice_is_handed_in_at_the_value_the_script_names_for_it() {
    let flow = trial();
    let script = Script::read("[flows.trial.values]\nverdict = \"no\"\n").unwrap();
    let Ends::Came(came) = script
        .round(&flow, cell(&flow, "review"), 1, 1, 1, &BTreeMap::new())
        .ends
    else {
        panic!("the round hands in");
    };
    assert_eq!(came.writes[0].value.as_deref(), Some("no"));
}

#[test]
fn a_round_takes_what_its_entry_says_and_else_what_the_script_says() {
    let flow = trial();
    let script = Script::read(
        r#"
        [took]
        ms = 900
        usd_micros = 40

        [[flows.trial.rounds]]
        cell = "build"
        run = 2
        took = { ms = 50 }
        hands_in = [{ block = "code", matters = false }]

        [[flows.trial.rounds]]
        cell = "merge"
        "#,
    )
    .unwrap();
    let planned = script.round(&flow, cell(&flow, "build"), 1, 2, 1, &BTreeMap::new());
    assert_eq!(planned.ms, 50);
    assert_eq!(planned.spent.usd_micros, 40);
    assert_eq!(
        planned.ends,
        Ends::Came(Came::hands_in(vec![Handed {
            matters: false,
            ..handed("code", "The code as build wrote it in its run 2, round 1.", None)
        }]))
    );
    // The engine's own act takes no cost, whatever the script says of rounds.
    let merge = script.round(&flow, cell(&flow, "merge"), 1, 1, 1, &BTreeMap::new());
    assert_eq!((merge.ms, merge.spent), (900, Spent::default()));
}

#[test]
fn an_act_says_the_turn_it_waits_for_and_the_ci_run_it_runs_as_where_its_entry_says() {
    let flow = trial();
    let script = Script::read(
        r#"
        [[flows.trial.rounds]]
        cell = "merge"
        turn = 3
        at = "https://github.example/actions/runs/1"
        "#,
    )
    .unwrap();
    let merge = script.round(&flow, cell(&flow, "merge"), 1, 1, 1, &BTreeMap::new());
    assert_eq!(
        (merge.turn, merge.at.as_deref()),
        (Some(3), Some("https://github.example/actions/runs/1"))
    );
    let again = script.round(&flow, cell(&flow, "merge"), 1, 2, 1, &BTreeMap::new());
    assert_eq!((again.turn, again.at), (None, None));
}

#[test]
fn a_round_asks_for_what_its_entry_says() {
    let flow = trial();
    let script = Script::read(
        r#"
        [[flows.trial.rounds]]
        cell = "build"
        asks = [{ tool = "query", input = "main" }, { tool = "query", input = "the items" }]
        "#,
    )
    .unwrap();
    let query = |input: &str| Tool {
        name: "query".to_string(),
        input: input.to_string(),
    };
    assert_eq!(
        script
            .round(&flow, cell(&flow, "build"), 1, 1, 1, &BTreeMap::new())
            .ends,
        Ends::Came(Came::asks(vec![query("main"), query("the items")]))
    );
}

#[test]
fn a_round_fails_where_its_entry_says_and_takes_what_it_says() {
    let flow = trial();
    let script = Script::read(
        r#"
        [[flows.trial.rounds]]
        cell = "plan"
        took = { ms = 70, usd_micros = 9 }
        fails = "The agent could not start."
        "#,
    )
    .unwrap();
    let planned = script.round(&flow, cell(&flow, "plan"), 1, 1, 1, &BTreeMap::new());
    assert_eq!(
        planned.ends,
        Ends::Fails {
            why: "The agent could not start.".to_string(),
            failure: decide::run::Failure::Other,
            resets: None,
        }
    );
    assert_eq!((planned.ms, planned.spent.usd_micros), (70, 9));
    // The round after it, given the same once the work is let on, does what the script says of it: by default it hands
    // in.
    assert!(matches!(
        script.round(&flow, cell(&flow, "plan"), 1, 1, 2, &BTreeMap::new()).ends,
        Ends::Came(Came { ref asks, .. }) if asks.is_empty()
    ));
}

#[test]
fn a_round_that_fails_is_told_it_neither_hands_in_nor_asks() {
    let script = Script::read(
        r#"
        [[flows.trial.rounds]]
        cell = "plan"
        fails = "No."
        asks = [{ tool = "ci" }]

        [[flows.trial.rounds]]
        cell = "build"
        fails = "No."
        hands_in = []
        "#,
    )
    .unwrap();
    assert_eq!(
        script.check(&[trial()], &words()),
        [
            "flows.trial.rounds[0]: a round that fails neither hands in nor asks",
            "flows.trial.rounds[0].asks[0]: the cell plan may not ask for the tool ci",
            "flows.trial.rounds[1]: a round that fails neither hands in nor asks",
        ]
    );
}

#[test]
fn a_tool_answers_what_the_script_says_or_what_it_ran_on() {
    let script = Script::read("[tools.ci]\nanswer = \"Green.\"\nms = 30\n").unwrap();
    assert_eq!(script.tool("ci", "main", None), ("Green.".to_string(), 30));
    assert_eq!(
        script.tool("lint", "main", None),
        ("lint ran on: main".to_string(), TOOL_MS)
    );
}

#[test]
fn a_round_of_one_work_follows_its_own_entry_and_that_of_another_work_the_entry_of_every_work() {
    let flow = trial();
    let script = Script::read(
        r#"
        [[flows.trial.rounds]]
        cell = "review"
        hands_in = [{ block = "verdict", value = "yes" }]

        [[flows.trial.rounds]]
        cell = "review"
        work = 3
        hands_in = [{ block = "verdict", value = "no" }]
        "#,
    )
    .unwrap();
    let verdict = |work| {
        let Ends::Came(came) = script
            .round(&flow, cell(&flow, "review"), work, 1, 1, &BTreeMap::new())
            .ends
        else {
            panic!("the round hands in");
        };
        came.writes[0].value.clone()
    };
    assert_eq!(verdict(3).as_deref(), Some("no"));
    assert_eq!(verdict(4).as_deref(), Some("yes"));
}

/// A question of the trial's plan that waits for the answer, recommending its first option.
fn named(name: &str, recommends: Option<u64>) -> Question {
    Question::Asked {
        name: name.to_string(),
        waits: true,
        recommends,
        ask: "Should the greeting name the user?".to_string(),
        options: vec![
            Way {
                does: "By name".to_string(),
                follows: "A warmer greeting.".to_string(),
            },
            Way {
                does: "Without a name".to_string(),
                follows: "A plainer one.".to_string(),
            },
        ],
        about: None,
        reasons: BTreeMap::new(),
    }
}

#[test]
fn a_round_asks_the_owner_the_questions_its_entry_says_and_withdraws_one_beside_what_it_hands_in() {
    let flow = trial();
    let script = Script::read(
        r#"
        [[flows.trial.rounds]]
        cell = "plan"

        [[flows.trial.rounds.questions]]
        name = "named"
        waits = true
        recommends = 1
        ask = "Should the greeting name the user?"
        options = [{ does = "By name", follows = "A warmer greeting." }, { does = "Without a name", follows = "A plainer one." }]

        [[flows.trial.rounds.questions]]
        name = "loud"
        withdrawn = "The greeting is quiet."
        "#,
    )
    .unwrap();
    let Ends::Came(came) = script.round(&flow, cell(&flow, "plan"), 1, 1, 1, &BTreeMap::new()).ends else {
        panic!("the round hands in");
    };
    let withdrawn = Question::Withdrawn {
        name: "loud".to_string(),
        why: "The greeting is quiet.".to_string(),
    };
    assert_eq!(came.said.questions, [named("named", Some(1)), withdrawn]);
    // What it hands in is what a round the script says nothing of hands in, which is never the record of the questions.
    let plan = "The plan as plan wrote it in its run 1, round 1.";
    assert_eq!(came.writes, [handed("plan", plan, None)]);
}

#[test]
fn a_round_the_script_says_nothing_of_hands_in_no_record_of_the_works_questions() {
    let flow = trial();
    let Ends::Came(came) = Script::default()
        .round(&flow, cell(&flow, "plan"), 1, 1, 1, &BTreeMap::new())
        .ends
    else {
        panic!("the round hands in");
    };
    let written: Vec<&str> = came.writes.iter().map(|h| h.block.as_str()).collect();
    assert_eq!(written, ["plan"]);
}

#[test]
fn a_round_ends_with_the_context_its_entry_says_and_else_the_scripts() {
    let flow = engineer();
    let script = Script::read(
        r#"
        [took]
        context_tokens = 9000

        [[flows.engineer.rounds]]
        cell = "I1"
        took = { context_tokens = 48000 }
        "#,
    )
    .unwrap();
    let context = |of: &str| {
        script
            .round(&flow, cell(&flow, of), 1, 1, 1, &BTreeMap::new())
            .spent
            .context_tokens
    };
    assert_eq!(context("I1"), Some(48000));
    // A round its entry says no context of ends with the script's; an act's, with none.
    assert_eq!(context("R1"), Some(9000));
    assert_eq!(context("M1"), None);
}

#[test]
fn a_time_a_script_names_counts_from_its_start() {
    let flow = trial();
    let script = Script::read(
        r#"
        start = 1794268800000

        [[flows.trial.rounds]]
        cell = "plan"
        fails = "You've hit your limit."
        failure = "usage_limit"
        resets = 3600000
        "#,
    )
    .unwrap();
    let ends = script.round(&flow, cell(&flow, "plan"), 1, 1, 1, &BTreeMap::new()).ends;
    let Ends::Fails { resets, .. } = ends else {
        panic!("the round fails");
    };
    assert_eq!(resets, Some(Time(1_794_268_800_000 + 3_600_000)));
}

#[test]
fn an_acts_round_reads_what_its_entry_says_and_an_agents_hands_it_in() {
    let flow = engineer();
    let script = Script::read(
        r#"
        [[flows.engineer.rounds]]
        cell = "M2"
        reads = [{ block = "head", content = "9c0ffee", for_round = false }]
        fails = "The work's head moved since its check."
        failure = "passing"

        [[flows.engineer.rounds]]
        cell = "M1"
        reads = [{ block = "main", content = "main at 7e57ed", for_round = true }]
        "#,
    )
    .unwrap();
    let merge = script.round(&flow, cell(&flow, "M2"), 1, 1, 1, &BTreeMap::new());
    let head = Read {
        block: "head".into(),
        content: "9c0ffee".to_string(),
        for_round: false,
    };
    assert_eq!(merge.reads, Some(vec![head]));
    let check = script.round(&flow, cell(&flow, "M1"), 1, 1, 1, &BTreeMap::new());
    let main = Read {
        block: "main".into(),
        content: "main at 7e57ed".to_string(),
        for_round: true,
    };
    assert_eq!(check.reads, Some(vec![main]));
    // What an act read it writes as it goes, not with what comes back; an act its entry says nothing of reads by
    // itself.
    let Ends::Came(came) = check.ends else {
        panic!("the check hands in");
    };
    assert!(came.read.is_empty(), "{:?}", came.read);
    assert_eq!(
        Script::default()
            .round(&flow, cell(&flow, "M1"), 1, 1, 1, &BTreeMap::new())
            .reads,
        None
    );
}

#[test]
fn an_ask_says_what_its_call_answers_and_how_long_it_takes_else_the_tool_does() {
    let script = Script::read(
        r#"
        [tools.query]
        answer = '{"rows": []}'
        ms = 300

        [[flows.trial.rounds]]
        cell = "build"
        asks = [{ tool = "query", input = "a", answer = '{"rows": [{"n": 1}]}', ms = 40 }, { tool = "query", input = "b" }]
        "#,
    )
    .unwrap();
    let asks = script.flows["trial"].rounds[0].asks.clone().unwrap();
    assert_eq!(
        script.tool("query", "a", Some(&asks[0])),
        (r#"{"rows": [{"n": 1}]}"#.to_string(), 40)
    );
    assert_eq!(
        script.tool("query", "b", Some(&asks[1])),
        (r#"{"rows": []}"#.to_string(), 300)
    );
}

// Every misuse of the keys a round says what it returns by, and of the script's start and its outside, is told in one
// pass, each with where it is: each would write records no engine reaches, or a time with nothing to count from.
#[test]
fn a_script_is_told_every_misuse_of_what_a_round_returns_at_once() {
    let script = Script::read(
        r#"
        [tools.check]

        [outside]
        finds = { failure = "passing", why = "A crash.", resets = 600000 }

        [[flows.engineer.rounds]]
        cell = "I1"
        hands_in = []
        reads = [{ block = "code", content = "4b8e2d1" }, { block = "hed", content = "4b8e2d1" }]

        [[flows.engineer.rounds]]
        cell = "I1"
        run = 2
        asks = [{ tool = "query" }]
        reads = [{ block = "head", content = "4b8e2d1" }]

        [[flows.engineer.rounds]]
        cell = "I1"
        run = 3
        fails = "No."
        reads = [{ block = "head", content = "4b8e2d1" }]

        [[flows.engineer.rounds]]
        cell = "I1"
        run = 4
        reads = [{ block = "head", content = "4b8e2d1", for_round = true }]

        [[flows.engineer.rounds]]
        cell = "I1"
        work = 2

        [[flows.engineer.rounds]]
        cell = "I1"
        work = 2

        [[flows.trial.rounds]]
        cell = "build"
        work = 0
        asks = [{ tool = "check" }]

        [[flows.trial.rounds]]
        cell = "build"
        hands_in = [{ block = "q_and_a", content = "Which name?" }]

        [[flows.trial.rounds]]
        cell = "build"
        run = 2

        [[flows.trial.rounds.questions]]
        name = "named"
        waits = true
        recommends = 1
        ask = "Which name?"
        options = [{ does = "By name", follows = "Warmer." }]

        [[flows.trial.rounds]]
        cell = "review"
        took = { context_tokens = 10 }

        [[flows.trial.rounds.engine]]
        withdrawn = "The work touches no protected file."

        [[flows.trial.rounds]]
        cell = "merge"
        reads = [{ block = "dependencies", content = "Merged." }]
        took = { context_tokens = 10 }

        [[flows.trial.rounds.engine]]
        withdrawn = "The work touches no protected file."

        [[flows.trial.rounds]]
        cell = "plan"
        fails = "No."
        resets = 60000

        [[flows.trial.rounds.questions]]
        name = "named"
        withdrawn = "No."

        [[flows.trial.rounds.engine]]
        withdrawn = "No."

        [[flows.trial.rounds]]
        cell = "build"
        run = 3
        asks = [{ tool = "query" }]

        [[flows.trial.rounds.engine]]
        withdrawn = "No."
        "#,
    )
    .unwrap();
    assert_eq!(
        script.check(&[trial(), engineer()], &words()),
        [
            "tools.check: no flow names the tool check",
            "outside.finds: the check of the outside finds only what holds every work: usage_limit, cannot_run, \
             offline or unavailable",
            "outside.finds: only a usage limit resets",
            "outside.finds.resets: a time counts from the script's start, which it does not say",
            "flows.engineer.rounds[0].reads[0]: no source outside the work writes code",
            "flows.engineer.rounds[0].reads[1]: the flow engineer has no block hed",
            "flows.engineer.rounds[1]: only an agent's round that does the cell's work reads",
            "flows.engineer.rounds[2]: only an agent's round that does the cell's work reads",
            "flows.engineer.rounds[3].reads[0]: only an act's round says for_round",
            "flows.engineer.rounds[5]: says again what flows.engineer.rounds[4] says",
            "flows.trial.rounds[0]: works, runs and rounds are counted from 1",
            "flows.trial.rounds[0].asks[0]: the cell build may not ask for the tool check",
            "flows.trial.rounds[1].hands_in[0]: q_and_a holds the work's questions, which a round asks in questions",
            "flows.trial.rounds[2]: the cell build writes no record of the work's questions, and asks the owner none",
            "flows.trial.rounds[3]: the engine asks nothing as a round of review ends, which it looks at no branch for",
            "flows.trial.rounds[4].reads[0]: an act's round says whether it writes what it read for_round",
            "flows.trial.rounds[4]: the engine asks nothing as a round of merge ends, which it looks at no branch for",
            "flows.trial.rounds[4]: an act of the engine takes no cost and no tokens",
            "flows.trial.rounds[5]: only a round that does the cell's work asks the owner",
            "flows.trial.rounds[5]: only a round that does the cell's work ends with what the engine asks",
            "flows.trial.rounds[5].resets: a time counts from the script's start, which it does not say",
            "flows.trial.rounds[6]: only a round that does the cell's work ends with what the engine asks",
        ]
    );
}

/// Work 1 as kept with its runs started, each of a cell, in order from run 1, and the given number of rounds of its
/// last run begun; the rest as given.
fn kept(cells: &[&str], rounds: usize, mut work: Work) -> Work {
    for (i, cell) in cells.iter().enumerate() {
        work.records.started.push(Started {
            run: RunId(i as u64 + 1),
            cell: (*cell).into(),
            at: Seq(100 + i as u64),
            because: Vec::new(),
            basis: BTreeMap::new(),
        });
    }
    for number in 0..rounds {
        let run = RunId(cells.len() as u64);
        work.records.rounds.push(decide::Round {
            run,
            at: Seq(200 + number as u64),
        });
    }
    work
}

/// The item of work 1, as a round or a call is given it.
fn item() -> Item {
    let first = First {
        title: "Greet the user".to_string(),
        level: None,
        project: None,
        depends_on: Vec::new(),
        tags: Vec::new(),
    };
    Item::new(ItemNumber(1), Time(0), first, Vec::new())
}

/// The first round of the last run of work 1 as kept, the run of this cell.
fn round(flow: &Compiled, of: &str, kept: Work) -> Round {
    let run = RunId(kept.records.started.len() as u64);
    Round {
        work: WorkNumber(1),
        flow: Arc::new(flow.clone()),
        cell: cell(flow, of).clone(),
        run,
        number: 1,
        given: work::Given::Task,
        item: item(),
        kept: Arc::new(kept),
        lines: Default::default(),
        account: None,
    }
}

/// What carriers that follow the script, started anew, say of the round up to its end, as the loop would take it.
async fn carried(script: &Script, round: Round) -> Vec<Event> {
    let scripted = Scripted::new(script.clone(), words());
    let (to, mut from) = mpsc::unbounded_channel();
    let reply = RoundReply::new(round.work, round.run, to);
    scripted.give(round, reply);
    let mut events = Vec::new();
    while let Some(message) = from.recv().await {
        let Message::Happened { event, .. } = message else {
            continue;
        };
        let ended = matches!(event, Event::Returned { .. } | Event::Failed { .. });
        events.push(event);
        if ended {
            break;
        }
    }
    events
}

/// The work with a version of the block written from outside, saying this.
fn written(mut work: Work, block: &str, content: &str) -> Work {
    let at = Seq(50);
    work.contents.insert((block.into(), at), content.to_string());
    work.records.written.push(Written {
        block: block.into(),
        by: Outsider::Outside,
        matters: true,
        value: None,
        empty: false,
        said_to: None,
        at,
    });
    work
}

// An act's round writes what it read as the real acts do: the merge a head that moved since the check from outside, so
// that the review and the check run on it (decision a-moved-head-is-reviewed-and-checked-again-before-the-merge, which
// the engineer's histories hold the engine to), and the check the main it takes in for its round, which then runs it
// no more. The stood-in check, its entry saying nothing of what it read, tells its round the main the work's block of
// main holds, as the real check tells its round the latest main it takes in, and only where that block holds none
// writes a main made up for its round, so that it never writes main back to an older head after main moved; what a
// block says already is not written again. Milliseconds alone, on the paused clock; no other test holds the stand-ins to
// what an act writes as it goes.
#[tokio::test(start_paused = true)]
async fn an_acts_round_writes_what_it_read_for_its_round_or_from_outside_as_the_real_acts_do() {
    let flow = engineer();
    let script = Script::read(
        r#"
        [[flows.engineer.rounds]]
        cell = "M2"
        reads = [{ block = "head", content = "9c0ffee", for_round = false }]
        fails = "The work's head moved since its check."
        failure = "passing"

        [[flows.engineer.rounds]]
        cell = "M1"
        run = 2
        reads = [{ block = "main", content = "main at 7e57ed", for_round = true }]
        "#,
    )
    .unwrap();
    let merged = carried(&script, round(&flow, "M2", kept(&["M2"], 1, Work::default()))).await;
    assert!(
        matches!(&merged[..], [
            Event::Wrote { block, by: Outsider::Outside, content, .. },
            Event::Failed { failure: Failure::Passing, .. },
        ] if block.as_str() == "head" && content == "9c0ffee"),
        "{merged:?}"
    );
    // The same head read again, as by a round given again after the engine started again, is not written again.
    let again = written(kept(&["M2"], 1, Work::default()), "head", "9c0ffee");
    let merged = carried(&script, round(&flow, "M2", again)).await;
    assert!(matches!(&merged[..], [Event::Failed { .. }]), "{merged:?}");
    let checked = carried(&script, round(&flow, "M1", kept(&["M1", "M1"], 1, Work::default()))).await;
    assert!(
        matches!(&checked[..], [Event::WroteForRound { block, content, .. }, Event::Returned { .. }]
            if block.as_str() == "main" && content == "main at 7e57ed"),
        "{checked:?}"
    );
    // Said nothing of, the check takes main in: a main made up for its round where the work's block holds none.
    let checked = carried(&script, round(&flow, "M1", kept(&["M1"], 1, Work::default()))).await;
    let made_up = "The main as M1 took it in in its run 1, round 1.";
    assert!(
        matches!(&checked[..], [Event::WroteForRound { block, content, .. }, Event::Returned { .. }]
            if block.as_str() == "main" && content == made_up),
        "{checked:?}"
    );
    // Where it holds one, main as it moved, its round is told that main, which it takes in, and never the head of an
    // older main.
    let moved = written(kept(&["M1"], 1, Work::default()), "main", "main at 0ddba11");
    let checked = carried(&script, round(&flow, "M1", moved)).await;
    assert!(
        matches!(&checked[..], [Event::WroteForRound { block, content, .. }, Event::Returned { .. }]
            if block.as_str() == "main" && content == "main at 0ddba11"),
        "{checked:?}"
    );
}

// What an agent's round read of the branch is handed in where it differs from what the block says, as the real carrier
// hands it in. Milliseconds alone, on the paused clock.
#[tokio::test(start_paused = true)]
async fn an_agents_round_hands_in_what_it_read_where_its_block_does_not_say_it() {
    let flow = engineer();
    let script = Script::read(
        r#"
        [[flows.engineer.rounds]]
        cell = "I1"
        hands_in = [{ block = "code", content = "The fix." }]
        reads = [{ block = "head", content = "4b8e2d1" }]
        "#,
    )
    .unwrap();
    let read = |events: &[Event]| match events.last() {
        Some(Event::Returned { came, .. }) => came.read.clone(),
        other => panic!("the round returns: {other:?}"),
    };
    let fresh = carried(&script, round(&flow, "I1", kept(&["I1"], 1, Work::default()))).await;
    assert_eq!(read(&fresh), [handed("head", "4b8e2d1", None)]);
    let said = written(kept(&["I1"], 1, Work::default()), "head", "4b8e2d1");
    assert_eq!(read(&carried(&script, round(&flow, "I1", said)).await), []);
}

// A question a round asks that recommends none of its options, or has the name of the question the engine itself asks
// in the words beside the flows, is found wrong with the return as a real return's reading finds it, so that the run's
// check sends it back. Milliseconds alone, on the paused clock.
#[tokio::test(start_paused = true)]
async fn the_questions_a_round_asks_are_found_wrong_as_a_returns_reading_finds_them() {
    let flow = trial();
    let script = Script::read(
        r#"
        [[flows.trial.rounds]]
        cell = "plan"
        hands_in = []

        [[flows.trial.rounds.questions]]
        name = "named"
        waits = true
        ask = "Should the greeting name the user?"
        options = [{ does = "By name", follows = "A warmer greeting." }, { does = "Without a name", follows = "A plainer one." }]

        [[flows.trial.rounds.questions]]
        name = "protected_files"
        waits = true
        recommends = 1
        ask = "Should the greeting name the user?"
        options = [{ does = "By name", follows = "A warmer greeting." }, { does = "Without a name", follows = "A plainer one." }]
        "#,
    )
    .unwrap();
    assert_eq!(words().asks.protected.name, "protected_files");
    let events = carried(&script, round(&flow, "plan", kept(&["plan"], 1, Work::default()))).await;
    let Some(Event::Returned { came, .. }) = events.last() else {
        panic!("the round returns: {events:?}");
    };
    assert_eq!(came.said.questions[0], named("named", None));
    assert_eq!(
        came.found,
        [
            Unmet::NotAnOption {
                name: "named".to_string()
            },
            Unmet::QuestionTaken {
                name: "protected_files".to_string()
            },
        ]
    );
}

/// The call of this number of the run of work 1 as kept, the first `query` its last round asked for, on this.
fn call(flow: &Compiled, kept: Work, call: CallId, input: &str) -> ToolCall {
    let run = RunId(kept.records.started.len() as u64);
    ToolCall {
        work: WorkNumber(1),
        flow: Arc::new(flow.clone()),
        run,
        call,
        name: "query".to_string(),
        input: input.to_string(),
        item: item(),
        kept: Arc::new(kept),
    }
}

/// The work as kept with these calls of its last run made, each a `query` on its input, in order.
fn called(mut work: Work, calls: &[(u64, &str)]) -> Work {
    let run = RunId(work.records.started.len() as u64);
    for (i, (call, input)) in calls.iter().enumerate() {
        work.records.called.push(Called {
            call: CallId(*call),
            run,
            on: None,
            at: Seq(300 + i as u64),
        });
        let ask = Ask::Tool {
            name: "query".to_string(),
            input: input.to_string(),
        };
        work.calls.insert(CallId(*call), Call::Asked(ask));
    }
    work
}

/// What the call answers, and how long it took on the paused clock.
async fn answered(scripted: &Scripted, call: ToolCall) -> (String, u64) {
    let (to, mut from) = mpsc::unbounded_channel();
    let reply = CallReply::new(call.work, call.call, to);
    let began = tokio::time::Instant::now();
    scripted.call(call, reply);
    loop {
        match from.recv().await {
            Some(Message::Happened {
                event: Event::Done { result, .. },
                ..
            }) => return (result, began.elapsed().as_millis() as u64),
            Some(_) => continue,
            None => panic!("the call has no result"),
        }
    }
}

// A call answers what the ask that made it says, found by the call's run and its place among the calls its last round
// asked for in the work's records, and else what the tool's entry says; given again by carriers started anew, as after
// the engine started again, it answers the same. Milliseconds alone, on the paused clock.
#[tokio::test(start_paused = true)]
async fn a_call_answers_what_its_ask_says_found_by_its_round_in_the_works_records() {
    let flow = trial();
    let text = r#"
        [tools.query]
        answer = '{"rows": []}'
        ms = 300

        [[flows.trial.rounds]]
        cell = "build"
        asks = [{ tool = "query", input = "a", answer = '{"rows": [{"n": 1}]}', ms = 40 }, { tool = "query", input = "b" }]
        "#;
    let kept = || called(kept(&["plan", "build"], 1, Work::default()), &[(3, "a"), (4, "b")]);
    for scripted in [
        Scripted::new(Script::read(text).unwrap(), words()),
        Scripted::new(Script::read(text).unwrap(), words()),
    ] {
        let own = answered(&scripted, call(&flow, kept(), CallId(3), "a")).await;
        assert_eq!(own, (r#"{"rows": [{"n": 1}]}"#.to_string(), 40));
        let tools = answered(&scripted, call(&flow, kept(), CallId(4), "b")).await;
        assert_eq!(tools, (r#"{"rows": []}"#.to_string(), 300));
    }
}

#[tokio::test]
async fn the_check_of_the_outside_finds_what_the_script_says_and_else_finds_it_well() {
    let script = Script::read(
        r#"
        start = 1794268800000

        [outside]
        finds = { failure = "usage_limit", why = "You've hit your limit.", resets = 3600000 }
        "#,
    )
    .unwrap();
    assert_eq!(
        Scripted::new(script, words()).check(None).await,
        Found::Down {
            failure: Failure::UsageLimit,
            why: "You've hit your limit.".to_string(),
            resets: Some(Time(1_794_268_800_000 + 3_600_000)),
        }
    );
    assert_eq!(Scripted::new(Script::default(), words()).check(None).await, Found::Well);
}

// A round of a line that only reads hands in nothing of what it read: as the real carrier does, what it found of the
// branch as it ended is written from outside before what comes back, a change none of its own, as a commit someone made
// during a review; read again, as by a round given again after a restart, it is not written again. Milliseconds alone,
// on the paused clock; no other test holds the stand-ins to how a round that only reads writes what it read.
#[tokio::test(start_paused = true)]
async fn a_round_of_a_line_that_only_reads_writes_what_it_read_from_outside_as_it_ends() {
    let flow = engineer();
    let script = Script::read(
        r#"
        [[flows.engineer.rounds]]
        cell = "T1"
        reads = [{ block = "head", content = "9c0ffee" }]
        "#,
    )
    .unwrap();
    assert!(cell(&flow, "T1").reads_only, "the review is on a line that only reads");
    let planned = script.round(&flow, cell(&flow, "T1"), 1, 1, 1, &BTreeMap::new());
    let head = Read {
        block: "head".into(),
        content: "9c0ffee".to_string(),
        for_round: false,
    };
    assert_eq!(planned.reads, Some(vec![head]));
    let Ends::Came(came) = planned.ends else {
        panic!("the review hands in");
    };
    assert!(came.read.is_empty(), "{:?}", came.read);
    let reviewed = carried(&script, round(&flow, "T1", kept(&["T1"], 1, Work::default()))).await;
    assert!(
        matches!(&reviewed[..], [
            Event::Wrote { block, by: Outsider::Outside, content, .. },
            Event::Returned { came, .. },
        ] if block.as_str() == "head" && content == "9c0ffee" && came.read.is_empty()),
        "{reviewed:?}"
    );
    let again = written(kept(&["T1"], 1, Work::default()), "head", "9c0ffee");
    let reviewed = carried(&script, round(&flow, "T1", again)).await;
    assert!(matches!(&reviewed[..], [Event::Returned { .. }]), "{reviewed:?}");
}

// What the engine asks as a round ends is its own question, which the carriers name as the words beside the flows name
// it: a script does not say its name. Milliseconds alone, on the paused clock.
#[tokio::test(start_paused = true)]
async fn what_the_engine_asks_as_a_round_ends_is_named_as_the_words_beside_the_flows_name_its_question() {
    let flow = engineer();
    let script = Script::read(
        r#"
        [[flows.engineer.rounds]]
        cell = "I1"

        [[flows.engineer.rounds.engine]]
        waits = true
        ask = "The work changes `AGENTS.md`."
        options = [{ does = "Merge it", follows = "The guide changes." }, { does = "Take it back", follows = "It stays." }]
        about = "AGENTS.md 4b8e2d1"
        reasons = { "AGENTS.md" = "The guide names the new command." }
        "#,
    )
    .unwrap();
    let events = carried(&script, round(&flow, "I1", kept(&["I1"], 1, Work::default()))).await;
    let Some(Event::Returned { came, .. }) = events.last() else {
        panic!("the round returns: {events:?}");
    };
    let way = |does: &str, follows: &str| Way {
        does: does.to_string(),
        follows: follows.to_string(),
    };
    assert_eq!(
        came.engine,
        [Question::Asked {
            name: words().asks.protected.name,
            waits: true,
            recommends: None,
            ask: "The work changes `AGENTS.md`.".to_string(),
            options: vec![way("Merge it", "The guide changes."), way("Take it back", "It stays.")],
            about: Some("AGENTS.md 4b8e2d1".to_string()),
            reasons: BTreeMap::from([("AGENTS.md".to_string(), "The guide names the new command.".to_string())]),
        }]
    );
}
