//! Which blocks keep a key of a cell's preconditions from holding, as the tree defines it: a block no one has written
//! counts as the default its flow gives it, and each kind of named test is kept as its definition says.

use std::collections::BTreeMap;

use decide::{BlockId, Cell, Condition, Flow, Outsider, Records, Seq, Test, TestKind, TestName, Written};
use reading::Reading;

/// A writer of the block `ready`, a choice that counts as `yes` until it is written, and a gate whose precondition is
/// this named test of it.
fn gated(kind: TestKind) -> Flow {
    let mut write = Cell::new("write");
    write.outputs = vec!["ready".into()];
    let mut gate = Cell::new("gate");
    gate.requires = vec![Condition::Test("ready".into())];
    let test = Test {
        kind,
        reads: vec!["ready".into()],
        is: Some("yes".to_string()),
    };
    let tests = BTreeMap::from([(TestName::from("ready"), test)]);
    let flow = Flow::new(vec![write, gate], tests, BTreeMap::new()).expect("the flow is one");
    flow.with_defaults(BTreeMap::from([(BlockId::from("ready"), "yes".to_string())]))
}

/// The blocks that keep the gate's precondition from holding as these records stand.
fn keeping(flow: &Flow, records: &Records) -> Vec<BlockId> {
    let key = Condition::Test("ready".into());
    let reading = Reading::of_flow(flow, records);
    reading.keeping(flow, &key, 1).into_iter().cloned().collect()
}

#[test]
fn a_block_no_one_has_written_counts_as_its_default_until_it_is_written() {
    let ready: Vec<BlockId> = vec!["ready".into()];
    let unwritten = Records::default();
    // Every block has the value by its default, and none lacks it: a test of every block holds, one of none does not.
    assert_eq!(keeping(&gated(TestKind::All), &unwritten), Vec::<BlockId>::new());
    assert_eq!(keeping(&gated(TestKind::Any), &unwritten), Vec::<BlockId>::new());
    assert_eq!(keeping(&gated(TestKind::None), &unwritten), ready);
    let flow = gated(TestKind::All);
    assert!(Reading::of_flow(&flow, &unwritten).holds(&flow, &"ready".into(), 1));
    // Written, the block says what its version says.
    let no = Records {
        written: vec![Written {
            block: "ready".into(),
            by: Outsider::Person,
            matters: true,
            value: Some("no".to_string()),
            empty: false,
            said_to: None,
            at: Seq(1),
        }],
        ..Records::default()
    };
    assert_eq!(keeping(&gated(TestKind::All), &no), ready);
    assert_eq!(keeping(&gated(TestKind::Any), &no), ready);
    assert_eq!(keeping(&gated(TestKind::None), &no), Vec::<BlockId>::new());
}
