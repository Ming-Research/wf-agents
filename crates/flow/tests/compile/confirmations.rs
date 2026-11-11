//! A test of what was confirmed: it reads blocks of any kind, and names as `by` a choice that people write, no cell
//! writes, and that offers its value; no test of another kind names one.

use decide::TestKind;
use flow::{FLOW_FILE, FaultKind::*};

use crate::support::{at, compiled, edit, faults, valid};

/// The valid flow with the merge waiting, besides, until people confirmed the plan in `answered`, whose declaration
/// is this.
fn confirming(answered: &str) -> flow::Source {
    let test = "\n[tests.plan_answered]\nsays = \"The owner answered the plan's questions\"\nkind = \"confirmed\"\n\
                reads = [\"plan\"]\nby = \"answered\"\nis = \"yes\"\n";
    let source = edit(valid(), FLOW_FILE, "is = \"red\"\n", &format!("is = \"red\"\n{test}"));
    let source = edit(
        source,
        FLOW_FILE,
        "[blocks.plan]",
        &format!("{answered}\n\n[blocks.plan]"),
    );
    edit(
        source,
        "merge.toml",
        r#"requires = ["may_merge"]"#,
        r#"requires = ["may_merge", "plan_answered"]"#,
    )
}

const ANSWERED: &str = "[blocks.answered]\nvalues = [\"yes\"]\npeople = true";

#[test]
fn a_test_of_what_was_confirmed_reads_prose_and_is_read_by_its_confirmation() {
    let flow = compiled(&confirming(ANSWERED));
    let test = &flow.flow.tests()[&"plan_answered".into()];
    assert_eq!(test.kind, TestKind::Confirmed);
    assert_eq!(test.reads, ["plan".into()]);
    assert_eq!(test.by, Some("answered".into()));
    let answered = &flow.blocks[&"answered".into()];
    assert_eq!(answered.readers.tests, ["plan_answered".into()]);
}

#[test]
fn the_confirmation_is_a_choice_people_alone_write_offering_the_value() {
    let unconfirmable = || {
        let what = Unconfirmable {
            test: "plan_answered".to_string(),
        };
        at(FLOW_FILE, "tests.plan_answered.by", what)
    };
    let not_people = "[blocks.answered]\nvalues = [\"yes\"]\noutside = \"main\"";
    let not_the_value = "[blocks.answered]\nvalues = [\"no\"]\npeople = true";
    let prose = "[blocks.answered]\npeople = true";
    for answered in [not_people, not_the_value, prose] {
        assert_eq!(faults(&confirming(answered)), [unconfirmable()], "{answered}");
    }
    // Written by a cell besides people.
    let source = edit(
        confirming(ANSWERED),
        "judge.toml",
        r#"outputs = ["needs_design", "opinions"]"#,
        r#"outputs = ["needs_design", "opinions", "answered"]"#,
    );
    assert_eq!(faults(&source), [unconfirmable()]);
    // Named by no test of what was confirmed.
    let source = edit(
        confirming(ANSWERED),
        FLOW_FILE,
        "kind = \"confirmed\"",
        "kind = \"all\"",
    );
    let source = edit(source, FLOW_FILE, "reads = [\"plan\"]", "reads = [\"confirmed\"]");
    assert_eq!(faults(&source), [unconfirmable()]);
    // A test of what was confirmed that names no confirmation.
    let source = edit(confirming(ANSWERED), FLOW_FILE, "by = \"answered\"\n", "");
    let unread = NoReader {
        block: "answered".to_string(),
    };
    assert_eq!(
        faults(&source),
        [at(FLOW_FILE, "blocks.answered", unread), unconfirmable()]
    );
}
