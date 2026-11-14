//! The reading of a return, on a small flow laid out one thing to a file: an agent cell, `write`, run by what people
//! ask for (`on_request`) and by the owner speaking to it (`on_spoken`), which writes the plan, a choice `ok` it hands
//! in every time, and what the owner and the cells say (`discussion`) through the field `reply`, and may ask for one
//! tool, `query`; and the merge after it, which `ok` runs, so that the work does not end with a run of `write`. What is written beside the flows is in the words of the flow crate's tests.

mod adversarial;
mod checks;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use super::*;

const FLOW: &str = r#"
name = "returning"
order = ["write", "merge"]
data = ["request", "discussion"]
tools = ["query"]

[limits]
runs = 3
returns = 2
calls = 40
"#;

const WRITE: &str = r#"
title = "Write"
kind = "agent"
effort = "high"
line = "engineer"
task = "Write the plan from the request."
triggered_by = ["on_request", "on_spoken"]
context = ["request", "discussion"]
outputs = ["plan", "ok", "discussion"]
must_write = ["ok"]

[kinds]
request = "latest"
discussion = "all"
"#;

const MERGE: &str = r#"
title = "Merge"
kind = "engine"
act = "merge"
triggered_by = ["on_ok"]
"#;

/// The small flow, compiled against what it names.
pub(crate) fn flow() -> Compiled {
    let files = [
        ("lines/engineer.toml", ""),
        ("blocks/plan.toml", "shown = true\n"),
        ("blocks/ok.toml", "values = [\"yes\", \"no\"]\nshown = true\n"),
        ("../data/request.toml", "from = \"owner\"\n"),
        (
            "../data/discussion.toml",
            "from = \"owner\"\nshown = true\nfield = \"reply\"\n",
        ),
        (
            "../triggers/of_a_datum/changed.toml",
            "name = \"on_{datum}\"\nread = [\"{datum}\"]\n",
        ),
        (
            "../triggers/of_a_cell/on_spoken.toml",
            "block = \"{cell}_spoken_to\"\nfrom = \"owner\"\nread = [\"discussion\"]\n",
        ),
    ];
    let source = flow::Source {
        laid_out: true,
        flow: FLOW.to_string(),
        cells: BTreeMap::from([
            ("write".to_string(), WRITE.to_string()),
            ("merge".to_string(), MERGE.to_string()),
        ]),
        rules: BTreeMap::new(),
        files: (files.iter())
            .map(|(path, text)| (path.to_string(), text.to_string()))
            .collect(),
    };
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<BTreeSet<_>>();
    let has = flow::Has {
        acts: names(&["merge"]),
        tools: names(&["query"]),
        fields: names(&["reply"]),
        ..Default::default()
    };
    flow::compile(&source, &has).unwrap_or_else(|faults| panic!("the small flow does not compile: {faults:#?}"))
}

/// The cell `write`.
pub(crate) fn cell(flow: &Compiled) -> &CellSpec {
    (flow.cells.iter())
        .find(|c| c.id.as_str() == "write")
        .expect("the cell")
}

/// What came back, read against the cell `write`.
fn read(r: &Value) -> Result<Came, String> {
    let flow = flow();
    came(r, &flow, cell(&flow), &crate::message::tests::words())
}

fn handed(block: &str, content: &str, matters: bool, value: Option<&str>) -> Handed {
    Handed {
        block: block.into(),
        content: content.to_string(),
        matters,
        value: value.map(str::to_string),
    }
}

/// A return that does the cell's work: it dealt with what people asked for, and writes the plan and its choice.
fn work() -> Value {
    json!({"outcome": {
        "dealt_with": ["on_request"],
        "outputs": {"plan": "Greet by name.", "ok": {"value": "yes", "why": "It says how."}},
    }})
}

/// What [`work`] is read as.
fn worked() -> Came {
    Came {
        dealt_with: Some(vec!["request".into()]),
        writes: vec![
            handed("plan", "Greet by name.", true, None),
            handed("ok", "It says how.", true, Some("yes")),
        ],
        ..Came::default()
    }
}

#[test]
fn the_cell_s_triggers_and_tools_are_those_of_the_flow() {
    let flow = flow();
    let write = cell(&flow);
    let triggers: Vec<(&str, &str)> = (write.triggers.iter())
        .map(|t| (t.name.as_str(), t.block.as_str()))
        .collect();
    assert_eq!(triggers, [("on_request", "request"), ("on_spoken", "write_spoken_to")]);
    assert_eq!(write.tools, ["query"]);
    assert_eq!(write.must_write, [BlockId::from("ok")]);
}

#[test]
fn a_return_of_the_work_is_read_its_triggers_as_their_blocks_a_choice_s_why_as_its_content() {
    assert_eq!(read(&work()), Ok(worked()));
}

#[test]
fn a_block_named_as_written_with_its_meaning_unchanged_runs_no_cell() {
    let mut r = work();
    r["outcome"]["meaning_unchanged"] = json!(["plan"]);
    let mut expected = worked();
    expected.writes[0].matters = false;
    assert_eq!(read(&r), Ok(expected));
}

#[test]
fn what_a_return_says_beside_its_work_is_kept() {
    let mut r = work();
    r["outcome"]["reply"] = json!("Yes, by first name.");
    r["outcome"]["observations"] = json!(["The greeting has no test."]);
    r["outcome"]["questions"] = json!([
        {"name": "which_name", "waits": true, "recommends": 1, "ask": "Which name?",
            "options": [{"does": "first name", "follows": "warmer"}]},
        {"name": "old", "withdrawn": "Settled by the request."},
    ]);
    let said = read(&r).expect("it is read").said;
    assert_eq!(
        said,
        Said {
            questions: vec![
                Question::Asked {
                    name: "which_name".to_string(),
                    waits: true,
                    recommends: 1,
                    ask: "Which name?".to_string(),
                    options: vec![Way {
                        does: "first name".to_string(),
                        follows: "warmer".to_string(),
                    }],
                },
                Question::Withdrawn {
                    name: "old".to_string(),
                    why: "Settled by the request.".to_string(),
                },
            ],
            observations: vec!["The greeting has no test.".to_string()],
            reply: Some("Yes, by first name.".to_string()),
        }
    );
    assert!(said.waits());
}

#[test]
fn a_question_that_waits_is_put_to_the_owner_in_the_words_of_the_files_and_one_that_does_not_is_only_kept() {
    let mut r = work();
    r["outcome"]["questions"] = json!([
        {"name": "which_name", "waits": true, "recommends": 2, "ask": "Which name? {options}",
            "options": [{"does": "the first", "follows": "warmer {ask}"}, {"does": "the full", "follows": "formal"}]},
        {"name": "later", "waits": false, "recommends": 1, "ask": "Which colour?",
            "options": [{"does": "blue", "follows": "calm"}]},
    ]);
    let came = read(&r).expect("it is read");
    // What the agent wrote is put in as it is, nothing in it filled.
    let question = "Which name? {options}\n1. the first, warmer {ask}\n2. the full, formal (yes)".to_string();
    assert_eq!(
        came.asks,
        [Ask::Person {
            who: Who::Owner,
            question
        }]
    );
    assert_eq!(came.writes, worked().writes, "it does the cell's work beside");
    assert_eq!(came.said.questions.len(), 2);
    assert!(came.found.is_empty(), "{:?}", came.found);
}

#[test]
fn a_return_of_both_kinds_is_found_so_whatever_each_holds() {
    let r = json!({"outcome": {"observations": ["x"]}, "calls": {}});
    assert_eq!(read(&r).expect("it is read").found, [Unmet::AsksAndWorks]);
}

#[test]
fn why_what_came_back_cannot_be_read_is_said_in_the_words_of_the_files() {
    let why = read(&json!("does the work")).unwrap_err();
    assert!(why.starts_with("Not of its form: "), "{why}");
    let why = read(&json!({"calls": {"query": {"sql": "select 1"}}})).unwrap_err();
    assert_eq!(why, "`query` has no list");
}

#[test]
fn a_return_that_asks_for_calls_asks_one_call_for_each_request_and_deals_with_nothing() {
    let r = json!({"calls": {"query": [{"sql": "select 1"}, {"sql": "select 2"}]}});
    let asks = vec![
        Ask::Tool {
            name: "query".to_string(),
            input: r#"{"sql":"select 1"}"#.to_string(),
        },
        Ask::Tool {
            name: "query".to_string(),
            input: r#"{"sql":"select 2"}"#.to_string(),
        },
    ];
    assert_eq!(read(&r), Ok(Came::asks(asks)));
}

#[test]
fn what_only_the_reading_can_see_wrong_is_found_in_the_order_of_the_return() {
    let r = json!({
        "outcome": {
            "dealt_with": ["on_request", "on_nothing"],
            "meaning_unchanged": ["ghost"],
            "outputs": {
                "plan": {"value": "yes", "why": "no"},
                "ok": 3,
                "discussion": "Hello.",
                "nowhere": "x",
            },
        },
        "calls": {"browse": [{"url": "x"}]},
    });
    let came = read(&r).expect("it is read");
    assert_eq!(
        came.found,
        [
            Unmet::NotATrigger {
                name: "on_nothing".to_string()
            },
            Unmet::NoValue { block: "ok".into() },
            Unmet::NotAnOutput {
                block: "discussion".into()
            },
            Unmet::NotAnOutput {
                block: "nowhere".into()
            },
            Unmet::UnchangedNotWritten { block: "ghost".into() },
            Unmet::NotATool {
                name: "browse".to_string()
            },
            Unmet::AsksAndWorks,
        ]
    );
    // A block that is no choice written as one is handed in so, for the run's check to find.
    assert_eq!(came.writes, [handed("plan", "no", true, Some("yes"))]);
    assert_eq!(came.dealt_with, Some(vec!["request".into()]));
}

#[test]
fn a_name_dealt_with_twice_stands_twice_for_the_run_s_check_to_find() {
    let r = json!({"outcome": {"dealt_with": ["on_request", "on_request"]}});
    let came = read(&r).expect("it is read");
    assert_eq!(came.dealt_with, Some(vec!["request".into(), "request".into()]));
}

#[test]
fn a_form_written_as_a_string_or_wrapped_in_one_key_is_read() {
    let text = work().to_string();
    for r in [
        json!(text),
        json!(format!("{text}}}")),
        json!(format!("{text}\n]}}")),
        json!({"return": work()}),
        json!({"return": text}),
        json!({"outcome": work()["outcome"].to_string()}),
    ] {
        assert_eq!(read(&r), Ok(worked()), "{r}");
    }
    assert!(read(&json!(format!("{text} and more"))).is_err());
    assert!(read(&json!({"return": work(), "more": 1})).is_err());
}

#[test]
fn a_list_or_the_blocks_left_out_null_or_written_as_a_string_is_read() {
    let r = json!({"outcome": {
        "dealt_with": "[\"on_request\"]",
        "meaning_unchanged": null,
        "questions": null,
        "reply": null,
        "outputs": work()["outcome"]["outputs"].to_string(),
    }});
    assert_eq!(read(&r), Ok(worked()));
    let nothing = Came {
        dealt_with: Some(Vec::new()),
        ..Came::default()
    };
    assert_eq!(read(&json!({})), Ok(nothing.clone()));
    assert_eq!(read(&json!({"outcome": {}})), Ok(nothing.clone()));
    assert_eq!(read(&json!({"outcome": null})), Ok(nothing.clone()));
    assert_eq!(read(&json!({"calls": {"query": null}})), Ok(nothing));
}

#[test]
fn a_null_of_the_other_shape_of_a_question_is_left_out() {
    let r = json!({"outcome": {"questions": [
        {"name": "q", "waits": false, "recommends": 1, "ask": "Which?", "options": [], "withdrawn": null},
        {"name": "p", "withdrawn": "No longer.", "waits": null, "ask": null},
    ]}});
    let questions = read(&r).expect("it is read").said.questions;
    assert!(matches!(&questions[0], Question::Asked { name, .. } if name == "q"));
    assert!(matches!(&questions[1], Question::Withdrawn { name, .. } if name == "p"));
}

#[test]
fn what_an_item_requires_is_never_made_up() {
    for r in [
        json!({"outcome": {"questions": [{"name": "q", "waits": false, "recommends": 1, "options": []}]}}),
        json!({"outcome": {"questions": [{"name": "q", "waits": null, "recommends": 1, "ask": "Which?", "options": []}]}}),
        json!({"outcome": {"questions": [{"name": null, "withdrawn": "No longer."}]}}),
        json!({"outcome": {"questions": [{"name": "q", "waits": false, "recommends": 1, "ask": "Which?",
            "options": [{"does": "this"}]}]}}),
    ] {
        assert!(read(&r).is_err(), "{r} is taken");
    }
}

#[test]
fn a_return_in_its_form_is_read_as_it_was() {
    let r = json!({"outcome": {
        "dealt_with": ["on_request"],
        "meaning_unchanged": [],
        "questions": [],
        "observations": [],
        "reply": "Yes.",
        "outputs": {"plan": "Greet."},
    }});
    assert_eq!(normalised(&r), r);
    let calls = json!({"calls": {"query": [{"sql": "select 1"}]}});
    assert_eq!(normalised(&calls), calls);
}

#[test]
fn a_return_not_of_its_form_is_not_read() {
    for r in [
        json!({"outcome": {}, "more": 1}),
        json!({"outcome": {"dealt_with": [], "more": 1}}),
        json!({"outcome": {"dealt_with": [1]}}),
        json!({"outcome": {"outputs": ["plan"]}}),
        json!({"calls": {"query": {"sql": "select 1"}}}),
        json!({"calls": []}),
        json!("does the work"),
    ] {
        assert!(read(&r).is_err(), "{r} is taken");
    }
}
