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
        Data::extending(name, SHARED, ci)
    }

    /// The same, the file the project extends being `shared`.
    fn extending(name: &str, shared: &str, ci: &str) -> Data {
        let dir = std::env::temp_dir().join(format!("wf-agents-config-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let put = |file: &str, text: &str| {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        put("install.toml", INSTALL);
        put("shared/common.toml", shared);
        put("rules/ground.md", "Write in `owner.language`.");
        put(
            "projects/p.toml",
            &format!("extends = [\"common\"]\nrepo = \"o/p\"\ncheckout = \"/tmp/p\"\nrules = [\"ground\"]\n[ci]\n{ci}"),
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
// project that does not run on demand, are read as before.
#[test]
fn a_ci_on_demand_has_its_fast_workflow_apart_from_the_full_ones() {
    let apart = Data::new("apart", "on_demand = true\nfast = \"fast\"\n");
    let c = config::working(apart.path()).unwrap();
    assert!(c.projects["p"].ci.on_demand);
    assert_eq!(c.projects["p"].ci.fast, "fast");
    // Off, one workflow for both stays as it was.
    let off = Data::new("off", "");
    let c = config::working(off.path()).unwrap();
    assert!(!c.projects["p"].ci.on_demand);
    assert_eq!(c.projects["p"].ci.fast, "ci");
    // The shared file names `ci` as both the fast workflow and the full one.
    let e = refused("same", "on_demand = true\n").expect("on demand with one workflow for both is refused");
    assert!(e.contains("projects/p.toml") && e.contains("on_demand"), "{e}");
}

// The input that takes the tests is a name: an empty one cannot be passed to the workflow, which the tool would run
// with an input of no name.
#[test]
fn a_ci_on_demand_names_its_input_to_the_workflow() {
    let e = refused("empty", "on_demand = true\nfast = \"fast\"\ntests_input = \"\"\n")
        .expect("an empty input name is refused");
    assert!(e.contains("projects/p.toml") && e.contains("tests_input"), "{e}");
}

// The checks the fast CI runs only when named, such as the lint, are run by hand through the input the configuration
// names: a list without the input, an input without a list or with no name, a name that is empty, and a list where the
// CI does not run on demand are each refused, all of them told at once; the list with its input on demand is read.
#[test]
fn the_checks_run_by_name_go_with_their_input_on_demand() {
    let on = "on_demand = true\nfast = \"fast\"\n";
    let data = Data::new(
        "named-checks",
        &format!("{on}named_checks = [\"lint\"]\nchecks_input = \"checks\"\n"),
    );
    let ci = config::working(data.path()).unwrap().projects["p"].ci.clone();
    assert_eq!(ci.named_checks, ["lint"]);
    assert_eq!(ci.checks_input.as_deref(), Some("checks"));
    // None by default, and left out of the settings written back.
    let data = Data::new("no-named-checks", on);
    let ci = config::working(data.path()).unwrap().projects["p"].ci.clone();
    assert!(ci.named_checks.is_empty() && ci.checks_input.is_none());
    let written = serde_json::to_value(&ci).unwrap();
    assert!(written.get("named_checks").is_none() && written.get("checks_input").is_none());
    for (name, ci, said) in [
        (
            "no-input",
            format!("{on}named_checks = [\"lint\"]\n"),
            "ci.named_checks needs ci.checks_input",
        ),
        (
            "no-list",
            format!("{on}checks_input = \"checks\"\n"),
            "lists no check to run by it",
        ),
        (
            "empty-input",
            format!("{on}named_checks = [\"lint\"]\nchecks_input = \" \"\n"),
            "ci.checks_input is empty",
        ),
        (
            "not-on-demand",
            "named_checks = [\"lint\", \"\"]\nchecks_input = \"checks\"\n".to_string(),
            "ci.named_checks needs ci.on_demand",
        ),
        (
            "empty-name",
            format!("{on}named_checks = [\"lint\", \"\"]\nchecks_input = \"checks\"\n"),
            "ci.named_checks holds an empty name",
        ),
    ] {
        let e = refused(name, &ci).unwrap_or_else(|| panic!("{name} is read"));
        assert!(e.contains("projects/p.toml") && e.contains(said), "{name}: {e}");
    }
    let e = refused("both", "named_checks = [\"lint\", \"\"]\nchecks_input = \"checks\"\n").unwrap();
    assert!(
        e.contains("needs ci.on_demand") && e.contains("holds an empty name"),
        "{e}"
    );
}

// The setting that says whether the CI runs on the engine's temporary refs is true unless a project says otherwise, is
// kept from the file a project extends, and goes with a tests_input and with a CI on demand, which do not read it; a
// project's settings written as JSON and read back keep it, whichever it is: false is written, and true, which is the
// default, is left out.
#[test]
fn the_switch_for_the_temporary_refs_is_true_by_default_and_kept() {
    let read_ci = |name: &str, ci: &str| {
        let data = Data::new(name, ci);
        config::working(data.path()).unwrap().projects["p"].ci.clone()
    };
    assert!(read_ci("default", "").runs_on_before_branches);
    // The shared file names a tests_input, which the setting is not read beside, and that is no mistake.
    assert!(!read_ci("false", "runs_on_before_branches = false\n").runs_on_before_branches);
    let both = read_ci(
        "both",
        "on_demand = true\nfast = \"fast\"\nruns_on_before_branches = false\n",
    );
    assert!(both.on_demand && !both.runs_on_before_branches);
    // Set in the file a project extends, and not said by the project, it holds.
    let shared = SHARED.replace("[limits]", "runs_on_before_branches = false\n[limits]");
    let data = Data::extending("shared", &shared, "");
    let inherited = config::working(data.path()).unwrap().projects["p"].ci.clone();
    assert!(!inherited.runs_on_before_branches);
    for ci in [inherited, read_ci("true", "runs_on_before_branches = true\n")] {
        let back: config::Ci = serde_json::from_value(serde_json::to_value(&ci).unwrap()).unwrap();
        assert_eq!(back.runs_on_before_branches, ci.runs_on_before_branches);
    }
}
