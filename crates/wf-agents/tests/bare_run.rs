use std::process::Command;

// A bare run names the subcommands on stderr and fails, so a script that drops the subcommand
// does not count as a success. The subcommand rows are matched by their indented name, because
// the about text also contains the word flow.
#[test]
fn bare_run_names_the_subcommands_and_fails() {
    let out = Command::new(env!("CARGO_BIN_EXE_wf-agents")).output().unwrap();
    assert!(!out.status.success(), "exit status {}", out.status);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Usage:"), "{stderr}");
    assert!(stderr.contains("\n  flow "), "{stderr}");
    assert!(stderr.contains("\n  config "), "{stderr}");
}
