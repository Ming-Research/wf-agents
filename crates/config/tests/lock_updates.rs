//! A project's `[[lock_updates]]` tables, read from a data directory's files as the engine reads them, which is also
//! what `wf-agents config check` tells: what an agent is told the project lists, and every fault of the tables at once.

use std::path::{Path, PathBuf};

const INSTALL: &str = r#"
[owner]
language = "Chinese"

[items]
title_max = 30

[machines]
workstation = "laptop"
heavy = "builds and tests"
poll_interval = 300

[machines.hosts.laptop]
slots = 2

[agents.clis.claude]
kind = "claude"
command = "claude"

[agents.levels.senior]
desc = "New decisions"
cli = "claude"
model = "m"

[gh]
allow = ["pr view"]

[query]
max_rows = 100
timeout_ms = 2000
"#;

const PROJECT: &str = r#"
repo = "o/p"
checkout = "/tmp/p"
design_goal = "correct, then fast"
test_paths = ["tests/"]
rules = ["ground"]
[ci]
fast = "ci"
full = ["ci"]
merge_checks = ["check"]
"#;

/// The setting an agent is told of, in the words the tool's input names it by.
const SETTING: &str = "`project.lock_updates`";

/// A data directory holding one project, `p`, with these tables; the directory is removed when this is dropped.
struct Data(PathBuf);

impl Data {
    fn new(name: &str, tables: &str) -> Data {
        let dir = std::env::temp_dir().join(format!("wf-agents-config-lock-updates-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let put = |file: &str, text: &str| {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        put("install.toml", INSTALL);
        put("rules/ground.md", "Write in `owner.language`.");
        put("projects/p.toml", &format!("{PROJECT}\n{tables}"));
        Data(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Data {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// A project's file with no such table reads as before, and an agent is told `none`; one that lists some tells an agent
// each as it is written, the time limit where it is set. A table with a field the list does not know is refused.
#[test]
fn a_project_lists_the_lock_files_the_tool_brings_in_line_and_an_agent_is_told_them() {
    let none = Data::new("none", "");
    let c = config::working(none.path()).unwrap();
    assert_eq!(
        c.named_settings("p", SETTING),
        [("project.lock_updates".to_string(), "none".to_string())]
    );
    let listed = Data::new(
        "listed",
        "[[lock_updates]]\nlock = \"Cargo.lock\"\n\
         command = \"env RUSTUP_AUTO_INSTALL=0 cargo update --workspace\"\n\
         [[lock_updates]]\nlock = \"client/package-lock.json\"\n\
         command = \"npm install --package-lock-only --ignore-scripts\"\ntimeout = 90\n",
    );
    let c = config::working(listed.path()).unwrap();
    assert_eq!(
        c.named_settings("p", SETTING),
        [(
            "project.lock_updates".to_string(),
            "[{\"lock\":\"Cargo.lock\",\"command\":\"env RUSTUP_AUTO_INSTALL=0 cargo update --workspace\"},\
             {\"lock\":\"client/package-lock.json\",\"command\":\"npm install --package-lock-only --ignore-scripts\",\
             \"timeout\":90}]"
                .to_string()
        )]
    );
    let colour = Data::new(
        "colour",
        "[[lock_updates]]\nlock = \"a.lock\"\ncommand = \"true\"\ncolour = \"red\"\n",
    );
    let e = config::working(colour.path()).expect_err("an unknown field is refused");
    let said = e.to_string();
    assert!(said.contains("projects/p.toml") && said.contains("colour"), "{said}");
}

// The check tells every fault of the tables at once, each with the file and the lock file it is in: a lock file that is
// not a path in the repository, whether it goes up or is absolute; a command with a quote that is not closed or with no
// words; a time limit of 0; and a lock file listed twice, however its path is spelled. A table with none of these adds
// nothing to what is told.
#[test]
fn the_check_tells_every_fault_of_the_lock_updates_at_once() {
    let data = Data::new(
        "faults",
        "[[lock_updates]]\nlock = \"../Cargo.lock\"\ncommand = \"cargo 'update\"\ntimeout = 0\n\
         [[lock_updates]]\nlock = \"/abs/Cargo.lock\"\ncommand = \"  \"\n\
         [[lock_updates]]\nlock = \"Cargo.lock\"\ncommand = \"true\"\n\
         [[lock_updates]]\nlock = \"./Cargo.lock\"\ncommand = \"true\"\n",
    );
    let e = config::working(data.path()).expect_err("the faults are refused");
    let said = e.to_string();
    for fault in [
        "projects/p.toml: lock_updates of \"../Cargo.lock\": lock is not a path in the project's repository",
        "projects/p.toml: lock_updates of \"../Cargo.lock\": command cannot be split into words: a quote is not closed",
        "projects/p.toml: lock_updates of \"../Cargo.lock\": timeout is 0",
        "projects/p.toml: lock_updates of \"/abs/Cargo.lock\": lock is not a path in the project's repository",
        "projects/p.toml: lock_updates of \"/abs/Cargo.lock\": command holds no word",
        "projects/p.toml: lock_updates of \"./Cargo.lock\": lock is listed twice",
    ] {
        assert!(said.contains(fault), "{fault}\n{said}");
    }
    // Nothing else is said: the faults are those and no others.
    assert_eq!(said.matches("lock_updates of ").count(), 6, "{said}");
}
