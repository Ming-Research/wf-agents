//! `wf-agents flow check`, run as the command: a definition is compiled against the sources outside a work and the
//! acts that the engine has, by their names, so that a source or an act misspelt is a fault.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The small flow the page is tested and shown with.
fn small() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/tests/flows/small")
}

fn check(dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .args(["flow", "check"])
        .arg(dir)
        .output()
        .expect("the command runs")
}

#[test]
fn a_definition_whose_blocks_name_sources_the_engine_has_compiles() {
    let out = check(&small());
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("small compiles: "));
}

/// A copy of the small flow with `from` replaced by `to` in one of its files; `from` must be there.
fn small_with(file: &str, from: &str, to: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a directory");
    for entry in fs::read_dir(small()).expect("the small flow is read") {
        let path = entry.expect("an entry").path();
        fs::copy(&path, dir.path().join(path.file_name().expect("a name"))).expect("the file is copied");
    }
    let path = dir.path().join(file);
    let text = fs::read_to_string(&path).expect("the file is read");
    assert!(text.contains(from), "{file} has no {from:?}");
    fs::write(&path, text.replacen(from, to, 1)).expect("the file is written");
    dir
}

#[test]
fn a_block_that_names_a_source_the_engine_does_not_have_is_a_fault() {
    let dir = small_with("flow.toml", "outside = \"main\"", "outside = \"mian\"");
    let out = check(dir.path());
    assert!(!out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "flow.toml blocks.main.outside: main is written by the source mian, which the engine does not have\n"
    );
}

#[test]
fn a_cell_that_names_an_act_the_engine_does_not_have_is_a_fault() {
    let dir = small_with("merge.toml", "act = \"merge\"", "act = \"deploy\"");
    let out = check(dir.path());
    assert!(!out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "merge.toml act: the engine has no act named \"deploy\"\n"
    );
}
