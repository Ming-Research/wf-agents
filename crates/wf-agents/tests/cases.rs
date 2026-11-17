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
