//! The script the test carriers follow: how it is read, checked against the flows, and what a round returns by it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use decide::{BlockId, Decision, End, Outsider, Seq};
use engine::{Came, Effect, Event, handle};
use flow::{CellSpec, Compiled};
use service::Script;
use service::script::{Ends, INPUT_TOKENS, OUTPUT_TOKENS, ROUND_MS, TOOL_MS, USD_MICROS};
use work::{Ask, Handed, Spent, Time, Work};

fn trial() -> Compiled {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/trial/flow");
    let source = flow::read(&dir).unwrap();
    let has = flow::Has {
        sources: BTreeSet::from([driver::DEPENDENCIES.to_string()]),
        acts: BTreeSet::from(["merge".to_string()]),
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
fn the_trial_script_fits_the_trial_flow() {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/trial/script.toml")).unwrap();
    assert_eq!(Script::read(&text).unwrap().check(&[trial()]), Vec::<String>::new());
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
        "#,
    )
    .unwrap();
    assert_eq!(
        script.check(&[trial()]),
        [
            "flows.other: no flow is named other",
            "flows.trial.values.plan: the block plan is no choice",
            "flows.trial.values.verdict: maybe is not one of the values of verdict",
            "flows.trial.rounds[0]: the flow trial has no cell bild",
            "flows.trial.rounds[1]: runs and rounds are counted from 1",
            "flows.trial.rounds[2]: a round either hands in or asks, not both",
            "flows.trial.rounds[2].hands_in[0]: the flow trial has no block kode",
            "flows.trial.rounds[3]: an act of the engine asks for nothing",
            "flows.trial.rounds[3]: an act of the engine takes no cost and no tokens",
            "flows.trial.rounds[4]: says again what flows.trial.rounds[2] says",
        ]
    );
}

#[test]
fn a_script_with_a_field_it_does_not_know_is_not_read() {
    assert!(Script::read("[took]\nseconds = 3\n").is_err());
    assert!(
        Script::read("[[flows.trial.rounds]]\ncell = \"plan\"\nasks = [{ tool = \"ci\", who = \"owner\" }]\n").is_err()
    );
}

#[test]
fn a_round_the_script_says_nothing_of_hands_in_the_outputs_no_earlier_cell_reads() {
    let flow = trial();
    let planned = Script::default().round(&flow, cell(&flow, "review"), 2, 3, &BTreeMap::new());
    // The opinions, which the build before it reads, are a way back, and are not written.
    let verdict = "The verdict as review wrote it in its run 2, round 3.";
    assert_eq!(
        planned.ends,
        Ends::Came(Came::HandsIn(vec![handed("verdict", verdict, Some("yes"))]))
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
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/trial/flow");
    let mut source = flow::read(&dir).unwrap();
    let review = source.cells.get_mut("review").unwrap();
    *review = review.replacen(
        "outputs = [\"opinions\", \"verdict\"]",
        "outputs = [\"opinions\", \"verdict\"]\nmust_write = [\"opinions\"]",
        1,
    );
    let has = flow::Has {
        sources: BTreeSet::from([driver::DEPENDENCIES.to_string()]),
        acts: BTreeSet::from(["merge".to_string()]),
    };
    flow::compile(&source, &has).unwrap()
}

#[test]
fn a_round_the_script_says_nothing_of_hands_in_too_every_block_its_cell_must_write() {
    let flow = trial_reviewing_every_time();
    // The opinions are a way back, and are written all the same: the review must write them every time.
    let words = |block: &str, run| format!("The {block} as review wrote it in its run {run}, round 1.");
    assert_eq!(
        Script::default()
            .round(&flow, cell(&flow, "review"), 1, 1, &BTreeMap::new())
            .ends,
        Ends::Came(Came::HandsIn(vec![
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
        Script::default().round(&flow, cell(&flow, "review"), 2, 1, &basis).ends,
        Ends::Came(Came::HandsIn(vec![
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
        let planned = Script::default().round(&flow, crate::cell(&flow, cell.as_str()), nth, number, &basis);
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
    let Ends::Came(Came::HandsIn(blocks)) = script.round(&flow, cell(&flow, "review"), 1, 1, &BTreeMap::new()).ends
    else {
        panic!("the round hands in");
    };
    assert_eq!(blocks[0].value.as_deref(), Some("no"));
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
    let planned = script.round(&flow, cell(&flow, "build"), 2, 1, &BTreeMap::new());
    assert_eq!(planned.ms, 50);
    assert_eq!(planned.spent.usd_micros, 40);
    assert_eq!(
        planned.ends,
        Ends::Came(Came::HandsIn(vec![Handed {
            matters: false,
            ..handed("code", "The code as build wrote it in its run 2, round 1.", None)
        }]))
    );
    // The engine's own act takes no cost, whatever the script says of rounds.
    let merge = script.round(&flow, cell(&flow, "merge"), 1, 1, &BTreeMap::new());
    assert_eq!((merge.ms, merge.spent), (900, Spent::default()));
}

#[test]
fn a_round_asks_for_what_its_entry_says() {
    let flow = trial();
    let script = Script::read(
        r#"
        [[flows.trial.rounds]]
        cell = "plan"
        asks = [{ question = "Which name?" }, { tool = "ci", input = "main" }]
        "#,
    )
    .unwrap();
    assert_eq!(
        script.round(&flow, cell(&flow, "plan"), 1, 1, &BTreeMap::new()).ends,
        Ends::Came(Came::Asks(vec![
            Ask::Person {
                who: decide::Who::Owner,
                question: "Which name?".to_string()
            },
            Ask::Tool {
                name: "ci".to_string(),
                input: "main".to_string()
            },
        ]))
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
    let planned = script.round(&flow, cell(&flow, "plan"), 1, 1, &BTreeMap::new());
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
        script.round(&flow, cell(&flow, "plan"), 1, 2, &BTreeMap::new()).ends,
        Ends::Came(Came::HandsIn(_))
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
        script.check(&[trial()]),
        [
            "flows.trial.rounds[0]: a round that fails neither hands in nor asks",
            "flows.trial.rounds[1]: a round that fails neither hands in nor asks",
        ]
    );
}

#[test]
fn a_tool_answers_what_the_script_says_or_what_it_ran_on() {
    let script = Script::read("[tools.ci]\nanswer = \"Green.\"\nms = 30\n").unwrap();
    assert_eq!(script.tool("ci", "main"), ("Green.".to_string(), 30));
    assert_eq!(script.tool("lint", "main"), ("lint ran on: main".to_string(), TOOL_MS));
}
