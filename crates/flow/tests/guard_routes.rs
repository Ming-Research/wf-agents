// Item #270: where adversarial testing (T1) goes by the results of the old-behaviour (guard) tests before the change.
//
// A guard test that cannot be built before the change (`cannot_run`) is judged by the review and sends nothing back;
// one that fails before the change (`red`), or on the head, sends the work to the fix. The reference is the flow's
// own text for the table: `guard_tests.on_base` says a `cannot_run` guard is "judged by the table review", and a `red`
// one "does not test old behaviour".

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

const FLOW: &str = include_str!("../../../flow/flow.json");

fn compiled() -> flow::Compiled {
    let context = flow::Context {
        relations: db::relations(db::SCHEMA).unwrap(),
        tables: db::index_of(db::SCHEMA).unwrap(),
    };
    flow::compile(FLOW, &context).unwrap_or_else(|e| panic!("{e}"))
}

/// The row adversarial testing takes for a return built from its sample of the row `clear`, with the guard table, the
/// new-behaviour table and the review's `@must` verdict replaced.
fn route(guards: Value, news: Option<Value>, must: &str) -> String {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let clear = samples["T1"]
        .iter()
        .find(|s| s["row"] == "clear")
        .expect("a sample of the row clear");
    let mut given: Map<String, Value> = clear["given"].as_object().cloned().unwrap_or_default();
    given.insert("@must".into(), json!({ "verdict": must, "why": "w" }));
    let mut x = clear["json"].clone();
    x["outcome"]["guard_tests"] = guards;
    if let Some(news) = news {
        x["outcome"]["new_tests"] = news;
    }
    g.take("T1", &x, &given).unwrap().name.clone()
}

fn guard(name: &str, by: &str, head: &str, base: &str) -> Value {
    json!({ "name": name, "by": by, "on_head": head, "on_base": base })
}

// A red guard test sends the work to the fix wherever it stands in the table and whoever wrote it, among rows that
// cannot be built or were not run; a table of only such rows does not.
#[test]
fn a_red_guard_row_among_unbuilt_ones_sends_testing_back_wherever_it_stands() {
    let unbuilt = |n: &str| guard(n, "engineer", "green", "cannot_run");
    let skipped = |n: &str| guard(n, "adversarial", "green", "not_run");
    let red_before = |by: &str| guard("r", by, "green", "red");
    for by in ["engineer", "adversarial"] {
        // At the front, the middle and the end of the table.
        let tables = [
            vec![red_before(by), unbuilt("a"), skipped("b")],
            vec![unbuilt("a"), red_before(by), skipped("b")],
            vec![unbuilt("a"), skipped("b"), red_before(by)],
        ];
        for t in tables {
            assert_eq!(
                route(Value::Array(t.clone()), None, "no"),
                "for_engineer",
                "{by}: {t:?}"
            );
        }
    }
    let all_unjudged = json!([
        unbuilt("a"),
        skipped("b"),
        guard("c", "engineer", "green", "cannot_run")
    ]);
    assert_eq!(route(all_unjudged, None, "no"), "clear");
}

// A guard test that cannot be built before the change but is red on the head is red: the head decides, and a reason for
// not building it before does not excuse a failure after the change.
#[test]
fn an_unbuilt_guard_row_that_is_red_on_the_head_sends_testing_back() {
    for by in ["engineer", "adversarial"] {
        let rows = json!([guard("a", by, "red", "cannot_run")]);
        assert_eq!(route(rows, None, "no"), "for_engineer", "{by}");
    }
}

// The review's verdict that something must be fixed still sends the work back when every guard row is one that cannot
// be built before the change: letting such a row through must not let the verdict through with it.
#[test]
fn an_unbuilt_guard_row_does_not_override_the_reviews_must_fix() {
    let rows = json!([guard("a", "engineer", "green", "cannot_run")]);
    assert_eq!(route(rows.clone(), None, "yes"), "for_engineer");
    assert_eq!(route(rows, None, "no"), "clear");
}

// An empty guard table passes, as does one that has a row cannot_run beside new tests that cannot be built before
// the change; a new-behaviour test that passes before the change is still sent back, whatever the guard rows say.
#[test]
fn guard_rows_that_cannot_be_built_do_not_change_how_the_new_tests_route() {
    let unbuilt = json!([guard("a", "engineer", "green", "cannot_run")]);
    let new = |base: &str| json!([{ "name": "n", "by": "adversarial", "on_head": "green", "on_base": base }]);
    assert_eq!(route(json!([]), None, "no"), "clear");
    assert_eq!(route(unbuilt.clone(), Some(new("cannot_run")), "no"), "clear");
    assert_eq!(route(unbuilt.clone(), Some(new("red")), "no"), "clear");
    assert_eq!(route(unbuilt, Some(new("green")), "no"), "for_engineer");
}
