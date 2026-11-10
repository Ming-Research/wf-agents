//! The script the test carriers follow: how it is read, checked against the flows, and what a round returns by it.

use std::collections::BTreeSet;
use std::path::Path;

use engine::Came;
use flow::{CellSpec, Compiled};
use service::Script;
use service::script::{INPUT_TOKENS, OUTPUT_TOKENS, ROUND_MS, TOOL_MS, USD_MICROS};
use work::{Ask, Handed, Spent};

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
    let planned = Script::default().round(&flow, cell(&flow, "review"), 2, 3);
    // The opinions, which the build before it reads, are a way back, and are not written.
    let verdict = "The verdict as review wrote it in its run 2, round 3.";
    assert_eq!(
        planned.came,
        Came::HandsIn(vec![handed("verdict", verdict, Some("yes"))])
    );
    assert_eq!(planned.ms, ROUND_MS);
    assert_eq!(
        planned.spent,
        Spent {
            usd_micros: USD_MICROS,
            input_tokens: INPUT_TOKENS,
            cached_tokens: 0,
            output_tokens: OUTPUT_TOKENS,
        }
    );
}

#[test]
fn a_choice_is_handed_in_at_the_value_the_script_names_for_it() {
    let flow = trial();
    let script = Script::read("[flows.trial.values]\nverdict = \"no\"\n").unwrap();
    let Came::HandsIn(blocks) = script.round(&flow, cell(&flow, "review"), 1, 1).came else {
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
    let planned = script.round(&flow, cell(&flow, "build"), 2, 1);
    assert_eq!(planned.ms, 50);
    assert_eq!(planned.spent.usd_micros, 40);
    assert_eq!(
        planned.came,
        Came::HandsIn(vec![Handed {
            matters: false,
            ..handed("code", "The code as build wrote it in its run 2, round 1.", None)
        }])
    );
    // The engine's own act takes no cost, whatever the script says of rounds.
    let merge = script.round(&flow, cell(&flow, "merge"), 1, 1);
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
        script.round(&flow, cell(&flow, "plan"), 1, 1).came,
        Came::Asks(vec![
            Ask::Person {
                who: decide::Who::Owner,
                question: "Which name?".to_string()
            },
            Ask::Tool {
                name: "ci".to_string(),
                input: "main".to_string()
            },
        ])
    );
}

#[test]
fn a_tool_answers_what_the_script_says_or_what_it_ran_on() {
    let script = Script::read("[tools.ci]\nanswer = \"Green.\"\nms = 30\n").unwrap();
    assert_eq!(script.tool("ci", "main"), ("Green.".to_string(), 30));
    assert_eq!(script.tool("lint", "main"), ("lint ran on: main".to_string(), TOOL_MS));
}
