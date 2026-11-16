//! The reading of a return, on a small flow laid out one thing to a file: an agent cell, `write`, which watches what
//! people ask for (`request`), its own words with the owner (`write_words`) and its own questions (`write_questions`),
//! and writes the plan, a choice `ok` it hands in every time, its words and its questions, and may ask for one tool,
//! `query`; and the merge after it, which `ok` runs, so that the work does not end with a run of `write`. What is
//! written beside the flows is in the words of the flow crate's tests.

mod adversarial;
mod checks;
mod groups;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use super::*;

const FLOW: &str = r#"
name = "returning"
order = ["write", "merge"]
data = ["request"]
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
watches = ["request", "write_questions", "write_words"]
outputs = ["plan", "ok", "write_words", "write_questions"]
must_write = ["ok"]

[kinds]
request = "latest"
write_questions = "all"
write_words = "all"
"#;

const MERGE: &str = r#"
title = "Merge"
kind = "engine"
act = "merge"
watches = ["ok"]
"#;

/// The small flow, compiled against what it names.
pub(crate) fn flow() -> Compiled {
    compiled(FLOW, WRITE, &[])
}

/// A small flow of these definitions of the flow and of `write`, with the merge after it, the files of the small flow
/// and these beside them, compiled against what the small flow names.
pub(crate) fn compiled(flow: &str, write: &str, more: &[(&str, &str)]) -> Compiled {
    let files = [
        ("lines/engineer.toml", ""),
        ("blocks/plan.toml", "shown = true\n"),
        ("blocks/ok.toml", "values = [\"yes\", \"no\"]\nshown = true\n"),
        ("../data/request.toml", "from = \"owner\"\n"),
        (
            "../data/of_a_cell/words.toml",
            "name = \"{cell}_words\"\nfrom = \"words\"\nshown = true\n\n[writes]\nowner = \"The owner:\\n\\n{words}\"\nreply = \"An agent:\\n\\n{reply}\"\n",
        ),
        (
            "../data/of_a_cell/questions.toml",
            "name = \"{cell}_questions\"\nfrom = \"answer\"\nshown = true\n\n[shape]\ntype = \"array\"\n",
        ),
        (
            "../agents/waits.toml",
            include_str!("../../../../flow/agents/waits.toml"),
        ),
    ];
    let source = flow::Source {
        laid_out: true,
        flow: flow.to_string(),
        cells: BTreeMap::from([
            ("write".to_string(), write.to_string()),
            ("merge".to_string(), MERGE.to_string()),
        ]),
        rules: BTreeMap::new(),
        files: (files.iter().chain(more))
            .map(|(path, text)| (path.to_string(), text.to_string()))
            .collect(),
    };
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<BTreeSet<_>>();
    let has = flow::Has {
        sources: names(&["answer", "words"]),
        acts: names(&["merge"]),
        tools: names(&["query"]),
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

/// What is written beside the flows, as the flow crate's tests have it.
pub(crate) fn words() -> flow::agents::Agents {
    let beside = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../flow/tests/laid_out");
    flow::agents::read(&beside).expect("the words beside the flows of the tests are read")
}

/// What came back, read against the cell `write`.
fn read(r: &Value) -> Result<Came, String> {
    let flow = flow();
    came(r, &flow, cell(&flow), &words()).map(|reading| reading.came)
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
        "dealt_with": ["request"],
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
fn what_the_cell_watches_and_its_tools_are_those_of_the_flow() {
    let flow = flow();
    let write = cell(&flow);
    let watches: Vec<&str> = write.watches.iter().map(|w| w.name.as_str()).collect();
    assert_eq!(watches, ["request", "write_questions", "write_words"]);
    assert_eq!(write.tools, ["query"]);
    assert_eq!(write.must_write, [BlockId::from("ok")]);
}

#[test]
fn a_return_of_the_work_is_read_what_it_dealt_with_as_its_blocks_a_choice_s_why_as_its_content() {
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
fn a_return_that_deals_with_the_owners_words_and_gives_no_answer_to_them_is_found_wanting() {
    let flow = flow();
    let found = |r: &Value| came(r, &flow, cell(&flow), &words()).expect("it is read").came.found;
    let mut r = work();
    r["outcome"]["dealt_with"] = json!(["request", "write_words"]);
    let unanswered = vec![Unmet::NotAnswered {
        name: "write_words".to_string(),
    }];
    assert_eq!(found(&r), unanswered);
    // Words that are only space answer nothing; words in the cell's own words do, and nowhere else.
    r["outcome"]["outputs"]["write_words"] = json!("  ");
    assert_eq!(found(&r), unanswered);
    r["outcome"]["outputs"]["write_words"] = json!("Done as you said.");
    assert_eq!(found(&r), []);
    let mut elsewhere = work();
    elsewhere["outcome"]["dealt_with"] = json!(["request", "write_words"]);
    elsewhere["outcome"]["outputs"]["plan"] = json!("Done as you said.");
    assert_eq!(found(&elsewhere), unanswered);
    // A return that answers what nobody said to it is not found wanting, nor one that deals with something else.
    let mut r = work();
    r["outcome"]["outputs"]["write_words"] = json!("Nobody asked.");
    assert_eq!(found(&r), []);
    assert_eq!(found(&work()), []);
}

// The answer to the owner is handed in as any block, in the cell's own words, and its words are kept as written; the
// engine makes them the cell's answer in the words of their file as its run finishes (crate `engine`, `spoken::entry`).
#[test]
fn an_answer_to_the_owner_is_a_block_handed_in_under_the_record_s_name() {
    let mut r = work();
    r["outcome"]["dealt_with"] = json!(["request"]);
    r["outcome"]["outputs"]["write_words"] = json!("By first name.");
    let mut expected = worked();
    expected
        .writes
        .push(handed("write_words", "By first name.", true, None));
    assert_eq!(read(&r), Ok(expected));
}

// What a cell may write is its outputs, and nothing else: a block that is not among them is sent back, a cell's
// questions and its words as any other, and none of it is taken, no question kept nor answer written.
#[test]
fn a_block_that_is_not_among_the_cell_s_outputs_is_found_the_questions_and_the_record_alike() {
    // The cell writes neither: both are still the flow's, written by their sources, and given to it.
    let narrow = compiled(
        FLOW,
        &WRITE.replace(
            r#"outputs = ["plan", "ok", "write_words", "write_questions"]"#,
            r#"outputs = ["plan", "ok"]"#,
        ),
        &[],
    );
    let mut r = work();
    r["outcome"]["outputs"]["write_words"] = json!("By first name.");
    r["outcome"]["outputs"]["write_questions"] = json!([
        {"name": "which", "waits": true, "recommends": 1, "ask": "Which name?",
            "options": [{"does": "first name", "follows": "warmer"}]},
    ]);
    r["outcome"]["outputs"]["nowhere"] = json!("x");
    let came = came(&r, &narrow, cell(&narrow), &words()).expect("it is read").came;
    let not = |block: &str| Unmet::NotAnOutput { block: block.into() };
    assert_eq!(came.found, [not("nowhere"), not("write_questions"), not("write_words")]);
    assert_eq!(came.writes, worked().writes);
    assert!(came.said.questions.is_empty(), "{:?}", came.said);
    // The cell that writes them takes them.
    let came = read(&r).expect("it is read");
    assert_eq!(came.found, [not("nowhere")]);
    assert_eq!(came.said.questions.len(), 1);
    assert!(came.writes.iter().any(|h| h.block.as_str() == "write_words"));
}

// The work's questions are a list of questions: written as anything else, as a text, the return is not of its form.
#[test]
fn the_work_s_questions_not_written_as_a_list_of_questions_are_not_of_the_form() {
    for questions in [json!("None."), json!({"name": "q"}), json!([{"name": "q"}])] {
        let mut r = work();
        r["outcome"]["outputs"]["write_questions"] = questions.clone();
        assert!(read(&r).is_err(), "{questions} is taken");
    }
    let mut r = work();
    r["outcome"]["outputs"]["write_questions"] = json!([]);
    assert_eq!(read(&r), Ok(worked()), "none asked");
}

#[test]
fn what_a_return_says_beside_its_work_is_kept() {
    let mut r = work();
    r["outcome"]["observations"] = json!(["The greeting has no test."]);
    r["outcome"]["outputs"]["write_questions"] = json!([
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
                    recommends: Some(1),
                    ask: "Which name?".to_string(),
                    options: vec![Way {
                        does: "first name".to_string(),
                        follows: "warmer".to_string(),
                    }],
                    about: None,
                    reasons: BTreeMap::new(),
                },
                Question::Withdrawn {
                    name: "old".to_string(),
                    why: "Settled by the request.".to_string(),
                },
            ],
            observations: vec!["The greeting has no test.".to_string()],
        }
    );
    assert!(said.waits());
}

#[test]
fn a_question_is_kept_as_the_agent_wrote_it_and_asks_for_no_call_whether_or_not_it_waits() {
    let mut r = work();
    r["outcome"]["outputs"]["write_questions"] = json!([
        {"name": "which_name", "waits": true, "recommends": 2, "ask": "Which name? {options}",
            "options": [{"does": "the first", "follows": "warmer {ask}"}, {"does": "the full", "follows": "formal"}]},
        {"name": "later", "waits": false, "recommends": 1, "ask": "Which colour?",
            "options": [{"does": "blue", "follows": "calm"}]},
    ]);
    let came = read(&r).expect("it is read");
    assert_eq!(came.asks, [], "a question is no call");
    assert_eq!(came.writes, worked().writes, "it does the cell's work beside");
    let Question::Asked { ask, recommends, .. } = &came.said.questions[0] else {
        panic!("the question is asked: {:?}", came.said.questions);
    };
    assert_eq!((ask.as_str(), *recommends), ("Which name? {options}", Some(2)));
    assert_eq!(came.said.questions.len(), 2);
    assert!(came.found.is_empty(), "{:?}", came.found);
}

#[test]
fn a_question_that_recommends_none_of_its_options_or_has_the_name_of_the_engine_s_own_is_found() {
    let mut r = work();
    let option = json!([{"does": "blue", "follows": "calm"}]);
    r["outcome"]["outputs"]["write_questions"] = json!([
        {"name": "none", "waits": false, "recommends": 0, "ask": "Which?", "options": option},
        {"name": "past", "waits": false, "recommends": 2, "ask": "Which?", "options": option},
        {"name": "empty", "waits": true, "recommends": 1, "ask": "Which?", "options": []},
        {"name": "protected_files", "waits": false, "recommends": 1, "ask": "Which?", "options": option},
        {"name": "protected_files", "withdrawn": "Settled."},
    ]);
    let found = read(&r).expect("it is read").found;
    let named = |name: &str| name.to_string();
    assert_eq!(
        found,
        [
            Unmet::NotAnOption { name: named("none") },
            Unmet::NotAnOption { name: named("past") },
            Unmet::NotAnOption { name: named("empty") },
            Unmet::QuestionTaken {
                name: named("protected_files")
            },
            Unmet::QuestionTaken {
                name: named("protected_files")
            },
        ]
    );
}

// Why a return says the work changes each file the project protects is taken from the return that does the cell's
// work, as it wrote it, for the engine's look at the branch; a return that only asks for tools cannot say it.
#[test]
fn why_a_return_changes_each_protected_file_is_read_for_the_look_at_the_branch() {
    let flow = flow();
    let reading = |r: &Value| came(r, &flow, cell(&flow), &words()).expect("it is read");
    let mut r = work();
    r["outcome"]["protected_changes"] = json!({"AGENTS.md": "The guide must say it.", "flow/cell.toml": " Needed. "});
    let said = reading(&r);
    let expected = BTreeMap::from([
        ("AGENTS.md".to_string(), "The guide must say it.".to_string()),
        ("flow/cell.toml".to_string(), " Needed. ".to_string()),
    ]);
    assert_eq!(said.protected.as_ref(), Some(&expected));
    assert_eq!(said.came, worked(), "it changes nothing else of the reading");
    // Left out, null, or written as a string that holds it.
    assert_eq!(reading(&work()).protected, Some(BTreeMap::new()));
    r["outcome"]["protected_changes"] = Value::Null;
    assert_eq!(reading(&r).protected, Some(BTreeMap::new()));
    r["outcome"]["protected_changes"] = json!(r#"{"AGENTS.md": "The guide must say it."}"#);
    assert_eq!(
        reading(&r).protected,
        Some(BTreeMap::from([expected.into_iter().next().unwrap()]))
    );
    // A reason that is no text is not of the form.
    r["outcome"]["protected_changes"] = json!({"AGENTS.md": 3});
    assert!(came(&r, &flow, cell(&flow), &words()).is_err());
    let tools = json!({"tools": {"query": [{"sql": "select 1"}]}});
    assert_eq!(reading(&tools).protected, None);
}

#[test]
fn a_return_of_both_kinds_is_found_so_whatever_each_holds() {
    let r = json!({"outcome": {"observations": ["x"]}, "tools": {}});
    assert_eq!(read(&r).expect("it is read").found, [Unmet::AsksAndWorks]);
}

#[test]
fn why_what_came_back_cannot_be_read_is_said_in_the_words_of_the_files() {
    let why = read(&json!("does the work")).unwrap_err();
    assert!(why.starts_with("Not of its form: "), "{why}");
    let why = read(&json!({"tools": {"query": {"sql": "select 1"}}})).unwrap_err();
    assert_eq!(why, "`query` has no list");
}

#[test]
fn a_return_that_asks_for_tools_asks_one_call_for_each_request_and_deals_with_nothing() {
    let r = json!({"tools": {"query": [{"sql": "select 1"}, {"sql": "select 2"}]}});
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
            "dealt_with": ["request", "on_nothing"],
            "meaning_unchanged": ["ghost"],
            "outputs": {
                "plan": {"value": "yes", "why": "no"},
                "ok": 3,
                "nowhere": "x",
            },
        },
        "tools": {"browse": [{"url": "x"}]},
    });
    let came = read(&r).expect("it is read");
    assert_eq!(
        came.found,
        [
            Unmet::NotWatched {
                name: "on_nothing".to_string()
            },
            Unmet::NoValue { block: "ok".into() },
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
    let r = json!({"outcome": {"dealt_with": ["request", "request"]}});
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
        "dealt_with": "[\"request\"]",
        "meaning_unchanged": null,
        "observations": null,
        "outputs": work()["outcome"]["outputs"].to_string(),
    }});
    assert_eq!(read(&r), Ok(worked()));
    // So are the work's questions, among the blocks.
    for questions in [json!(null), json!("[]")] {
        let mut r = work();
        r["outcome"]["outputs"]["write_questions"] = questions;
        assert_eq!(read(&r), Ok(worked()));
    }
    let nothing = Came {
        dealt_with: Some(Vec::new()),
        ..Came::default()
    };
    assert_eq!(read(&json!({})), Ok(nothing.clone()));
    assert_eq!(read(&json!({"outcome": {}})), Ok(nothing.clone()));
    assert_eq!(read(&json!({"outcome": null})), Ok(nothing.clone()));
    assert_eq!(read(&json!({"tools": {"query": null}})), Ok(nothing));
}

#[test]
fn a_null_of_the_other_shape_of_a_question_is_left_out() {
    let r = json!({"outcome": {"outputs": {"write_questions": [
        {"name": "q", "waits": false, "recommends": 1, "ask": "Which?", "options": [], "withdrawn": null},
        {"name": "p", "withdrawn": "No longer.", "waits": null, "ask": null},
    ]}}});
    let questions = read(&r).expect("it is read").said.questions;
    assert!(matches!(&questions[0], Question::Asked { name, .. } if name == "q"));
    assert!(matches!(&questions[1], Question::Withdrawn { name, .. } if name == "p"));
}

#[test]
fn what_an_item_requires_is_never_made_up() {
    for r in [
        json!({"outcome": {"outputs": {"write_questions": [{"name": "q", "waits": false, "recommends": 1, "options": []}]}}}),
        json!({"outcome": {"outputs": {"write_questions": [{"name": "q", "waits": null, "recommends": 1, "ask": "Which?",
            "options": []}]}}}),
        json!({"outcome": {"outputs": {"write_questions": [{"name": null, "withdrawn": "No longer."}]}}}),
        json!({"outcome": {"outputs": {"write_questions": [{"name": "q", "waits": false, "recommends": 1, "ask": "Which?",
            "options": [{"does": "this"}]}]}}}),
    ] {
        assert!(read(&r).is_err(), "{r} is taken");
    }
}

#[test]
fn a_return_in_its_form_is_read_as_it_was() {
    let r = json!({"outcome": {
        "dealt_with": ["request"],
        "meaning_unchanged": [],
        "observations": [],
        "protected_changes": {"AGENTS.md": "The guide must say it."},
        "outputs": {"plan": "Greet.", "write_questions": []},
    }});
    assert_eq!(normalised(&r, &["write_questions"]), r);
    let tools = json!({"tools": {"query": [{"sql": "select 1"}]}});
    assert_eq!(normalised(&tools, &["write_questions"]), tools);
}

#[test]
fn a_return_not_of_its_form_is_not_read() {
    for r in [
        json!({"outcome": {}, "more": 1}),
        json!({"outcome": {"dealt_with": [], "more": 1}}),
        json!({"outcome": {"dealt_with": [1]}}),
        json!({"outcome": {"outputs": ["plan"]}}),
        json!({"tools": {"query": {"sql": "select 1"}}}),
        json!({"tools": []}),
        json!({"outcome": {"write_questions": []}}),
        json!({"outcome": {"reply": "Yes."}}),
        json!("does the work"),
    ] {
        assert!(read(&r).is_err(), "{r} is taken");
    }
}
