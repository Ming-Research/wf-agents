//! A view of a work as it is now, rule by rule: each test builds a small work through the shared helper and says
//! what the view shows, from the documentation of `decide::view` and the words of the design (current, meaning, what
//! a record knows, skipped, at rest), never from its code. The tests are kept in modules by subject.

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
use support::Says::{DoesNotMatter, Matters, Value};
use support::{World, cell};

/// The work's view.
fn seen(w: &World) -> View {
    view(&w.flow, &w.records, &w.named)
}

/// The view of one cell.
fn cell_of(w: &World, id: &str) -> CellView {
    (seen(w).cells.into_iter())
        .find(|c| c.cell.as_str() == id)
        .expect("the cell is in the view")
}

/// The view of one input of one cell.
fn input_of(w: &World, id: &str, block: &str) -> InputView {
    (cell_of(w, id).inputs.into_iter())
        .find(|i| i.block.as_str() == block)
        .expect("the cell reads the block")
}

fn input(block: &str, current: Option<Seq>, known: Option<Seq>, changed: bool) -> InputView {
    InputView {
        block: block.into(),
        current,
        known,
        changed,
    }
}

fn version(at: Seq, by: Writer, meaning: Seq, value: Option<&str>) -> VersionView {
    VersionView {
        at,
        by,
        meaning,
        value: value.map(str::to_string),
    }
}

/// A cell that runs only when the block's current value is this one.
fn when(block: &str, value: &str, mut c: Cell) -> Cell {
    c.applies = Some(Condition::Is {
        block: block.into(),
        value: value.into(),
    });
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
