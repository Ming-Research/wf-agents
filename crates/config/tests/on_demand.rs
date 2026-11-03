//! A project's CI that runs on demand, read from a data directory's files as the engine reads them. The reference is
//! the guarantee a merge rests on: a head merges only on a run of the full CI, and a run made by hand with tests named
//! passes the check of the workflow that ran it, so the workflow that takes tests by hand must not be one of the full
//! CI's.

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

/// A data directory holding one project that extends the shared file and takes `ci` as its own `[ci]` table; the
/// directory is removed when this is dropped.
struct Data(PathBuf);

impl Data {
    fn new(name: &str, ci: &str) -> Data {
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
                "extends = [\"common\"]\nrepo = \"o/p\"\ncheckout = \"/tmp/p\"\nrules = [\"ground\"]\n[ci]\n{ci}"
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

fn refused(name: &str, ci: &str) -> Option<String> {
    let data = Data::new(name, ci);
    config::working(data.path()).err().map(|e| e.to_string())
}

// A project whose CI runs on demand has a fast workflow that is not one of its full CI's: the fast workflow is run by
// hand with the tests named, and its green means those tests passed, so that, as one of the full workflows, it would
// give the merge a check that the whole CI never ran. The same project with another workflow for the fast CI, and the
// project that does not run on demand, are read as before. The input that takes the tests is a name: an empty one
// cannot be passed to the workflow.
#[test]
fn a_ci_on_demand_has_its_fast_workflow_apart_from_the_full_ones_and_names_its_input() {
    // The shared file names `ci` as both the fast workflow and the full one.
    let e = refused("same", "on_demand = true\n").expect("on demand with one workflow for both is refused");
    assert!(e.contains("projects/p.toml") && e.contains("on_demand"), "{e}");
    let e = refused("empty", "on_demand = true\nfast = \"fast\"\ntests_input = \"\"\n")
        .expect("an empty input name is refused");
    assert!(e.contains("projects/p.toml") && e.contains("tests_input"), "{e}");
    let apart = Data::new("apart", "on_demand = true\nfast = \"fast\"\n");
    let c = config::working(apart.path()).unwrap();
    assert!(c.projects["p"].ci.on_demand);
    assert_eq!(c.projects["p"].ci.fast, "fast");
    // Off, one workflow for both stays as it was.
    let off = Data::new("off", "");
    let c = config::working(off.path()).unwrap();
    assert!(!c.projects["p"].ci.on_demand);
    assert_eq!(c.projects["p"].ci.fast, "ci");
}
