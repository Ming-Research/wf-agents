//! The named tests as they come out for a work's records: each kind over blocks written, unwritten and several, on
//! the value each block has now.

use std::collections::BTreeMap;

use decide::{BlockId, Finished, Outcome, Outsider, Records, RunId, Seq, Started, Tests, Who, Write, Written};
use flow::{Compiled, Source};

/// Two choices people write, a third a cell writes, and a test of each kind over the first two; a cell before the
/// merge uses them all.
fn compiled() -> Compiled {
    let flow = r#"
        name = "named"
        order = ["choose", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.main]
        [blocks.a]
        values = ["yes", "no"]
        people = true
        [blocks.b]
        values = ["yes", "no"]
        people = true
        [blocks.c]
        values = ["yes", "no"]
        [tests.both]
        says = "Both are yes"
        kind = "all"
        reads = ["a", "b"]
        is = "yes"
        [tests.either]
        says = "Either is yes"
        kind = "any"
        reads = ["a", "b"]
        is = "yes"
        [tests.neither]
        says = "Neither is yes"
        kind = "none"
        reads = ["a", "b"]
        is = "yes"
        [tests.chosen]
        says = "The cell chose yes"
        kind = "all"
        reads = ["c"]
        is = "yes"
    "#;
    let choose = r#"
        title = "Choose"
        kind = "agent"
        line = "main"
        task = "Choose yes or no."
        requires = ["both", "either", "neither"]
        outputs = ["c"]
    "#;
    let merge = "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nrequires = [\"chosen\"]\n";
    let source = Source {
        flow: flow.to_string(),
        cells: [("choose", choose), ("merge", merge)]
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
    };
    let has = flow::Has {
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the definition compiles")
}

/// Records in which people wrote these values, in this order.
fn written(values: &[(&str, &str)]) -> Records {
    let written = (values.iter().enumerate())
        .map(|(i, (block, value))| Written {
            block: (*block).into(),
            by: Outsider::Person,
            matters: true,
            value: Some(value.to_string()),
            at: Seq(i as u64 + 1),
        })
        .collect();
    Records {
        written,
        ..Records::default()
    }
}

const WAITS: Outcome = Outcome::Waits { who: Who::Owner };

/// What each of the tests over `a` and `b` comes to on these records.
fn outcomes(records: &Records) -> [Outcome; 3] {
    let flow = compiled();
    let tests = flow.tests(records);
    ["both", "either", "neither"].map(|t| tests.test(&t.into()))
}

#[test]
fn with_nothing_written_only_a_test_that_none_has_the_value_holds() {
    assert_eq!(outcomes(&Records::default()), [WAITS, WAITS, Outcome::Holds]);
}

#[test]
fn with_one_block_written_with_the_value_and_one_unwritten_only_a_test_that_one_has_it_holds() {
    assert_eq!(outcomes(&written(&[("a", "yes")])), [WAITS, Outcome::Holds, WAITS]);
    assert_eq!(outcomes(&written(&[("b", "yes")])), [WAITS, Outcome::Holds, WAITS]);
}

#[test]
fn with_one_block_written_with_another_value_only_a_test_that_none_has_it_holds() {
    assert_eq!(outcomes(&written(&[("a", "no")])), [WAITS, WAITS, Outcome::Holds]);
}

#[test]
fn with_every_block_written_with_the_value_a_test_that_all_or_one_has_it_holds() {
    let records = written(&[("a", "yes"), ("b", "yes")]);
    assert_eq!(outcomes(&records), [Outcome::Holds, Outcome::Holds, WAITS]);
    let records = written(&[("a", "yes"), ("b", "no")]);
    assert_eq!(outcomes(&records), [WAITS, Outcome::Holds, WAITS]);
}

#[test]
fn a_test_reads_the_value_a_block_has_now() {
    let records = written(&[("a", "yes"), ("b", "yes"), ("a", "no")]);
    assert_eq!(outcomes(&records), [WAITS, Outcome::Holds, WAITS]);
    let records = written(&[("a", "yes"), ("a", "no"), ("b", "no")]);
    assert_eq!(outcomes(&records), [WAITS, WAITS, Outcome::Holds]);
}

#[test]
fn a_test_reads_what_a_run_wrote_as_it_finished_as_it_reads_what_people_wrote() {
    let flow = compiled();
    let run = RunId(1);
    let started = Started {
        run,
        cell: "choose".into(),
        at: Seq(1),
        because: Vec::new(),
        basis: BTreeMap::from([(BlockId::from("c"), None)]),
    };
    let finished = |value: &str| Finished {
        run,
        at: Seq(2),
        writes: vec![Write {
            block: "c".into(),
            matters: true,
            value: Some(value.to_string()),
        }],
    };
    let mut records = Records {
        started: vec![started],
        ..Records::default()
    };
    assert_eq!(flow.tests(&records).test(&"chosen".into()), WAITS);
    records.finished = vec![finished("yes")];
    assert_eq!(flow.tests(&records).test(&"chosen".into()), Outcome::Holds);
    records.finished = vec![finished("no")];
    assert_eq!(flow.tests(&records).test(&"chosen".into()), WAITS);
}

#[test]
fn a_name_the_definition_does_not_declare_never_holds() {
    let flow = compiled();
    let records = written(&[("a", "yes"), ("b", "yes")]);
    assert_eq!(flow.tests(&records).test(&"nobody".into()), WAITS);
}
