//! What the flow says of the owner's merge conditions: the merge gate holds them on rows of its own, and what is neither
//! ticked nor recorded holds nothing, so the engineer who hears one asks the architect, and the architect records it.

use serde_json::Value;

const FLOW: &str = include_str!("../../../flow/flow.json");

fn compiled() -> flow::Compiled {
    let cx = flow::Context {
        relations: db::relations(db::SCHEMA).unwrap(),
        tables: db::index_of(db::SCHEMA).unwrap(),
    };
    flow::compile(FLOW, &cx).unwrap_or_else(|e| panic!("{e}"))
}

// The engineer does not record a merge condition: the rule that holds every cell of its lines says to ask the architect
// instead, and each can ask, by the row the asking part gives it or, a cell that is asked itself, by handing on to the
// architect, so that the rule is one each can follow. The one exception is `A7`, which only says how long the question of
// the cell that called it goes on waiting and goes back to that cell, in the same session, where it can ask.
#[test]
fn every_cell_told_to_ask_the_architect_for_a_merge_condition_can() {
    let v: Value = serde_json::from_str(FLOW).unwrap();
    let flows = v["flows"].as_array().unwrap();
    let engineer = flows.iter().find(|f| f["name"] == "engineer").unwrap();
    let rules = engineer["rules"].as_array().unwrap();
    let engineering = rules.iter().find(|r| r["name"] == "engineering").unwrap();
    let text = engineering["text"].to_string();
    assert!(text.contains("ask the architect to record it"), "{text}");

    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let told: Vec<&flow::CompiledCell> = g
        .cells
        .iter()
        .filter(|x| x.kind == "agent" && x.id != "A7" && x.rules.iter().any(|r| r.name == "engineering"))
        .collect();
    assert!(told.iter().any(|x| x.id == "I1") && told.iter().any(|x| x.id == "T6"));
    for cell in told {
        assert!(
            cell.rows
                .iter()
                .any(|r| r.name == "ask_architect" || r.name == "to_architect"),
            "{} cannot ask the architect: {:?}",
            cell.id,
            cell.rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>()
        );
    }
}

// The rows of the merge gate: the owner asked of a condition with the work waiting, an answer that judged nothing going
// to the engineer; the implementation and the fix given the conditions, reporting on them under their own check.
#[test]
fn the_merge_gate_holds_a_condition_on_rows_of_its_own() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let gate = g.cell("M1").unwrap();
    let row = |name: &str| gate.rows.iter().find(|r| r.name == name).unwrap();
    assert_eq!(row("owner_judges").to, "M1");
    assert!(row("owner_judges").waits);
    assert_eq!(row("condition_answered").to, "A4");
    for cell in ["R1", "D1", "I1", "T6", "M1"] {
        assert!(
            g.cell(cell)
                .unwrap()
                .inputs
                .iter()
                .any(|i| i.from == [flow::format::Src::Engine("merge_conditions".into())]),
            "{cell}"
        );
    }
    for cell in ["I1", "T6"] {
        assert!(
            g.cell(cell)
                .unwrap()
                .checks
                .iter()
                .any(|x| x.check == "owner_conditions_answered" && x.at.as_deref() == Some("owner_conditions")),
            "{cell}"
        );
    }
    // A condition recorded after the gate was passed goes back to it from the merge.
    let late = g.cell("M2").unwrap().rows.iter().find(|r| r.name == "condition_set").unwrap();
    assert_eq!(late.to, "M1");
    // The levels are said in one order.
    let v: Value = serde_json::from_str(FLOW).unwrap();
    let values = v["engine"]["values"]["gates"].as_str().unwrap();
    let levels = ["items", "set_aside", "approvals", "conditions", "owner"];
    let at: Vec<usize> = levels.iter().map(|l| values.find(&format!("{l},")).unwrap()).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{values}");
}
