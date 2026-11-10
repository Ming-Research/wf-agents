//! `wf-agents flow check`, run as the command: a definition is compiled against the sources outside a work that the
//! engine has, by their names, so that a source misspelt is a fault.

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

#[test]
fn a_block_that_names_a_source_the_engine_does_not_have_is_a_fault() {
    let dir = tempfile::tempdir().expect("a directory");
    for entry in fs::read_dir(small()).expect("the small flow is read") {
        let path = entry.expect("an entry").path();
        fs::copy(&path, dir.path().join(path.file_name().expect("a name"))).expect("the file is copied");
    }
    let flow = dir.path().join("flow.toml");
    let text = fs::read_to_string(&flow).expect("flow.toml is read");
    assert!(text.contains("outside = \"main\""));
    fs::write(&flow, text.replacen("outside = \"main\"", "outside = \"mian\"", 1)).expect("flow.toml is written");
    let out = check(dir.path());
    assert!(!out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "flow.toml blocks.main.outside: main is written by the source mian, which the engine does not have\n"
    );
}
