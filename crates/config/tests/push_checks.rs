//! The commands a project's configuration has the engine's ci tool run before it pushes (`[[ci.push_checks]]`), read
//! from a data directory's files as the engine reads them.

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

const SHARED: &str = r#"
design_goal = "correct, then fast"
test_paths = ["tests/"]
review_list = ["one", "two"]
[ci]
fast = "ci"
full = ["ci"]
merge_checks = ["check"]
tests_input = "tests"
[limits]
fixes = 5
"#;

/// The setting the ci tool's text names, which an agent is shown with its value for the project.
const SETTING: &str = "`project.ci.push_checks`";

/// A data directory holding one project whose file ends with `tables`, written after its `[ci]` table; the directory is
/// removed when this is dropped.
struct Data(PathBuf);

impl Data {
    fn new(name: &str, tables: &str) -> Data {
        let dir = std::env::temp_dir().join(format!("wf-agents-config-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let put = |file: &str, text: &str| {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        put("install.toml", INSTALL);
        put("shared/common.toml", SHARED);
        put("rules/ground.md", "Write in `owner.language`.");
        put(
            "projects/p.toml",
            &format!(
                "extends = [\"common\"]\nrepo = \"o/p\"\ncheckout = \"/tmp/p\"\nrules = [\"ground\"]\n[ci]\n{tables}"
            ),
        );
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

/// Why the configuration is refused, or none when it is read.
fn refused(name: &str, tables: &str) -> Option<String> {
    let data = Data::new(name, tables);
    config::working(data.path()).err().map(|e| e.to_string())
}

// A project lists the commands the ci tool runs before it pushes as `[[ci.push_checks]]` tables. One that lists none
// reads as `none` in the settings an agent is shown and writes none back; one that lists some keeps their order and
// reads as the list; the time limit is 120 seconds unless set.
#[test]
fn a_project_lists_the_commands_run_before_a_push() {
    let none = Data::new("push-none", "");
    let c = config::working(none.path()).unwrap();
    assert!(c.projects["p"].ci.push_checks.is_empty());
    assert!(
        serde_json::to_value(&c.projects["p"].ci)
            .unwrap()
            .get("push_checks")
            .is_none()
    );
    assert_eq!(
        c.named_settings("p", SETTING),
        [("project.ci.push_checks".to_string(), "none".to_string())]
    );
    let listed = Data::new(
        "push-listed",
        "[[ci.push_checks]]\nname = \"format\"\ncommand = \"cargo fmt --all --check\"\n\
         [[ci.push_checks]]\nname = \"types\"\ncommand = \"sh -c 'cd client && tsc --noEmit'\"\ntimeout = 30\n",
    );
    let c = config::working(listed.path()).unwrap();
    let checks = &c.projects["p"].ci.push_checks;
    assert_eq!(
        checks.iter().map(|k| k.name.as_str()).collect::<Vec<_>>(),
        ["format", "types"]
    );
    assert_eq!((checks[0].timeout_secs(), checks[1].timeout_secs()), (120, 30));
    // The value an agent is shown is the list as JSON, read here as a value so that the order of a table's keys does not
    // matter.
    let shown = c.named_settings("p", SETTING);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].0, "project.ci.push_checks");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&shown[0].1).unwrap(),
        serde_json::json!([
            { "name": "format", "command": "cargo fmt --all --check" },
            { "name": "types", "command": "sh -c 'cd client && tsc --noEmit'", "timeout": 30 },
        ])
    );
}

// The check of the configuration tells every fault of the tables at once, each with the file, the check and what is
// wrong: a name that is empty, has spaces around it or is listed twice, a command that cannot be split into words or
// holds none, and a time limit of 0; a table with a field the list does not know is refused too.
#[test]
fn the_check_tells_every_fault_of_the_push_checks_at_once() {
    let faulty = "[[ci.push_checks]]\nname = \"\"\ncommand = \"cargo 'fmt\"\ntimeout = 0\n\
                  [[ci.push_checks]]\nname = \" lint\"\ncommand = \"  \"\n\
                  [[ci.push_checks]]\nname = \"format\"\ncommand = \"true\"\n\
                  [[ci.push_checks]]\nname = \"format\"\ncommand = \"true\"\n";
    let e = refused("push-faulty", faulty).expect("faulty tables are refused");
    for what in [
        "name is empty or has spaces around it",
        "command cannot be split into words",
        "timeout is 0",
        "command holds no word",
        "name is listed twice",
    ] {
        assert!(
            e.lines()
                .any(|l| l.contains("projects/p.toml: ci.push_checks of ") && l.contains(what)),
            "{what}: {e}"
        );
    }
    let colour = "[[ci.push_checks]]\nname = \"a\"\ncommand = \"true\"\ncolour = \"red\"\n";
    let e = refused("push-colour", colour).expect("a field the list does not know is refused");
    assert!(e.contains("projects/p.toml") && e.contains("colour"), "{e}");
}
