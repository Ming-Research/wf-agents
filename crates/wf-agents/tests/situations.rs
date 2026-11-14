//! `wf-agents flow situations`, run as the command on the repository's engineer flow: a file for each situation an
//! entry of a cell can be in, with the system prompt, the message and the form, as many as it says.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn engineer() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer")
}

fn situations(out: &Path, cell: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .args(["flow", "situations"])
        .arg(engineer())
        .arg("--out")
        .arg(out)
        .args(["--cell", cell])
        .output()
        .expect("the command runs")
}

/// The text of the file of the situation of this name, under the cell's directory.
fn file(dir: &Path, name: &str) -> String {
    let found = (fs::read_dir(dir).expect("the cell's directory is read"))
        .map(|entry| entry.expect("an entry").path())
        .find(|path| path.to_string_lossy().ends_with(&format!("-{name}.md")));
    fs::read_to_string(found.unwrap_or_else(|| panic!("no situation {name}"))).expect("the file is read")
}

/// The part of a message under the heading of this datum, to the next heading of its level.
fn datum(text: &str, block: &str) -> String {
    let message = text.split("===== message =====").nth(1).expect("a message");
    let message = message.split("===== return form =====").next().unwrap_or_default();
    let heading = format!("## `{block}`");
    let Some(at) = message.find(&heading).map(|at| &message[at..]) else {
        return String::new();
    };
    let end = at[heading.len()..]
        .find("\n## ")
        .map_or(at.len(), |end| end + heading.len());
    at[..end].trim_end().to_string()
}

#[test]
fn each_situation_of_a_cell_is_written_with_its_system_prompt_its_message_and_the_form() {
    let out = tempfile::tempdir().expect("a directory");
    let ran = situations(out.path(), "R2");
    assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stderr));
    let dir = out.path().join("R2");
    let written = fs::read_dir(&dir).expect("the cell's directory is read").count();
    let said = String::from_utf8_lossy(&ran.stdout);
    assert!(said.starts_with(&format!("R2: {written} situations, in ")), "{said}");
    let text = file(&dir, "later-on_plan-ran_holds_last-rest_lacks");
    assert!(text.starts_with("situation = \"later-on_plan-ran_holds_last-rest_lacks\"\n"));
    for part in ["system prompt", "message", "return form"] {
        assert!(text.contains(&format!("\n===== {part} =====\n\n")), "{part}");
    }
    // Asked again, a cell's directory that holds files is not written over.
    let again = situations(out.path(), "R2");
    assert!(!again.status.success());
}

#[test]
fn a_datum_given_latest_the_session_lacks_is_sent_whole_and_one_it_holds_new_its_held_line() {
    let out = tempfile::tempdir().expect("a directory");
    assert!(situations(out.path(), "R2").status.success());
    let dir = out.path().join("R2");
    let lacks = datum(&file(&dir, "later-on_plan-ran_holds_last-rest_lacks"), "background");
    let holds_latest = datum(
        &file(&dir, "later-on_plan-ran_holds_last-rest_holds_latest"),
        "background",
    );
    let holds = datum(&file(&dir, "later-on_plan-ran_holds_last-rest_holds"), "background");
    assert!(lacks.ends_with("{content}"), "{lacks}");
    assert!(
        !holds_latest.is_empty() && !holds_latest.contains("{content}"),
        "{holds_latest}"
    );
    assert_ne!(lacks, holds_latest);
    assert_eq!(holds, "");
}

#[test]
fn a_trigger_on_a_session_begun_anew_runs_a_first_or_reopened_entry_never_a_later_one() {
    let out = tempfile::tempdir().expect("a directory");
    assert!(situations(out.path(), "R0").status.success());
    let names: Vec<String> = (fs::read_dir(out.path().join("R0")).expect("the cell's directory is read"))
        .map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned())
        .collect();
    let with = |entry: &str| names.iter().any(|n| n.contains(&format!("-{entry}-on_session-")));
    assert!(with("first") && with("reopened") && !with("later"), "{names:?}");
    // Reopened, the background the cell worked from, which is not new, is sent whole.
    let reopened = file(&out.path().join("R0"), "reopened-on_session-rest_lacks");
    assert!(datum(&reopened, "background").ends_with("{content}"), "{reopened}");
}
