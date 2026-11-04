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
// and the checks to run as the inputs the configuration names, and holds the lint, the tests and the summary, never a
// job named `check`: a head that only this workflow has run must not look like one the full CI passed.
#[test]
fn the_fast_workflow_runs_only_by_hand_and_has_no_job_a_merge_waits_for() {
    let text = workflow("workflows/fast.yml");
    assert_eq!(children(&block(&text, "on")), ["workflow_dispatch"]);
    let on = block(&text, "on").join("\n");
    for input in ["tests", "checks"] {
        assert!(
            on.contains("inputs:") && on.contains(&format!("      {input}:")),
            "no input `{input}`:\n{on}"
        );
    }
    // A run asked for with no check names none: the default is an empty list.
    assert!(on.contains("default: '[]'"), "{on}");
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

/// The steps of a workflow or action file, each as its lines trimmed, comments and blank lines left out: a step starts
/// at a line that starts with `- `.
fn steps(text: &str) -> Vec<Vec<&str>> {
    let mut out: Vec<Vec<&str>> = Vec::new();
    for l in text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        if l.starts_with("- ") {
            out.push(Vec::new());
        }
        if let Some(step) = out.last_mut() {
            step.push(l);
        }
    }
    out
}

/// The one step of `text` that has a line holding `needle`.
fn step_with<'a>(text: &'a str, needle: &str) -> Vec<&'a str> {
    let found: Vec<_> = steps(text)
        .into_iter()
        .filter(|s| s.iter().any(|l| l.contains(needle)))
        .collect();
    assert_eq!(found.len(), 1, "{} steps hold `{needle}`: {found:?}", found.len());
    found.into_iter().next().unwrap_or_default()
}

// The page the crate `web` is built with is got in one place, by an action the two workflows and the lint use, so that
// they cannot differ in the key it is kept under. It is restored when a build of the same `client/` is kept and built
// only when none is; the dependencies are installed only when it is built or the job runs the page's own commands; and
// only a push to main keeps a build, by one job: a run on a branch that kept one would add to its time for a build
// nobody else reads.
#[test]
fn the_page_is_got_by_one_action_and_only_a_push_to_main_keeps_it() {
    let action = workflow("actions/page/action.yml");
    assert!(action.contains("using: composite"));
    let key = step_with(&action, "git rev-parse HEAD:client").join("\n");
    assert!(key.contains("hashFiles('.github/actions/page/action.yml')"), "{key}");
    let miss = "steps.kept.outputs.cache-hit != 'true'";
    let build = step_with(&action, "npm run build").join("\n");
    assert!(build.contains(&format!("if: {miss}")), "{build}");
    let install = step_with(&action, "npm ci").join("\n");
    assert!(
        install.contains(&format!("if: inputs.modules == 'true' || {miss}")),
        "{install}"
    );
    let node = step_with(&action, "actions/setup-node").join("\n");
    assert!(
        node.contains(&format!("if: inputs.modules == 'true' || {miss}")),
        "{node}"
    );
    let touch = step_with(&action, "touch").join("\n");
    assert!(touch.contains("if: steps.kept.outputs.cache-hit == 'true'"), "{touch}");
    assert!(touch.contains("find client/dist -exec touch {} +"), "{touch}");
    let save = step_with(&action, "actions/cache/save").join("\n");
    assert!(save.contains(&format!("if: inputs.save == 'true' && {miss}")), "{save}");
    assert!(save.contains("path: client/dist"), "{save}");
    let restore = step_with(&action, "actions/cache/restore").join("\n");
    assert!(restore.contains("path: client/dist"), "{restore}");
    assert!(restore.contains("key: ${{ steps.key.outputs.key }}"), "{restore}");
    assert!(
        !restore.contains("restore-keys"),
        "only the build of this very content is any use: {restore}"
    );

    let (fast, full, lint) = (
        workflow("workflows/fast.yml"),
        workflow("workflows/ci.yml"),
        workflow("actions/lint/action.yml"),
    );
    for (file, text) in [("fast.yml", &fast), ("ci.yml", &full), ("the lint action", &lint)] {
        for own in ["npm ci", "npm run build", "actions/setup-node"] {
            let written: Vec<_> = steps(text)
                .into_iter()
                .filter(|s| s.iter().any(|l| l.contains(own)))
                .collect();
            assert!(
                written.is_empty(),
                "{file} writes `{own}` itself, not through the action: {written:?}"
            );
        }
    }
    // The test job of the fast workflow installs the dependencies also when the page's tests are to run, and keeps
    // nothing; the full workflow's keeps the build from main's push, in the first of its three test jobs only; the lint
    // always needs the dependencies and keeps nothing.
    let page = step_with(&fast, "uses: ./.github/actions/page").join("\n");
    assert!(page.contains("modules: ${{ steps.names.outputs.page }}"), "{page}");
    assert!(page.contains("save: false"), "{page}");
    let page = step_with(&full, "uses: ./.github/actions/page").join("\n");
    let want = "save: ${{ matrix.shard == 1 && github.ref == 'refs/heads/main' && github.event_name == 'push' }}";
    assert!(page.contains(want), "{page}");
    assert!(!page.contains("modules"), "{page}");
    let page = step_with(&lint, "uses: ./.github/actions/page").join("\n");
    assert!(page.contains("modules: true"), "{page}");
    assert!(!page.contains("save"), "{page}");
}

// The fast workflow runs the lint only when it is named, by its job's name in the input of the checks, as the engine
// gives them (`ci.named_checks`); and it is the same lint in both workflows: one action holds its steps, so that they
// cannot differ.
#[test]
fn the_lint_is_one_action_and_the_fast_workflow_runs_it_only_when_named() {
    let fast = workflow("workflows/fast.yml");
    let lint = job(&block(&fast, "jobs"), "lint").join("\n");
    assert!(
        lint.contains("if: ${{ contains(fromJSON(inputs.checks || '[]'), 'lint') }}"),
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
