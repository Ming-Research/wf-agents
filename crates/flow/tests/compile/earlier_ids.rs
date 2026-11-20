//! The table of earlier ids: what a renumbering may write (decision
//! `the-table-of-earlier-ids-is-a-list-of-renumberings`). A renumbering that gives an id a cell of its stage still had
//! is refused, since what was recorded at that cell would read as the one given its id; each other malformed
//! renumbering is refused where it is written.

use decide::CellId;
use flow::FaultKind::*;
use flow::{FLOW_FILE, Renumbering, Source};

use crate::support::{at, compiled, edit, faults, laid_out};

/// The small flow laid out one thing to a file, its `flow.toml` holding this table of earlier ids.
fn with_table(table: &str) -> Source {
    edit(
        laid_out(),
        FLOW_FILE,
        "data = [\"request\", \"main\"]",
        &format!("data = [\"request\", \"main\"]\nearlier_ids = {table}"),
    )
}

#[test]
fn a_renumbering_that_gives_an_id_a_cell_of_its_stage_still_had_is_refused() {
    // T4 moves to T3, and the cell that had T3 neither moves nor is taken out: its records would read as T4's.
    assert_eq!(
        faults(&with_table(r#"[{ ids = { T4 = "T3" } }]"#)),
        vec![at(FLOW_FILE, "earlier_ids[0].ids.T4", IdStillHeld { id: "T3".into() })]
    );
}

#[test]
fn a_renumbering_that_moves_a_cell_past_its_stage_or_takes_out_the_cell_whose_id_it_gives_compiles() {
    // T2 moves to T4, past every id its stage had up to T2; and T4 takes T3 where the T3 before is taken out.
    let moved = compiled(&with_table(r#"[{ ids = { T2 = "T4" } }]"#));
    let ids = [(CellId::from("T2"), CellId::from("T4"))].into_iter().collect();
    assert_eq!(
        moved.earlier_ids,
        vec![Renumbering {
            ids,
            taken_out: Vec::new()
        }]
    );
    let taken = compiled(&with_table(r#"[{ ids = { T4 = "T3" }, taken_out = ["T3"] }]"#));
    assert_eq!(taken.earlier_ids[0].taken_out, vec![CellId::from("T3")]);
    // A flow that writes no table has none.
    assert_eq!(compiled(&laid_out()).earlier_ids, Vec::new());
}

#[test]
fn a_renumbering_that_names_no_id_gives_one_twice_or_to_itself_or_moves_and_takes_out_one_is_refused() {
    let table = r#"[
        { ids = { t1 = "T2" } },
        { ids = { T1 = "T5", T2 = "T5" } },
        { ids = { T3 = "T3" } },
        { ids = { T6 = "T7" }, taken_out = ["T6", "D1", "D1", "x"] },
    ]"#;
    assert_eq!(
        faults(&with_table(table)),
        vec![
            at(FLOW_FILE, "earlier_ids[0].ids.t1", NotACellId { id: "t1".into() }),
            at(FLOW_FILE, "earlier_ids[1].ids.T2", IdGivenTwice { id: "T5".into() }),
            at(FLOW_FILE, "earlier_ids[2].ids.T3", IdToItself { id: "T3".into() }),
            at(FLOW_FILE, "earlier_ids[3].ids.T6", MovedAndTakenOut { id: "T6".into() }),
            at(FLOW_FILE, "earlier_ids[3].taken_out[2]", Twice { name: "D1".into() }),
            at(FLOW_FILE, "earlier_ids[3].taken_out[3]", NotACellId { id: "x".into() }),
        ]
    );
}
