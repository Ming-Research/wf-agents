//! The command line given no subcommand: it fails, as a script that left the subcommand out should see, and says what
//! it could have been given. The version flag, which needs none, still works.

use std::process::{Command, Output};

fn wf(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .args(args)
        .env_remove("WF_AGENTS_DATA")
        .output()
        .unwrap()
}

// With no arguments, or with only the data directory, the command exits non-zero and writes its usage and the names of
// its subcommands, nested ones included, to the standard error, and nothing to the standard output.
#[test]
fn no_subcommand_fails_with_the_usage_and_the_subcommands() {
    for args in [&[][..], &["--data", "somewhere"][..]] {
        let out = wf(args);
        let said = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {said}");
        assert!(
            out.stdout.is_empty(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stdout)
        );
        assert!(said.contains("Usage:"), "{args:?}: {said}");
        for name in ["flow", "config", "init", "run", "item", "queue"] {
            assert!(said.contains(name), "{args:?}: {name} is not in {said}");
        }
    }
    // A subcommand that has subcommands of its own is refused the same way, naming them.
    for (parent, child) in [("flow", "check"), ("config", "check")] {
        let out = wf(&[parent]);
        let said = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{parent}: {said}");
        assert!(said.contains("Usage:") && said.contains(child), "{parent}: {said}");
    }
}

// The version flag asks for no subcommand and still succeeds, printing the workspace's version.
#[test]
fn the_version_flag_still_succeeds_without_a_subcommand() {
    let out = wf(&["--version"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!("wf-agents {}\n", env!("CARGO_PKG_VERSION"))
    );
}
