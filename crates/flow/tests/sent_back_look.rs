//! The corners of `rounds.sentBack.look`: which rounds sections the compiler accepts with it, and the row that pauses at
//! returns sent back.

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

// The reference is the field's own comment: with `look` each return sent back is the architect's to look at, and the
// pause still comes after `pauseAfter` in a row, which is at least 2 whether or not the architect looks: the first
// return sent back never pauses a work, and a pause after none is no bound. The row that pauses is named the same, and
// takes the number, either way.
#[test]
fn a_pause_at_returns_sent_back_comes_after_at_least_two_in_a_row_whatever_look_says() {
    for look in [true, false] {
        let two = compile_with("engineer", |s| {
            s["pauseAfter"] = 2.into();
            s["look"] = look.into();
        })
        .unwrap();
        assert_eq!(
            pause_row(&two, "engineer"),
            (
                "sent_back_too_often".to_string(),
                vec![flow::format::Atom::SentBack(Some(2))]
            ),
            "look {look}"
        );
        for after in [0, 1] {
            let refused = compile_with("engineer", |s| {
                s["pauseAfter"] = after.into();
                s["look"] = look.into();
            })
            .unwrap_err();
            assert!(
                refused.contains("rounds.sentBack.pauseAfter is less than 2"),
                "pauseAfter {after}, look {look}: {refused}"
            );
        }
    }
}

// The architect's own graph has no architect above it, and the engine queues nothing for it: a flow that says each of
// its returns sent back is looked at there would have a matter queued for no one, so the compiler refuses it, whatever
// the number of returns in a row that pause.
#[test]
fn the_architect_s_own_graph_cannot_say_its_returns_sent_back_are_looked_at() {
    for after in [2, 3] {
        let refused = compile_with("architect", |s| {
            s["pauseAfter"] = after.into();
            s["look"] = true.into();
        })
        .unwrap_err();
        assert!(
            refused.contains("rounds.sentBack.look is set in graph architect"),
            "pauseAfter {after}: {refused}"
        );
    }
}
