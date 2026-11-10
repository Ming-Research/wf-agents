//! The engineer's flow, as the repository defines it, passes `wf-agents flow check`: it compiles against the sources
//! the engine has.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn the_engineers_flow_compiles() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
    let out = Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .args(["flow", "check"])
        .arg(&dir)
        .output()
        .expect("the command runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}
