//! The named tests: each a cell uses is declared, each declared is one the engine has and is used.

use decide::TestName;
use flow::{FLOW_FILE, FaultKind::*};

use crate::support::{at, edit, faults, faults_with, known, valid};

const REQUIRES: &str = r#"requires = ["may_merge"]"#;

#[test]
fn every_test_a_cell_requires_is_declared_and_named_once() {
    let source = edit(
        valid(),
        "merge.toml",
        REQUIRES,
        r#"requires = ["may_merge", "nobody", "may_merge"]"#,
    );
    let undeclared = UndeclaredTest {
        test: "nobody".to_string(),
    };
    let twice = Twice {
        name: "may_merge".to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at("merge.toml", "requires[1]", undeclared),
            at("merge.toml", "requires[2]", twice),
        ]
    );
}

#[test]
fn every_declared_test_is_one_the_engine_has() {
    let mut known = known();
    known.remove(&TestName::from("may_merge"));
    let what = UnknownTest {
        test: "may_merge".to_string(),
    };
    assert_eq!(faults_with(&valid(), &known), [at(FLOW_FILE, "tests.may_merge", what)]);
}

#[test]
fn every_declared_test_is_used_by_a_cell() {
    let line = "may_merge = \"The work may merge\"";
    let source = edit(valid(), FLOW_FILE, line, &format!("{line}\nextra = \"Something else\""));
    let mut known = known();
    known.insert("extra".into());
    let what = UnusedTest {
        test: "extra".to_string(),
    };
    assert_eq!(faults_with(&source, &known), [at(FLOW_FILE, "tests.extra", what)]);
}
