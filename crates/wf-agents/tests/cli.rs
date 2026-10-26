use std::process::Command;

// The built command runs and reports the workspace version: the binary builds, starts and parses its arguments.
#[test]
fn version_names_the_command_and_the_workspace_version() {
    let out = Command::new(env!("CARGO_BIN_EXE_wf-agents")).arg("--version").output().unwrap();
    assert!(out.status.success(), "exit status {}", out.status);
    assert_eq!(String::from_utf8(out.stdout).unwrap(), format!("wf-agents {}\n", env!("CARGO_PKG_VERSION")));
}
