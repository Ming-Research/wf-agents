//! `wf-agents cases`, run as the command on the repository's engineer flow and its cases.

use std::path::PathBuf;
use std::process::{Command, Output};

fn engineer() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer")
}

fn cases(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .arg("cases")
        .args(args)
        .output()
        .expect("the command runs")
}

#[test]
fn the_repository_s_cases_check_against_its_flow_and_every_commit_they_name_is_in_the_made_project() {
    let ran = cases(&["check", "--flow", &engineer().to_string_lossy()]);
    let said = String::from_utf8_lossy(&ran.stdout);
    assert!(ran.status.success(), "{}{said}", String::from_utf8_lossy(&ran.stderr));
    assert!(
        said.lines().any(|line| line.starts_with("R1: 20 cases, in R1.toml")),
        "{said}"
    );
}

#[test]
fn a_case_naming_a_commit_the_made_project_lacks_is_refused_with_the_case_that_names_it() {
    let dir = tempfile::tempdir().expect("a directory");
    let cases_dir = dir.path().join("engineer");
    std::fs::create_dir_all(&cases_dir).expect("made");
    let flows = engineer().join("../cases");
    let common = std::fs::read_to_string(flows.join("engineer/common.toml")).expect("read");
    std::fs::write(cases_dir.join("common.toml"), common).expect("written");
    std::fs::create_dir_all(dir.path().join("projects")).expect("made");
    std::fs::copy(
        flows.join("projects/tally.fast-export"),
        dir.path().join("projects/tally.fast-export"),
    )
    .expect("copied");
    let cell = "cell = \"R1\"\n\n[[cases]]\nid = \"R1-01\"\nitem = 21\nran_before = false\nnew = [\"background\"]\nowes = {}\n\n[[cases.history]]\nmain = \"no-such-branch\"\n";
    std::fs::write(cases_dir.join("R1.toml"), cell).expect("written");
    let ran = cases(&[
        "check",
        "--flow",
        &engineer().to_string_lossy(),
        "--cases",
        &cases_dir.to_string_lossy(),
    ]);
    assert!(!ran.status.success());
    let said = String::from_utf8_lossy(&ran.stderr);
    assert_eq!(said.trim_end(), "R1-01: the made project has no commit no-such-branch");
}

/// The settings of an installation the cases run as: the sample the installation's settings are written from.
fn settings() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../carry/install.sample.toml")
}

/// The cases kept beside the flows, in a directory of their own with the made project, but for R1's, which are these.
fn cases_with(dir: &std::path::Path, r1: &str) -> PathBuf {
    let flows = engineer().join("../cases");
    let cases_dir = dir.join("engineer");
    std::fs::create_dir_all(&cases_dir).expect("made");
    let common = std::fs::read_to_string(flows.join("engineer/common.toml")).expect("read");
    std::fs::write(cases_dir.join("common.toml"), common).expect("written");
    std::fs::write(cases_dir.join("R1.toml"), r1).expect("written");
    std::fs::create_dir_all(dir.join("projects")).expect("made");
    let export = "projects/tally.fast-export";
    std::fs::copy(flows.join(export), dir.join(export)).expect("copied");
    cases_dir
}

#[test]
fn every_case_kept_beside_the_flows_is_built_through_the_engine_its_cell_entered_as_it_says() {
    let data = tempfile::tempdir().expect("a directory");
    let ran = cases(&[
        "build",
        "--flow",
        &engineer().to_string_lossy(),
        "--settings",
        &settings().to_string_lossy(),
        "--data",
        &data.path().to_string_lossy(),
    ]);
    let said = String::from_utf8_lossy(&ran.stdout);
    assert!(ran.status.success(), "{}{said}", String::from_utf8_lossy(&ran.stderr));
    let lines: Vec<&str> = said.lines().collect();
    assert_eq!(lines.iter().filter(|l| l.starts_with("R1-")).count(), 20, "{said}");
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("R1-09: R1 entered for plan_review, having run before; system ")),
        "{said}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("R1-01: R1 entered for guide, background, first; system ")),
        "{said}"
    );
    // What each build made is removed.
    let runs = data.path().join("cell-cases/runs");
    let left: Vec<_> = std::fs::read_dir(&runs)
        .map(|d| d.flatten().collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "{left:?}");
}

#[test]
fn a_case_whose_cell_is_entered_otherwise_or_whose_history_the_walk_does_not_take_is_refused_with_why() {
    let dir = tempfile::tempdir().expect("a directory");
    let r1 = r#"
cell = "R1"

[[cases]]
id = "R1-01"
item = 21
ran_before = true
new = ["background"]
owes = {}

[[cases]]
id = "R1-02"
item = 21
ran_before = true
new = ["plan_review"]
owes = {}

[[cases.history]]
cell = "P1"
blocks.plan_ok.value = "yes"
"#;
    let cases_dir = cases_with(dir.path(), r1);
    let data = tempfile::tempdir().expect("a directory");
    let ran = cases(&[
        "build",
        "--flow",
        &engineer().to_string_lossy(),
        "--cases",
        &cases_dir.to_string_lossy(),
        "--settings",
        &settings().to_string_lossy(),
        "--data",
        &data.path().to_string_lossy(),
    ]);
    assert!(!ran.status.success());
    let said = String::from_utf8_lossy(&ran.stderr);
    assert!(
        said.contains("R1-01: R1 was entered otherwise: the case says [\"background\"] ran it, and [\"background\", \"guide\"] did; the case says it ran before, and it did not"),
        "{said}"
    );
    assert!(
        said.contains("R1-02: the walk entered R0, which the history does not give there, and nothing stopped it"),
        "{said}"
    );
}
