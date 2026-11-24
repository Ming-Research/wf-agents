//! `wf-agents flow situations`, run as the command: a file for each situation a round of a cell was found in by the
//! search of works of the flow, with the system prompt, the message and the form, as many as it says; and the same files
//! each time it is run.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn engineer() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer")
}

fn small() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../flow/tests/project/small")
}

fn situations(flow: &Path, out: &Path, cell: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wf-agents"));
    command.args(["flow", "situations"]).arg(flow).arg("--out").arg(out);
    if let Some(cell) = cell {
        command.args(["--cell", cell]);
    }
    command.output().expect("the command runs")
}

/// Each file under the directory, by its path from it, with its text.
fn files(dir: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, at: &Path, into: &mut BTreeMap<String, String>) {
        for entry in fs::read_dir(at).expect("the directory is read") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(root, &path, into);
            } else {
                let name = path.strip_prefix(root).expect("under the root").display().to_string();
                into.insert(name, fs::read_to_string(&path).expect("the file is read"));
            }
        }
    }
    let mut all = BTreeMap::new();
    walk(dir, dir, &mut all);
    all
}

#[test]
fn each_situation_of_a_cell_is_written_with_its_system_prompt_its_message_and_the_form_the_same_each_time() {
    let (first, second) = (
        tempfile::tempdir().expect("a directory"),
        tempfile::tempdir().expect("a directory"),
    );
    let ran = situations(&engineer(), first.path(), Some("R1"));
    assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stderr));
    let dir = first.path().join("R1");
    let written = fs::read_dir(&dir).expect("the cell's directory is read").count();
    let said = String::from_utf8_lossy(&ran.stdout);
    assert!(
        written > 0 && said.starts_with(&format!("R1: {written} situations, in ")),
        "{said}"
    );
    // A file holds the situation, then the system prompt, the message and the form, each under a line that says which it
    // is; and each setting a message names is given beside its name, which what the cell returns names.
    let mut named = 0;
    for (path, text) in files(first.path()) {
        assert!(text.starts_with("situation = \""), "{path}");
        for part in ["system prompt", "message", "return form"] {
            assert!(text.contains(&format!("\n===== {part} =====\n\n")), "{path}: {part}");
        }
        let message = text.split("===== message =====").nth(1).expect("a message");
        let bare = message.matches("`owner.language`").count();
        let given = message.matches("`owner.language` ({owner.language})").count();
        assert_eq!(bare, given, "{path}");
        named += given;
    }
    assert!(named > 0);
    // Asked again, a cell's directory that holds files is not written over.
    assert!(!situations(&engineer(), first.path(), Some("R1")).status.success());
    // Run again, the same files are written.
    let again = situations(&engineer(), second.path(), Some("R1"));
    assert!(again.status.success(), "{}", String::from_utf8_lossy(&again.stderr));
    assert_eq!(files(first.path()), files(second.path()));
}

#[test]
fn a_flow_is_written_the_same_situations_each_time_it_is_run() {
    let (first, second) = (
        tempfile::tempdir().expect("a directory"),
        tempfile::tempdir().expect("a directory"),
    );
    for out in [&first, &second] {
        let ran = situations(&small(), out.path(), None);
        assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stderr));
    }
    let all = files(first.path());
    assert!(!all.is_empty());
    assert_eq!(all, files(second.path()));
}
