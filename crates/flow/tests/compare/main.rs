//! The comparison of two compiled definitions: each test changes the small valid flow in one way or a few, and says
//! the changes it is given, as a whole and in their order. The tests are kept in modules by subject, so that a new test
//! goes into the module of what it tests.

// The compile's tests use all of the support; these use what builds a definition and compiles it.
#[allow(dead_code)]
#[path = "../support/mod.rs"]
mod support;

mod blocks;
mod cells;
mod describes;
mod fields;
mod flow_itself;
mod levels;
mod order;

use flow::{Change, Compiled, Source};

/// The small flow's files without the file of this cell.
fn without(source: Source, cell: &str) -> Source {
    let mut source = source;
    assert!(source.cells.remove(cell).is_some(), "the flow has no cell {cell}");
    source
}

/// The definition compiled against what the small flow names, with one more source, `mirror`, and one more act, `test`,
/// which a test may name.
fn compiled(source: &Source) -> Compiled {
    let mut has = support::has();
    has.sources.insert("mirror".to_string());
    has.acts.insert("test".to_string());
    match flow::compile(source, &has) {
        Ok(compiled) => compiled,
        Err(faults) => panic!("the definition should compile: {faults:#?}"),
    }
}

/// What changed from the small valid flow to this definition.
fn from_valid(after: &Source) -> Vec<Change> {
    flow::compare(&compiled(&support::valid()), &compiled(after))
}
