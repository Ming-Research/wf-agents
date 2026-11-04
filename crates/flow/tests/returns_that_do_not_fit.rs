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

/// A path with each index of a list written `[]`: `columns[0].evidence` as `columns[].evidence`.
fn without_indexes(path: &str) -> String {
    let mut out = String::new();
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if c == '[' {
            let mut digits = String::new();
            while let Some(d) = chars.next_if(char::is_ascii_digit) {
                digits.push(d);
            }
            if chars.peek() != Some(&']') {
                out.push_str(&digits);
            }
        }
    }
    out
}

/// Whether the field of a place is the one its sentence is about, read from the sentence by the words it is made of:
/// of a field that is missing (`X has no r`) or not one of the schema's (`X has k, which is not one of its fields`),
/// the path of that field, `X.r` or `X.k`; of any other, the path the sentence begins with. Each with the indexes of
/// lists written `[]`, and none for the return as a whole.
fn names_its_field(p: &flow::Misfit) -> bool {
    let text = without_indexes(&p.text);
    let named = match text.rsplit_once(" has no ").filter(|(_, r)| !r.contains(' ')) {
        Some((head, name)) => Some((head.to_string(), name.to_string())),
        None => text
            .split_once(", which is not one of its fields: ")
            .and_then(|(before, _)| before.rsplit_once(" has "))
            .map(|(head, name)| (head.to_string(), name.to_string())),
    };
    let path = |head: &str, name: &str| {
        if head == "the return" {
            name.to_string()
        } else {
            format!("{head}.{name}")
        }
    };
    match (&p.field, named) {
        (Some(f), Some((head, name))) => *f == path(&head, &name),
        // A field that is missing or not allowed has a path, even in the return itself.
        (None, Some(_)) => false,
        (Some(f), None) => text.strip_prefix(f.as_str()).is_some_and(|rest| rest.starts_with(' ')),
        (None, None) => text.starts_with("the return "),
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
// return that fits as it is, and tells only paths that were null and are gone. Each place `misfits` tells is also told
// with the field it is about, and `take_or_places` refuses as `take` does, with those places.
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
                // Each place is told with the field it is about, in the same order and the same words.
                let located = flow::misfit_places(&schema, &m, "");
                assert_eq!(
                    located.iter().map(|p| p.text.clone()).collect::<Vec<_>>(),
                    places,
                    "{cell}: {m}"
                );
                for p in &located {
                    assert!(names_its_field(p), "{cell}: {p:?} in {m}");
                }
                // `take_or_places` is `take`: the same row or the same text, and for a return that does not fit the
                // places `misfit_places` tells; for one no row takes, its sentence as its one place, of no field.
                let by_places = g.take_or_places(cell, &m, &given);
                assert_eq!(
                    by_places.as_ref().map(|r| r.name.clone()).map_err(|n| n.text.clone()),
                    g.take(cell, &m, &given).map(|r| r.name.clone()),
                    "{cell}: {m}"
                );
                if let Err(n) = &by_places {
                    if n.text.starts_with(&refusal) {
                        assert_eq!(n.places, located, "{cell}: {m}");
                    } else {
                        let only = flow::Misfit {
                            field: None,
                            text: n.text.clone(),
                        };
                        assert_eq!(n.places, [only], "{cell}: {m}");
                    }
                }
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
    // The shapes of a refusal that the changes above seldom or never make, with their exact words.
    refusals_give_their_places(g, &samples);
}

/// A schema written here, not read from the flow: a list of objects of two forms, one of which holds a union of its own.
fn nested_schema() -> Value {
    json!({
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
    })
}

// A value is told in every place it does not fit through unions inside lists and inside unions, where the expected
// verdict of each value is known from how it was made (the schema is written here, not read from the flow): a list of
// objects of two forms, one of which holds a union of its own.
#[test]
fn a_value_is_told_in_every_place_it_does_not_fit_through_unions_inside_unions() {
    let schema = nested_schema();
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

// Each place is told with the field it is about, where the expected fields are known from how each value was made, on
// the schema above: a field that is missing or not allowed by its own path, a value of the wrong kind or not among the
// allowed ones by its own, the index of every item of a list written `[]` (the sentence keeps it, with two digits too),
// so that one fault in two items is one field, and none for the return as a whole.
#[test]
fn a_place_is_told_with_the_field_it_is_about_through_unions_inside_unions() {
    let schema = nested_schema();
    let item = |n: usize| json!({ "kind": "y", "b": n });
    let eleven: Vec<Value> = (0..11).map(item).collect();
    let twelve: Vec<Value> = eleven
        .iter()
        .cloned()
        .chain([json!({ "kind": "y", "b": "s" })])
        .collect();
    let cases: Vec<(Value, Vec<Option<&str>>)> = vec![
        (json!({ "label": "l", "items": eleven }), vec![]),
        (json!(3), vec![None]),
        (json!({ "items": [] }), vec![Some("label")]),
        (json!({ "label": 3, "items": [] }), vec![Some("label")]),
        (json!({ "label": "l", "items": {} }), vec![Some("items")]),
        (json!({ "label": "l", "items": ["s"] }), vec![Some("items[]")]),
        (
            json!({ "label": "l", "items": [{ "kind": "x" }] }),
            vec![Some("items[].a")],
        ),
        (
            json!({ "label": "l", "items": [{ "kind": "y", "b": 1, "a": "s" }] }),
            vec![Some("items[].a")],
        ),
        (
            json!({ "label": "l", "items": [{ "kind": "z" }] }),
            vec![Some("items[].kind")],
        ),
        (
            json!({ "label": "l", "items": [{ "kind": "x", "a": "s", "inner": { "mode": "q" } }] }),
            vec![Some("items[].inner.m")],
        ),
        (
            json!({ "label": "l", "items": [{ "kind": "x", "a": "s", "inner": { "mode": "r" } }] }),
            vec![Some("items[].inner.mode")],
        ),
        // The same fault in two items: two places, one field.
        (
            json!({ "label": "l", "items": [{ "kind": "x" }, { "kind": "y", "b": 1 }, { "kind": "x" }] }),
            vec![Some("items[].a"), Some("items[].a")],
        ),
        // The twelfth item: the index has two digits in the sentence, and none in the field.
        (json!({ "label": "l", "items": twelve }), vec![Some("items[].b")]),
        // Everything wrong at once, in the order the places are told.
        (
            json!({ "label": 3, "items": [
                { "kind": "x", "a": "s", "inner": { "mode": "q", "m": "no" } },
                { "kind": "y", "b": "s" },
                5,
                { "kind": "x" },
            ] }),
            vec![
                Some("label"),
                Some("items[].inner.m"),
                Some("items[].b"),
                Some("items[]"),
                Some("items[].a"),
            ],
        ),
    ];
    for (value, fields) in cases {
        let places = flow::misfit_places(&schema, &value, "");
        let told: Vec<Option<&str>> = places.iter().map(|p| p.field.as_deref()).collect();
        assert_eq!(told, fields, "{value}\n{places:?}");
        for p in &places {
            assert!(names_its_field(p), "{p:?} in {value}");
        }
    }
    // The sentence keeps the index of the item.
    let twelve: Vec<Value> = (0..11)
        .map(|n| json!({ "kind": "y", "b": n }))
        .chain([json!({ "kind": "y", "b": "s" })])
        .collect();
    let places = flow::misfit_places(&schema, &json!({ "label": "l", "items": twelve }), "");
    assert!(places[0].text.starts_with("items[11].b "), "{places:?}");
    // A path given to start from is the field's, with its indexes written `[]` too.
    let places = flow::misfit_places(&schema, &json!(5), "outcome.rows[2]");
    assert_eq!(places[0].field.as_deref(), Some("outcome.rows[]"), "{places:?}");
    assert!(places[0].text.starts_with("outcome.rows[2] "), "{places:?}");
}

// A field the schema does not have is told with the name the agent wrote, whatever characters it holds: the field is
// the path of its parent and the name, and a name is an object's key, not an item of a list, so it is not written `[]`
// where it looks like an index; a dot in it is part of it. The reference is the definition of `Misfit::field`. The
// schema is written here: closed objects, one of which holds a list of closed objects.
#[test]
fn a_name_the_agent_wrote_is_the_field_as_it_wrote_it() {
    let schema = json!({
        "type": "object", "required": ["label", "items"], "additionalProperties": false,
        "properties": {
            "label": { "type": "string" },
            "items": { "type": "array", "items": {
                "type": "object", "required": ["a"], "additionalProperties": false,
                "properties": { "a": { "type": "string" } } } },
        },
    });
    let cases: Vec<(Value, Vec<Option<&str>>)> = vec![
        // A name that ends like an index.
        (json!({ "label": "l", "items": [], "x[3]": 1 }), vec![Some("x[3]")]),
        // A name that is an index and nothing else.
        (json!({ "label": "l", "items": [], "[7]": 1 }), vec![Some("[7]")]),
        // A name with a dot: its parent's path and its own name, not told as two fields.
        (json!({ "label": "l", "items": [], "a.b": 1 }), vec![Some("a.b")]),
        // A name with a bracket that is not closed.
        (json!({ "label": "l", "items": [], "m[": 1 }), vec![Some("m[")]),
        // In an item of a list: the item's index is written `[]`, the name is not; the same name in two items is one field.
        (
            json!({ "label": "l", "items": [{ "a": "s", "k[12]": 1 }, { "a": "s", "k[12]": 2 }] }),
            vec![Some("items[].k[12]"), Some("items[].k[12]")],
        ),
    ];
    for (value, fields) in cases {
        let places = flow::misfit_places(&schema, &value, "");
        let told: Vec<Option<&str>> = places.iter().map(|p| p.field.as_deref()).collect();
        assert_eq!(told, fields, "{value}\n{places:?}");
    }
}

/// A return a cell refuses is refused with the places it is about besides the text `take` gives: a return that does not
/// fit the schema with each place of it, one that no row takes with its sentence as its one place of no field (the row
/// of cell R1 that takes a return is chosen by the values the engine gives it), and a cell that is not there likewise.
/// Part of the test of the four statements, which has the flow compiled already.
fn refusals_give_their_places(g: &flow::CompiledGraph, samples: &BTreeMap<String, Vec<Value>>) {
    let sample = samples["R1"]
        .iter()
        .find(|s| s["row"] == "start")
        .expect("a sample of R1 that a row takes by what the engine gives");
    let given: Map<String, Value> = sample["given"].as_object().cloned().unwrap();
    // Given what the engine gives, the row is taken.
    let row = g.take_or_places("R1", &sample["json"], &given).unwrap();
    assert_eq!(row.name, "start");
    // Given nothing, no row takes it: the one sentence, said as `take` says it.
    let none = g.take_or_places("R1", &sample["json"], &Map::new()).unwrap_err();
    assert_eq!(none.text, "no row of R1 takes the return");
    assert_eq!(
        none.places,
        [flow::Misfit {
            field: None,
            text: none.text.clone()
        }]
    );
    assert_eq!(g.take("R1", &sample["json"], &Map::new()).unwrap_err(), none.text);
    // A cell that is not there.
    let missing = g.take_or_places("ZZ", &json!({}), &Map::new()).unwrap_err();
    assert_eq!(missing.text, "no such cell: ZZ");
    assert_eq!(
        missing.places,
        [flow::Misfit {
            field: None,
            text: "no such cell: ZZ".to_string()
        }]
    );
    // A return that does not fit in two places: the text names both, and each place is a field.
    let mut wrong = sample["json"].clone();
    wrong["progress"] = json!(7);
    wrong["flow_feedback"] = json!(7);
    let refused = g.take_or_places("R1", &wrong, &given).unwrap_err();
    let fields: Vec<Option<&str>> = refused.places.iter().map(|p| p.field.as_deref()).collect();
    assert_eq!(fields, [Some("progress"), Some("flow_feedback")], "{refused:?}");
    assert_eq!(
        refused.text,
        format!(
            "the return does not fit the schema of R1 in 2 places:\n{}\n{}",
            refused.places[0].text, refused.places[1].text
        )
    );
    assert_eq!(g.take("R1", &wrong, &given).unwrap_err(), refused.text);
}
