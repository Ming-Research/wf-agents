//! The repository's two CI workflows as the engine relies on them: `fast` runs only when the engine asks for it and has
//! no job a merge waits for, `ci` keeps that job, and neither cancels the other's runs. The engine reads the jobs of a
//! run by name, so a job renamed or added in the wrong workflow changes what a green run means; GitHub is the reference
//! for the workflow syntax, and the design of the on-demand CI for what each workflow must and must not hold.

use std::path::PathBuf;

fn workflow(file: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.github")
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The lines under a key at the top of a workflow file, up to the next top-level key: comments and blank lines left out.
fn block<'a>(text: &'a str, key: &str) -> Vec<&'a str> {
    let head = format!("{key}:");
    let mut out = Vec::new();
    for l in text.lines().skip_while(|l| !l.starts_with(&head)).skip(1) {
        let trimmed = l.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if !l.starts_with(' ') {
            break;
        }
        out.push(l);
    }
    out
}

/// The keys one level down in a block: those at its smallest indentation.
fn children(lines: &[&str]) -> Vec<String> {
    let indent = |l: &str| l.len() - l.trim_start().len();
    let Some(least) = lines.iter().map(|l| indent(l)).min() else {
        return Vec::new();
    };
    lines
        .iter()
        .filter(|l| indent(l) == least)
        .filter_map(|l| l.trim().split_once(':').map(|(k, _)| k.to_string()))
        .collect()
}

/// The lines of one job: from its key to the next job's.
fn job<'a>(jobs: &[&'a str], name: &str) -> Vec<&'a str> {
    let key = format!("  {name}:");
    let mut out = Vec::new();
    let mut inside = false;
    for l in jobs {
        let at_job = l.starts_with("  ") && !l.starts_with("   ");
        if at_job {
            inside = l.starts_with(&key);
        }
        if inside {
            out.push(*l);
        }
    }
    out
}

fn group(text: &str) -> String {
    block(text, "concurrency")
        .iter()
        .find_map(|l| l.trim().strip_prefix("group:"))
        .expect("a concurrency group")
        .trim()
        .to_string()
}

// The fast workflow is run by hand and by nothing else (a push of a work's branch must start nothing), takes the tests
// as the input the configuration names, and holds the lint, the tests and the summary, never a job named `check`: a
// head that only this workflow has run must not look like one the full CI passed.
#[test]
fn the_fast_workflow_runs_only_by_hand_and_has_no_job_a_merge_waits_for() {
    let text = workflow("workflows/fast.yml");
    assert_eq!(children(&block(&text, "on")), ["workflow_dispatch"]);
    let on = block(&text, "on").join("\n");
    assert!(
        on.contains("inputs:") && on.contains("      tests:"),
        "no input `tests`:\n{on}"
    );
    let jobs = block(&text, "jobs");
    let names = children(&jobs);
    assert_eq!(names, ["lint", "test", "fast"], "the jobs of fast.yml");
    // The summary is what the engine reads as the run's verdict, and it waits for the other two whatever they came to.
    let summary = job(&jobs, "fast").join("\n");
    assert!(summary.contains("needs: [lint, test]"), "{summary}");
    assert!(summary.contains("if: always()"), "{summary}");
}

// The full workflow keeps the job a merge waits for, and runs on a push to main and by hand only: a push of a work's
// branch, or of the engine's temporary branches of the code before a change, which the fast workflow runs by hand,
// starts nothing.
#[test]
fn the_full_workflow_keeps_the_check_a_merge_waits_for() {
    let text = workflow("workflows/ci.yml");
    let jobs = block(&text, "jobs");
    let names = children(&jobs);
    for want in ["lint", "test", "check"] {
        assert!(names.iter().any(|n| n == want), "no job {want} in {names:?}");
    }
    let check = job(&jobs, "check").join("\n");
    assert!(check.contains("needs: [lint, test]"), "{check}");
    assert!(check.contains("if: always()"), "{check}");
    let on = block(&text, "on");
    assert_eq!(children(&on), ["push", "workflow_dispatch"]);
    let on = on.join("\n");
    assert!(on.contains("    branches:\n      - main"), "{on}");
}

// A run of one workflow on a branch must not cancel a run of the other on it: the engine waits for a run of each, and
// a cancelled run reads as no pass.
#[test]
fn the_two_workflows_do_not_share_a_concurrency_group() {
    let (fast, full) = (
        group(&workflow("workflows/fast.yml")),
        group(&workflow("workflows/ci.yml")),
    );
    assert!(fast.starts_with("fast-"), "{fast}");
    assert!(full.starts_with("ci-"), "{full}");
    assert_ne!(fast, full);
}

// The lint is skipped on the engine's temporary branches of the code before a change, named by the prefix the engine
// gives them, and it is the same lint in both workflows: one action holds its steps, so that they cannot differ.
#[test]
fn the_lint_is_one_action_and_the_fast_workflow_skips_it_on_a_temporary_branch() {
    let fast = workflow("workflows/fast.yml");
    let lint = job(&block(&fast, "jobs"), "lint").join("\n");
    assert!(
        lint.contains("startsWith(github.ref_name, 'wf-agents-before/')"),
        "{lint}"
    );
    assert!(lint.contains("uses: ./.github/actions/lint"), "{lint}");
    let full = workflow("workflows/ci.yml");
    let lint = job(&block(&full, "jobs"), "lint").join("\n");
    assert!(lint.contains("uses: ./.github/actions/lint"), "{lint}");
    for (file, text) in [("fast.yml", &fast), ("ci.yml", &full)] {
        assert!(!text.contains("cargo fmt"), "{file} writes the lint's steps itself");
        assert!(!text.contains("cargo clippy"), "{file} writes the lint's steps itself");
    }
    let action = workflow("actions/lint/action.yml");
    assert!(action.contains("using: composite"));
    assert!(action.contains("cargo fmt --all --check"));
    assert!(action.contains("cargo clippy --workspace --all-targets --locked -- -D warnings"));
}
