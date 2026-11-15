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
    // The program's own part, then the form, given with every call, in its parts: the engine's fields and the blocks
    // of the line, the small flow naming no tool; then the system prompt and the message.
    let head: Vec<(&serde_json::Value, &serde_json::Value)> =
        plan[..3].iter().map(|p| (&p["name"], &p["carried"])).collect();
    assert_eq!(
        head,
        [
            (&json!("program"), &json!("program")),
            (&json!("form"), &json!("call")),
            (&json!("outputs"), &json!("call")),
        ]
    );
    assert_eq!(plan[3]["carried"], json!("system"));
    assert_eq!(
        plan[1]["source"],
        json!("flow/agents/return/return.toml, flow/agents/return/fields/")
    );
    let task = plan.iter().find(|p| p["what"] == json!("task")).expect("the task");
    assert_eq!(task["carried"], json!("message"));
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
            "dependencies",
            "main",
            "session",
            "start",
            "tree",
            "words",
        ]),
        acts: names(&["start", "check", "merge", "deliver"]),
        tools: names(&["query", "run_tests"]),
        system: names(&["guide", "constitution", "tree_rules", "how_you_work_here"]),
        kept: names(&["questions"]),
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
    // A name the flow defines is not said: each block and each tool of the flow, what a tool's file defines for its
    // result, and the settings the texts name.
    for known in [
        "background",
        "query",
        "rows",
        "cut",
        "count",
        "rest",
        "bytes",
        "trimmed",
        "substr",
        "owner.language",
        "agents.subagents.most",
    ] {
        assert!(!unknown.iter().any(|u| u["name"] == json!(known)), "{known}");
    }
}

#[tokio::test]
async fn a_name_counts_as_defined_only_where_a_tool_s_file_lists_it_in_defines() {
    let dir = copied();
    let file = dir.path().join("agents/tools/query.toml");
    let text = std::fs::read_to_string(&file).unwrap();
    let line = text
        .lines()
        .find(|l| l.starts_with("defines = "))
        .expect("the file lists what it defines");
    let given_of = |dir: &Path| {
        let router = web::router(Flows::dirs(dir.join("engineer"), Box::new(the_engine_s)));
        async move { get_json(&router, "/api/v1/flow/given").await }
    };
    let listed = |given: &serde_json::Value, name: &str| {
        (given["unknown"].as_array().expect("the unknown names").iter())
            .find(|u| u["name"] == json!(name))
            .cloned()
    };
    // Where the file lists none, the names its words use that nothing else defines are said, with the file of the
    // words they are in.
    std::fs::write(&file, text.replace(line, "")).unwrap();
    let given = given_of(dir.path()).await;
    for name in ["cut", "substr"] {
        let said = listed(&given, name).unwrap_or_else(|| panic!("{name} is said"));
        assert!(
            said["source"].as_str().unwrap().ends_with("/agents/return/return.toml"),
            "{said}"
        );
    }
    // A name it lists defines itself and no other.
    std::fs::write(&file, text.replace(line, "defines = [\"cut\"]")).unwrap();
    let given = given_of(dir.path()).await;
    assert!(listed(&given, "cut").is_none());
    assert!(listed(&given, "substr").is_some());
    // The page shows the key as the file writes it.
    let words = given["toolFiles"]["query"]["words"]
        .as_array()
        .expect("the tool's words");
    let shown = words
        .iter()
        .find(|w| w[0] == json!("defines"))
        .expect("the tool's `defines`");
    assert!(shown[1][0]["text"].as_str().unwrap().contains("cut"), "{shown}");
}

#[tokio::test]
async fn each_agent_cell_of_the_repository_s_flow_is_sent_what_its_file_and_the_files_beside_the_flows_say() {
    let router = the_engineer_s_flow();
    let given = get_json(&router, "/api/v1/flow/given").await;
    let definition = get_json(&router, "/api/v1/flow").await;
    assert_eq!(definition["state"], json!("compiled"), "{definition}");
    let flow = &definition["flow"];
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
        // The form's parts: the engine's fields, in the kind that does the cell's work; the cell's own tools, in the
        // kind that asks for them; and among the blocks of its line, each it writes, with its own file and its cell's.
        let part = |name: &str| -> serde_json::Value {
            serde_json::from_str(&text_of(named("form", name))).expect("the part of the form is JSON")
        };
        let fields = part("form");
        let tools = strings(&cell["tools"]);
        assert!(!tools.is_empty(), "{id}: the flow gives its agent cells tools");
        let asked = part("tools")["properties"].as_object().expect("the tools").clone();
        for tool in &tools {
            assert!(asked.contains_key(tool), "{id}: the form lacks the tool {tool}");
        }
        let outcome = fields["properties"]["outcome"]["properties"]
            .as_object()
            .expect("the outcome");
        assert!(outcome.contains_key("outputs"), "{id}: the outcome holds the blocks");
        let blocks = part("outputs")["properties"].as_object().expect("the blocks").clone();
        let files = named("form", "outputs")["source"].as_str().unwrap().to_string();
        assert!(
            files.contains(&format!("flow/engineer/cells/{id}.toml")),
            "{id}: {files}"
        );
        for block in strings(&cell["outputs"]) {
            assert!(blocks.contains_key(&block), "{id}: the form lacks the block {block}");
        }
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
        for block in strings(&cell["outputs"]).iter() {
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
async fn the_form_each_cell_is_shown_is_the_one_the_engine_gives_the_program_for_its_line() {
    let given = get_json(&beside_the_repository_s_flows(), "/api/v1/flow/given").await;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    let beside = flow::agents::read(&root).expect("what is written beside the flows is read");
    let compiled = flow::compile(&tiny(), &has(&tiny())).expect("the small flow compiles");
    for (cell, parts) in given["cells"].as_object().expect("the cells") {
        let line = (compiled.cells.iter())
            .find(|c| c.id.as_str() == cell)
            .and_then(|c| c.line.clone())
            .expect("an agent's cell has a line");
        let made = flow::form::parts(&compiled, &beside, &line).expect("the form is made");
        let shown: Vec<(&serde_json::Value, &serde_json::Value)> = (parts.as_array().expect("its parts").iter())
            .filter(|p| p["what"] == json!("form"))
            .map(|p| (&p["name"], &p["pieces"]))
            .collect();
        let text = |text: &str| json!([{ "piece": "text", "text": text }]);
        let blocks = made.blocks.as_deref().expect("the form has outputs");
        assert_eq!(
            shown,
            [
                (&json!("form"), &text(&made.fields)),
                (&json!("outputs"), &text(blocks)),
            ],
            "{cell}"
        );
        assert_eq!(made.tools, None, "the small flow names no tool");
    }
}
