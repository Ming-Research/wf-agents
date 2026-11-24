//! `GET /api/v1/settings` for the projects: each project's tables of the installation's file are shown with the project,
//! not among the installation's, beside the project's own declaration as the engine read it from its main as it
//! started, table by table as the declaration writes them; or why the engine holds the project; or, for a project the
//! file names and the engine did not start with, that the engine read none. A project the engine runs and the file
//! leaves out is shown too, as any table the file leaves out is.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Router;
use serde_json::{Value, json};
use service::{Scripted, Service, TokioClock};
use store::Store;
use web::settings::{DeclarationText, Install};

use crate::{START, get_json, trial_flow, trial_has, trial_script};

/// The route of the settings.
const PATH: &str = "/api/v1/settings";

/// The installation's file as it is now: the agents, and three projects, the first hosted on GitHub, the last added
/// after the engine started.
const FILE: &str = r#"[agents]
command = "claude"
slots = 1

[projects.wf-agents]
repo = "/repos/wf-agents"
main = "main"

[projects.wf-agents.github]
repository = "owner/wf-agents"

[projects.notes]
repo = "/repos/notes"
main = "main"

[projects.drafts]
repo = "/repos/drafts"
main = "main"
"#;

/// The declaration of `wf-agents` on its main as the engine read it, with comments, a table with no keys, and a value
/// over lines.
const DECLARATION: &str = r#"# The project's declaration.
tree = "design/"

[files.guide]
path = "AGENTS.md" # its guide

[heavy]
commands = [
  "cargo",
]

[environment]
"#;

/// Why the engine holds `notes`, in its words.
const HELD: &str = "refs/heads/main:wf-agents.toml, the project's declaration on main where works branch from, cannot \
                    be read: fatal: path 'wf-agents.toml' does not exist in 'refs/heads/main'";

/// The stand-in for the engine's reading of the settings: the file's tables as they are.
fn read(text: &str) -> Result<Value, String> {
    let table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
    serde_json::to_value(&table).map_err(|e| e.to_string())
}

/// The settings' routes over [`FILE`], the engine having started on another file: one without `drafts`, and with
/// `old`, which the file has since lost; it read the declarations of `wf-agents` and `old`, and holds `notes`.
fn serving() -> (Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let flow = flow::compile(&trial_flow(), &trial_has(&flow::Source::default())).unwrap();
    let clock = Arc::new(TokioClock::new(work::Time(START)));
    let carriers = Scripted::new(trial_script()).carriers();
    let (service, inbox) = Service::open(Box::new(store), vec![flow], carriers, clock).unwrap();
    let handle = service.handle();
    tokio::spawn(service.run(inbox));
    let file = dir.path().join("install.toml");
    std::fs::write(&file, FILE).unwrap();
    let mut running = read(FILE).unwrap();
    let projects = running["projects"].as_object_mut().unwrap();
    projects.remove("drafts");
    projects.insert("old".to_string(), json!({ "repo": "/repos/old", "main": "main" }));
    let declarations = BTreeMap::from([
        (
            "wf-agents".to_string(),
            Ok(DeclarationText {
                at: "refs/remotes/origin/main:wf-agents.toml".to_string(),
                text: DECLARATION.to_string(),
            }),
        ),
        ("notes".to_string(), Err(HELD.to_string())),
        (
            "old".to_string(),
            Ok(DeclarationText {
                at: "refs/heads/main:wf-agents.toml".to_string(),
                text: "tree = \"docs/\"\n".to_string(),
            }),
        ),
    ]);
    let install = Install {
        file,
        read: Box::new(read),
        running,
        declarations,
        git: &[],
    };
    (web::settings::router(install, handle), dir)
}

/// A key of a table as the page is given it.
fn key(key: &str, written: Option<&str>, running: Option<&str>) -> Value {
    json!({ "key": key, "written": written, "running": running, "change": null })
}

/// A table as the page is given it.
fn table(name: &str, left_out: bool, keys: Vec<Value>) -> Value {
    json!({ "name": name, "leftOut": left_out, "keys": keys })
}

// The installation's own tables hold no project's; each project, in the order of its first table, then the one the
// engine runs that the file leaves out, has its tables, each key beside what the engine runs with where that differs,
// and its declaration: read, with where it was read and its tables as it writes them, its comments left out and a
// value over lines kept so; held, with why in the engine's words; or not read, for the one added since the engine
// started.
#[tokio::test(start_paused = true)]
async fn each_project_is_shown_with_its_tables_of_the_file_and_its_declaration_as_the_engine_holds_it() {
    let (router, _dir) = serving();
    let page = get_json(&router, PATH).await;
    let installation: Vec<&str> = (page["tables"].as_array().expect("the tables").iter())
        .map(|table| table["name"].as_str().expect("a name"))
        .collect();
    assert_eq!(installation, ["agents"]);
    assert_eq!(page["declarationFile"], "wf-agents.toml");
    let repo = |name: &str| format!("\"/repos/{name}\"");
    assert_eq!(
        page["projects"],
        json!([
            {
                "name": "wf-agents",
                "tables": [
                    table("projects.wf-agents", false, vec![
                        key("repo", Some(repo("wf-agents").as_str()), None),
                        key("main", Some("\"main\""), None),
                    ]),
                    table("projects.wf-agents.github", false, vec![
                        key("repository", Some("\"owner/wf-agents\""), None),
                    ]),
                ],
                "declaration": {
                    "kind": "read",
                    "at": "refs/remotes/origin/main:wf-agents.toml",
                    "tables": [
                        table("", false, vec![key("tree", Some("\"design/\""), None)]),
                        table("files.guide", false, vec![key("path", Some("\"AGENTS.md\""), None)]),
                        table("heavy", false, vec![key("commands", Some("[\n  \"cargo\",\n]"), None)]),
                        table("environment", false, vec![]),
                    ],
                },
            },
            {
                "name": "notes",
                "tables": [table("projects.notes", false, vec![
                    key("repo", Some(repo("notes").as_str()), None),
                    key("main", Some("\"main\""), None),
                ])],
                "declaration": { "kind": "held", "why": HELD },
            },
            {
                "name": "drafts",
                "tables": [table("projects.drafts", false, vec![
                    key("repo", Some(repo("drafts").as_str()), Some("none")),
                    key("main", Some("\"main\""), Some("none")),
                ])],
                "declaration": { "kind": "notRead" },
            },
            {
                "name": "old",
                "tables": [table("projects.old", true, vec![
                    key("main", None, Some("\"main\"")),
                    key("repo", None, Some(repo("old").as_str())),
                ])],
                "declaration": {
                    "kind": "read",
                    "at": "refs/heads/main:wf-agents.toml",
                    "tables": [table("", false, vec![key("tree", Some("\"docs/\""), None)])],
                },
            },
        ])
    );
}
