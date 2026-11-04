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

// The engineer does not record a merge condition: every cell of a line that holds the rule to ask the architect instead
// can ask, by the row the asking part gives it or, a cell that is asked itself, by handing on to the architect, so that
// the rule is one each can follow.
#[test]
fn every_cell_told_to_ask_the_architect_for_a_merge_condition_can() {
    let c = compiled();
    let g = c.graph("engineer").unwrap();
    let told: Vec<&flow::CompiledCell> = g
        .cells
        .iter()
        .filter(|x| x.kind == "agent" && x.rules.iter().any(|r| r.name == "engineering"))
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
    // The four levels are said in one order wherever the flow says them.
    let v: Value = serde_json::from_str(FLOW).unwrap();
    let values = v["engine"]["values"]["gates"].as_str().unwrap();
    let levels = ["items", "set_aside", "conditions", "owner"];
    let at: Vec<usize> = levels.iter().map(|l| values.find(&format!("{l},")).unwrap()).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{values}");
}
