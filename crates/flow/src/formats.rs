//! The texts the flow gives an agent besides its rules: the forms every agent's return shares (`format:common`), a
//! cell's own answer (`format:<cell>`) and what each numbered input of a cell is (`inputs:<cell>`), written from the
//! compiled flow alone. The engine gives them in a round's message, with the value of each setting they name beside
//! it and the kinds of question the project cannot meet left out; the owner's page shows them as the flow has them.

use std::collections::{BTreeMap, HashMap};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::{AskNeed, CompiledCell, CompiledGraph};

/// The hash a content is known by, as a round's record names each conditional input it gave or referred to: SHA-256
/// of its JSON with every object's keys in order, so that the same content is one conditional input however its keys
/// were written.
pub fn hash(v: &Value) -> String {
    let mut s = String::new();
    canonical(v, &mut s);
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A value as JSON with every object's keys in order, so that the same content is one text however its keys were
/// written.
pub fn canonical(v: &Value, out: &mut String) {
    match v {
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(k.clone()).to_string());
                out.push(':');
                canonical(&o[k], out);
            }
            out.push('}');
        }
        Value::Array(xs) => {
            out.push('[');
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canonical(x, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

/// Merges one schema into another: the alternatives of a `oneOf` or `anyOf` are joined, each once, and the fields of
/// an object merged by name.
fn merge(a: &mut Value, b: &Value) {
    if a == b {
        return;
    }
    let (Value::Object(x), Value::Object(y)) = (a, b) else {
        return;
    };
    for (k, vb) in y {
        match x.get_mut(k) {
            Some(Value::Array(xs)) if k == "oneOf" || k == "anyOf" => {
                for v in vb.as_array().into_iter().flatten() {
                    if !xs.contains(v) {
                        xs.push(v.clone());
                    }
                }
            }
            Some(va) => merge(va, vb),
            None => {
                x.insert(k.clone(), vb.clone());
            }
        }
    }
}

/// A schema as the formats write it: `additionalProperties: false` left out, since the formats say once that no object
/// holds fields it does not list.
fn lean(schema: &Value) -> Value {
    match schema {
        Value::Object(o) => Value::Object(
            o.iter()
                .filter(|(k, v)| !(*k == "additionalProperties" && **v == Value::Bool(false)))
                .map(|(k, v)| (k.clone(), lean(v)))
                .collect(),
        ),
        Value::Array(xs) => Value::Array(xs.iter().map(lean).collect()),
        other => other.clone(),
    }
}

/// How long, as compact JSON, a part of a schema must be for the formats to give it once when it repeats.
const LARGE: usize = 300;

/// A schema with each large part it repeats given once, under `$defs`, and referred to by `$ref` wherever it
/// repeats, the longest first: a field several forms share, such as the kinds of ask under each role an agent may
/// ask, is written once.
fn once(schema: Value) -> Value {
    let mut s = schema;
    let mut defs = Map::new();
    loop {
        let mut seen: HashMap<String, (usize, String, Value)> = HashMap::new();
        repeats(&s, "part", true, &mut seen);
        let Some((key, (_, name, part))) = seen
            .into_iter()
            .filter(|(k, (n, _, _))| *n > 1 && k.len() >= LARGE)
            .max_by(|a, b| a.0.len().cmp(&b.0.len()).then_with(|| b.0.cmp(&a.0)))
        else {
            break;
        };
        let mut id = name.clone();
        let mut i = 2;
        while defs.contains_key(&id) {
            id = format!("{name}_{i}");
            i += 1;
        }
        refer(&mut s, &key, &json!({ "$ref": format!("#/$defs/{id}") }));
        defs.insert(id, part);
    }
    if let (Value::Object(o), false) = (&mut s, defs.is_empty()) {
        o.insert("$defs".into(), Value::Object(defs));
    }
    s
}

/// Counts every object in a schema by its canonical text, with the name of the field it describes; the root is not
/// counted.
fn repeats(v: &Value, name: &str, root: bool, seen: &mut HashMap<String, (usize, String, Value)>) {
    match v {
        Value::Object(o) => {
            if !root {
                let mut key = String::new();
                canonical(v, &mut key);
                seen.entry(key).or_insert_with(|| (0, name.to_string(), v.clone())).0 += 1;
            }
            for (k, x) in o {
                // A part is named by the field it describes: what holds fields, items or alternatives keeps its name.
                let structural = matches!(k.as_str(), "properties" | "items" | "oneOf" | "anyOf");
                repeats(x, if structural { name } else { k }, false, seen);
            }
        }
        Value::Array(xs) => {
            for x in xs {
                repeats(x, name, false, seen);
            }
        }
        _ => {}
    }
}

/// Puts `with` in place of every object whose canonical text is `key`.
fn refer(v: &mut Value, key: &str, with: &Value) {
    match v {
        Value::Object(o) => {
            let mut text = String::new();
            canonical(&Value::Object(o.clone()), &mut text);
            if text == key {
                *v = with.clone();
                return;
            }
            for x in o.values_mut() {
                refer(x, key, with);
            }
        }
        Value::Array(xs) => {
            for x in xs {
                refer(x, key, with);
            }
        }
        _ => {}
    }
}

fn quoted(names: &[String]) -> String {
    names.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ")
}

/// A schema without the kinds of question left out: each taken out of the list of values of the field that names a
/// kind, and its meaning out of that field's description, where the flow joins the kinds' meanings with "; ".
fn without_kinds(schema: &mut Value, gone: &[&AskNeed]) {
    match schema {
        Value::Object(o) => {
            if let Some(Value::Array(values)) = o.get_mut("enum")
                && values
                    .iter()
                    .any(|v| gone.iter().any(|k| v.as_str() == Some(k.kind.as_str())))
            {
                values.retain(|v| !gone.iter().any(|k| v.as_str() == Some(k.kind.as_str())));
                if let Some(Value::String(d)) = o.get_mut("description") {
                    for k in gone {
                        *d = d
                            .replace(&format!("; {}", k.text), "")
                            .replace(&format!("{}; ", k.text), "");
                    }
                }
            }
            for v in o.values_mut() {
                without_kinds(v, gone);
            }
        }
        Value::Array(xs) => {
            for x in xs {
                without_kinds(x, gone);
            }
        }
        _ => {}
    }
}

/// The inputs every agent's cell of those given may have besides its own, each by its name with what it is, in the
/// order the cells have them.
fn common_inputs<'g>(cells: &[&'g CompiledCell]) -> Vec<(&'g str, &'g str)> {
    let mut out: Vec<(&str, &str)> = Vec::new();
    for x in cells
        .iter()
        .copied()
        .filter(|c| c.kind == "agent")
        .flat_map(|c| &c.inputs)
    {
        if let Some(n) = x.name.as_deref().filter(|_| x.common)
            && !out.iter().any(|(m, _)| *m == n)
        {
            out.push((n, x.what.as_str()));
        }
    }
    out
}

/// The forms every agent's return in a graph shares, with each cell's own answer apart, and the inputs every agent's
/// cell may have: [`common_format_of`] every agent's cell.
pub fn common_format(g: &CompiledGraph, gone: &[&AskNeed]) -> String {
    let cells: Vec<&CompiledCell> = g.cells.iter().filter(|c| c.kind == "agent").collect();
    common_format_of(g, &cells, gone)
}

/// The forms the returns of the agents' cells given share, with each cell's own answer apart, and the inputs those
/// cells may have: what `format:common` gives a session whose rounds are at those cells. A form some cells have with
/// fewer alternatives (fewer tools, fewer roles to ask) is given with all of them; `format:<cell>` says which a cell
/// has. The kinds of question left out for the project are not in it.
pub fn common_format_of(g: &CompiledGraph, cells: &[&CompiledCell], gone: &[&AskNeed]) -> String {
    let mut forms: Vec<(String, Value)> = Vec::new();
    let mut every: Map<String, Value> = Map::new();
    let mut without: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut within = Value::Null;
    for c in cells.iter().filter(|c| c.kind == "agent") {
        let Some(props) = c.schema.as_ref().and_then(|s| s["properties"].as_object()) else {
            continue;
        };
        let alts = props
            .get(&g.within)
            .and_then(|w| w["anyOf"].as_array())
            .cloned()
            .unwrap_or_default();
        if within.is_null() {
            within = props
                .get(&g.within)
                .and_then(|w| w.get("description"))
                .cloned()
                .unwrap_or(Value::Null);
        }
        for (f, a) in c.forms.iter().zip(&alts) {
            let Some(name) = &f.field else { continue };
            match forms.iter_mut().find(|(n, _)| n == name) {
                Some((_, s)) => merge(s, a),
                None => forms.push((name.clone(), a.clone())),
            }
            let left = without.entry(name.clone()).or_default();
            for x in &f.without {
                if !left.contains(x) {
                    left.push(x.clone());
                }
            }
        }
        for (k, v) in props.iter().filter(|(k, _)| **k != g.within) {
            match every.get_mut(k) {
                Some(s) => merge(s, v),
                None => {
                    every.insert(k.clone(), v.clone());
                }
            }
        }
    }
    // A return's shape, each field a stand-in of its type: it shows where the fields every return holds sit.
    let stand_in = |v: &Value| match v["type"].as_str() {
        Some("array") => "[…]",
        Some("object") => "{…}",
        Some("string") => "\"…\"",
        _ => "…",
    };
    // The short fields first and the forms' field last, as the model is to write them: after the long answer, only
    // closing brackets are left.
    let shape: Vec<String> = every
        .iter()
        .map(|(k, v)| format!("\"{k}\": {}", stand_in(v)))
        .chain(std::iter::once(format!("\"{}\": {{…}}", g.within)))
        .collect();
    let every_names: Vec<String> = every.keys().cloned().collect();
    let mut schema = Map::new();
    schema.extend(every);
    schema.insert(
        g.within.clone(),
        json!({ "description": within, "anyOf": forms.iter().map(|x| x.1.clone()).collect::<Vec<_>>() }),
    );
    let names: Vec<String> = forms.iter().map(|x| x.0.clone()).collect();
    let leaves: Vec<String> = without
        .iter()
        .filter(|(_, xs)| !xs.is_empty())
        .map(|(f, xs)| format!("a return of the form `{f}` leaves out {}", quoted(xs)))
        .collect();
    let leaves = if leaves.is_empty() {
        String::new()
    } else {
        format!("; {}", leaves.join("; "))
    };
    let mut parts = vec![format!(
        "The forms every agent's return shares. A return is one JSON object; its field `{w}` holds exactly one form: \
         the cell's own answer (`format:<cell>`), or one of these that `format:<cell>` names: {forms}. Every return \
         also holds {every}{leaves}. Its shape is `{{{shape}}}`: the fields every return holds sit beside `{w}`, not in \
         it.",
        shape = shape.join(", "),
        w = g.within,
        forms = quoted(&names),
        every = quoted(&every_names),
    )];
    if !g.write.is_empty() {
        let lines: Vec<String> = g.write.iter().map(|l| format!("- {l}")).collect();
        parts.push(format!("How a return is written:\n{}", lines.join("\n")));
    }
    let inputs = common_inputs(cells);
    if !inputs.is_empty() {
        let lines: Vec<String> = inputs.iter().map(|(n, what)| format!("- {n}: {what}")).collect();
        parts.push(format!(
            "The inputs every agent's cell may have besides its own, which each cell's `inputs:<cell>` names by their \
             number only:\n{}",
            lines.join("\n")
        ));
    }
    let mut schema = lean(&Value::Object(schema));
    without_kinds(&mut schema, gone);
    let mut schema = once(schema);
    // The parts of the cells' own answers given once: each as the first of the cells that holds it has it.
    let shared: Vec<(&String, Value)> = g
        .once
        .iter()
        .filter_map(|name| {
            let part = cells
                .iter()
                .find_map(|c| crate::compiled::own_answer(c, &g.within)?.get("properties")?.get(name))?;
            let mut part = lean(part);
            without_kinds(&mut part, gone);
            Some((name, part))
        })
        .collect();
    if !shared.is_empty() {
        let names: Vec<String> = shared.iter().map(|(n, _)| (*n).clone()).collect();
        parts.push(format!(
            "A part several cells' own answers hold alike is given once, here under `$defs`: {}. `format:<cell>` refers \
             to it as `format:common#/$defs/<name>`.",
            quoted(&names)
        ));
        if let Value::Object(o) = &mut schema {
            let defs = o.entry("$defs").or_insert_with(|| json!({}));
            for (name, part) in shared {
                defs[name.as_str()] = part;
            }
        }
    }
    parts.push(format!(
        "Each field, as JSON Schema; no object holds fields it does not list, and a part used in more than one place \
         is under `$defs`:\n\n{schema}"
    ));
    parts.join("\n\n")
}

/// A cell's own answer, and which of the forms every return shares the cell has: what `format:<cell>` gives. The
/// kinds of question left out for the project are not in it.
pub fn cell_format(g: &CompiledGraph, cell: &CompiledCell, gone: &[&AskNeed]) -> String {
    let alts = cell
        .schema
        .as_ref()
        .and_then(|s| s["properties"][&g.within]["anyOf"].as_array())
        .cloned()
        .unwrap_or_default();
    let mut own = Value::Null;
    let mut others = Vec::new();
    for (f, a) in cell.forms.iter().zip(&alts) {
        match &f.field {
            None => own = a.clone(),
            Some(name) if *name == "tools" || a["properties"][name]["items"]["oneOf"].is_array() => {
                others.push(format!("`{name}` (of {})", cell.tools.join(", ")));
            }
            Some(name) => {
                let roles: Vec<&str> = a["properties"][name]["oneOf"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|b| b["properties"].as_object())
                    .flat_map(|p| p.values().filter_map(|v| v["const"].as_str()))
                    .collect();
                if roles.is_empty() {
                    others.push(format!("`{name}`"));
                } else {
                    others.push(format!("`{name}` (to {})", roles.join(", ")));
                }
            }
        }
    }
    let mut own = lean(&own);
    without_kinds(&mut own, gone);
    // A part given once in the forms every return shares is referred to there.
    for name in &g.once {
        if let Some(part) = own.get_mut("properties").and_then(|p| p.get_mut(name)) {
            *part = json!({ "$ref": format!("format:common#/$defs/{name}") });
        }
    }
    format!(
        "The answer of cell {id} ({title}), which `{w}` holds, as JSON Schema; instead of it, a return of this cell may \
         hold these forms of `format:common`: {others}.\n\n{json}",
        id = cell.id,
        title = cell.title,
        w = g.within,
        others = others.join(", "),
        json = once(own),
    )
}

/// What each numbered input of a cell is, the inputs the agent reads itself left out: what `inputs:<cell>` gives, and
/// what the numbers in a message's list of inputs to work from stand for. An input every agent's cell has is named
/// only: `format:common` says what it is.
pub fn cell_inputs(cell: &CompiledCell) -> String {
    let lines: Vec<String> = cell
        .inputs
        .iter()
        .filter(|x| !x.from.iter().all(|s| *s == crate::format::Src::Itself))
        .enumerate()
        .map(|(i, x)| match x.name.as_deref().filter(|_| x.common) {
            Some(name) => format!("{}. {name}, which format:common describes", i + 1),
            None => format!("{}. {}", i + 1, x.what),
        })
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    format!(
        "What each input of cell {} ({}) is, by the number a round's list of inputs gives it:\n{}",
        cell.id,
        cell.title,
        lines.join("\n")
    )
}
