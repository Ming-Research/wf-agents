//! The schema the command line is given for a line of cells, held against the schemas of the cells it is made from, with
//! a validator written here from the JSON Schema keywords (type, const, enum, properties, required,
//! additionalProperties, items, allOf, anyOf, oneOf, if/then/else), so that the command line's verdict and the cells'
//! are compared by something that is neither. The one rule of the merge is that the line's schema refuses nothing a cell
//! takes; the corners are the unions of every shape the flow has, merged in every order.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use sessions::shape::Shape;

fn kind(x: &Value) -> &'static str {
    match x {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Whether `x` is valid against `s`, by the keywords of JSON Schema (draft 2020-12) and nothing else.
fn fits(s: &Value, x: &Value) -> bool {
    let Some(o) = s.as_object() else { return true };
    if o.get("const").is_some_and(|c| c != x) {
        return false;
    }
    if let Some(Value::Array(e)) = o.get("enum")
        && !e.contains(x)
    {
        return false;
    }
    if let Some(t) = o.get("type") {
        let types: Vec<&str> = match t {
            Value::String(t) => vec![t.as_str()],
            Value::Array(ts) => ts.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        let k = kind(x);
        if !types.iter().any(|t| *t == k || (*t == "number" && k == "integer")) {
            return false;
        }
    }
    if let Some(xo) = x.as_object() {
        if let Some(Value::Array(r)) = o.get("required")
            && r.iter().filter_map(Value::as_str).any(|r| !xo.contains_key(r))
        {
            return false;
        }
        let props = o.get("properties").and_then(Value::as_object);
        for (k, v) in xo {
            match props.and_then(|p| p.get(k)) {
                Some(p) if !fits(p, v) => return false,
                Some(_) => {}
                None if o.get("additionalProperties") == Some(&Value::Bool(false)) => return false,
                None => {}
            }
        }
    }
    if let (Some(xs), Some(it)) = (x.as_array(), o.get("items"))
        && !xs.iter().all(|v| fits(it, v))
    {
        return false;
    }
    let list = |k: &str| o.get(k).and_then(Value::as_array);
    if list("allOf").is_some_and(|a| !a.iter().all(|b| fits(b, x))) {
        return false;
    }
    if list("anyOf").is_some_and(|a| !a.iter().any(|b| fits(b, x))) {
        return false;
    }
    if list("oneOf").is_some_and(|a| a.iter().filter(|b| fits(b, x)).count() != 1) {
        return false;
    }
    if let Some(c) = o.get("if") {
        let branch = if fits(c, x) { "then" } else { "else" };
        if o.get(branch).is_some_and(|b| !fits(b, x)) {
            return false;
        }
    }
    true
}

/// An object that fixes the field `by` to `value`, with other fields, those it requires and whether it takes no other.
fn branch(by: &str, value: Value, fields: Value, required: &[&str], closed: bool) -> Value {
    let mut props = Map::new();
    props.insert(by.into(), json!({ "const": value }));
    props.extend(fields.as_object().unwrap().clone());
    let mut b = json!({ "type": "object", "required": required, "properties": props });
    if closed {
        b["additionalProperties"] = json!(false);
    }
    b
}

fn u1() -> Value {
    json!({ "oneOf": [
        branch("kind", json!("x"), json!({ "p": { "type": "string" } }), &["kind", "p"], true),
        branch("kind", json!("y"), json!({ "q": { "type": "string" } }), &["kind", "q"], true),
    ] })
}

fn u2() -> Value {
    json!({ "oneOf": [
        branch(
            "kind",
            json!("x"),
            json!({ "p": { "type": "integer" }, "r": { "type": "boolean" } }),
            &["kind"],
            false,
        ),
        branch(
            "kind",
            json!("z"),
            json!({ "s": { "type": "array", "items": { "type": "string" } } }),
            &["kind", "s"],
            true,
        ),
    ] })
}

/// The schemas the corners are made of: unions of the same field with the same and different values, of another field,
/// of a fixed boolean and a fixed null, a string, a union as a field, as a field beside another, and as the items of a
/// list; one of the unions has a branch that takes other fields.
fn pool() -> Vec<(&'static str, Value)> {
    vec![
        ("u1", u1()),
        ("u2", u2()),
        (
            "by_on",
            json!({ "oneOf": [
                branch("on", json!(true), json!({ "v": { "type": "string" } }), &["on", "v"], true),
                branch("on", json!(false), json!({ "w": { "type": "integer" } }), &["on", "w"], true),
            ] }),
        ),
        (
            "by_null",
            json!({ "oneOf": [
                branch("kind", Value::Null, json!({ "v": { "type": "string" } }), &["kind", "v"], true),
                branch("kind", json!("x"), json!({ "w": { "type": "integer" } }), &["kind", "w"], true),
            ] }),
        ),
        ("string", json!({ "type": "string" })),
        (
            "field_u1",
            json!({ "type": "object", "required": ["a"], "additionalProperties": false,
                    "properties": { "a": u1() } }),
        ),
        (
            "field_u2",
            json!({ "type": "object", "required": ["a"],
                    "properties": { "a": u2(), "b": { "type": "string" } } }),
        ),
        ("items_u1", json!({ "type": "array", "items": u1() })),
        (
            "items_string",
            json!({ "type": "array", "items": { "type": "string" } }),
        ),
    ]
}

/// Values for the schemas of the pool to take or refuse, some valid for several of them, some for none.
fn corpus() -> Vec<Value> {
    vec![
        json!({ "kind": "x", "p": "a" }),
        json!({ "kind": "x", "p": 5 }),
        json!({ "kind": "x", "p": 5, "r": true }),
        json!({ "kind": "x" }),
        json!({ "kind": "x", "q": "b" }),
        json!({ "kind": "x", "p": null }),
        json!({ "kind": "x", "w": 1 }),
        json!({ "kind": "y", "q": "b" }),
        json!({ "kind": "y", "p": "a" }),
        json!({ "kind": "y" }),
        json!({ "kind": "z", "s": ["a"] }),
        json!({ "kind": "z", "s": [] }),
        json!({ "kind": "z", "s": ["a"], "t": 1 }),
        json!({ "kind": "z", "p": "a" }),
        json!({ "kind": "w" }),
        json!({ "kind": null, "v": "a" }),
        json!({ "on": true, "v": "a" }),
        json!({ "on": false, "w": 1 }),
        json!({ "on": true, "w": 1 }),
        json!({}),
        json!("s"),
        json!(5),
        json!(null),
        json!(["s"]),
        json!([]),
        json!([{ "kind": "x", "p": "a" }]),
        json!([{ "kind": "y", "q": "b" }, "s"]),
        json!({ "a": { "kind": "x", "p": "s" } }),
        json!({ "a": { "kind": "x", "p": 3 }, "b": "z" }),
        json!({ "a": { "kind": "z", "s": ["q"] } }),
        json!({ "a": { "kind": "y", "q": "q" } }),
        json!({ "a": { "kind": "y", "q": "q" }, "b": "z" }),
        json!({ "a": { "kind": "x", "q": "q" } }),
    ]
}

// The schema of two or three schemas merged takes every value any of them takes, in every order they are merged in, and
// says the same of every value whichever way they are grouped: a value valid against one of the cells' schemas must
// never be one the command line refuses, or the cell is held in a retry it cannot get out of. A schema made of one
// takes what it takes.
#[test]
fn a_merged_schema_takes_what_any_of_its_schemas_takes_in_any_order_and_grouping() {
    let pool = pool();
    let values = corpus();
    let shapes: Vec<Shape> = pool.iter().map(|(_, s)| Shape::of(s)).collect();
    let mut taken = 0;
    for (i, (name_a, a)) in pool.iter().enumerate() {
        let alone = shapes[i].schema();
        for v in &values {
            if fits(a, v) {
                taken += 1;
                assert!(fits(&alone, v), "{name_a} alone refuses {v}\n{alone}");
            }
        }
    }
    assert!(taken > 20, "{taken}");
    for i in 0..pool.len() {
        for j in 0..pool.len() {
            let ab = shapes[i].merged(&shapes[j]).schema();
            for k in 0..pool.len() {
                let (a, b, c) = (&pool[i], &pool[j], &pool[k]);
                let left = shapes[i].merged(&shapes[j]).merged(&shapes[k]).schema();
                let right = shapes[i].merged(&shapes[j].merged(&shapes[k])).schema();
                let swapped = shapes[k].merged(&shapes[j]).merged(&shapes[i]).schema();
                for v in &values {
                    let by_a_cell = fits(&a.1, v) || fits(&b.1, v) || fits(&c.1, v);
                    let (l, r, s) = (fits(&left, v), fits(&right, v), fits(&swapped, v));
                    let what = format!("{} + {} + {}: {v}", a.0, b.0, c.0);
                    if by_a_cell {
                        assert!(l, "refused by the merge to the left: {what}\n{left}");
                        assert!(r, "refused by the merge to the right: {what}\n{right}");
                        assert!(s, "refused by the merge in the other order: {what}\n{swapped}");
                    }
                    assert_eq!(l, r, "grouping changes the verdict: {what}");
                    assert_eq!(l, s, "order changes the verdict: {what}");
                }
            }
            for v in &values {
                if fits(&pool[i].1, v) || fits(&pool[j].1, v) {
                    assert!(fits(&ab, v), "{} + {}: {v}\n{ab}", pool[i].0, pool[j].0);
                }
            }
        }
    }
}

// A field written in a branch it does not belong to is refused where the merge can know it: where every schema merged
// has that branch's value as a branch that takes no other field. Where one of them takes other fields in the branch, or
// has no such branch, nothing is refused that the schemas merged take.
#[test]
fn a_field_of_another_branch_is_refused_only_where_every_schema_merged_closes_the_branch() {
    let (u1, u2) = (Shape::of(&u1()), Shape::of(&u2()));
    // Alone, and merged with itself: `q` belongs to the branch `y`, and `p` to `x`.
    for schema in [u1.schema(), u1.merged(&u1).schema()] {
        assert!(!fits(&schema, &json!({ "kind": "x", "p": "a", "q": "b" })));
        assert!(!fits(&schema, &json!({ "kind": "x", "q": "b" })));
        assert!(!fits(&schema, &json!({ "kind": "y", "q": "b", "p": "a" })));
        assert!(!fits(&schema, &json!({ "kind": "y", "p": "a" })));
        assert!(fits(&schema, &json!({ "kind": "x", "p": "a" })));
        assert!(fits(&schema, &json!({ "kind": "y", "q": "b" })));
    }
    // With the union whose `x` takes other fields: `x` takes `q` (the second takes it, being open), `y` is only the
    // first's and still takes no `p`, and `z` is only the second's and takes no `p`.
    for schema in [u1.merged(&u2).schema(), u2.merged(&u1).schema()] {
        assert!(fits(&schema, &json!({ "kind": "x", "q": "b" })));
        assert!(fits(&schema, &json!({ "kind": "x", "p": 5, "r": true })));
        assert!(!fits(&schema, &json!({ "kind": "y", "p": "a" })));
        assert!(!fits(&schema, &json!({ "kind": "z", "s": [], "p": "a" })));
        assert!(!fits(&schema, &json!({ "kind": "y" })));
        assert!(!fits(&schema, &json!({ "kind": "z" })));
    }
}

/// The paths of a value, the value itself first.
fn places(v: &Value, at: &mut Vec<Value>, out: &mut Vec<Vec<Value>>) {
    out.push(at.clone());
    match v {
        Value::Object(o) => {
            for (k, x) in o {
                at.push(json!(k));
                places(x, at, out);
                at.pop();
            }
        }
        Value::Array(xs) => {
            for (i, x) in xs.iter().enumerate() {
                at.push(json!(i));
                places(x, at, out);
                at.pop();
            }
        }
        _ => {}
    }
}

fn at_mut<'a>(v: &'a mut Value, path: &[Value]) -> Option<&'a mut Value> {
    let mut cur = v;
    for s in path {
        cur = match s {
            Value::String(k) => cur.get_mut(k.as_str())?,
            Value::Number(i) => cur.get_mut(i.as_u64()? as usize)?,
            _ => return None,
        };
    }
    Some(cur)
}

// The cells' own schemas against the one the command line is given for their line: a sample of each cell changed in
// one place (a field taken away, a string made empty, a list given a second item) that the cell's schema still takes is
// taken by the line's schema too. The samples are the flow's own, with every form and every depth of the engineer's
// cells.
#[test]
fn a_sample_changed_in_one_place_that_its_cell_still_takes_is_taken_on_the_line_of_the_cell() {
    let relations = db::relations(db::SCHEMA).unwrap();
    let tables = db::index_of(db::SCHEMA).unwrap();
    let c = flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap_or_else(|e| panic!("{e}"));
    let g = c.graph("engineer").unwrap();
    let samples: BTreeMap<String, Vec<Value>> =
        serde_json::from_str(include_str!("../../flow/tests/samples.json")).unwrap();
    let (mut tried, mut taken_by_cell) = (0, 0);
    for (cell, returns) in &samples {
        let cc = g.cell(cell).unwrap();
        // The command line runs the cells of agents only.
        let Some(schema) = cc.schema.clone().filter(|_| cc.kind == "agent") else {
            continue;
        };
        let line = sessions::cli::envelope(g, cc.line.as_deref());
        for s in returns {
            let mut all = Vec::new();
            places(&s["json"], &mut Vec::new(), &mut all);
            for p in &all {
                let mut changes = Vec::new();
                if let Some((Value::String(k), parent)) = p.split_last() {
                    let mut gone = s["json"].clone();
                    if let Some(Value::Object(o)) = at_mut(&mut gone, parent) {
                        o.shift_remove(k.as_str());
                    }
                    changes.push(gone);
                }
                let mut m = s["json"].clone();
                if let Some(x) = at_mut(&mut m, p) {
                    match x {
                        Value::String(t) => {
                            t.clear();
                            changes.push(m.clone());
                        }
                        Value::Array(xs) if !xs.is_empty() => {
                            let first = xs[0].clone();
                            xs.push(first);
                            changes.push(m.clone());
                        }
                        _ => {}
                    }
                }
                for changed in changes {
                    tried += 1;
                    if flow::valid(&schema, &changed) {
                        taken_by_cell += 1;
                        assert!(fits(&line, &changed), "{cell}: the line refuses {changed}");
                    }
                }
            }
        }
    }
    assert!(tried > 500, "{tried}");
    assert!(taken_by_cell > 150, "{taken_by_cell}");
}

// An object that lists some fields and takes others takes any type in the others, so a schema merged from it and from
// one that types a field it does not list takes that field of any type: a value valid against a cell's schema is not
// one the line may refuse because another cell has a type for a field this cell leaves open. The flow has no such
// object now (its objects take no other field), so this holds the merge for the day one has.
#[test]
fn a_field_one_schema_leaves_open_is_not_typed_by_another_that_lists_it() {
    let open = json!({ "type": "object", "properties": { "kind": { "type": "string" } } });
    let typed = json!({ "type": "object", "additionalProperties": false, "required": ["p"],
                        "properties": { "p": { "type": "string" } } });
    let cases = [
        (
            open.clone(),
            typed.clone(),
            vec![json!({ "kind": "x", "p": 5 }), json!({ "kind": "x", "p": ["a"] })],
        ),
        // The branch `x` of the second union takes other fields, so `q` is of any type there; the first types it.
        (
            u1(),
            u2(),
            vec![json!({ "kind": "x", "q": 5 }), json!({ "kind": "x", "q": { "n": 1 } })],
        ),
    ];
    for (a, b, values) in cases {
        let (sa, sb) = (Shape::of(&a), Shape::of(&b));
        for v in values {
            assert!(fits(&a, &v) || fits(&b, &v), "the value is not valid for either: {v}");
            for merged in [sa.merged(&sb).schema(), sb.merged(&sa).schema()] {
                assert!(fits(&merged, &v), "refused: {v}\n{merged}");
            }
        }
    }
}
