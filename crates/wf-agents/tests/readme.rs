//! A test of the built command against the README: what the command makes in a data directory is what the README says.

use std::process::Command;

const README: &str = include_str!("../../../README.md");

// The command keeps its database where the README says: the first command to open it makes `db/` in the data
// directory, with `wf-agents.sqlite` in it, and the README names both.
#[test]
fn the_database_is_where_the_readme_says() {
    let dir = std::env::temp_dir().join(format!("wf-agents-readme-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .args(["status", "--data"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let mut made = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        made.push(entry.unwrap().file_name().to_string_lossy().into_owned());
    }
    made.sort();
    let made_file = dir.join("db").join("wf-agents.sqlite").is_file();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(made, ["db"]);
    assert!(made_file, "db/wf-agents.sqlite was not made");
    for named in ["`db/`", "`wf-agents.sqlite`"] {
        assert!(README.contains(named), "the README does not name {named}");
    }
}
