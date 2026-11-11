//! The deliver flow, the flow of a deliverable item, as the repository defines it, passes `wf-agents flow check`: it
//! compiles against the sources and the acts the engine has.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn the_deliver_flow_compiles() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../flow/deliver");
    let out = Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .args(["flow", "check"])
        .arg(&dir)
        .output()
        .expect("the command runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(said.starts_with("deliver compiles: 1 cell, "), "{said}");
}
