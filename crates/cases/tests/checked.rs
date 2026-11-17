//! The cases' files read and checked against a compiled flow: the repository's own, and made ones with every kind of
//! fault the check finds, each said at once.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use flow::{Compiled, Has};

/// The directory that holds the repository's flows.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow")
}

/// The engineer's flow, compiled against what the engine has for it.
fn engineer() -> Compiled {
    let beside = flow::agents::read(&root()).expect("the files beside the flows are read");
    let source = flow::read(&root().join("engineer")).expect("the engineer's flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<BTreeSet<_>>();
    let has = Has {
        sources: names(&[
            "answer",
            "asked",
            "dependencies",
            "guide",
            "main",
            "session",
            "start",
            "head",
            "tree",
            "words",
        ]),
        acts: names(&["start", "check", "merge", "deliver"]),
        tools: names(&["query", "run_tests"]),
        system: flow::system::reserved(&beside).into_iter().collect(),
    };
    flow::compile(&source, &has).expect("the engineer's flow compiles")
}

/// The faults the check finds in these files, each as it says it.
fn faults(common: &str, cell: &str) -> Vec<String> {
    let dir = tempfile::tempdir().expect("a directory");
    std::fs::write(dir.path().join(cases::COMMON), common).expect("written");
    std::fs::write(dir.path().join("R1.toml"), cell).expect("written");
    let read = cases::read(dir.path()).expect("the files read");
    match cases::check(&engineer(), &read) {
        Ok(_) => Vec::new(),
        Err(faults) => faults.iter().map(|f| f.to_string()).collect(),
    }
}

const COMMON: &str = r##"
project = "tally"

[tests]
mark = "#[test]"
name_after = "fn "
under = ["crates/"]

[items.21]
title = "月报"
main = "v-bug"
background = "The monthly report puts January to September in one row."

[texts]
POINT = "The test asserts only the number of rows."

[sets.PLAN]
deep_needed = { value = "no", content = "One round finds it." }
plan_owner = "Cut seven characters."
plan_agent = "report.rs: cut seven."
acceptance = "The test passes."

[results.month]
fails = [{ names = ["*"], new = true, said = "assertion failed" }]
"##;

#[test]
fn the_repository_s_cases_check_against_its_flow_each_cell_with_twenty() {
    let read = cases::read(&root().join("cases/engineer")).expect("the repository's cases read");
    let checked = match cases::check(&engineer(), &read) {
        Ok(checked) => checked,
        Err(faults) => panic!(
            "{}",
            faults.iter().map(|f| f.to_string()).collect::<Vec<_>>().join("\n")
        ),
    };
    assert!(!checked.cells.is_empty());
    for of in &checked.cells {
        assert_eq!(of.cases.len(), 20, "{}", of.file);
    }
}

#[test]
fn a_case_whose_names_fit_the_flow_is_given_with_its_history_resolved() {
    let cell = r#"
cell = "R1"

[[cases]]
id = "R1-01"
item = 21
ran_before = true
new = ["plan_review"]
results = "month"

[[cases.history]]
cell = "R1"
set = "PLAN"
commit = "w-monthly-test"

[[cases.history]]
cell = "P1"
blocks.plan_review.text = "POINT"
blocks.plan_ok.value = "no"

[[cases.history]]
owner = "words"
to = "R1"
content = "Why not the categories?"

[cases.owes]
writes = { deep_needed = "no" }
changes = ["plan"]
answers = ["plan_review", "r1_words"]
question = "none"
"#;
    let dir = tempfile::tempdir().expect("a directory");
    std::fs::write(dir.path().join(cases::COMMON), COMMON).expect("written");
    std::fs::write(dir.path().join("R1.toml"), cell).expect("written");
    let read = cases::read(dir.path()).expect("the files read");
    let checked = cases::check(&engineer(), &read).expect("the case checks");
    let case = &checked.cells[0].cases[0];
    assert_eq!(case.cell.as_str(), "R1");
    let cases::Step::Round(r1) = &case.history[0] else {
        panic!("a round first");
    };
    let blocks: Vec<&str> = r1.hands_in.iter().map(|h| h.block.as_str()).collect();
    assert_eq!(blocks, ["acceptance", "deep_needed", "plan_agent", "plan_owner"]);
    assert_eq!(r1.commit.as_deref(), Some("w-monthly-test"));
    let cases::Step::Round(p1) = &case.history[1] else {
        panic!("a round second");
    };
    let of = |block: &str| {
        p1.hands_in
            .iter()
            .find(|h| h.block.as_str() == block)
            .expect("handed in")
    };
    assert_eq!(of("plan_review").content, "The test asserts only the number of rows.");
    assert_eq!(of("plan_ok").value.as_deref(), Some("no"));
    assert_eq!(
        case.history[2],
        cases::Step::Words {
            to: "R1".into(),
            text: "Why not the categories?".into()
        }
    );
}

#[test]
fn every_fault_of_every_case_is_said_at_once_and_a_case_with_one_is_left_out() {
    let cell = r#"
cell = "R1"

[[cases]]
id = "R1-01"
item = 99
ran_before = false
new = ["plan"]
results = "none_such"

[[cases.history]]
cell = "R1"
blocks.plan_ok.value = "yes"
blocks.deep_needed.value = "maybe"
blocks.found.value = "yes"
blocks.plan_owner.content = " "

[[cases.history]]
cell = "R1"
owner = "background"

[[cases.history]]
owner = "words"
to = "P1"
content = "Why?"

[[cases.history]]
owner = "background"
text = "NO_SUCH"

[cases.owes]
writes = { plan_ok = "yes", deep_needed = "maybe" }
not_writes = ["merged"]
changes = ["change"]
answers = ["found"]
question = "sometimes"
"#;
    let said = faults(COMMON, cell);
    let expected = [
        "R1.toml: R1-01: item 99 is no made item of common.toml",
        "R1.toml: R1-01: history 1: plan_ok is none of what R1 writes",
        "R1.toml: R1-01: history 1: deep_needed: maybe is none of its values (yes, no)",
        "R1.toml: R1-01: history 1: found: it is no choice, and takes no value",
        "R1.toml: R1-01: history 1: plan_owner: it is handed in with nothing in it",
        "R1.toml: R1-01: history 2: an entry is one of a round of a `cell`, an act of the `owner` and `main` moving",
        "R1.toml: R1-01: history 3: words go to an agent's cell that has words of its own, not \"P1\"",
        "R1.toml: R1-01: history 4: no text is named NO_SUCH",
        "R1.toml: R1-01: new: plan is none of what R1 watches",
        "R1.toml: R1-01: results: no rules are named none_such",
        "R1.toml: R1-01: owes: writes plan_ok, none of what R1 writes",
        "R1.toml: R1-01: owes: writes deep_needed = maybe: `written`, or one of a choice's values",
        "R1.toml: R1-01: owes: not_writes merged, none of what R1 writes",
        "R1.toml: R1-01: owes: change is no datum or group R1 writes",
        "R1.toml: R1-01: owes: answers found, which is no dialogue R1 writes in",
        "R1.toml: R1-01: owes: question is one of none, waits, any, not sometimes",
    ];
    for fault in expected {
        assert!(
            said.iter().any(|s| s == fault),
            "missing: {fault}\nsaid:\n{}",
            said.join("\n")
        );
    }
    assert_eq!(said.len(), expected.len(), "said:\n{}", said.join("\n"));
}

#[test]
fn a_file_of_a_cell_that_is_no_agent_s_and_two_cases_of_one_id_are_refused() {
    let engine = "cell = \"M1\"\n";
    let said = faults(COMMON, engine);
    assert_eq!(said, ["R1.toml: M1 is no agent's cell of the flow"]);
    let twice = r#"
cell = "R1"

[[cases]]
id = "R1-01"
item = 21
ran_before = false
new = ["background"]
owes = {}

[[cases]]
id = "R1-01"
item = 21
ran_before = false
new = ["background"]
owes = {}
"#;
    assert_eq!(faults(COMMON, twice), ["R1.toml: R1-01: another case has its id"]);
}
