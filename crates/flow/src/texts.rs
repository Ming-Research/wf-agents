//! The cells the texts of the flow name. A text names a cell by the word cell and the cell's name in backticks, its
//! stage before a dot where its stage has others, as cell `A4`, or several after the word cells, as cells
//! `T2`, `M2` and `L1`; a field or a value in backticks, with no such word before it, names
//! no cell. Every cell a text names is a cell of its graph: a text of a graph names that graph's cells, and a text
//! every graph shares, the cells of some graph.
//!
//! And the texts that reach no agent and show on no chart ([`unread`]), which the flow does not keep.

use std::collections::BTreeSet;

use serde_json::Value;

/// What joins one name of a list to the next.
const JOINS: [&str; 5] = [", and `", ", or `", ", `", " and `", " or `"];

/// The names of the cells a text names, in the order it names them.
pub fn cells_named(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for (i, _) in text.match_indices(['c', 'C']) {
        if text[..i]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            continue;
        }
        let after = &text[i + 1..];
        let Some(mut rest) = after.strip_prefix("ells `").or_else(|| after.strip_prefix("ell `")) else {
            continue;
        };
        while let Some(end) = rest.find('`') {
            let name = &rest[..end];
            let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
            if !name.starts_with(word) || !name.ends_with(word) || !name.chars().all(|c| word(c) || c == '.') {
                break;
            }
            out.push(name);
            rest = &rest[end + 1..];
            match JOINS.iter().find_map(|j| rest.strip_prefix(*j)) {
                Some(r) => rest = r,
                None => break,
            }
        }
    }
    out
}

/// Every text of the flow that names a cell its graph does not have: where the text is, and the name.
pub(crate) fn unknown_cells(flow: &Value) -> Vec<String> {
    let cells_of = |f: &Value| -> BTreeSet<String> {
        f["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|n| n["id"].as_str().map(str::to_string))
            .collect()
    };
    let graphs: Vec<(String, BTreeSet<String>)> = flow["flows"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| (f["name"].as_str().unwrap_or_default().to_string(), cells_of(f)))
        .collect();
    let mut out = Vec::new();
    let Some(top) = flow.as_object() else { return out };
    for (key, v) in top {
        if key != "flows" {
            let mut texts = Vec::new();
            walk(v, key.clone(), &mut texts);
            for (at, name) in texts {
                if !graphs.iter().any(|(_, cells)| cells.contains(name)) {
                    out.push(format!("{at} names cell `{name}`, which no graph has"));
                }
            }
        }
    }
    for (f, (graph, cells)) in flow["flows"].as_array().into_iter().flatten().zip(&graphs) {
        let mut texts = Vec::new();
        walk(f, format!("flows.{graph}"), &mut texts);
        for (at, name) in texts {
            if !cells.contains(name) {
                out.push(format!("{graph}: {at} names cell `{name}`, which its graph has not"));
            }
        }
    }
    out
}

/// Takes out of a flow every text that reaches no agent and shows on no chart, and says where each was. The compiled
/// flow carries every text the format holds to an agent or to the chart (`crate::format`); these are the texts a flow
/// written before held that it does not carry: a field's title and the titles of a gate's answer and of its why, which
/// the owner's page headed them with; the name of an input that not every agent's cell has, which the page headed the
/// input with; and what the sessions, the lines, the stages and the limits were, and the rules of the decision card,
/// written for people reading the flow. A flow that runs holds none of them, and one only shown is read without them.
/// Where each was is said as [`unknown_cells`] says where a text is.
pub(crate) fn unread(flow: &mut Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(Value::Object(shapes)) = flow.get_mut("shapes") {
        for (name, shape) in shapes.iter_mut() {
            titles(shape, &format!("shapes.{name}"), &mut out);
        }
    }
    for (i, tool) in items(flow.get_mut("tools")) {
        let at = place("tools", i, tool);
        for key in ["in", "out"] {
            fields(tool.get_mut(key), &format!("{at}.{key}"), &mut out);
        }
    }
    for (i, f) in items(flow.get_mut("flows")) {
        let at = place("flows", i, f);
        take(f, "designCard", &at, &mut out);
        if let Some(s) = f.get_mut("sessions") {
            let at = format!("{at}.sessions");
            take(s, "rules", &at, &mut out);
            for (j, line) in items(s.get_mut("lines")) {
                let at = place(&format!("{at}.lines"), j, line);
                take(line, "desc", &at, &mut out);
            }
        }
        if let Some(s) = f.get_mut("stages") {
            let at = format!("{at}.stages");
            take(s, "rules", &at, &mut out);
            for (j, stage) in items(s.get_mut("list")) {
                let at = place(&format!("{at}.list"), j, stage);
                take(stage, "desc", &at, &mut out);
            }
        }
        if let Some(l) = f.get_mut("limits") {
            for key in ["where", "waits", "over"] {
                take(l, key, &format!("{at}.limits"), &mut out);
            }
        }
        if let Some(g) = f.get_mut("gate") {
            let at = format!("{at}.gate");
            for key in ["answerTitle", "whyTitle"] {
                take(g, key, &at, &mut out);
            }
            names(g.get_mut("inputs"), &format!("{at}.inputs"), &mut out);
        }
        if let Some(c) = f.get_mut("agentCommon") {
            fields(c.get_mut("out"), &format!("{at}.agentCommon.out"), &mut out);
        }
        for (j, role) in items(f.get_mut("asking").and_then(|a| a.get_mut("roles"))) {
            let at = place(&format!("{at}.asking.roles"), j, role);
            fields(role.get_mut("fields"), &format!("{at}.fields"), &mut out);
        }
        for (j, n) in items(f.get_mut("nodes")) {
            let at = place(&format!("{at}.nodes"), j, n);
            names(n.get_mut("inputs"), &format!("{at}.inputs"), &mut out);
            fields(n.get_mut("out"), &format!("{at}.out"), &mut out);
        }
    }
    out
}

/// The items of a list, each with its place; none where there is no list.
fn items(list: Option<&mut Value>) -> impl Iterator<Item = (usize, &mut Value)> {
    list.and_then(Value::as_array_mut).into_iter().flatten().enumerate()
}

/// Where an item of a list is: by its id, its name or its value when it has one, else by its place.
fn place(at: &str, i: usize, x: &Value) -> String {
    match ["id", "name", "value"]
        .iter()
        .find_map(|k| x.get(*k).and_then(Value::as_str))
    {
        Some(k) => format!("{at}.{k}"),
        None => format!("{at}[{i}]"),
    }
}

/// Takes `key` out of an object, saying where it was.
fn take(v: &mut Value, key: &str, at: &str, out: &mut Vec<String>) {
    if v.as_object_mut().and_then(|o| o.shift_remove(key)).is_some() {
        out.push(format!("{at}.{key}: text that reaches no agent and shows on no chart"));
    }
}

/// Takes the name out of each input of a list, each by its place, as the name it has is the text taken.
fn names(inputs: Option<&mut Value>, at: &str, out: &mut Vec<String>) {
    for (i, x) in items(inputs) {
        take(x, "name", &format!("{at}[{i}]"), out);
    }
}

/// Takes the titles out of each field of a list.
fn fields(list: Option<&mut Value>, at: &str, out: &mut Vec<String>) {
    for (i, f) in items(list) {
        let at = place(at, i, f);
        titles(f, &at, out);
    }
}

/// Takes the title out of a field and out of every field it holds: its own fields, its item, the fields every branch
/// has, each branch's own, and what it was earlier.
fn titles(f: &mut Value, at: &str, out: &mut Vec<String>) {
    take(f, "title", at, out);
    for key in ["fields", "common"] {
        fields(f.get_mut(key), &format!("{at}.{key}"), out);
    }
    if let Some(item) = f.get_mut("item") {
        titles(item, &format!("{at}.item"), out);
    }
    for (i, b) in items(f.get_mut("branches")) {
        let at = place(&format!("{at}.branches"), i, b);
        fields(b.get_mut("fields"), &format!("{at}.fields"), out);
    }
    if let Some(e) = f.get_mut("earlier").filter(|e| e.is_object()) {
        titles(e, &format!("{at}.earlier"), out);
    }
}

/// Each cell every text under `v` names, with where the text is: an item of a list by its id or name when it has one,
/// else by its place.
fn walk<'v>(v: &'v Value, at: String, out: &mut Vec<(String, &'v str)>) {
    match v {
        Value::String(s) => out.extend(cells_named(s).into_iter().map(|n| (at.clone(), n))),
        Value::Array(xs) => {
            for (i, x) in xs.iter().enumerate() {
                let key = ["id", "name"].iter().find_map(|k| x[*k].as_str());
                let at = match key {
                    Some(k) => format!("{at}.{k}"),
                    None => format!("{at}[{i}]"),
                };
                walk(x, at, out);
            }
        }
        Value::Object(o) => {
            for (k, x) in o {
                walk(x, format!("{at}.{k}"), out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A cell is named after the word cell or cells, in backticks, alone or in a list; a field in backticks, a word
    // that holds cell, and a name that is not one are not.
    #[test]
    fn a_text_names_a_cell_after_the_word_cell() {
        assert_eq!(cells_named("goes to cell `A4` at once"), ["A4"]);
        assert_eq!(
            cells_named("Cells `T2`, `M2` and `L1` count, as cell `a` or `b` does"),
            ["T2", "M2", "L1", "a", "b"]
        );
        assert!(cells_named("hand in `tools`, without `progress`; the cell that asked").is_empty());
        assert!(cells_named("an excellent `x`, a subcell `y`, cell `format:common`, cell `.x`, cell 11").is_empty());
    }

    // A text of a graph names a cell of that graph, and a text every graph shares a cell of some graph; each that
    // does not is told with where it is.
    #[test]
    fn a_text_that_names_no_cell_of_its_graph_is_found() {
        let flow = serde_json::json!({
            "engine": { "steps": { "s": "Goes on at cell `b`, then cell `c`." } },
            "flows": [
                { "name": "one", "nodes": [{ "id": "a", "prompt": ["After cell `a`, cells `b` and `x`."] }] },
                { "name": "two", "nodes": [{ "id": "b" }] }
            ]
        });
        assert_eq!(
            unknown_cells(&flow),
            [
                "engine.steps.s names cell `c`, which no graph has",
                "one: flows.one.nodes.a.prompt[0] names cell `b`, which its graph has not",
                "one: flows.one.nodes.a.prompt[0] names cell `x`, which its graph has not",
            ]
        );
    }

    /// What [`unread`] says after where a text was.
    const UNREAD: &str = ": text that reaches no agent and shows on no chart";

    // Each text that reaches no agent and shows on no chart is taken out of a flow and found, with where it was: a
    // field's title at any depth, in a shape, a tool, a role's question or a cell's return; the titles of a gate's answer
    // and why; the name of an input of a cell or a gate; and what was written of the sessions, the lines, the stages,
    // the limits and the decision card for people. What reaches an agent or the chart stays: a field's name, a cell's
    // title, the name of an input every agent's cell has, a button's title and what it does.
    #[test]
    fn a_text_that_reaches_no_agent_and_shows_on_no_chart_is_taken_out_and_found() {
        let mut flow = serde_json::json!({
            "shapes": { "s": { "title": "S", "type": "obj", "fields": [{ "name": "a", "title": "A", "type": "text" }] } },
            "tools": [{ "name": "ci", "in": [{ "name": "head", "title": "Head", "type": "text" }], "out": [] }],
            "flows": [{
                "name": "one",
                "designCard": ["A rule."],
                "sessions": { "lines": [{ "name": "l", "desc": "A line.", "colour": "blue" }], "rules": ["A rule."] },
                "stages": { "rules": ["A rule."], "list": [{ "name": "st", "desc": "A stage.", "nodes": ["R1"] }] },
                "limits": { "where": "Here.", "waits": "Waits.", "over": "Over.", "counters": [] },
                "gate": { "answer": "The answer.", "answerTitle": "Answer", "whyTitle": "Why",
                          "inputs": [{ "name": "Gate's", "what": "What it is." }] },
                "agentCommon": { "inputs": [{ "name": "Every cell's", "what": "What it is." }],
                                 "out": [{ "name": "ask", "title": "Ask", "type": "text" }] },
                "asking": { "roles": [{ "name": "advisor", "fields": [{ "name": "q", "title": "Q", "type": "text" }] }] },
                "nodes": [{
                    "id": "R1", "title": "Read", "inputs": [{ "name": "Cell's", "what": "What it is." }],
                    "choices": [{ "value": "on", "title": "Go on", "desc": "It goes on." }],
                    "out": [{ "name": "u", "title": "U", "type": "list", "item": { "type": "union", "by": "v", "branches": [
                        { "value": "b", "fields": [{ "name": "c", "title": "C", "type": "text",
                                                      "earlier": { "type": "text", "title": "E" } }] }
                    ] } }]
                }]
            }]
        });
        let found = unread(&mut flow);
        assert!(found.iter().all(|l| l.ends_with(UNREAD)), "{found:?}");
        let places: Vec<&str> = found.iter().map(|l| l.trim_end_matches(UNREAD)).collect();
        assert_eq!(
            places,
            [
                "shapes.s.title",
                "shapes.s.fields.a.title",
                "tools.ci.in.head.title",
                "flows.one.designCard",
                "flows.one.sessions.rules",
                "flows.one.sessions.lines.l.desc",
                "flows.one.stages.rules",
                "flows.one.stages.list.st.desc",
                "flows.one.limits.where",
                "flows.one.limits.waits",
                "flows.one.limits.over",
                "flows.one.gate.answerTitle",
                "flows.one.gate.whyTitle",
                "flows.one.gate.inputs[0].name",
                "flows.one.agentCommon.out.ask.title",
                "flows.one.asking.roles.advisor.fields.q.title",
                "flows.one.nodes.R1.inputs[0].name",
                "flows.one.nodes.R1.out.u.title",
                "flows.one.nodes.R1.out.u.item.branches.b.fields.c.title",
                "flows.one.nodes.R1.out.u.item.branches.b.fields.c.earlier.title",
            ]
        );
        let kept = serde_json::json!({
            "shapes": { "s": { "type": "obj", "fields": [{ "name": "a", "type": "text" }] } },
            "tools": [{ "name": "ci", "in": [{ "name": "head", "type": "text" }], "out": [] }],
            "flows": [{
                "name": "one",
                "sessions": { "lines": [{ "name": "l", "colour": "blue" }] },
                "stages": { "list": [{ "name": "st", "nodes": ["R1"] }] },
                "limits": { "counters": [] },
                "gate": { "answer": "The answer.", "inputs": [{ "what": "What it is." }] },
                "agentCommon": { "inputs": [{ "name": "Every cell's", "what": "What it is." }],
                                 "out": [{ "name": "ask", "type": "text" }] },
                "asking": { "roles": [{ "name": "advisor", "fields": [{ "name": "q", "type": "text" }] }] },
                "nodes": [{
                    "id": "R1", "title": "Read", "inputs": [{ "what": "What it is." }],
                    "choices": [{ "value": "on", "title": "Go on", "desc": "It goes on." }],
                    "out": [{ "name": "u", "type": "list", "item": { "type": "union", "by": "v", "branches": [
                        { "value": "b", "fields": [{ "name": "c", "type": "text", "earlier": { "type": "text" } }] }
                    ] } }]
                }]
            }]
        });
        assert_eq!(flow, kept);
        assert!(unread(&mut flow).is_empty());
    }

    // A flow written before, with such a text, as at an older commit the flow page compares with, is shown as the flow
    // without it; as a flow to run it is refused, each such text named by where it is.
    #[test]
    fn a_flow_with_a_text_that_reaches_no_one_is_shown_without_it_and_does_not_run() {
        let cx = crate::Context {
            relations: db::relations(db::SCHEMA).unwrap(),
            tables: db::index_of(db::SCHEMA).unwrap(),
        };
        let mut v: Value = serde_json::from_str(crate::FLOW).unwrap();
        let r1 = v["flows"][0]["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|n| n["id"] == "R1")
            .unwrap();
        r1["inputs"][0]["name"] = "The item itself".into();
        r1["out"][0]["title"] = "Understanding".into();
        let field = r1["out"][0]["name"].as_str().unwrap().to_string();
        let text = v.to_string();
        let refused = crate::compile(&text, &cx).unwrap_err().problems;
        assert_eq!(
            refused,
            [
                format!("flows.engineer.nodes.R1.inputs[0].name{UNREAD}"),
                format!("flows.engineer.nodes.R1.out.{field}.title{UNREAD}"),
            ]
        );
        let shown = crate::compile_shown(&text, &cx).unwrap_or_else(|e| panic!("{e}"));
        let runs = crate::compile(crate::FLOW, &cx).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(shown.digest(), runs.digest());
    }
}
