//! A project's `test_paths`, read from a data directory's files as the engine reads them and given to the flow as the
//! setting `{"config": "test_paths"}`. Their reference is the configuration file itself: the list the owner wrote is
//! the list the flow reads, in the same order, each pattern as written, and an empty list is a list.

use std::path::{Path, PathBuf};

use serde_json::json;

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

/// A data directory holding one project, `p`, whose `test_paths` is the given line; removed when this is dropped.
struct Data(PathBuf);

impl Data {
    fn new(name: &str, test_paths: &str) -> Data {
        let dir = std::env::temp_dir().join(format!("wf-agents-config-test-paths-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let put = |file: &str, text: &str| {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        put("install.toml", INSTALL);
        put("rules/ground.md", "Write in `owner.language`.");
        put(
            "projects/p.toml",
            &format!(
                "repo = \"o/p\"\ncheckout = \"/tmp/p\"\ndesign_goal = \"correct, then fast\"\n{test_paths}\n\
                 rules = [\"ground\"]\n[ci]\nfast = \"ci\"\nfull = [\"ci\"]\nmerge_checks = [\"check\"]\n"
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

// An empty list is a setting with nothing in it, not a setting that is missing: the flow tells the adversarial tester
// that no path holds tests, which is not the project saying nothing. A list with patterns comes out as written, in its
// order, `**` and `*` included and a path with a colon whole.
#[test]
fn the_setting_is_the_list_the_project_wrote_and_an_empty_list_is_still_given() {
    assert!(config::SETTINGS.contains(&"test_paths"));
    let empty = Data::new("empty", "test_paths = []");
    let c = config::working(empty.path()).unwrap();
    assert_eq!(c.projects["p"].setting("test_paths"), Some(json!([])));
    let written = [
        "crates/*/tests/",
        "crates/*/src/**/tests.rs",
        "client/src/**/*.test.ts",
        "odd:dir/file.rs",
        "tests/",
    ];
    let line = format!(
        "test_paths = [{}]",
        written.iter().map(|p| format!("{p:?}")).collect::<Vec<_>>().join(", ")
    );
    let listed = Data::new("listed", &line);
    let c = config::working(listed.path()).unwrap();
    assert_eq!(c.projects["p"].setting("test_paths"), Some(json!(written)));
}
