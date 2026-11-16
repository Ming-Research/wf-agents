//! Notes: a block people write may be a note, their words to the cells that read it, which a paused work still
//! answers; a block people do not write is none, and nor is a choice. And the owner's own acts, the owner's words and
//! answers, which a paused work answers too, the words alone turning to the cell they run.

use flow::{FLOW_FILE, FaultKind::*};

use crate::support::{at, compiled, edit, faults, has, valid};

#[test]
fn a_block_people_write_may_be_a_note_and_the_deciding_code_knows_it() {
    let source = edit(
        valid(),
        FLOW_FILE,
        "[blocks.opinions]\npeople = true\n",
        "[blocks.opinions]\npeople = true\nnote = true\n",
    );
    let c = compiled(&source);
    assert!(c.blocks[&"opinions".into()].writers.note);
    assert!(!c.blocks[&"comments".into()].writers.note);
    assert!(c.flow.is_note(&"opinions".into()));
    assert!(!c.flow.is_note(&"comments".into()));
}

#[test]
fn a_choice_is_no_note() {
    let source = edit(
        valid(),
        FLOW_FILE,
        "[blocks.confirmed]\nvalues = [\"yes\", \"no\"]\npeople = true",
        "[blocks.confirmed]\nvalues = [\"yes\", \"no\"]\npeople = true\nnote = true",
    );
    let what = NoteIsAChoice {
        block: "confirmed".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "blocks.confirmed.note", what)]);
}

#[test]
fn a_block_people_do_not_write_is_no_note() {
    let source = edit(valid(), FLOW_FILE, "[blocks.plan]", "[blocks.plan]\nnote = true");
    let what = NoteNotByPeople {
        block: "plan".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "blocks.plan.note", what)]);
}

#[test]
fn a_block_no_source_writes_says_nobody_changes_what_a_source_writes() {
    let source = edit(
        valid(),
        FLOW_FILE,
        "[blocks.confirmed]\nvalues = [\"yes\", \"no\"]\npeople = true",
        "[blocks.confirmed]\nvalues = [\"yes\", \"no\"]\npeople = true\nchanged_by = \"owner\"",
    );
    let what = ChangedByWithoutSource {
        block: "confirmed".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "blocks.confirmed.changed_by", what)]);
}

/// The small flow with a block of the owner's words and one of the owner's answers, which the planning cell reads,
/// compiled with the sources that write them.
fn spoken_and_answered() -> flow::Compiled {
    let blocks = "[blocks.plan]\n\n[blocks.spoken]\noutside = \"words\"\n\n[blocks.answered]\noutside = \"answer\"\n";
    let source = edit(valid(), FLOW_FILE, "[blocks.plan]\n", blocks);
    let inputs = r#"inputs = ["request", "opinions", "spoken", "answered"]"#;
    let source = edit(source, "plan.toml", r#"inputs = ["request", "opinions"]"#, inputs);
    let mut has = has();
    has.sources.insert("answer".to_string());
    has.sources.insert("words".to_string());
    flow::compile(&source, &has).unwrap_or_else(|faults| panic!("the definition should compile: {faults:#?}"))
}

#[test]
fn only_the_owner_s_words_stop_a_later_round_and_enter_a_cell_that_waits() {
    // Design document `data-and-context`, 32 and 35: a cell that waits is entered only for the owner's words, and a
    // round of a later cell is stopped for them alone.
    let c = spoken_and_answered();
    assert!(c.flow.interrupts(&"spoken".into()));
    assert!(!c.flow.interrupts(&"answered".into()));
    assert!(!c.flow.interrupts(&"main".into()));
}

#[test]
fn a_paused_work_answers_the_owner_s_words_and_answers_alike() {
    // The owner's pauses stay as they are (design document `data-and-context`): a paused work answers the owner.
    let c = spoken_and_answered();
    assert!(c.flow.owner_acts(&"spoken".into()));
    assert!(c.flow.owner_acts(&"answered".into()));
    assert!(!c.flow.owner_acts(&"main".into()));
}
