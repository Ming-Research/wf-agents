//! Notes: a block people write may be a note, their words to the cells that read it, which a paused work still
//! answers; a block people do not write is none.

use flow::{FLOW_FILE, FaultKind::*};

use crate::support::{at, compiled, edit, faults, valid};

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
fn a_block_people_do_not_write_is_no_note() {
    let source = edit(valid(), FLOW_FILE, "[blocks.plan]", "[blocks.plan]\nnote = true");
    let what = NoteNotByPeople {
        block: "plan".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "blocks.plan.note", what)]);
}
