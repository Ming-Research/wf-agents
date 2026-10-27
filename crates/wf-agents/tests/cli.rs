use std::process::Command;

// The built command runs and reports the workspace version: the binary builds, starts and parses its arguments.
#[test]
fn version_names_the_command_and_the_workspace_version() {
    let out = Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(out.status.success(), "exit status {}", out.status);
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!("wf-agents {}\n", env!("CARGO_PKG_VERSION"))
    );
}

// The repository's flow passes its check; a file that is not a flow fails it, saying why.
#[test]
fn flow_check_passes_the_repository_flow_and_fails_a_broken_one() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let out = Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .args(["flow", "check", &format!("{root}/flow/flow.json")])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.starts_with("engineer: 33 cells, 205 rows"), "{stdout}");
    let out = Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .args(["flow", "check", &format!("{root}/db/schema.sql")])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("flow.json"));
}

// A configuration that fits the flow passes its check; a limit on a counter the flow does not have fails it.
#[test]
fn config_check_reads_the_data_directory_against_the_flow() {
    let dir = std::env::temp_dir().join(format!("wf-agents-cli-config-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("projects")).unwrap();
    std::fs::create_dir_all(dir.join("rules")).unwrap();
    std::fs::write(
        dir.join("install.toml"),
        r#"
[owner]
language = "English"
[items]
title_max = 40
[machines]
workstation = "here"
heavy = "builds"
poll_interval = 60
[machines.hosts.here]
slots = 1
[agents.clis.claude]
kind = "claude"
command = "claude"
[agents.levels.l]
desc = "all work"
cli = "claude"
model = "m"
[gh]
allow = ["pr view"]
[query]
max_rows = 10
timeout_ms = 1000
"#,
    )
    .unwrap();
    let project = r#"
repo = "o/p"
checkout = "/tmp/p"
design_goal = "simple"
design_paths = ["design/"]
test_paths = ["tests/"]
rules = ["ground"]
[ci]
fast = "ci"
full = ["ci"]
merge_checks = ["check"]
[limits]
fixes = 5
"#;
    std::fs::write(dir.join("projects/p.toml"), project).unwrap();
    std::fs::write(dir.join("rules/ground.md"), "Facts first.").unwrap();
    let check = || {
        Command::new(env!("CARGO_BIN_EXE_wf-agents"))
            .args(["config", "check", "--data"])
            .arg(&dir)
            .output()
            .unwrap()
    };
    let out = check();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("1 projects (p), 1 rules"));
    std::fs::write(
        dir.join("projects/p.toml"),
        project.replace("fixes = 5", "nonsense = 5"),
    )
    .unwrap();
    let out = check();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("the flow has no counter nonsense"));
    let _ = std::fs::remove_dir_all(&dir);
}
