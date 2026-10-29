//! `client/src/api.ts` is generated from the Rust types; a file that differs from what they generate fails here, as
//! a database that differs from `db/schema.sql` would. The flow the page's tests read is generated here too.

use std::path::Path;

#[test]
fn the_page_types_are_what_the_rust_types_generate() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../client/src/api.ts");
    let generated = api::typescript();
    if std::env::var_os("WF_AGENTS_GENERATE").is_some() {
        std::fs::write(&path, &generated).unwrap();
        return;
    }
    let kept = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        kept == generated,
        "{} is not what the Rust types generate: run `WF_AGENTS_GENERATE=1 cargo test -p api --test typescript`",
        path.display()
    );
}

// The page's tests run the flow page's parts on `flow/flow.json` as the engine stores it compiled, which leaves out
// what a field does not hold, and on the view the interface serves of that, so that they read the flow the page
// reads. It changes with every change to the flow, so it is not kept but written where the page's tests run, before
// them: `WF_AGENTS_GENERATE=1` writes it with `api.ts`, as CI does.
#[test]
fn the_page_tests_read_the_flow_as_the_engine_stores_it() {
    if std::env::var_os("WF_AGENTS_GENERATE").is_none() {
        return;
    }
    let relations = db::relations(db::SCHEMA).unwrap();
    let tables = db::index_of(db::SCHEMA).unwrap();
    let compiled = flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap_or_else(|e| panic!("{e}"));
    let stored = compiled.json();
    let read: flow::Compiled = serde_json::from_str(&stored).unwrap();
    let view = serde_json::to_string(&api::view(&read)).unwrap();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../client/src/stored-flow.json");
    std::fs::write(&path, format!(r#"{{"compiled":{stored},"view":{view}}}"#)).unwrap();
}
