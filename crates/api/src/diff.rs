//! What changed from one compiled flow to another, cell by cell: each cell added or removed, and in each cell kept,
//! what changed of its title, kind, line and stage, its inputs, rules, prompt, forms and return fields, its rows,
//! checks, what it does every time and what it hands over; and in each graph, its start and end, its asking part,
//! its rules, counters and interjection, and what it does with rounds that end without a return it takes.

use std::collections::BTreeMap;

use flow::{Compiled, CompiledCell, CompiledGraph, CompiledInput};
use serde::Serialize;
use serde_json::{Value, json};

use crate::view::returns;
use crate::{CellDiff, Change, ChangeKind, Field, FlowDiff, GraphDiff, Part};

pub fn diff(from: &Compiled, to: &Compiled) -> FlowDiff {
    let mut graphs = Vec::new();
    for g in &to.graphs {
        match from.graph(&g.name) {
            Some(old) => {
                let d = graph(old, g);
                if !d.changes.is_empty() || !d.cells.is_empty() {
                    graphs.push(d);
                }
            }
            None => graphs.push(GraphDiff {
                name: g.name.clone(),
                status: ChangeKind::Added,
                changes: Vec::new(),
                cells: Vec::new(),
            }),
        }
    }
    for g in from.graphs.iter().filter(|g| to.graph(&g.name).is_none()) {
        graphs.push(GraphDiff {
            name: g.name.clone(),
            status: ChangeKind::Removed,
            changes: Vec::new(),
            cells: Vec::new(),
        });
    }
    FlowDiff { graphs }
}

fn value<T: Serialize>(x: &T) -> Value {
    serde_json::to_value(x).unwrap_or(Value::Null)
}

/// A change when the two differ.
fn changed<T: Serialize>(out: &mut Vec<Change>, part: Part, key: &str, before: &T, after: &T) {
    let (b, a) = (value(before), value(after));
    if b != a {
        out.push(Change {
            part,
            key: key.to_string(),
            before: Some(b),
            after: Some(a),
        });
    }
}

/// The changes between two lists of named things: each added, removed, or changed under its name.
fn by_name<T: Serialize>(out: &mut Vec<Change>, part: Part, before: &[(String, T)], after: &[(String, T)]) {
    for (k, a) in after {
        match before.iter().find(|(n, _)| n == k) {
            Some((_, b)) => changed(out, part, k, b, a),
            None => out.push(Change {
                part,
                key: k.clone(),
                before: None,
                after: Some(value(a)),
            }),
        }
    }
    for (k, b) in before {
        if !after.iter().any(|(n, _)| n == k) {
            out.push(Change {
                part,
                key: k.clone(),
                before: Some(value(b)),
                after: None,
            });
        }
    }
}

fn graph(from: &CompiledGraph, to: &CompiledGraph) -> GraphDiff {
    let mut changes = Vec::new();
    changed(&mut changes, Part::Start, "", &from.start, &to.start);
    changed(&mut changes, Part::End, "", &from.end, &to.end);
    changed(&mut changes, Part::Asking, "", &from.asking, &to.asking);
    changed(&mut changes, Part::Receive, "", &from.receive, &to.receive);
    let named =
        |g: &CompiledGraph| -> Vec<(String, Value)> { g.rules.iter().map(|r| (r.name.clone(), value(r))).collect() };
    by_name(&mut changes, Part::Rule, &named(from), &named(to));
    let counters =
        |g: &CompiledGraph| -> Vec<(String, Value)> { g.counters.iter().map(|c| (c.name.clone(), value(c))).collect() };
    by_name(&mut changes, Part::Counter, &counters(from), &counters(to));
    changed(&mut changes, Part::Interjection, "", &from.interject, &to.interject);
    changed(&mut changes, Part::Rounds, "", &from.rounds, &to.rounds);
    let mut cells = Vec::new();
    for c in &to.cells {
        match from.cell(&c.id) {
            Some(old) => {
                let ch = cell(from, old, to, c);
                if !ch.is_empty() {
                    cells.push(CellDiff {
                        id: c.id.clone(),
                        title: c.title.clone(),
                        status: ChangeKind::Changed,
                        changes: ch,
                    });
                }
            }
            None => cells.push(CellDiff {
                id: c.id.clone(),
                title: c.title.clone(),
                status: ChangeKind::Added,
                changes: Vec::new(),
            }),
        }
    }
    for c in from.cells.iter().filter(|c| to.cell(&c.id).is_none()) {
        cells.push(CellDiff {
            id: c.id.clone(),
            title: c.title.clone(),
            status: ChangeKind::Removed,
            changes: Vec::new(),
        });
    }
    GraphDiff {
        name: to.name.clone(),
        status: ChangeKind::Changed,
        changes,
        cells,
    }
}

/// What changed in a cell kept from one flow to the other.
fn cell(gf: &CompiledGraph, from: &CompiledCell, gt: &CompiledGraph, to: &CompiledCell) -> Vec<Change> {
    let mut out = Vec::new();
    changed(&mut out, Part::Title, "", &from.title, &to.title);
    changed(&mut out, Part::Kind, "", &from.kind, &to.kind);
    changed(&mut out, Part::Gate, "", &from.gate, &to.gate);
    changed(&mut out, Part::Line, "", &from.line, &to.line);
    changed(&mut out, Part::Stage, "", &from.stage, &to.stage);
    changed(&mut out, Part::Step, "", &from.step, &to.step);
    changed(&mut out, Part::Needs, "", &from.needs, &to.needs);
    inputs(&mut out, &from.inputs, &to.inputs);
    let rules =
        |c: &CompiledCell| -> Vec<(String, Value)> { c.rules.iter().map(|r| (r.name.clone(), json!(true))).collect() };
    by_name(&mut out, Part::Rule, &rules(from), &rules(to));
    changed(&mut out, Part::Prompt, "", &from.prompt, &to.prompt);
    changed(&mut out, Part::PromptBy, "", &from.prompt_by, &to.prompt_by);
    changed(&mut out, Part::Forms, "", &from.forms, &to.forms);
    by_name(&mut out, Part::Field, &fields_of(gf, from), &fields_of(gt, to));
    let rows =
        |c: &CompiledCell| -> Vec<(String, Value)> { c.rows.iter().map(|r| (r.name.clone(), value(r))).collect() };
    by_name(&mut out, Part::Row, &rows(from), &rows(to));
    let order = |c: &CompiledCell| -> Vec<String> { c.rows.iter().map(|r| r.name.clone()).collect() };
    // The rows kept, in another order: their numbers changed.
    let (of, ot) = (order(from), order(to));
    let kept_f: Vec<&String> = of.iter().filter(|n| ot.contains(n)).collect();
    let kept_t: Vec<&String> = ot.iter().filter(|n| of.contains(n)).collect();
    if kept_f != kept_t {
        changed(&mut out, Part::RowOrder, "", &of, &ot);
    }
    let checks = |c: &CompiledCell| -> Vec<(String, Value)> {
        c.checks
            .iter()
            .map(|k| {
                (
                    format!(
                        "{}{}",
                        k.check,
                        k.at.as_ref().map(|a| format!(" @ {a}")).unwrap_or_default()
                    ),
                    value(k),
                )
            })
            .collect()
    };
    by_name(&mut out, Part::Check, &checks(from), &checks(to));
    let always = |c: &CompiledCell| -> Vec<(String, Value)> {
        c.always
            .iter()
            .map(|a| {
                let what = a.view.as_deref().or(a.action.as_deref()).unwrap_or_default();
                (format!("{} → {what}", a.at), value(a))
            })
            .collect()
    };
    by_name(&mut out, Part::Always, &always(from), &always(to));
    changed(&mut out, Part::Handoff, "", &from.handoff, &to.handoff);
    changed(&mut out, Part::Shown, "", &from.shown, &to.shown);
    let inenums =
        |c: &CompiledCell| -> Vec<(String, Value)> { c.inenums.iter().map(|(k, v)| (k.clone(), value(v))).collect() };
    by_name(&mut out, Part::Inenum, &inenums(from), &inenums(to));
    out
}

/// The changes between two cells' inputs. An input is matched by what it is, then by its sources: one whose text
/// changed but whose sources did not is the same input changed.
fn inputs(out: &mut Vec<Change>, from: &[CompiledInput], to: &[CompiledInput]) {
    let mut left: Vec<&CompiledInput> = from.iter().collect();
    let mut added: Vec<&CompiledInput> = Vec::new();
    let mut pairs: Vec<(&CompiledInput, &CompiledInput)> = Vec::new();
    for x in to {
        match left.iter().position(|y| y.what == x.what) {
            Some(i) => pairs.push((left.remove(i), x)),
            None => added.push(x),
        }
    }
    let mut unmatched = Vec::new();
    for x in added {
        match left.iter().position(|y| value(&y.from) == value(&x.from)) {
            Some(i) => pairs.push((left.remove(i), x)),
            None => unmatched.push(x),
        }
    }
    for (b, a) in pairs {
        changed(out, Part::Input, &a.what, b, a);
    }
    for a in unmatched {
        out.push(Change {
            part: Part::Input,
            key: a.what.clone(),
            before: None,
            after: Some(value(a)),
        });
    }
    for b in left {
        out.push(Change {
            part: Part::Input,
            key: b.what.clone(),
            before: Some(value(b)),
            after: None,
        });
    }
}

/// Every field of a cell's return by its path, with what it is, without what reads it or the fields it holds.
fn fields_of(g: &CompiledGraph, c: &CompiledCell) -> Vec<(String, Value)> {
    fn walk(fs: &[Field], form: &str, out: &mut BTreeMap<String, Value>) {
        for f in fs {
            out.insert(
                format!("{form}{}", f.path),
                json!({
                    "kind": f.kind, "item": f.item, "desc": f.desc, "optional": f.optional, "min": f.min,
                    "values": f.values, "disc": f.disc,
                    "branches": f.branches.iter().map(|b| (&b.value, &b.desc)).collect::<Vec<_>>(),
                }),
            );
            walk(&f.fields, form, out);
            walk(&f.common, form, out);
            for b in &f.branches {
                walk(&b.fields, form, out);
            }
        }
    }
    let r = returns(g, c);
    let mut out = BTreeMap::new();
    for f in &r.forms {
        walk(&f.fields, "", &mut out);
    }
    walk(&r.every, "", &mut out);
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compiled() -> Compiled {
        let relations = db::relations(db::SCHEMA).unwrap();
        flow::compile(flow::FLOW, &flow::Context { relations }).unwrap()
    }

    #[test]
    fn a_flow_against_itself_has_no_changes() {
        let c = compiled();
        assert_eq!(diff(&c, &c), FlowDiff { graphs: Vec::new() });
    }

    #[test]
    fn each_change_is_named_by_its_cell_and_part() {
        let from = compiled();
        let mut to = from.clone();
        let g = &mut to.graphs[0];
        let start = g.start.clone();
        let gone = g.cells.iter().position(|c| c.kind == "agent" && c.id != start).unwrap();
        let removed = g.cells.remove(gone).id;
        let c = g.cells.iter_mut().find(|c| c.kind == "agent").unwrap();
        let id = c.id.clone();
        c.prompt.push("A new line.".into());
        let input = c.inputs.remove(0).what;
        let row = c.rows[0].name.clone();
        c.rows[0].main = !c.rows[0].main;
        let d = diff(&from, &to);
        assert_eq!(d.graphs.len(), 1);
        let cells = &d.graphs[0].cells;
        let mine = cells.iter().find(|x| x.id == id).unwrap();
        assert_eq!(mine.status, ChangeKind::Changed);
        let has = |part: Part, key: &str| mine.changes.iter().any(|x| x.part == part && x.key == key);
        assert!(has(Part::Prompt, ""));
        assert!(has(Part::Input, &input));
        assert!(has(Part::Row, &row));
        let input_change = mine.changes.iter().find(|x| x.part == Part::Input).unwrap();
        assert!(input_change.after.is_none());
        let last = cells.last().unwrap();
        assert_eq!((last.id.as_str(), last.status), (removed.as_str(), ChangeKind::Removed));
    }
}
