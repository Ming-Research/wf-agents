//! What the engine says of a return that does not fit a cell's schema, held against itself on many returns made from the
//! flow's own samples: `valid` says whether a return fits, `misfits` tells where it does not, `take` refuses with the
//! places `misfits` tells, and `without_null_options` takes out nulls and nothing else. Each sample is changed in one
//! place at a time (a field taken away, made null, given another type, an unknown field added, a null added in a field
//! of some other form), so that every depth of every cell's schema is met.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

const FLOW: &str = include_str!("../../../flow/flow.json");

fn compiled() -> flow::Compiled {
    let context = flow::Context {
        relations: db::relations(db::SCHEMA).unwrap(),
        tables: db::index_of(db::SCHEMA).unwrap(),
    };
    flow::compile(FLOW, &context).unwrap_or_else(|e| panic!("{e}"))
}

/// A step down into a value: a field, or an item.
#[derive(Clone, Debug)]
enum Seg {
    Key(String),
    Index(usize),
}

/// Every place in a value, the value itself (the empty path) first.
fn paths(v: &Value, at: &mut Vec<Seg>, out: &mut Vec<Vec<Seg>>) {
    out.push(at.clone());
    match v {
        Value::Object(o) => {
            for (k, x) in o {
                at.push(Seg::Key(k.clone()));
                paths(x, at, out);
                at.pop();
            }
        }
        Value::Array(xs) => {
            for (i, x) in xs.iter().enumerate() {
                at.push(Seg::Index(i));
                paths(x, at, out);
                at.pop();
            }
        }
        _ => {}
    }
}

fn at<'a>(v: &'a Value, path: &[Seg]) -> Option<&'a Value> {
    let mut cur = v;
    for s in path {
        cur = match s {
            Seg::Key(k) => cur.get(k.as_str())?,
            Seg::Index(i) => cur.get(*i)?,
        };
    }
    Some(cur)
}

fn at_mut<'a>(v: &'a mut Value, path: &[Seg]) -> Option<&'a mut Value> {
    let mut cur = v;
    for s in path {
        cur = match s {
            Seg::Key(k) => cur.get_mut(k.as_str())?,
            Seg::Index(i) => cur.get_mut(*i)?,
        };
    }
    Some(cur)
}

/// A path as `misfits` and `without_null_options` write it, `outcome.deviations[3].why`, as steps.
fn parse(path: &str) -> Vec<Seg> {
    let mut out = Vec::new();
    for part in path.split('.') {
        let (name, mut rest) = match part.find('[') {
            Some(i) => (&part[..i], &part[i..]),
            None => (part, ""),
        };
        if !name.is_empty() {
            out.push(Seg::Key(name.to_string()));
        }
        while let Some(r) = rest.strip_prefix('[') {
            let end = r.find(']').unwrap();
            out.push(Seg::Index(r[..end].parse().unwrap()));
            rest = &r[end + 1..];
        }
    }
    out
}

/// Every name any object of a schema lists among its properties.
fn names_in(s: &Value, out: &mut BTreeSet<String>) {
    match s {
        Value::Object(o) => {
            if let Some(Value::Object(p)) = o.get("properties") {
                out.extend(p.keys().cloned());
            }
            o.values().for_each(|v| names_in(v, out));
        }
        Value::Array(xs) => xs.iter().for_each(|v| names_in(v, out)),
        _ => {}
    }
}

/// The values that are not null, by where they stand.
fn leaves(v: &Value, at: String, out: &mut BTreeMap<String, Value>) {
    match v {
        Value::Object(o) => o.iter().for_each(|(k, x)| leaves(x, format!("{at}.{k}"), out)),
        Value::Array(xs) => xs
            .iter()
            .enumerate()
            .for_each(|(i, x)| leaves(x, format!("{at}[{i}]"), out)),
        Value::Null => {}
        other => {
            out.insert(at, other.clone());
        }
    }
}

/// A sample changed in one place at a time.
fn changed(sample: &Value, others: &BTreeSet<String>) -> Vec<Value> {
    let mut places = Vec::new();
    paths(sample, &mut Vec::new(), &mut places);
    let mut out = Vec::new();
    for p in &places {
        let node = at(sample, p).unwrap();
        if let Some((last, parent)) = p.split_last() {
            for new in [Value::Null, json!(7), json!("zzz"), json!({}), json!([])] {
                let mut m = sample.clone();
                *at_mut(&mut m, p).unwrap() = new;
                out.push(m);
            }
            if let Seg::Key(k) = last {
                let mut m = sample.clone();
                if let Some(Value::Object(o)) = at_mut(&mut m, parent) {
                    o.shift_remove(k);
                }
                out.push(m);
            }
        }
        if let Value::Object(o) = node {
            let mut m = sample.clone();
            if let Some(Value::Object(x)) = at_mut(&mut m, p) {
                x.insert("zz_extra".into(), json!(1));
            }
            out.push(m);
            let mut m = sample.clone();
            if let Some(Value::Object(x)) = at_mut(&mut m, p) {
                x.values_mut().for_each(|v| *v = Value::Null);
            }
            out.push(m);
            // A null in a field some form of the cell has, in an object that may be of another.
            for name in others.iter().filter(|n| !o.contains_key(*n)) {
                let mut m = sample.clone();
                if let Some(Value::Object(x)) = at_mut(&mut m, p) {
                    x.insert(name.clone(), Value::Null);
                }
                out.push(m);
            }
        }
    }
    out
}

// The four statements the engine makes of a return, which a return may not make differ: it fits (`valid`) exactly when
// `misfits` tells no place; `take` refuses exactly the returns that do not fit, with the places `misfits` tells, counted
// and one to a line when there are several; and `without_null_options` takes nothing but null out of a return, leaves a
// return that fits as it is, and tells only paths that were null and are gone.
#[test]
fn the_engines_four_statements_of_a_return_agree_on_every_change_of_every_sample() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let samples: BTreeMap<String, Vec<Value>> = serde_json::from_str(include_str!("samples.json")).unwrap();
    let (mut all, mut refused, mut left_out) = (0, 0, 0);
    for (cell, returns) in &samples {
        // A cell with no schema (a cell the engine runs) takes whatever it is given.
        let Some(schema) = g.cell(cell).unwrap().schema.clone() else {
            continue;
        };
        let mut others = BTreeSet::new();
        names_in(&schema, &mut others);
        let refusal = format!("the return does not fit the schema of {cell}");
        for s in returns {
            let given: Map<String, Value> = s["given"].as_object().cloned().unwrap_or_default();
            for m in changed(&s["json"], &others) {
                all += 1;
                let valid = flow::valid(&schema, &m);
                let places = flow::misfits(&schema, &m, "");
                assert_eq!(valid, places.is_empty(), "{cell}: {m}\n{places:?}");
                assert!(places.iter().all(|p| !p.is_empty()), "{cell}: {m}");
                // `take` refuses what does not fit, as `misfits` says, and nothing else for that reason.
                match g.take(cell, &m, &given) {
                    Err(e) if e.starts_with(&refusal) => {
                        refused += 1;
                        assert!(!valid, "{cell}: {m}");
                        match e.strip_prefix(&format!("{refusal} in ")) {
                            Some(rest) => {
                                let (count, lines) = rest.split_once(" places:\n").unwrap();
                                assert_eq!(count.parse::<usize>().unwrap(), places.len(), "{e}");
                                assert_eq!(lines, places.join("\n"), "{e}");
                                assert!(places.len() > 1, "{e}");
                            }
                            None => assert_eq!(e, format!("{refusal}: {}", places[0]), "{cell}: {m}"),
                        }
                    }
                    Err(e) => assert!(valid, "{cell}: {m}\n{e}"),
                    Ok(_) => assert!(valid, "{cell}: {m}"),
                }
                // Nulls taken out are nulls, and they are the only thing taken out.
                let mut after = m.clone();
                let foreign = flow::without_null_options(&schema, &mut after);
                if valid {
                    assert_eq!(after, m, "{cell}");
                    assert!(foreign.is_empty(), "{cell}: {m}");
                }
                let (mut before_leaves, mut after_leaves) = (BTreeMap::new(), BTreeMap::new());
                leaves(&m, String::new(), &mut before_leaves);
                leaves(&after, String::new(), &mut after_leaves);
                assert_eq!(before_leaves, after_leaves, "{cell}: {m}\n{after}");
                for p in &foreign {
                    left_out += 1;
                    let steps = parse(p);
                    assert_eq!(at(&m, &steps), Some(&Value::Null), "{cell}: {p} in {m}");
                    assert!(at(&after, &steps).is_none(), "{cell}: {p} in {after}");
                }
            }
        }
    }
    // The changes were made, many of them refused, and some nulls of another form were left out: the loop is no vacuity.
    assert!(all > 5_000, "{all}");
    assert!(refused > 1_000, "{refused}");
    assert!(left_out > 0, "{left_out}");
}

// A value is told in every place it does not fit through unions inside lists and inside unions, where the expected
// verdict of each value is known from how it was made (the schema is written here, not read from the flow): a list of
// objects of two forms, one of which holds a union of its own.
#[test]
fn a_value_is_told_in_every_place_it_does_not_fit_through_unions_inside_unions() {
    let schema = json!({
        "type": "object", "required": ["label", "items"], "additionalProperties": false,
        "properties": {
            "label": { "type": "string" },
            "items": { "type": "array", "items": { "oneOf": [
                { "type": "object", "required": ["kind", "a"], "additionalProperties": false, "properties": {
                    "kind": { "const": "x" },
                    "a": { "type": "string" },
                    "inner": { "oneOf": [
                        { "type": "object", "required": ["mode"], "additionalProperties": false, "properties": {
                            "mode": { "const": "p" }, "n": { "type": "integer" } } },
                        { "type": "object", "required": ["mode", "m"], "additionalProperties": false, "properties": {
                            "mode": { "const": "q" }, "m": { "type": "boolean" } } },
                    ] },
                } },
                { "type": "object", "required": ["kind", "b"], "additionalProperties": false, "properties": {
                    "kind": { "const": "y" }, "b": { "type": "integer" } } },
            ] } },
        },
    });
    // The value, whether it fits, and the places a reader is to be told, as the names the value gives them.
    let cases: Vec<(Value, bool, Vec<&str>)> = vec![
        (json!({ "label": "l", "items": [] }), true, vec![]),
        (
            json!({ "label": "l", "items": [{ "kind": "x", "a": "s" }, { "kind": "y", "b": 1 },
                { "kind": "x", "a": "s", "inner": { "mode": "p", "n": 1 } }] }),
            true,
            vec![],
        ),
        (
            json!({ "label": "l", "items": [{ "kind": "x" }] }),
            false,
            vec!["items[0]"],
        ),
        (
            json!({ "label": "l", "items": [{ "kind": "y", "b": 1, "a": "s" }] }),
            false,
            vec!["items[0]"],
        ),
        (
            json!({ "label": "l", "items": [{ "kind": "x", "a": "s", "inner": { "mode": "q" } }] }),
            false,
            vec!["items[0].inner"],
        ),
        (
            json!({ "label": "l", "items": [{ "kind": "x", "a": "s", "inner": { "mode": "r" } }] }),
            false,
            vec!["items[0].inner.mode"],
        ),
        (
            json!({ "label": "l", "items": [{ "kind": "z" }] }),
            false,
            vec!["items[0].kind"],
        ),
        (json!({ "label": "l", "items": ["s"] }), false, vec!["items[0]"]),
        (json!({ "label": "l", "items": {} }), false, vec!["items"]),
        (json!({ "label": 3, "items": [] }), false, vec!["label"]),
        (json!({ "items": [] }), false, vec!["label"]),
        // Everything wrong at once, in four places of three depths: each is told, and each once.
        (
            json!({ "label": 3, "items": [
                { "kind": "x", "a": "s", "inner": { "mode": "q", "m": "no" } },
                { "kind": "y", "b": "s" },
                5,
                { "kind": "x" },
            ] }),
            false,
            vec!["label", "items[0].inner.m", "items[1].b", "items[2]", "items[3]"],
        ),
    ];
    for (value, fits, told) in cases {
        assert_eq!(flow::valid(&schema, &value), fits, "{value}");
        let places = flow::misfits(&schema, &value, "");
        assert_eq!(places.is_empty(), fits, "{value}\n{places:?}");
        for name in told {
            assert!(
                places.iter().any(|p| p.contains(name)),
                "{name} is not told in {value}\n{places:?}"
            );
        }
        let unique: BTreeSet<&String> = places.iter().collect();
        assert_eq!(unique.len(), places.len(), "a place told twice: {places:?}");
    }
}
