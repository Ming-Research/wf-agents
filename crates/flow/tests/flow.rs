use std::collections::BTreeMap;

use serde_json::{Map, Value};

const FLOW: &str = include_str!("../../../flow/flow.json");

fn context() -> flow::Context {
    flow::Context {
        relations: db::relations(db::SCHEMA).unwrap(),
    }
}

fn compiled() -> flow::Compiled {
    flow::compile(FLOW, &context()).unwrap_or_else(|e| panic!("{e}"))
}

// The engineer's graph compiles with no finding, and comes out as the generator the flow was designed with worked
// it out: as many cells, rows, states and places, and the same inputs and rules already in each agent's session.
#[test]
fn the_engineer_graph_compiles_as_its_reference_has_it() {
    let c = compiled();
    let r: Value = serde_json::from_str(include_str!("reference.json")).unwrap();
    let s = &c.stats[0];
    let want = |k: &str| r[k].as_u64().unwrap() as usize;
    assert_eq!(
        (s.cells, s.rows, s.states, s.places),
        (want("cells"), want("rows"), want("states"), want("places"))
    );
    let g = c.graph("engineer").unwrap();
    // The reference holds an input with nothing to give, one read from the database or the repository, as held; the
    // compiled flow says a session holds only inputs read from returns.
    for (k, v) in r["held"].as_object().unwrap() {
        let (cell, i) = k.split_once('.').unwrap();
        let x = &g.cell(cell).unwrap().inputs[i.parse::<usize>().unwrap()];
        let returns = x.from.iter().all(|s| matches!(s, flow::format::Src::Place(..)));
        assert_eq!(
            x.held == Some(flow::Held::Session),
            returns && v.as_bool().unwrap(),
            "input {k}"
        );
    }
    for (k, v) in r["rules"].as_object().unwrap() {
        let (cell, rule) = k.split_once('.').unwrap();
        let c = g.cell(cell).unwrap();
        let held = if rule == "project" {
            c.project_rules
        } else {
            c.rules.iter().find(|x| x.name == rule).map(|x| x.held)
        };
        assert_eq!(held == Some(flow::Held::Session), v.as_bool().unwrap(), "rule {k}");
    }
}

// What the engine stores reads back as it was, with the same digest each time.
#[test]
fn the_compiled_flow_reads_back_with_a_stable_digest() {
    let c = compiled();
    let back: flow::Compiled = serde_json::from_str(&c.json()).unwrap();
    assert_eq!(back.json(), c.json());
    assert_eq!(compiled().digest(), c.digest());
    assert_eq!(c.digest().len(), 64);
}

// Every sample return fits its cell's schema and takes the row it was written for.
#[test]
fn every_sample_takes_its_row() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let mut n = 0;
    for (cell, xs) in &samples {
        for s in xs {
            let given: Map<String, Value> = s["given"].as_object().cloned().unwrap_or_default();
            let row = g
                .take(cell, &s["json"], &given)
                .unwrap_or_else(|e| panic!("{cell}: {e}"));
            assert_eq!(row.name, s["row"].as_str().unwrap(), "a sample of {cell}");
            n += 1;
        }
    }
    assert_eq!(n, 34);
}

// A return with its form outside the field it sits in fits no schema; a recorded value changes the row taken.
#[test]
fn a_sample_that_does_not_fit_or_takes_another_row_is_found() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let s = &samples["12"][0]["json"];
    let mut flat = s["outcome"].as_object().unwrap().clone();
    flat.insert("side_reports".into(), Value::Array(Vec::new()));
    flat.insert("progress".into(), "x".into());
    let e = g.take("12", &Value::Object(flat), &Map::new()).unwrap_err();
    assert!(e.contains("does not fit the schema"), "{e}");
    let s = &samples["18"][0];
    let mut given = s["given"].as_object().unwrap().clone();
    given.insert("interjection".into(), "yes".into());
    assert_eq!(g.take("18", &s["json"], &given).unwrap().name, "interjection");
}

// A return that does not fit says where and how, following the alternative it picks.
#[test]
fn a_return_that_does_not_fit_says_where() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let sample = &samples["2b"][0]["json"];
    let refused = |edit: &dyn Fn(&mut Value)| {
        let mut x = sample.clone();
        edit(&mut x);
        g.take("2b", &x, &Map::new()).unwrap_err()
    };
    let e = refused(&|x| x["outcome"]["better"]["verdict"] = "maybe".into());
    assert!(
        e.ends_with(r#"outcome.better.verdict is "maybe", and must be one of "yes", "no""#),
        "{e}"
    );
    let e = refused(&|x| {
        x["outcome"]["better"].as_object_mut().unwrap().remove("why");
    });
    assert!(e.ends_with("outcome.better has no why"), "{e}");
    let e = refused(&|x| x["outcome"] = Value::String(x["outcome"].to_string()));
    assert!(e.ends_with("outcome must be an object, and is a string"), "{e}");
}

// An agent's return schema holds its forms one level down, and a field a form leaves out is not required.
#[test]
fn an_agent_schema_holds_its_forms_in_outcome() {
    let c = compiled();
    let s = c.graph("engineer").unwrap().cell("4").unwrap().schema.clone().unwrap();
    assert!(s["properties"]["outcome"]["anyOf"].is_array());
    let required: Vec<&str> = s["required"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(required, ["outcome", "side_reports"]);
}

// A work moves as the rows say: into the tool cell and back to the agent that called it, out to the advisor and,
// through choosing the way on, back to where it stopped.
#[test]
fn a_work_moves_by_its_rows() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let take = |p: &flow::Position, name: &str| {
        let row = g.cell(&p.at).unwrap().rows.iter().find(|r| r.name == name).unwrap();
        g.step(p, row).unwrap()
    };
    let p = g.start_position();
    assert_eq!(p.at, "0");
    let p = take(&p, "started");
    assert_eq!((p.at.as_str(), p.recorded["stage"].as_str()), ("1", "research"));
    let p = take(&p, "tools");
    assert_eq!(
        (p.at.as_str(), p.callers.as_slice()),
        ("13", ["1".to_string()].as_slice())
    );
    let p = take(&p, "ran");
    assert_eq!(
        (p.at.as_str(), p.from.as_deref(), p.callers.len()),
        ("1", Some("13"), 0)
    );
    let p = take(&p, "ask_advisor");
    assert_eq!(
        (p.at.as_str(), p.stopped.as_deref(), p.pointer("@caller")),
        ("12", Some("1"), Some("1"))
    );
    let p = take(&p, "answered");
    assert_eq!(
        (p.at.as_str(), p.stopped.as_deref(), p.from.as_deref()),
        ("11", Some("1"), Some("12"))
    );
    let p = take(&p, "carry_on");
    assert_eq!(p.at, "11a");
    let p = take(&p, "back");
    assert_eq!((p.at.as_str(), p.stopped.as_deref()), ("1", None));
    let p = take(&p, "done");
    assert_eq!((p.at.as_str(), p.recorded["rethought"].as_str()), ("1a", "no"));
    let wrong = g.cell("11a").unwrap().rows.iter().find(|r| r.name == "back").unwrap();
    assert!(g.step(&p, wrong).is_err());
}
