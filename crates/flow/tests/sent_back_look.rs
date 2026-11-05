//! The corners of `rounds.sentBack.look`: which rounds sections the compiler accepts with it, and the row it gives.

use serde_json::Value;

const FLOW: &str = include_str!("../../../flow/flow.json");

fn context() -> flow::Context {
    flow::Context {
        relations: db::relations(db::SCHEMA).unwrap(),
        tables: db::index_of(db::SCHEMA).unwrap(),
    }
}

/// The flow with `change` made to the `sentBack` part of the rounds section of the graph named `graph`, compiled.
fn compile_with(graph: &str, change: impl FnOnce(&mut Value)) -> Result<flow::Compiled, String> {
    let mut v: Value = serde_json::from_str(FLOW).unwrap();
    let g = v["flows"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|f| f["name"] == graph)
        .unwrap_or_else(|| panic!("no graph {graph}"));
    change(&mut g["rounds"]["sentBack"]);
    flow::compile(&v.to_string(), &context()).map_err(|e| e.to_string())
}

/// The shared row of the first agent cell of the graph that pauses at returns sent back, as the compiler gives it.
fn pause_row(c: &flow::Compiled, graph: &str) -> (String, Vec<flow::format::Atom>) {
    let g = c.graph(graph).unwrap();
    let cell = g.cells.iter().find(|x| x.kind == "agent").unwrap();
    let row = cell
        .rows
        .iter()
        .find(|r| {
            r.pauses
                && r.conditions
                    .iter()
                    .any(|a| matches!(a, flow::format::Atom::SentBack(Some(_))))
        })
        .unwrap_or_else(|| panic!("{graph}: no row pauses at a return sent back"));
    (row.name.clone(), row.conditions.clone())
}

// The reference is the field's own comment: `look` allows `pauseAfter` to be 1 and makes the pause the architect's to
// look at, so the row is the architect's whatever the number is; without it the row keeps the owner's name for any
// number from 2. `pauseAfter` 0 means a pause after none, which neither allows.
#[test]
fn the_row_is_named_by_whom_the_pause_is_for_whatever_the_number_is() {
    let looked = compile_with("engineer", |s| s["pauseAfter"] = 3.into()).unwrap();
    assert_eq!(
        pause_row(&looked, "engineer"),
        (
            "sent_back_looked_at".to_string(),
            vec![flow::format::Atom::SentBack(Some(3))]
        )
    );
    let owners = compile_with("engineer", |s| {
        s.as_object_mut().unwrap().remove("look");
        s["pauseAfter"] = 2.into();
    })
    .unwrap();
    assert_eq!(
        pause_row(&owners, "engineer"),
        (
            "sent_back_too_often".to_string(),
            vec![flow::format::Atom::SentBack(Some(2))]
        )
    );
    let none = compile_with("engineer", |s| s["pauseAfter"] = 0.into()).unwrap_err();
    assert!(none.contains("rounds.sentBack.pauseAfter is less than 2"), "{none}");
}

// The architect's own graph has no architect above it, and the engine pauses its work for the owner whatever `look`
// says. A flow that says it looks at a return sent back there would draw a row, `sent_back_looked_at`, that nothing
// takes: the chart is how the owner checks the flow, so the compiler must not accept it.
#[test]
fn the_architect_s_own_graph_cannot_say_its_pause_is_looked_at() {
    let architect = compile_with("architect", |s| {
        s["pauseAfter"] = 1.into();
        s["look"] = true.into();
    });
    // The compiler either refuses the flow, or the architect's graph has no row that says its pause is looked at.
    let looked = architect
        .as_ref()
        .is_ok_and(|c| pause_row(c, "architect").0 == flow::format::SENT_BACK_LOOKED_AT);
    assert!(
        !looked,
        "the architect's own graph compiled, with the row {}",
        flow::format::SENT_BACK_LOOKED_AT
    );
}
