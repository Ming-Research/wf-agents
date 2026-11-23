//! A view of a work as it is now, rule by rule: each test builds a small work through the shared helper and says
//! what the view shows, from the documentation of `decide::view` and the words of the design (current, meaning, what
//! a cell consumed, what is new to it, skipped, at rest), never from its code. The tests are kept in modules by
//! subject.

// The view's tests use only part of the helper the walk's tests share.
#[allow(dead_code)]
#[path = "../support/mod.rs"]
mod support;

mod agreement;
mod blocks;
mod faults;
mod inputs;
mod stages;

use decide::{
    BlockId, CallId, Cause, Cell, CellView, Condition, Decision, InputView, OpenRun, RunId, Seq, VersionView, View,
    Who, Writer, view,
};
use support::Says::{DoesNotMatter, Matters, Nothing, Value};
use support::{World, cell};

/// The work's view.
fn seen(w: &World) -> View {
    view(&w.flow, &w.records)
}

/// The view of one cell.
fn cell_of(w: &World, id: &str) -> CellView {
    (seen(w).cells.into_iter())
        .find(|c| c.cell.as_str() == id)
        .expect("the cell is in the view")
}

/// The view of one block that runs one cell.
fn input_of(w: &World, id: &str, block: &str) -> InputView {
    (cell_of(w, id).runs_on.into_iter())
        .find(|i| i.block.as_str() == block)
        .expect("the block runs the cell")
}

/// A block that runs a cell, the cell having worked from the version it consumed.
fn input(block: &str, current: Option<Seq>, consumed: Option<Seq>, new_versions: &[Seq]) -> InputView {
    InputView {
        block: block.into(),
        current,
        consumed,
        worked: consumed,
        new_versions: new_versions.to_vec(),
    }
}

/// A version that holds something.
fn version(at: Seq, by: Writer, meaning: Seq, value: Option<&str>) -> VersionView {
    VersionView {
        at,
        by,
        meaning: Some(meaning),
        value: value.map(str::to_string),
        empty: false,
        ruled: false,
    }
}

/// A cell that runs only when the block's current value is this one.
fn when(block: &str, value: &str, mut c: Cell) -> Cell {
    c.applies = vec![Condition::Is {
        block: block.into(),
        value: value.into(),
    }];
    c
}

/// A research and the implementation of its plan, with a merge that waits for the owner.
fn planned() -> World {
    World::with_gate(vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan"], &["code"]),
    ])
}

/// The run that finished at this place.
fn finished_by(w: &World, at: Seq) -> RunId {
    w.records.finished.iter().find(|f| f.at == at).expect("a finish").run
}
