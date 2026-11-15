//! The table of earlier ids (principle `cell-numbering`): each renumbering of the cells, oldest first, compiled as it
//! is written and checked on its own; what a version recorded at an earlier id is read through the renumberings after
//! its own as the cell it ran at. And the version a definition is: its files, named by their digest, from which it is
//! compiled again.

use decide::CellId;
use flow::FaultKind::*;
use flow::{FLOW_FILE, Source};

use crate::support::{at, compiled, edit, faults, laid_out, valid};

/// The small flow with this table of earlier ids written at the end of its `flow.toml`.
fn renumbered(table: &str) -> Source {
    let mut source = valid();
    source.flow.push_str(table);
    source
}

/// Two renumberings: in the first, the cell that is `plan` had the id `draft`, and the one that is `judge` had `plan`;
/// in the second, the cell that is `answer` had `judge`, which the first gave it.
const TWO: &str = r#"
[[earlier_ids]]
plan = "draft"
judge = "plan"

[[earlier_ids]]
answer = "judge"
"#;

fn id(id: &str) -> CellId {
    CellId::from(id)
}

#[test]
fn a_cell_recorded_at_an_earlier_id_reads_through_each_renumbering_after_its_version_as_the_cell_it_ran_at() {
    let flow = compiled(&renumbered(TWO));
    assert_eq!(flow.earlier_ids.len(), 2);
    assert_eq!(flow.earlier_ids[1].get(&id("answer")), Some(&id("judge")));
    // Recorded on a version before both: `draft` became `plan`; `plan` became `judge`, and then `answer`.
    assert_eq!(flow.reads_as(0, &id("draft")), id("plan"));
    assert_eq!(flow.reads_as(0, &id("plan")), id("answer"));
    // Recorded on a version that had the first: its `judge` is `answer` now, and its `plan` is `plan` still.
    assert_eq!(flow.reads_as(1, &id("judge")), id("answer"));
    assert_eq!(flow.reads_as(1, &id("plan")), id("plan"));
    // Recorded on this version, or at an id no renumbering gave another, it reads as it was recorded.
    assert_eq!(flow.reads_as(2, &id("judge")), id("judge"));
    assert_eq!(flow.reads_as(0, &id("check")), id("check"));
}

#[test]
fn a_version_is_read_on_a_later_one_only_where_the_later_table_begins_with_its_whole() {
    let first = compiled(&renumbered("\n[[earlier_ids]]\nplan = \"draft\"\njudge = \"plan\"\n"));
    let both = compiled(&renumbered(TWO));
    let none = compiled(&valid());
    assert!(both.follows(&first));
    assert!(both.follows(&none));
    assert!(both.follows(&both));
    assert!(!first.follows(&both), "an earlier table is not a later one's");
    let rewritten = compiled(&renumbered(
        "\n[[earlier_ids]]\nplan = \"sketch\"\n\n[[earlier_ids]]\nanswer = \"judge\"\n",
    ));
    assert!(
        !rewritten.follows(&first),
        "a renumbering written otherwise is no renumbering after it"
    );
}

#[test]
fn a_renumbering_that_names_no_cell_gives_a_cell_its_own_id_or_two_cells_one_earlier_id_is_a_fault() {
    let source = renumbered(
        r#"
[[earlier_ids]]

[[earlier_ids]]
design = "draft"
judge = "draft"
plan = "plan"
"#,
    );
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "earlier_ids[0]", RenumberedNothing),
            at(FLOW_FILE, "earlier_ids[1].judge", EarlierIdTwice { id: "draft".into() }),
            at(FLOW_FILE, "earlier_ids[1].plan", SameEarlierId { id: "plan".into() }),
        ]
    );
}

#[test]
fn a_flow_laid_out_one_thing_to_a_file_writes_its_table_in_its_flow_toml() {
    let source = edit(
        laid_out(),
        FLOW_FILE,
        "[limits]",
        "earlier_ids = [{ plan = \"draft\" }]\n\n[limits]",
    );
    let flow = compiled(&source);
    assert_eq!(flow.reads_as(0, &id("draft")), id("plan"));
}

#[test]
fn a_version_is_its_files_named_by_their_digest_and_is_compiled_again_from_them() {
    let source = laid_out();
    let flow = compiled(&source);
    assert_eq!(flow.version, source.digest());
    assert_eq!(flow.source, source);
    // Its files make the same definition again, as a version kept of it is read.
    assert_eq!(Source::of_files(source.files()), Some(source.clone()));
    assert_eq!(Source::of_files(valid().files()), Some(valid()));
    assert_eq!(flow.again(&source), Ok(flow.clone()));
    // A byte of any file is another version.
    let other = edit(source.clone(), FLOW_FILE, "name = \"small\"", "name = \"small\" ");
    assert_ne!(other.digest(), source.digest());
    assert_eq!(compiled(&other).version, other.digest());
}
