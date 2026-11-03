//! A work on an older flow version, when it goes on. It moves to the version the engine runs, the same work at the same
//! cell in the same sessions, when the cell it stands at is unchanged there and every return it handed in still fits
//! its cell there; otherwise a new work on that version takes its item over ([`crate::Engine`]'s takeover).
//!
//! The cell is unchanged when the version has a cell of the same id, of the same kind and line, whose inputs have the
//! same names and sources and whose rows the same names, conditions and targets, each in the same order; the cells it
//! goes back to, the cell it left for the asking part and those waiting for an answer, must still be cells of the
//! same kind. A return still fits when the version has its cell and the cell's schema there takes the return as the
//! round stored it, as it does when the version added a field the return may leave out or a value a field may take.
//! Every return the work handed in counts, not only those the cells ahead name among their inputs: every agent's cell
//! reads the cell it came from, which may be any cell the work ran, and the engine's own steps read returns by their
//! cell, so any earlier return may be read again. A return is what the cell's schema in the work's own version takes:
//! what the engine records of a row it took itself, with no return (an ask withdrawn, a turn waited for, a session
//! begun afresh), is none, and the new version reads it as the old one did.

use flow::{CompiledCell, CompiledGraph, Position};
use serde_json::{Value, json};

use crate::RoundId;

/// What a round of a work handed in, as the round stored it: the round, its cell and the output.
#[derive(Debug, Clone)]
pub struct Handed {
    pub round: RoundId,
    pub cell: String,
    pub output: Value,
}

/// Why a work standing at `pos`, having handed in `handed`, cannot move from graph `old`, the graph of the version it
/// is on, to the same graph `new` of the version the engine runs, as what the version changed; none when it can. Both
/// graphs are as the work's project's agents are asked them.
pub fn why_not(old: &CompiledGraph, new: &CompiledGraph, pos: &Position, handed: &[Handed]) -> Option<String> {
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
    for h in handed {
        // What the cell's schema in the work's own version does not take is no return of the cell.
        let taken = old
            .cell(&h.cell)
            .map(|c| c.schema.as_ref().is_none_or(|s| flow::valid(s, &h.output)));
        if taken == Some(false) {
            continue;
        }
        let Some(is) = new.cell(&h.cell) else {
            return Some(format!(
                "cell {}, which the work handed a return in at, is gone",
                h.cell
            ));
        };
        let misfits = is.schema.as_ref().map(|s| {
            let (schema, output) = outcome_only(s, &h.output);
            flow::misfits(&schema, &output, "")
        });
        if let Some(why) = misfits.filter(|m| !m.is_empty()) {
            return Some(format!(
                "the return of cell {} in round {} no longer fits it ({})",
                h.cell,
                h.round,
                why.join("; ")
            ));
        }
    }
    None
}

/// The part of an agent's return a later round reads, its outcome, with the schema cut down to it: the fields every
/// agent's return holds beside it (what it told the owner, its progress, its side reports, its flow feedback) are acted
/// on as its round ends and read by no later row, so a version that changes them, as one that narrows the kinds of side
/// reports, moves a work all the same. A return of a tool or of the engine, which has no outcome, is whole.
fn outcome_only(schema: &Value, output: &Value) -> (Value, Value) {
    if schema["properties"].get("outcome").is_none() {
        return (schema.clone(), output.clone());
    }
    let mut schema = schema.clone();
    if let Some(p) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        p.retain(|k, _| k == "outcome");
    }
    if let Some(r) = schema.get_mut("required").and_then(Value::as_array_mut) {
        r.retain(|k| k == "outcome");
    }
    let outcome = output.get("outcome").cloned().unwrap_or(Value::Null);
    (schema, json!({ "outcome": outcome }))
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

    /// The fields of a cell's schema, to change.
    fn fields<'g>(g: &'g mut CompiledGraph, id: &str) -> &'g mut serde_json::Map<String, Value> {
        cell(g, id).schema.as_mut().unwrap()["properties"]
            .as_object_mut()
            .unwrap()
    }

    /// A tool's result: a command of `gh` that ran.
    fn gh() -> Value {
        json!({ "tool": "gh", "ran": { "verdict": "ran", "exit": 0, "output": "x" } })
    }

    /// A return of tool cell `X1`, as a round stores it: one tool's result.
    fn ran(work: i64, n: i64, result: Value) -> Handed {
        Handed {
            round: RoundId::new(work, n),
            cell: "X1".into(),
            output: json!({ "results": [result], "failed": [] }),
        }
    }

    // A work moves when its cell is as it was and its returns still fit, whatever else the version changed: another
    // cell's rows, its own task and what its inputs and fields are said to be.
    #[test]
    fn a_work_moves_when_its_cell_and_its_returns_are_unchanged() {
        let old = engineer();
        let mut new = old.clone();
        let handed = [ran(1, 3, gh())];
        assert_eq!(why_not(&old, &old, &at("R2"), &handed), None);
        cell(&mut new, "D2").rows[0].name = "renamed".into();
        let r2 = cell(&mut new, "R2");
        r2.title = "Another title".into();
        r2.prompt = vec!["Another task.".into()];
        for x in &mut r2.inputs {
            x.what = "Said otherwise.".into();
        }
        if let Some(Value::Object(s)) = &mut cell(&mut new, "X1").schema {
            s.insert("description".into(), json!("Said otherwise."));
        }
        assert_eq!(why_not(&old, &new, &at("R2"), &handed), None);
    }

    // A work whose cell changed its rows, inputs, line or kind, or is gone, is taken over.
    #[test]
    fn a_work_whose_cell_changed_is_taken_over() {
        let old = engineer();
        let none = [];
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
            at: "A2".into(),
            stopped: Some("R2".into()),
            callers: vec!["R2".into()],
            ..Position::default()
        };
        assert_eq!(
            why_not(&old, &gone, &asking, &none).as_deref(),
            Some("cell R2, which the work goes back to, is gone or of another kind")
        );
    }

    // A work moves when the new version adds to a return's schema what its returns may leave out, a field or a value a
    // field may take: every return it handed in still fits.
    #[test]
    fn a_work_whose_returns_fit_an_added_field_moves() {
        let old = engineer();
        let handed = [ran(1, 3, gh())];
        let mut added = old.clone();
        fields(&mut added, "X1").insert("more".into(), json!({ "type": "string" }));
        assert_eq!(why_not(&old, &added, &at("R2"), &handed), None);
    }

    // A work one of whose returns does not fit its cell in the new version is taken over, saying the round and where it
    // does not fit: a field the return holds taken out, or a field it lacks required.
    #[test]
    fn a_work_whose_return_no_longer_fits_is_taken_over() {
        let now = engineer();
        let mut old = now.clone();
        fields(&mut old, "X1").insert("before".into(), json!({ "type": "string" }));
        let mut handed = ran(4, 7, gh());
        handed.output["before"] = json!("kept");
        assert_eq!(
            why_not(&old, &now, &at("R2"), &[handed]).as_deref(),
            Some(
                "the return of cell X1 in round R4-7 no longer fits it (the return has before, which is not one of its \
                 fields: results, failed)"
            )
        );
        let mut required = now.clone();
        fields(&mut required, "X1").insert("after".into(), json!({ "type": "string" }));
        let schema = cell(&mut required, "X1").schema.as_mut().unwrap();
        schema["required"].as_array_mut().unwrap().push(json!("after"));
        let fits = ran(4, 7, gh());
        assert_eq!(
            why_not(&now, &required, &at("R2"), std::slice::from_ref(&fits)).as_deref(),
            Some("the return of cell X1 in round R4-7 no longer fits it (the return has no after)")
        );
        let mut gone = now.clone();
        gone.cells.retain(|c| c.id != "X1");
        assert_eq!(
            why_not(&now, &gone, &at("R2"), &[fits]).as_deref(),
            Some("cell X1, which the work handed a return in at, is gone")
        );
    }

    // An agent's return whose fields beside the outcome no longer fit the new version moves all the same: what it told
    // the owner and its side reports were acted on as its round ended, and no later row reads them. Its outcome still
    // has to fit.
    #[test]
    fn a_work_whose_return_differs_only_beside_its_outcome_moves() {
        let now = architect();
        let pick = |extra: Value| {
            let mut h = handed_in(1, "Q1", json!({ "pick": { "matter": 4, "why": "w" } }));
            for (k, v) in extra.as_object().unwrap() {
                h.output[k] = v.clone();
            }
            h
        };
        let mut old = now.clone();
        let schema = cell(&mut old, "Q1").schema.as_mut().unwrap();
        schema["properties"]["tell_owner"] = json!({ "type": "array" });
        schema["properties"]["side_reports"] = json!({ "type": "array" });
        let told = pick(json!({ "tell_owner": [], "side_reports": [{ "kind": "other", "what": "w" }] }));
        let taken = old.cell("Q1").unwrap().schema.as_ref().unwrap();
        assert!(
            flow::valid(taken, &told.output),
            "{:?}",
            flow::misfits(taken, &told.output, "")
        );
        assert!(!flow::valid(
            now.cell("Q1").unwrap().schema.as_ref().unwrap(),
            &told.output
        ));
        assert_eq!(why_not(&old, &now, &at("B2"), &[told]), None);
    }

    // A return that does not fit the new version in several places is told with every place, joined by semicolons, not
    // with the first alone.
    #[test]
    fn a_work_whose_return_no_longer_fits_in_several_places_is_told_every_place() {
        let now = engineer();
        let mut old = now.clone();
        fields(&mut old, "X1").insert("before".into(), json!({ "type": "string" }));
        let mut new = now.clone();
        fields(&mut new, "X1").insert("after".into(), json!({ "type": "string" }));
        let schema = cell(&mut new, "X1").schema.as_mut().unwrap();
        schema["required"].as_array_mut().unwrap().push(json!("after"));
        let mut handed = ran(4, 7, gh());
        handed.output["before"] = json!("kept");
        assert_eq!(
            why_not(&old, &new, &at("R2"), &[handed]).as_deref(),
            Some(
                "the return of cell X1 in round R4-7 no longer fits it (the return has no after; the return has \
                 before, which is not one of its fields: results, failed, after)"
            )
        );
    }

    fn architect() -> CompiledGraph {
        let relations = db::relations(db::SCHEMA).unwrap();
        let tables = db::index_of(db::SCHEMA).unwrap();
        let f = flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap();
        f.graph(crate::ARCHITECT).unwrap().clone()
    }

    /// A schema without the property `name` anywhere in it, and without it in any list of required ones: the schema as
    /// it was before a version added that field.
    fn without(schema: &mut Value, name: &str) {
        match schema {
            Value::Object(o) => {
                if let Some(Value::Object(p)) = o.get_mut("properties") {
                    p.remove(name);
                }
                if let Some(Value::Array(r)) = o.get_mut("required") {
                    r.retain(|x| x != name);
                }
                o.values_mut().for_each(|v| without(v, name));
            }
            Value::Array(xs) => xs.iter_mut().for_each(|v| without(v, name)),
            _ => {}
        }
    }

    /// A return of an agent's cell of the architect's graph, as a round stores it: the pick of the next matter, on a
    /// line of its own, tells the owner nothing.
    fn handed_in(n: i64, cell: &str, outcome: Value) -> Handed {
        let mut output = json!({ "outcome": outcome, "progress": "p", "flow_feedback": "none" });
        if cell != "Q1" {
            output["tell_owner"] = json!([]);
        }
        Handed {
            round: RoundId::new(1, n),
            cell: cell.into(),
            output,
        }
    }

    // Item #228: an architect's work that has handed in the returns of before the architect could pick a group (a pick
    // of one matter, a finding, a way) moves to the version that lets it, at the pick, the cell that finds what a report
    // is, the cell that takes it in or the review of the batch: the version only added a field the returns may leave out,
    // to the three cells' schemas, and left their inputs and rows as they were. A return that names others to go with a
    // matter fits the new version and not the old, which had no such field.
    #[test]
    fn an_architect_work_with_returns_from_before_groups_moves() {
        let now = architect();
        let mut old = now.clone();
        for id in ["Q1", "H4", "H5"] {
            without(cell(&mut old, id).schema.as_mut().unwrap(), "with");
        }
        let finding = json!({ "what": "w", "grounds": "g", "sure": "checked" });
        let change = json!({ "kind": "edit_item", "item": "1", "part": "title", "text": "Parse first", "reason": "r" });
        let mut named = change.clone();
        named["matter"] = json!(5);
        let before = [
            handed_in(1, "Q1", json!({ "pick": { "matter": 4, "why": "w" } })),
            handed_in(2, "H4", json!({ "finding": finding })),
            handed_in(
                3,
                "H5",
                json!({ "take": { "way": "fold_in", "why": "w" }, "effects": { "item_changes": [change.clone()] } }),
            ),
        ];
        for h in &before {
            let schema = old.cell(&h.cell).unwrap().schema.as_ref().unwrap();
            assert!(
                flow::valid(schema, &h.output),
                "{}: {:?}",
                h.cell,
                flow::misfits(schema, &h.output, "")
            );
        }
        for stands_at in ["Q1", "H4", "H5", "B2"] {
            assert_eq!(why_not(&old, &now, &at(stands_at), &before), None, "{stands_at}");
        }
        // The new field is an addition: what names others fits the new version only.
        let group = [
            handed_in(4, "Q1", json!({ "pick": { "matter": 4, "why": "w", "with": [5, 6] } })),
            handed_in(
                5,
                "H4",
                json!({ "finding": finding, "with": [{ "matter": 5, "what": "w", "grounds": "g", "sure": "likely" }] }),
            ),
            handed_in(
                6,
                "H5",
                json!({ "take": { "way": "nothing", "why": "w" }, "effects": { "item_changes": [named] },
                        "with": [{ "matter": 5, "way": "merged", "why": "w" }] }),
            ),
        ];
        for h in &group {
            assert!(
                flow::valid(now.cell(&h.cell).unwrap().schema.as_ref().unwrap(), &h.output),
                "{}",
                h.cell
            );
            assert!(
                !flow::valid(old.cell(&h.cell).unwrap().schema.as_ref().unwrap(), &h.output),
                "{}",
                h.cell
            );
        }
        // Their cells keep their inputs and rows, which a work standing there is moved by.
        let own = |id: &str| -> (Vec<String>, Vec<(String, String)>) {
            let c = now.cell(id).unwrap();
            (
                c.inputs.iter().filter_map(|x| x.name.clone()).collect(),
                c.rows
                    .iter()
                    // The cell's own rows that go on to another cell, not the engine's that keep the work where it is.
                    .filter(|r| !r.common && r.to != "@self")
                    .map(|r| (r.name.clone(), r.to.clone()))
                    .collect(),
            )
        };
        assert_eq!(own("Q1").1, [("picked".to_string(), "Q2".to_string())]);
        assert_eq!(own("H4").1, [("found".to_string(), "Q0".to_string())]);
        assert_eq!(own("H5").1, [("done".to_string(), "B1".to_string())]);
        assert_eq!(own("Q1").0.first().map(String::as_str), Some("The queue"));
        assert_eq!(own("H4").0.first().map(String::as_str), Some("The report"));
        assert_eq!(own("H5").0.first().map(String::as_str), Some("What the report is"));
    }

    // What the engine records of a row it took itself, which its cell's schema does not take, is no return: the work
    // moves whatever the new version says of that cell, even that it is gone.
    #[test]
    fn a_row_the_engine_took_itself_is_no_return() {
        let old = engineer();
        let mut new = old.clone();
        new.cells.retain(|c| c.id != "A2");
        let withdrawn = Handed {
            round: RoundId::new(4, 9),
            cell: "A2".into(),
            output: json!({}),
        };
        assert_eq!(why_not(&old, &new, &at("R2"), &[withdrawn]), None);
    }
}
