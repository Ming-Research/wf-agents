//! `GET /api/v1/flow/given`: what an agent is sent as the engine enters each agent's cell of the definition shown, part
//! by part, read from the definition and from what the engine says to every agent beside the flows.

use std::path::Path;

use serde_json::json;
use web::Flows;
use web::given::Alike;

use crate::{Shared, get_json, has, serve, tiny};

/// The interface over the small flow, with what every agent is sent read from the repository's own files beside its
/// flows.
fn beside_the_repository_s_flows() -> axum::Router {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    web::router(Flows {
        shown: Shared::new(Ok(tiny())).read(),
        has: Box::new(has),
        alike: Alike {
            root: Some(root),
            flow: None,
            project: None,
        },
    })
}

#[tokio::test]
async fn where_the_page_was_not_told_where_the_flows_are_nothing_is_read() {
    let router = serve(&Shared::new(Ok(tiny())));
    assert_eq!(get_json(&router, "/api/v1/flow/given").await["state"], json!("unread"));
}

#[tokio::test]
async fn each_agent_s_cell_is_given_its_parts_in_the_order_sent_and_each_act_what_it_writes() {
    let given = get_json(&beside_the_repository_s_flows(), "/api/v1/flow/given").await;
    assert_eq!(given["state"], json!("compiled"), "{given}");
    let cells = given["cells"].as_object().expect("the cells");
    assert_eq!(cells.keys().collect::<Vec<_>>(), ["plan", "review"]);
    let plan = given["cells"]["plan"].as_array().expect("the plan's parts");
    // The program's own part, then the form, given with every call, then the system prompt and the message.
    assert_eq!(
        (
            &plan[0]["name"],
            &plan[0]["carried"],
            &plan[1]["name"],
            &plan[1]["carried"]
        ),
        (&json!("program"), &json!("program"), &json!("form"), &json!("call"))
    );
    let task = plan.iter().find(|p| p["what"] == json!("task")).expect("the task");
    assert_eq!(task["carried"], json!("message"));
    assert_eq!(given["tools"], json!(["query", "run_tests"]));
    // The merge is an act of the engine's: what it writes is read from its file beside the flows, key by key.
    assert_eq!(given["acts"]["merge"]["source"], json!("flow/agents/acts/merge.toml"));
    assert_eq!(given["acts"]["merge"]["words"][0][0], json!("text"));
}

/// What the engine has, by name, for the repository's own flows, as `wf-agents` compiles them
/// (`crates/wf-agents/src/main.rs`).
fn the_engine_s(_: &flow::Source) -> flow::Has {
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    flow::Has {
        sources: names(&[
            "answer",
            "asked",
            "words",
            "dependencies",
            "main",
            "session",
            "start",
            "tree",
        ]),
        acts: names(&["start", "check", "merge", "deliver"]),
        tools: names(&["query", "run_tests"]),
        system: names(&["guide", "constitution", "tree_rules", "how_you_work_here"]),
        fields: names(&[
            "dealt_with",
            "meaning_unchanged",
            "observations",
            "questions",
            "reply",
            "summary",
        ]),
        details: names(&["files"]),
    }
}

/// The interface over the engineer's flow as the repository defines it, served as `wf-agents flow page` serves it.
fn the_engineer_s_flow() -> axum::Router {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
    web::router(Flows::dirs(dir, Box::new(the_engine_s)))
}

/// The text of a part's pieces, a place to fill in a run kept as `{name}`.
fn text_of(part: &serde_json::Value) -> String {
    let pieces = part["pieces"].as_array().expect("the part's pieces");
    (pieces.iter())
        .map(|p| match p["piece"].as_str() {
            Some("slot") => format!("{{{}}}", p["name"].as_str().unwrap()),
            _ => p["text"].as_str().unwrap().to_string(),
        })
        .collect()
}

/// The names in a JSON list.
fn strings(list: &serde_json::Value) -> Vec<String> {
    let items = list.as_array().expect("a list");
    items.iter().map(|v| v.as_str().unwrap().to_string()).collect()
}

#[tokio::test]
async fn the_page_served_alone_reads_what_an_agent_is_sent_from_beside_the_flow_it_shows() {
    let given = get_json(&the_engineer_s_flow(), "/api/v1/flow/given").await;
    assert_eq!(given["state"], json!("compiled"), "{given}");
}

/// The repository's flows and what is beside them, copied into a directory of the test's own.
fn copied() -> tempfile::TempDir {
    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let path = entry.unwrap().path();
            let to = to.join(path.file_name().unwrap());
            if path.is_dir() {
                copy(&path, &to);
            } else {
                std::fs::copy(&path, &to).unwrap();
            }
        }
    }
    let dir = tempfile::TempDir::new().unwrap();
    copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow"), dir.path());
    dir
}

#[tokio::test]
async fn a_name_a_text_sent_gives_between_backticks_that_nothing_defines_is_said_with_its_file() {
    let dir = copied();
    let rules = dir.path().join("agents/rules/general.md");
    let text = std::fs::read_to_string(&rules).unwrap();
    std::fs::write(&rules, format!("{text}\nMind `no_such_thing`.\n")).unwrap();
    let router = web::router(Flows::dirs(dir.path().join("engineer"), Box::new(the_engine_s)));
    let given = get_json(&router, "/api/v1/flow/given").await;
    let unknown = given["unknown"].as_array().expect("the unknown names");
    let said = (unknown.iter())
        .find(|u| u["name"] == json!("no_such_thing"))
        .expect("the name is said");
    let source = said["source"].as_str().unwrap();
    assert!(source.ends_with("/agents/rules/general.md"), "{source}");
    // A name the flow defines is not said: each block and each tool of the flow.
    for known in ["background", "query"] {
        assert!(!unknown.iter().any(|u| u["name"] == json!(known)), "{known}");
    }
}

#[tokio::test]
async fn each_agent_cell_of_the_repository_s_flow_is_sent_what_its_file_and_the_files_beside_the_flows_say() {
    let router = the_engineer_s_flow();
    let given = get_json(&router, "/api/v1/flow/given").await;
    let definition = get_json(&router, "/api/v1/flow").await;
    assert_eq!(definition["state"], json!("compiled"), "{definition}");
    let flow = &definition["flow"];
    let blocks = flow["blocks"].as_array().expect("the blocks");
    let field_of = |block: &str| {
        let found = blocks.iter().find(|b| b["name"] == json!(block));
        found.is_some_and(|b| !b["field"].is_null())
    };
    let agents: Vec<&serde_json::Value> = (flow["cells"].as_array().unwrap().iter())
        .filter(|c| c["kind"] == json!("agent"))
        .collect();
    assert!(!agents.is_empty());
    for cell in agents {
        let id = cell["id"].as_str().unwrap();
        let parts = given["cells"][id]
            .as_array()
            .unwrap_or_else(|| panic!("the parts of {id}"));
        let named = |what: &str, name: &str| {
            (parts.iter())
                .find(|p| p["what"] == json!(what) && p["name"] == json!(name))
                .unwrap_or_else(|| panic!("{id}: no part {what} {name}"))
        };
        // The form holds the cell's own tools among its calls, and its outcome holds the blocks it writes.
        let form: serde_json::Value = serde_json::from_str(&text_of(named("form", "form"))).expect("the form is JSON");
        let tools = strings(&cell["tools"]);
        assert!(!tools.is_empty(), "{id}: the flow gives its agent cells tools");
        let asked = form["properties"]["calls"]["properties"]
            .as_object()
            .expect("the calls");
        for tool in &tools {
            assert!(asked.contains_key(tool), "{id}: the form lacks the tool {tool}");
        }
        let outcome = form["properties"]["outcome"]["properties"]
            .as_object()
            .expect("the outcome");
        assert!(
            outcome.contains_key("outputs"),
            "{id}: the outcome holds the blocks it writes"
        );
        // How it returns tells each of its tools in the words of the tool's file.
        let told = text_of(named("system", "how_you_return"));
        for tool in &tools {
            let words = given["toolFiles"][tool]["words"].as_array().expect("the tool's words");
            let said = words
                .iter()
                .find(|w| w[0] == json!("words"))
                .expect("the tool's `words`");
            let first = said[1][0]["text"].as_str().unwrap().trim().lines().next().unwrap();
            assert!(told.contains(first), "{id}: how it returns lacks the words of {tool}");
        }
        // What it returns names each block it writes as a block.
        let outputs = (parts.iter())
            .find(|p| p["what"] == json!("outputs"))
            .expect("what it returns");
        let outputs = text_of(outputs);
        for block in strings(&cell["outputs"]).iter().filter(|b| !field_of(b)) {
            assert!(
                outputs.contains(&format!("`{block}`")),
                "{id}: what it returns lacks {block}"
            );
        }
        // What runs it has a line for each trigger its file names, by the trigger's name, which names it.
        let lines = named("message", "triggers")["items"]
            .as_array()
            .expect("the triggers' lines");
        for trigger in cell["triggers"].as_array().unwrap() {
            let name = trigger["name"].as_str().unwrap();
            let line = lines.iter().find(|l| l[0] == json!(name));
            let pieces = line.unwrap_or_else(|| panic!("{id}: what runs it lacks {name}"));
            assert!(text_of(&json!({ "pieces": pieces[1] })).contains(name), "{id}: {name}");
        }
        // Each datum of its context, in the order it names them, in the kind it gives it in.
        let inputs: Vec<&serde_json::Value> = parts.iter().filter(|p| p["what"] == json!("input")).collect();
        let given_blocks = strings(&cell["given"]);
        assert_eq!(
            inputs.len(),
            given_blocks.len(),
            "{id}: one part for each datum it is given"
        );
        for (part, block) in inputs.iter().zip(&given_blocks) {
            assert_eq!(part["block"], json!(block), "{id}");
            let kind = match cell["input"][block]["given"]["kind"].as_str() {
                Some("lastAndLatest") => "last_and_latest",
                Some(kind) => kind,
                None => panic!("{id}: {block} is given in no kind"),
            };
            assert_eq!(part["kind"], json!(kind), "{id}: {block}");
            assert!(!text_of(part).is_empty(), "{id}: {block} is sent no words");
        }
        // Its task, from its own file, or the file it is written in.
        let task = (parts.iter()).find(|p| p["what"] == json!("task")).expect("the task");
        let source = match cell["taskFile"].as_str() {
            Some(file) => format!("flow/engineer/{file}"),
            None => format!("flow/engineer/cells/{id}.toml"),
        };
        assert_eq!(task["source"], json!(source), "{id}");
        assert!(!text_of(task).is_empty(), "{id}: no task is sent");
    }
}

#[tokio::test]
async fn the_form_each_cell_is_shown_is_the_one_the_engine_gives_the_program_for_the_flow() {
    let given = get_json(&beside_the_repository_s_flows(), "/api/v1/flow/given").await;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    let beside = flow::agents::read(&root).expect("what is written beside the flows is read");
    let compiled = flow::compile(&tiny(), &has(&tiny())).expect("the small flow compiles");
    let form = flow::form::form(&compiled, &beside).expect("the form is made");
    for (cell, parts) in given["cells"].as_object().expect("the cells") {
        let parts = parts.as_array().expect("its parts");
        let shown = parts.iter().find(|p| p["name"] == json!("form")).expect("the form");
        assert_eq!(shown["pieces"], json!([{ "piece": "text", "text": form }]), "{cell}");
    }
}
