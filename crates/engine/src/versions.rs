//! A work on an older flow version, when it goes on. It moves to the version the engine runs, the same work at the same
//! cell in the same sessions, when the cell it stands at is unchanged there and every return it handed in keeps its
//! shape; otherwise a new work on that version takes its item over ([`crate::Engine`]'s takeover).
//!
//! The cell is unchanged when the version has a cell of the same id, of the same kind and line, whose inputs have the
//! same names and sources and whose rows the same names, conditions and targets, each in the same order; the cells it
//! goes back to, the cell it left for the asking part and those waiting for an answer, must still be cells of the
//! same kind. A return keeps its shape when its cell's schema has the same field names and types, the same values
//! a field may take and the same fields required, whatever its descriptions say. Every return the work handed in
//! counts, not only those the cells ahead name among their inputs: every agent's cell reads the cell it came from,
//! which may be any cell the work ran, and the engine's own steps read returns by their cell, so any earlier return may
//! be read again.

use std::collections::BTreeSet;

use flow::{CompiledCell, CompiledGraph, Position};
use serde_json::{Map, Value};

/// Why a work standing at `pos`, having handed in returns at the cells `returned`, cannot move from graph `old` to
/// the same graph `new` of the version the engine runs, as what the version changed; none when it can.
pub fn why_not(
    old: &CompiledGraph,
    new: &CompiledGraph,
    pos: &Position,
    returned: &BTreeSet<String>,
) -> Option<String> {
    let at = pos.at.as_str();
    let (Some(was), Some(is)) = (old.cell(at), new.cell(at)) else {
        return Some(format!("cell {at} is gone"));
    };
    if let Some(what) = changed(was, is) {
        return Some(format!("cell {at} changed ({what})"));
    }
    for back in pos.stopped.iter().chain(&pos.callers) {
        match (old.cell(back), new.cell(back)) {
            (Some(x), Some(y)) if x.kind == y.kind => {}
            _ => {
                return Some(format!(
                    "cell {back}, which the work goes back to, is gone or of another kind"
                ));
            }
        }
    }
    for cell in returned {
        let shape_in = |g: &CompiledGraph| g.cell(cell).map(|c| c.schema.as_ref().map(shape));
        let was = shape_in(old);
        if was.is_none() || was != shape_in(new) {
            return Some(format!("the return of cell {cell} has another shape"));
        }
    }
    None
}

/// What of a cell changed between two versions, in words: its kind, its line, its inputs or its rows; none when none
/// did.
fn changed(was: &CompiledCell, is: &CompiledCell) -> Option<&'static str> {
    let inputs = |c: &CompiledCell| -> Vec<(Option<String>, Option<String>, Vec<flow::format::Src>)> {
        c.inputs
            .iter()
            .map(|x| (x.id.clone(), x.name.clone(), x.from.clone()))
            .collect()
    };
    let rows = |c: &CompiledCell| -> Vec<(String, Vec<flow::format::Atom>, String)> {
        c.rows
            .iter()
            .map(|r| (r.name.clone(), r.conditions.clone(), r.to.clone()))
            .collect()
    };
    if was.kind != is.kind {
        Some("its kind")
    } else if was.line != is.line {
        Some("its line")
    } else if inputs(was) != inputs(is) {
        Some("its inputs")
    } else if rows(was) != rows(is) {
        Some("its rows")
    } else {
        None
    }
}

/// The shape of a return's schema: its field names and types, the values a field may take and the fields required,
/// with every description, title and limit left out.
pub fn shape(schema: &Value) -> Value {
    let Value::Object(o) = schema else {
        return schema.clone();
    };
    let mut out = Map::new();
    for (k, v) in o {
        let kept = match k.as_str() {
            "type" | "enum" | "const" | "required" | "$ref" => v.clone(),
            "properties" | "$defs" => match v {
                Value::Object(fields) => Value::Object(fields.iter().map(|(n, s)| (n.clone(), shape(s))).collect()),
                _ => v.clone(),
            },
            "items" | "additionalProperties" => shape(v),
            "anyOf" | "oneOf" | "allOf" | "prefixItems" => match v {
                Value::Array(xs) => Value::Array(xs.iter().map(shape).collect()),
                _ => v.clone(),
            },
            _ => continue,
        };
        out.insert(k.clone(), kept);
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn engineer() -> CompiledGraph {
        let relations = db::relations(db::SCHEMA).unwrap();
        let tables = db::index_of(db::SCHEMA).unwrap();
        let f = flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap();
        f.graph(crate::ENGINEER).unwrap().clone()
    }

    fn at(cell: &str) -> Position {
        Position {
            at: cell.to_string(),
            ..Position::default()
        }
    }

    fn cell<'g>(g: &'g mut CompiledGraph, id: &str) -> &'g mut CompiledCell {
        g.cells.iter_mut().find(|c| c.id == id).unwrap()
    }

    // A work moves when its cell is as it was and its returns keep their shape, whatever else the version changed:
    // another cell's rows, its own task and what its inputs and fields are said to be.
    #[test]
    fn a_work_moves_when_its_cell_and_its_returns_are_unchanged() {
        let old = engineer();
        let mut new = old.clone();
        let returned: BTreeSet<String> = ["R1".to_string()].into();
        cell(&mut new, "D2").rows[0].name = "renamed".into();
        let r2 = cell(&mut new, "R2");
        r2.title = "Another title".into();
        r2.prompt = vec!["Another task.".into()];
        for x in &mut r2.inputs {
            x.what = "Said otherwise.".into();
        }
        let r1 = cell(&mut new, "R1");
        if let Some(Value::Object(s)) = &mut r1.schema {
            s.insert("description".into(), json!("Said otherwise."));
        }
        assert_eq!(why_not(&old, &new, &at("R2"), &returned), None);
    }

    // A work whose cell changed its rows, inputs, line or kind, or is gone, is taken over.
    #[test]
    fn a_work_whose_cell_changed_is_taken_over() {
        let old = engineer();
        let none = BTreeSet::new();
        let mut rows = old.clone();
        cell(&mut rows, "R2").rows[0].to = "elsewhere".into();
        assert_eq!(
            why_not(&old, &rows, &at("R2"), &none).as_deref(),
            Some("cell R2 changed (its rows)")
        );
        let mut inputs = old.clone();
        cell(&mut inputs, "R2").inputs[0].name = Some("Another name".into());
        assert_eq!(
            why_not(&old, &inputs, &at("R2"), &none).as_deref(),
            Some("cell R2 changed (its inputs)")
        );
        let mut line = old.clone();
        cell(&mut line, "R2").line = Some("another".into());
        assert_eq!(
            why_not(&old, &line, &at("R2"), &none).as_deref(),
            Some("cell R2 changed (its line)")
        );
        let mut gone = old.clone();
        gone.cells.retain(|c| c.id != "R2");
        assert_eq!(
            why_not(&old, &gone, &at("R2"), &none).as_deref(),
            Some("cell R2 is gone")
        );
        // Nor does it move while it waits to go back to a cell that is gone.
        let asking = Position {
            at: "A3".into(),
            stopped: Some("R2".into()),
            callers: vec!["R2".into()],
            ..Position::default()
        };
        assert_eq!(
            why_not(&old, &gone, &asking, &none).as_deref(),
            Some("cell R2, which the work goes back to, is gone or of another kind")
        );
    }

    // A work one of whose earlier returns has another shape in the new version is taken over: a field added, or a
    // field's type changed.
    #[test]
    fn a_work_whose_earlier_return_changed_its_shape_is_taken_over() {
        let old = engineer();
        let returned: BTreeSet<String> = ["R1".to_string()].into();
        let mut added = old.clone();
        if let Some(Value::Object(s)) = &mut cell(&mut added, "R1").schema {
            s.insert("properties".into(), json!({ "more": { "type": "string" } }));
        }
        assert_eq!(
            why_not(&old, &added, &at("R2"), &returned).as_deref(),
            Some("the return of cell R1 has another shape")
        );
        assert_eq!(
            shape(&json!({ "type": "string", "description": "a name", "maxLength": 9 })),
            json!({ "type": "string" })
        );
        assert_ne!(
            shape(&json!({ "type": "object", "properties": { "n": { "type": "string" } } })),
            shape(&json!({ "type": "object", "properties": { "n": { "type": "integer" } } }))
        );
    }
}
