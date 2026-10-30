//! What the engine reads of the fast CI's workflow, `.github/workflows/ci.yml`, besides the lines of the logs: the steps
//! `Build` and `Test` of the job `test`, by name and in that order, and the three shards that each run their third of the
//! tests to the end. Read as lines, since no test here needs more than the workflow's indentation.

const WORKFLOW: &str = include_str!("../../../.github/workflows/ci.yml");

/// The lines of one job of the workflow: from its header at two spaces to the next job's header or the end.
fn job(name: &str) -> Vec<&'static str> {
    let lines: Vec<&str> = WORKFLOW.lines().collect();
    let header = format!("  {name}:");
    let start = lines
        .iter()
        .position(|l| *l == header)
        .unwrap_or_else(|| panic!("no job {name} in ci.yml"));
    let is_header =
        |l: &&str| l.starts_with("  ") && !l.starts_with("   ") && l.ends_with(':') && !l.trim().starts_with('#');
    let end = lines[start + 1..]
        .iter()
        .position(is_header)
        .map_or(lines.len(), |i| start + 1 + i);
    lines[start..end].to_vec()
}

/// The steps of a job: each with its name (empty for a step that has none) and all its lines but comments.
fn steps(job: &[&'static str]) -> Vec<(String, Vec<&'static str>)> {
    let mut out: Vec<(String, Vec<&'static str>)> = Vec::new();
    let mut in_steps = false;
    for l in job {
        if *l == "    steps:" {
            in_steps = true;
            continue;
        }
        if !in_steps || l.trim().starts_with('#') {
            continue;
        }
        if l.starts_with("      - ") {
            out.push((String::new(), Vec::new()));
        }
        if let Some((name, lines)) = out.last_mut() {
            let t = l.trim().trim_start_matches("- ");
            if let Some(n) = t.strip_prefix("name: ") {
                *name = n.to_string();
            }
            lines.push(l);
        }
    }
    out
}

// The engine tells a build that failed from tests that were not run by the conclusions of the steps `Build` and `Test`
// of the test job: so both are steps of that job and of no other, `Build` comes before `Test` (a build that fails inside
// `Test` would be read as a name no test has), and neither is conditional or lets a failure pass, which would make a
// job green while its tests did not run.
#[test]
fn the_step_build_comes_before_the_step_test_in_the_test_job_alone_and_neither_hides_a_failure() {
    let named = |job: &[&'static str], n: &str| -> Vec<usize> {
        steps(job)
            .iter()
            .enumerate()
            .filter(|(_, (name, _))| name == n)
            .map(|(i, _)| i)
            .collect()
    };
    let test = job("test");
    let (build, run) = (named(&test, "Build"), named(&test, "Test"));
    assert_eq!((build.len(), run.len()), (1, 1), "Build {build:?}, Test {run:?}");
    assert!(build[0] < run[0], "Build is step {}, Test step {}", build[0], run[0]);
    for step in [build[0], run[0]] {
        let (name, lines) = &steps(&test)[step];
        let text = lines.join("\n");
        assert!(
            !text.contains("continue-on-error") && !text.contains("\n        if:"),
            "the step {name} is conditional or lets a failure pass:\n{text}"
        );
    }
    // A step of another job with these names would make the engine blame the build for tests it never ran.
    for other in ["lint", "check"] {
        let j = job(other);
        assert!(
            named(&j, "Build").is_empty() && named(&j, "Test").is_empty(),
            "the job {other} has a step named Build or Test"
        );
    }
}

// Every test runs to its end only while no shard cancels another, the shards together run every test, and the check
// the projects wait for is red whenever a shard is: the three things the results of ci and ci_before rest on.
#[test]
fn the_three_shards_run_apart_to_the_end_and_together_run_every_test() {
    let test = job("test");
    let text = test.join("\n");
    assert!(
        text.contains("      fail-fast: false"),
        "a red shard cancels the others:\n{text}"
    );
    assert!(
        text.contains("        shard: [1, 2, 3]"),
        "the shards are not 1, 2 and 3:\n{text}"
    );
    let steps = steps(&test);
    let (_, run) = steps.iter().find(|(n, _)| n == "Test").expect("no step Test");
    assert!(
        run.iter()
            .any(|l| l.trim().starts_with("run: ") && l.contains("--partition count:${{ matrix.shard }}/3")),
        "the step Test does not run the shard's third:\n{}",
        run.join("\n")
    );
    let check = job("check").join("\n");
    assert!(
        check.contains("needs: [lint, test]") && check.contains("if: always()"),
        "{check}"
    );
}
