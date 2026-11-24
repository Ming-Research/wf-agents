//! The repository's CI workflows as the engine relies on them. The engine runs the workflows the project declares
//! (`ci` in wf-agents.toml) by hand on the commit it gives, finds its own run by the value it gives (`call`) in the
//! run's name, and judges a run by the conclusions of its jobs and by the JUnit of its tests, or of its timings, in the
//! artifact `test-results`; so a job renamed, or added to the wrong workflow, or a trigger changed, or JUnit left out,
//! changes what a green run means. GitHub is the reference for the workflow syntax; the comments at the head of each
//! workflow say what it must and must not hold.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn workflow(file: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.github/workflows")
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
    (lines.iter())
        .filter(|l| indent(l) == least)
        .filter_map(|l| l.trim().split_once(':').map(|(k, _)| k.to_string()))
        .collect()
}

/// The lines of one key of a block, its own included: from its line to the next key at its indentation.
fn under<'a>(lines: &[&'a str], key: &str) -> Vec<&'a str> {
    let indent = |l: &str| l.len() - l.trim_start().len();
    let Some(at) = lines.iter().position(|l| l.trim().strip_suffix(':') == Some(key)) else {
        return Vec::new();
    };
    let depth = indent(lines[at]);
    let rest = lines[at + 1..].iter().take_while(|l| indent(l) > depth);
    std::iter::once(lines[at]).chain(rest.copied()).collect()
}

/// Each script a workflow runs, with the line its `run:` stands on: what follows `run:` on that line, or the lines under
/// `run: |` without their indentation, as GitHub gives them to the shell.
fn scripts(text: &str) -> Vec<(usize, String)> {
    let indent = |l: &str| l.len() - l.trim_start().len();
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        let Some(rest) = line.trim().trim_start_matches("- ").strip_prefix("run:") else {
            continue;
        };
        // A block that keeps its lines is `|`, with `-` or `+` for the line breaks at its end. A folded one, `>`, joins
        // its lines into one, which this does not do: no script is written so, and one that is fails here.
        let block = rest.trim();
        assert!(!block.starts_with('>'), "line {}: a folded script is not read", at + 1);
        if !matches!(block, "|" | "|-" | "|+") {
            out.push((at + 1, block.to_string()));
            continue;
        }
        let body: Vec<&str> = (lines[at + 1..].iter().copied())
            .take_while(|l| l.trim().is_empty() || indent(l) > indent(line))
            .collect();
        let written = body.iter().filter(|l| !l.trim().is_empty());
        let least = written.map(|l| indent(l)).min().unwrap_or(0);
        let script: Vec<&str> = body.iter().map(|l| l.get(least..).unwrap_or("")).collect();
        out.push((at + 1, script.join("\n")));
    }
    out
}

// The full workflow keeps a job `check` that is red whenever `scope`, `tree` or a job of the code did not pass, cancelled
// ones included, and that makes the JUnit of those jobs one artifact. The four jobs of the code need `scope` and do not
// run where it says only the design tree changed, and none of them runs for a fork's pull request: `scope` and `tree`
// run on GitHub's machines for a fork too, so the guard is on each job of the code.
#[test]
fn the_full_workflow_keeps_the_checks_a_merge_waits_for() {
    let text = workflow("ci.yml");
    let jobs = block(&text, "jobs");
    let names = children(&jobs);
    for want in ["scope", "tree", "page", "page-tests", "lint", "test", "check"] {
        assert!(names.iter().any(|n| n == want), "no job {want} in {names:?}");
    }
    let check = under(&jobs, "check").join("\n");
    assert!(
        check.contains("needs: [scope, tree, page, page-tests, lint, test]"),
        "{check}"
    );
    assert!(check.contains("if: ${{ always() &&"), "{check}");
    assert!(check.contains(r#"[ "$r" = success ] || red=true"#), "{check}");
    for job in ["scope", "tree", "page", "check"] {
        assert!(
            !under(&jobs, job).iter().any(|l| l.starts_with("    name:")),
            "the job {job} is given another name, which the engine would not find"
        );
    }
    let tests = under(&jobs, "page-tests");
    assert!(tests.contains(&"    name: page tests"), "{tests:?}");
    for job in ["page", "page-tests", "lint", "test"] {
        let lines = under(&jobs, job);
        let condition = lines
            .iter()
            .find(|l| l.starts_with("    if:"))
            .copied()
            .unwrap_or_default();
        assert!(lines.contains(&"    needs: scope"), "{job} does not need scope");
        assert!(
            condition.contains("needs.scope.outputs.tree_only != 'true'"),
            "{job} runs where only the design tree changed: {condition}"
        );
        assert!(
            condition.contains("github.event.pull_request.head.repo.full_name == github.repository"),
            "{job} runs for a fork's pull request: {condition}"
        );
    }
    for job in ["scope", "tree"] {
        let lines = under(&jobs, job);
        assert!(
            lines.iter().any(|l| l.trim() == "runs-on: ubuntu-24.04"),
            "{job} is not on GitHub's machines"
        );
    }
}

// `scope` says only the design tree changed where GitHub lists at least one file of the change, fewer than the 300 it
// stops at, and every file and every old path of a renamed file is under `design/`; where the answer is missing, not what
// it should be, or lists a file outside, it says not, and every job runs. The script of the step that says it is run
// under bash on made answers: a wrong "only the tree" skips the whole suite, and no other test reaches it. About twenty
// milliseconds a case, which is the start of python.
#[test]
fn scope_says_only_the_design_tree_changed_only_where_every_file_listed_is_in_it() {
    let text = workflow("ci.yml");
    let found: Vec<String> = (scripts(&text).into_iter())
        .map(|(_, script)| script)
        .filter(|script| script.contains("tree_only="))
        .collect();
    assert_eq!(found.len(), 1, "scripts of ci.yml that say where only the tree changed");
    let files = |names: &[&str]| {
        let files: Vec<String> = names.iter().map(|n| format!(r#"{{"filename":"{n}"}}"#)).collect();
        format!(r#"{{"files":[{}]}}"#, files.join(","))
    };
    let many = |count: usize| {
        let names: Vec<String> = (0..count).map(|n| format!("design/a{n}.md")).collect();
        files(&names.iter().map(String::as_str).collect::<Vec<_>>())
    };
    let renamed = r#"{"files":[{"filename":"design/moved.rs","previous_filename":"crates/carry/src/moved.rs","status":"renamed"}]}"#;
    let renamed_within =
        r#"{"files":[{"filename":"design/b.md","previous_filename":"design/a.md","status":"renamed"}]}"#;
    let cases: [(&str, Option<String>, &str); 14] = [
        ("one file of the tree", Some(files(&["design/decisions/a.md"])), "true"),
        (
            "several, one the check's own",
            Some(files(&["design/README.md", "design/lint.py"])),
            "true",
        ),
        (
            "a file renamed within the tree",
            Some(renamed_within.to_string()),
            "true",
        ),
        ("299 files, all of the tree", Some(many(299)), "true"),
        (
            "a file of the code beside the tree's",
            Some(files(&["design/a.md", "crates/carry/src/lib.rs"])),
            "false",
        ),
        (
            "a file whose name only begins like the tree's",
            Some(files(&["design-notes/a.md"])),
            "false",
        ),
        (
            "a file renamed out of the code into the tree",
            Some(renamed.to_string()),
            "false",
        ),
        ("no file listed", Some(files(&[])), "false"),
        ("300 files, which GitHub may have cut", Some(many(300)), "false"),
        (
            "no files key, as GitHub answers a request it refuses",
            Some(r#"{"message":"Not Found"}"#.to_string()),
            "false",
        ),
        ("files that are no list", Some(r#"{"files":null}"#.to_string()), "false"),
        ("an entry with no name", Some(r#"{"files":[{}]}"#.to_string()), "false"),
        ("an answer that is no JSON", Some("not json".to_string()), "false"),
        ("no answer, the request having failed", None, "false"),
    ];
    for (case, answer, says) in cases {
        let dir = tempfile::tempdir().unwrap();
        if let Some(answer) = answer {
            std::fs::write(dir.path().join("compare.json"), answer).unwrap();
        }
        let out = dir.path().join("output");
        let env = [
            ("RUNNER_TEMP", dir.path().to_str().unwrap()),
            ("GITHUB_OUTPUT", out.to_str().unwrap()),
        ];
        run_script(&found[0], dir.path(), &env);
        assert_eq!(read(&out), format!("tree_only={says}\n"), "{case}");
    }
}

// Where `scope` says only the design tree changed, `check` is red unless `scope` and `tree` passed, and a job of the code
// that did not run is no fault; anywhere else a job of the code that was skipped, cancelled or failed makes it red. A
// few milliseconds each, under bash, the `gh` that reads the failed jobs' logs stood in for.
#[test]
fn check_passes_a_run_of_only_the_design_tree_and_is_red_for_a_job_not_run_anywhere_else() {
    let text = workflow("ci.yml");
    let found: Vec<String> = (scripts(&text).into_iter())
        .map(|(_, script)| script)
        .filter(|script| script.contains("red=false"))
        .collect();
    assert_eq!(
        found.len(),
        1,
        "scripts of ci.yml that sum up the jobs: {}",
        found.len()
    );
    let dir = tempfile::tempdir().unwrap();
    let gh = dir.path().join("gh.sh");
    std::fs::write(&gh, "gh() { return 1; }\n").unwrap();
    let run = |scope: &str, tree: &str, code: [&str; 4], tree_only: &str| {
        let [page, tests, lint, test] = code;
        let env = [
            ("SCOPE", scope),
            ("TREE", tree),
            ("PAGE", page),
            ("PAGE_TESTS", tests),
            ("LINT", lint),
            ("TEST", test),
            ("TREE_ONLY", tree_only),
            ("BASH_ENV", gh.to_str().unwrap()),
            ("GITHUB_REPOSITORY", "owner/repository"),
            ("GITHUB_RUN_ID", "1"),
            ("GITHUB_RUN_ATTEMPT", "1"),
            ("RUNNER_TEMP", dir.path().to_str().unwrap()),
        ];
        try_script(&found[0], dir.path(), &env).0
    };
    let ok = ["success"; 4];
    let skipped = ["skipped"; 4];
    assert!(run("success", "success", ok, "false"), "a run of the whole code passes");
    assert!(
        run("success", "success", skipped, "true"),
        "a run of only the tree passes"
    );
    assert!(
        !run("success", "success", skipped, "false"),
        "jobs not run where the code changed"
    );
    assert!(
        !run("success", "success", skipped, ""),
        "jobs not run where `scope` said nothing"
    );
    assert!(!run("success", "failure", skipped, "true"), "the tree check failed");
    assert!(
        !run("success", "skipped", skipped, "true"),
        "the tree check did not run"
    );
    assert!(!run("failure", "success", skipped, "true"), "`scope` failed");
    assert!(!run("skipped", "success", skipped, ""), "`scope` did not run");
    assert!(
        !run(
            "success",
            "success",
            ["success", "cancelled", "success", "success"],
            "true"
        ),
        "a job of the code cancelled, though only the tree changed"
    );
    assert!(
        !run(
            "success",
            "success",
            ["success", "success", "failure", "success"],
            "false"
        ),
        "a job of the code failed"
    );
}

// The jobs `check` prints the logs of are chosen by the filter it gives `gh --jq`: reference is jq's own semantics, which
// the filter is run through here, on made lists of the run's jobs, with a `gh` that applies the filter it is given to the
// list and answers a log request with one line. Where only the design tree changed, a job of the code that was skipped
// is not listed, and one that failed or has not ended is; anywhere else a skipped one is listed too. A job of another
// name (`check` itself) is never listed. About fifteen milliseconds a case (bash and jq start); the stub of the test
// before this one answers nothing, so the filter is read by no other test.
#[test]
fn check_lists_the_jobs_that_did_not_pass_and_leaves_out_those_not_run_for_a_change_of_only_the_tree() {
    let text = workflow("ci.yml");
    let found: Vec<String> = (scripts(&text).into_iter())
        .map(|(_, script)| script)
        .filter(|script| script.contains("red=false"))
        .collect();
    assert_eq!(found.len(), 1, "scripts of ci.yml that sum up the jobs");
    let dir = tempfile::tempdir().unwrap();
    let jobs = r#"{"jobs":[
        {"id":1,"name":"scope","conclusion":"success"},
        {"id":2,"name":"tree","conclusion":"success"},
        {"id":3,"name":"page","conclusion":"skipped"},
        {"id":4,"name":"page tests","conclusion":"skipped"},
        {"id":5,"name":"lint","conclusion":"skipped"},
        {"id":6,"name":"test 1 of 3","conclusion":"skipped"},
        {"id":7,"name":"check","conclusion":null,"status":"in_progress"},
        {"id":8,"name":"test 2 of 3","conclusion":"failure"},
        {"id":9,"name":"test 3 of 3","conclusion":null,"status":"queued"}]}"#;
    std::fs::write(dir.path().join("jobs.json"), jobs).unwrap();
    let gh = dir.path().join("gh.sh");
    std::fs::write(
        &gh,
        r#"gh() {
  local filter="" a
  while [ $# -gt 0 ]; do
    if [ "$1" = --jq ]; then filter=$2; fi
    a=$1; shift
  done
  if [ -n "$filter" ]; then jq -r "$filter" "$RUNNER_TEMP/jobs.json"; else echo "log line"; fi
}
"#,
    )
    .unwrap();
    let run = |tree_only: &str, tree: &str, others: &str| {
        let env = [
            ("SCOPE", "success"),
            ("TREE", tree),
            ("PAGE", others),
            ("PAGE_TESTS", others),
            ("LINT", others),
            ("TEST", "failure"),
            ("TREE_ONLY", tree_only),
            ("BASH_ENV", gh.to_str().unwrap()),
            ("GITHUB_REPOSITORY", "owner/repository"),
            ("GITHUB_RUN_ID", "1"),
            ("GITHUB_RUN_ATTEMPT", "1"),
            ("RUNNER_TEMP", dir.path().to_str().unwrap()),
        ];
        let (ok, said) = try_script(&found[0], dir.path(), &env);
        assert!(!ok, "a run with a failed job ends red: {said}");
        said.lines()
            .find(|l| l.starts_with("Did not pass:"))
            .unwrap_or_else(|| panic!("no line of the jobs that did not pass: {said}"))
            .to_string()
    };
    assert_eq!(
        run("true", "success", "skipped"),
        "Did not pass: test 2 of 3, test 3 of 3.",
        "only the tree changed: the skipped jobs of the code are left out"
    );
    assert_eq!(
        run("false", "success", "skipped"),
        "Did not pass: page, page tests, lint, test 1 of 3, test 2 of 3, test 3 of 3.",
        "the code changed: a skipped job is one that did not pass"
    );
    assert_eq!(
        run("", "success", "skipped"),
        "Did not pass: page, page tests, lint, test 1 of 3, test 2 of 3, test 3 of 3.",
        "`scope` said nothing"
    );
}

// The full workflow runs when the engine asks for it by hand, and on each pull request into `main` that is neither a
// work's nor a fork's, and on no push: the engine reads only the runs it asked for, which the pull request of a work's
// branch would only repeat; a fork's code must not run on the owner's machine; and nothing reads a push to `main`.
#[test]
fn the_full_workflow_runs_by_hand_and_on_the_pull_requests_of_neither_a_work_nor_a_fork() {
    let text = workflow("ci.yml");
    let on = block(&text, "on");
    assert_eq!(children(&on), ["workflow_dispatch", "pull_request"]);
    let branches: Vec<&str> = (under(&on, "pull_request").iter())
        .filter_map(|l| l.trim().strip_prefix("- "))
        .collect();
    assert_eq!(branches, ["main"], "the branches of pull_request");
    assert!(
        !text.contains("refs/heads/next") && !text.lines().any(|l| l.trim() == "- next"),
        "the branch `next` is no part of the workflow"
    );
    let jobs = block(&text, "jobs");
    // The prefix of a work's branch is the engine's: ci.yml must name the one the engine gives its branches.
    let branch = carry::git::place(Path::new("/data"), "tag", store::WorkNumber(1)).branch;
    assert!(branch.starts_with(carry::git::BRANCH_PREFIX), "{branch}");
    let work = format!("!startsWith(github.head_ref, '{}')", carry::git::BRANCH_PREFIX);
    // What each job's own `if` says is checked whole, not by its parts: the same parts joined by other operators run a
    // fork's code on the owner's machine or red a work's pull request, and nothing before the merge evaluates them.
    // The jobs of the code also stay out of a run where only the design tree changed, as `scope` says.
    let guard = format!(
        "if: ${{{{ needs.scope.outputs.tree_only != 'true' && (github.event_name != 'pull_request' || (github.event.pull_request.head.repo.full_name == github.repository && {work})) }}}}"
    );
    // `scope` and `tree` build nothing and run on GitHub's machines, so a fork's pull request gets them; a work's of this
    // repository does not, as with `check`.
    let light = format!(
        "if: ${{{{ github.event_name != 'pull_request' || github.event.pull_request.head.repo.full_name != github.repository || {work} }}}}"
    );
    // `check` is skipped with the rest on a work's pull request of this repository, so that the run is not red there; on a
    // fork's, whatever its branch is called, it stays, red, since its jobs are skipped.
    let check = format!(
        "if: ${{{{ always() && (github.event_name != 'pull_request' || github.event.pull_request.head.repo.full_name != github.repository || {work}) }}}}"
    );
    for (job, want) in [
        ("scope", &light),
        ("tree", &light),
        ("page", &guard),
        ("page-tests", &guard),
        ("lint", &guard),
        ("test", &guard),
        ("check", &check),
    ] {
        // The job's own condition stands at the job's indentation; those of its steps are deeper.
        let own: Vec<&str> = (under(&jobs, job).into_iter())
            .filter(|l| l.starts_with("    if:"))
            .map(str::trim)
            .collect();
        assert_eq!(own, [want.as_str()], "the condition of the job {job}");
    }
}

// A workflow the engine runs by hand takes the commit to test and the value that finds its run, and its name holds that
// value, so that the engine tells its run from every other of the workflow; each job tests that commit.
#[test]
fn each_workflow_the_engine_runs_takes_a_commit_and_a_call_and_names_its_run_by_the_call() {
    for file in ["ci.yml", "fast.yml"] {
        let text = workflow(file);
        let dispatch = under(&block(&text, "on"), "workflow_dispatch");
        let inputs = children(&under(&dispatch, "inputs")[1..]);
        for input in ["commit", "call"] {
            assert!(
                inputs.iter().any(|i| i == input),
                "{file}: no input {input} in {inputs:?}"
            );
        }
        let run_name = text.lines().find(|l| l.starts_with("run-name:")).unwrap_or_default();
        assert!(run_name.contains("inputs.call"), "{file}: {run_name:?}");
    }
    let text = workflow("ci.yml");
    let checkouts = text.matches("uses: actions/checkout@").count();
    assert!(checkouts >= 3, "{checkouts} checkouts");
    assert_eq!(
        text.matches("ref: ${{ inputs.commit }}").count(),
        checkouts,
        "each checkout of ci.yml is at the commit given"
    );
}

// A test's stop comes from the engine: the line it gives as the input `test_stop`, in whole seconds, is put in a copy of
// nextest's file, which the step runs nextest with; with none, nextest runs with the repository's file as it is, and
// a value that is no whole number of seconds fails the step before any test runs. The step of each workflow that runs
// nextest is run under bash with a `cargo` that says what it was given, and the copy it names is read as TOML, which
// also holds the repository's file to carry no `[profile.ci]` table of its own, as the copy would then name it twice.
// A few milliseconds under bash.
#[test]
fn the_stop_the_engine_gives_goes_into_a_copy_of_nextests_file_and_with_none_the_file_is_used_as_it_is() {
    for file in ["ci.yml", "fast.yml"] {
        let text = workflow(file);
        let on = block(&text, "on");
        let dispatch = under(&on, "workflow_dispatch");
        let inputs = children(&under(&dispatch, "inputs")[1..]);
        assert!(
            inputs.iter().any(|i| i == "test_stop"),
            "{file}: no input test_stop in {inputs:?}"
        );
    }
    let ci: Vec<String> = (scripts(&workflow("ci.yml")).into_iter())
        .map(|(_, script)| script)
        .filter(|script| script.contains("cargo nextest run"))
        .collect();
    assert_eq!(ci.len(), 1, "scripts of ci.yml that run the tests: {}", ci.len());
    for script in [ci[0].clone(), fast_script("filter=$(cat")] {
        let dir = tempfile::tempdir().unwrap();
        let temp = dir.path().join("temp");
        std::fs::create_dir_all(temp.join("names")).unwrap();
        std::fs::write(temp.join("names/filter"), "test(=a)").unwrap();
        std::fs::create_dir_all(dir.path().join(".config")).unwrap();
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.config/nextest.toml");
        std::fs::copy(repository, dir.path().join(".config/nextest.toml")).unwrap();
        let calls = dir.path().join("calls");
        let cargo = dir.path().join("cargo.sh");
        std::fs::write(&cargo, format!("cargo() {{ echo \"$*\" >> '{}'; }}\n", calls.display())).unwrap();
        let run = |stop: &str| {
            std::fs::write(&calls, "").unwrap();
            let env = [
                ("BASH_ENV", cargo.to_str().unwrap()),
                ("PARTITION", "1"),
                ("RUNNER_TEMP", temp.to_str().unwrap()),
                ("TEST_STOP", stop),
            ];
            let (ok, said) = try_script(&script, dir.path(), &env);
            (ok, said, read(&calls))
        };
        // The config file each call of nextest names.
        let named = |calls: &str| -> Vec<String> {
            calls
                .lines()
                .filter(|call| call.starts_with("nextest run"))
                .map(|call| {
                    let mut words = call.split_whitespace().skip_while(|w| *w != "--config-file");
                    words.nth(1).expect("the call names a config file").to_string()
                })
                .collect()
        };
        let (ok, said, calls) = run("");
        assert!(ok, "{said}");
        assert!(!named(&calls).is_empty(), "nextest was run: {calls}");
        assert!(
            named(&calls).iter().all(|config| config == ".config/nextest.toml"),
            "{calls}"
        );
        let (ok, said, calls) = run("12");
        assert!(ok, "{said}");
        assert!(!named(&calls).is_empty(), "nextest was run: {calls}");
        for config in named(&calls) {
            let config: toml::Table = read(&dir.path().join(&config)).parse().expect("the copy is TOML");
            let profile = &config["profile"]["ci"];
            assert_eq!(profile["slow-timeout"]["period"].as_str(), Some("12s"), "{config}");
            assert_eq!(
                profile["slow-timeout"]["terminate-after"].as_integer(),
                Some(1),
                "{config}"
            );
            assert_eq!(profile["junit"]["path"].as_str(), Some("junit.xml"), "{config}");
        }
        for wrong in ["twelve", "0", "1.5", "-3", "12 s"] {
            let (ok, said, calls) = run(wrong);
            assert!(
                !ok && said.contains("test_stop is not a whole number of seconds"),
                "{wrong}: {said}"
            );
            assert_eq!(calls, "", "{wrong}: no test runs");
        }
    }
}

// What a run uploads for the engine is JUnit, all of it in the one artifact `test-results`: the full workflow's jobs
// each upload theirs in a file named by the job, nextest's of each third of the Rust tests and vitest's of the page's
// tests, and the job `check` makes them one; the fast workflow uploads its own. The job `tree` always uploads a file, so
// that a run of only the design tree has results the engine reads.
#[test]
fn every_test_of_a_run_is_reported_as_junit_in_the_artifact_test_results() {
    let text = workflow("ci.yml");
    let jobs = block(&text, "jobs");
    let tests = under(&jobs, "page-tests").join("\n");
    assert!(
        tests.contains("--reporter=junit") && tests.contains("page tests.xml"),
        "{tests}"
    );
    assert!(tests.contains("name: junit-page-tests"), "{tests}");
    assert!(tests.contains("JOB: page tests"), "{tests}");
    let page = under(&jobs, "page").join("\n");
    assert!(
        !page.contains("--reporter=junit") && !page.contains("npm test") && !page.contains("playwright"),
        "the job page runs the page's tests: {page}"
    );
    assert!(
        page.contains("name: junit-page") && page.contains("JOB: page"),
        "{page}"
    );
    let tree = under(&jobs, "tree").join("\n");
    assert!(
        tree.contains("name: junit-tree") && tree.contains("JOB: tree"),
        "{tree}"
    );
    let test = under(&jobs, "test").join("\n");
    assert!(
        test.contains("name: test ${{ matrix.partition }} of 3")
            && test.contains("JOB: test ${{ matrix.partition }} of 3")
            && test.contains("path: ${{ runner.temp }}/test ${{ matrix.partition }} of 3.xml")
            && test.contains("name: junit-${{ matrix.partition }}"),
        "{test}"
    );
    let check = under(&jobs, "check").join("\n");
    assert!(check.contains("actions/upload-artifact/merge@"), "{check}");
    assert!(
        check.contains("name: test-results") && check.contains("pattern: junit-*"),
        "{check}"
    );
    let fast = workflow("fast.yml");
    assert!(fast.contains("name: test-results"), "fast.yml uploads no test-results");
    assert!(fast.contains("--reporter=junit"), "fast.yml reads no JUnit of the page");
}

/// Where a job of ci.yml names its results (`RESULTS`, in the step that names them) and the path its upload takes.
fn results_and_upload(text: &str, job: &str, artifact: &str) -> (String, String) {
    let jobs = block(text, "jobs");
    let lines = under(&jobs, job);
    let value = |key: &str, after: usize| {
        let line = lines[after..]
            .iter()
            .find(|l| l.trim().starts_with(key))
            .unwrap_or_else(|| panic!("{job}: no {key}"));
        line.trim()[key.len()..].trim().to_string()
    };
    let upload = lines
        .iter()
        .position(|l| l.trim() == format!("name: {artifact}"))
        .unwrap_or_else(|| panic!("{job}: no upload of {artifact}"));
    (value("RESULTS:", 0), value("path:", upload))
}

// The file a job's step `Name the results` writes (`RESULTS`) is the file the job's upload takes. The upload ignores a
// path that holds no file (`if-no-files-found: ignore`), so a job whose two places differ uploads nothing and is read as
// one that reported nothing; and a job whose results stand where a run keeps nothing from an earlier one is the one
// whose results are this run's: each job names theirs in the runner's temporary directory, which GitHub empties as each
// job begins (the variable `RUNNER_TEMP` of GitHub's documented default variables). The same reading of a text whose
// step names the results somewhere else finds the two places differ. Under a millisecond.
#[test]
fn each_job_uploads_the_file_it_names_its_results_in_and_keeps_it_in_the_runners_temporary_directory() {
    let text = workflow("ci.yml");
    let uploads = [
        ("tree", "junit-tree"),
        ("page", "junit-page"),
        ("page-tests", "junit-page-tests"),
        ("test", "junit-${{ matrix.partition }}"),
    ];
    for (job, artifact) in uploads {
        let (results, path) = results_and_upload(&text, job, artifact);
        assert_eq!(results, path, "{job}: the file named is not the file uploaded");
        assert!(
            results.starts_with("${{ runner.temp }}/"),
            "{job}: the results are named at {results}, not in the runner's temporary directory"
        );
    }
    let elsewhere = text
        .replace("RESULTS: ${{ runner.temp }}/test", "RESULTS: target/nextest/ci/test")
        .replace("RESULTS: ${{ runner.temp }}/tree.xml", "RESULTS: tree.xml")
        .replace("RESULTS: ${{ runner.temp }}/page tests.xml", "RESULTS: page tests.xml")
        .replace("RESULTS: ${{ runner.temp }}/page.xml", "RESULTS: page.xml");
    for (job, artifact) in uploads {
        let (results, path) = results_and_upload(&elsewhere, job, artifact);
        assert_ne!(
            results, path,
            "{job}: a step that names its results elsewhere is not seen"
        );
    }
}

// The fast workflow runs by hand and by nothing else: it runs on the owner's machine, and a trigger by a push or a pull
// request would run anyone's code there. It has no job named as a check a merge counts, so that a head only it has run
// is not one that may merge; and no concurrency group, since every run of it is a request an agent waits for, which
// another request must not cancel.
#[test]
fn the_fast_workflow_runs_only_by_hand_and_has_no_check_a_merge_waits_for() {
    let text = workflow("fast.yml");
    assert_eq!(children(&block(&text, "on")), ["workflow_dispatch"]);
    let names = children(&block(&text, "jobs"));
    for merge in ["page", "check"] {
        assert!(
            !names.iter().any(|n| n == merge),
            "fast.yml has a job {merge}: {names:?}"
        );
    }
    assert!(
        block(&text, "concurrency").is_empty(),
        "fast.yml has a concurrency group"
    );
}

// A test is named to the fast workflow by the name its results give it, and the workflow selects it by that name as it
// is: a Rust test by nextest's exact name filter, a page test by the exact name vitest lists. It speaks of no
// separator a project's tests might use, and of no setting or tool the engine no longer has.
#[test]
fn the_fast_workflow_selects_tests_by_their_names_as_they_are() {
    let text = workflow("fast.yml");
    assert!(
        text.contains("test(=$n)"),
        "the Rust tests are selected by an exact name"
    );
    for gone in [
        "github.fast",
        "check_before",
        "separators",
        "test_name.rs",
        "wf-agents-before",
        "git grep",
    ] {
        assert!(!text.contains(gone), "fast.yml still speaks of {gone}");
    }
}

/// The one script of a workflow that holds `marker`.
fn script_of(file: &str, marker: &str) -> String {
    let mut found: Vec<String> = (scripts(&workflow(file)).into_iter())
        .map(|(_, script)| script)
        .filter(|script| script.contains(marker))
        .collect();
    assert_eq!(
        found.len(),
        1,
        "scripts of {file} that hold {marker:?}: {}",
        found.len()
    );
    found.remove(0)
}

/// The one script of fast.yml that holds `marker`.
fn fast_script(marker: &str) -> String {
    script_of("fast.yml", marker)
}

/// Runs a script of a workflow under bash in `dir`, with the environment a step is given: whether it ended well, and
/// what it printed, its standard error after its standard output.
fn try_script(script: &str, dir: &Path, env: &[(&str, &str)]) -> (bool, String) {
    let ended = Command::new("bash")
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .envs(env.iter().copied())
        .output()
        .expect("bash starts");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&ended.stdout),
        String::from_utf8_lossy(&ended.stderr)
    );
    (ended.status.success(), said)
}

/// Runs a script of a workflow as `try_script` does, which must end well, and returns what it printed.
fn run_script(script: &str, dir: &Path, env: &[(&str, &str)]) -> String {
    let (ok, said) = try_script(script, dir, env);
    assert!(ok, "{said}");
    said
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

// Every name given is looked for among the page's tests, whatever its form: `works` is a Rust test's name by its form
// and a top-level `it('works')` of the page's, and a run that looked for it only among the Rust tests left the page's
// test out without a word. A name of the form of a Rust test's is selected among the Rust tests too, and a name of any
// other form only among the page's.
#[test]
fn the_names_given_are_all_looked_for_among_the_pages_tests() {
    let script = fast_script("Named: ");
    let dir = tempfile::tempdir().expect("a directory");
    let out = dir.path().join("output");
    let temp = dir.path().join("temp");
    let run = |tests: &str| {
        std::fs::write(&out, "").expect("the output file");
        let env = [
            ("TESTS", tests),
            ("RUNNER_TEMP", temp.to_str().expect("a path")),
            ("GITHUB_OUTPUT", out.to_str().expect("a path")),
        ];
        run_script(&script, dir.path(), &env);
    };
    let names = temp.join("names");

    run("works\nmod::a_row\nsome describe > its title\n");
    assert_eq!(
        read(&names.join("page")),
        "works\nmod::a_row\nsome describe > its title\n"
    );
    assert_eq!(read(&names.join("rust")), "works\nmod::a_row\n");
    assert_eq!(read(&names.join("filter")), "test(=mod::a_row) | test(=works)");
    assert_eq!(read(&out), "rust=true\npage_named=true\n");

    // A name of no Rust test's form goes to the page's tests alone, and the run says it names a test.
    run("some describe > its title\n");
    assert_eq!(read(&names.join("page")), "some describe > its title\n");
    assert_eq!(read(&names.join("rust")), "");
    assert_eq!(read(&out), "rust=false\npage_named=true\n");
    assert!(!temp.join("test-results/none.xml").exists());

    // A run that names nothing reports no test, and says so.
    run("");
    assert_eq!(read(&out), "rust=false\npage_named=false\n");
    assert_eq!(read(&temp.join("test-results/none.xml")), "<testsuites/>\n");
}

// The page's tests named are found in vitest's own list of them: first the Node project's, and only for a name it does
// not have, the browser project's, the one time Chromium is installed and started. A name a page test could have alone
// and none has is said; one of the form of a Rust test's is left to the results, since a Rust test may have it.
#[test]
fn the_pages_tests_are_found_by_the_names_vitest_lists_and_chromium_starts_only_for_a_name_not_found_before() {
    let script = fast_script("vitest list --project unit");
    let dir = tempfile::tempdir().expect("a directory");
    let client = dir.path().join("client");
    let temp = dir.path().join("temp");
    for d in [&client, &temp.join("names"), &temp.join("test-results")] {
        std::fs::create_dir_all(d).expect("a directory");
    }
    // What vitest lists: each test with the name its results give it and its file, from the root the list is run in.
    let listed = |tests: &[(&str, &str)]| {
        let rows: Vec<String> = (tests.iter())
            .map(|(name, file)| format!(r#"{{"name":"{name}","file":"@ROOT@/{file}"}}"#))
            .collect();
        format!("[{}]", rows.join(","))
    };
    std::fs::write(
        dir.path().join("unit.json"),
        listed(&[("works", "src/a.test.ts"), ("a describe > its title", "src/b.test.ts")]),
    )
    .expect("the list");
    std::fs::write(
        dir.path().join("browser.json"),
        listed(&[("drawn", "src/c.browser.test.tsx")]),
    )
    .expect("the list");
    // `npx` as vitest and playwright answer it, a function that bash reads before the script (`BASH_ENV`): the lists,
    // and each call kept. A list goes into the file `--json=<path>` names, or to the standard output where `--json`
    // names none, as vitest has it (`--json [true/path]`; the real vitest's file is held by client/src/listing.test.ts),
    // and a notice of vitest's own stands in the standard output first, as the probe of run 45002604590 saw it when
    // three lists were read at once (`Port 63315 is in use, trying another one...`). The project named in the file
    // `writes-no-list` gets no file.
    let stub = dir.path().join("stub.sh");
    std::fs::write(
        &stub,
        r#"npx() {
  echo "$*" >> "$STUB_DIR/calls"
  case "$*" in
    *"vitest list --project unit"*) project=unit ;;
    *"vitest list --project browser"*) project=browser ;;
    *) return 0 ;;
  esac
  echo "Port 63315 is in use, trying another one..."
  list=$(sed "s|@ROOT@|$PWD|g" "$STUB_DIR/$project.json")
  to=""
  for arg in "$@"; do
    case "$arg" in --json=*) to=${arg#--json=} ;; esac
  done
  if [ -z "$to" ]; then
    printf '%s\n' "$list"
  elif [ "$(cat "$STUB_DIR/writes-no-list" 2>/dev/null)" != "$project" ]; then
    printf '%s\n' "$list" > "$to"
  fi
}
"#,
    )
    .expect("the stub");
    let names = temp.join("names");
    let out = dir.path().join("output");
    let try_run = |page: &str, rust: &str, rust_ran: bool, no_list: &str| {
        for file in ["calls", "output"] {
            std::fs::write(dir.path().join(file), "").expect("a file");
        }
        // A run begins with none of the lists of an earlier one, as a job does in a temporary directory of its own.
        for list in ["unit.json", "browser.json"] {
            std::fs::remove_file(names.join(list)).ok();
        }
        std::fs::write(dir.path().join("writes-no-list"), no_list).expect("the project");
        std::fs::write(names.join("page"), page).expect("the names");
        std::fs::write(names.join("rust"), rust).expect("the names");
        let env = [
            ("BASH_ENV", stub.to_str().expect("a path")),
            ("STUB_DIR", dir.path().to_str().expect("a path")),
            ("RUNNER_TEMP", temp.to_str().expect("a path")),
            ("GITHUB_OUTPUT", out.to_str().expect("a path")),
            ("RUST", if rust_ran { "true" } else { "false" }),
        ];
        try_script(&script, &client, &env)
    };
    let run = |page: &str, rust: &str, rust_ran: bool| {
        let (ok, said) = try_run(page, rust, rust_ran, "");
        assert!(ok, "{said}");
        said
    };
    let calls = || read(&dir.path().join("calls"));

    // Every name is in the Node project: the browser is not started, and a plain name is found like any other.
    let said = run("works\na describe > its title\n", "works\n", true);
    assert_eq!(
        read(&names.join("found")),
        "works\tsrc/a.test.ts\na describe > its title\tsrc/b.test.ts\n"
    );
    assert_eq!(read(&names.join("files")), "src/a.test.ts\nsrc/b.test.ts\n");
    assert_eq!(read(&names.join("pattern")), "^(a describe its title|works)$\n");
    assert_eq!(read(&out), "page=true\nbrowser=false\n");
    assert_eq!(calls().lines().count(), 1, "{}", calls());
    assert!(!said.contains("No page test"), "{said}");

    // A name the Node project has not: the browser's list is read, after Chromium is installed. What neither has is
    // said where only a page test could have it, and is not said where a Rust test may.
    let said = run(
        "drawn\nmod::a_row\nno such > page\nworks\n",
        "drawn\nmod::a_row\nworks\n",
        true,
    );
    let made = calls();
    let (install, list) = (
        made.find("playwright install"),
        made.find("vitest list --project browser"),
    );
    assert!(install.is_some() && install < list, "{made}");
    assert_eq!(read(&names.join("pattern")), "^(drawn|works)$\n");
    assert_eq!(read(&names.join("files")), "src/a.test.ts\nsrc/c.browser.test.tsx\n");
    assert_eq!(read(&out), "page=true\nbrowser=true\n");
    assert!(said.contains("No page test is named: no such > page"), "{said}");
    assert!(
        !said.contains("named: mod::a_row") && !said.contains("named: drawn"),
        "{said}"
    );

    // Nothing found and no Rust test selected: no test is reported.
    let said = run("no such > page\n", "", false);
    assert_eq!(read(&out), "page=false\nbrowser=false\n");
    assert!(said.contains("No page test is named: no such > page"), "{said}");
    assert_eq!(read(&temp.join("test-results/none.xml")), "<testsuites/>\n");

    // The notice vitest's own prints stands in the log and in no list, and a list vitest wrote no file for fails the
    // step with a sentence that names the project, the Node project's before the browser's.
    let said = run("works\n", "", false);
    assert!(
        said.contains("Port 63315 is in use"),
        "the log keeps what vitest printed: {said}"
    );
    let (ok, said) = try_run("drawn\n", "", false, "browser");
    assert!(
        !ok && said.contains("vitest wrote no list of the browser project's tests"),
        "{said}"
    );
    let (ok, said) = try_run("works\n", "", false, "unit");
    assert!(
        !ok && said.contains("vitest wrote no list of the unit project's tests"),
        "{said}"
    );
}

// What `check` prints of a job that did not pass is that job's own log; the engine reads each failed job's log
// itself. The log of a job holds the escapes that colour cargo's output, and gh gives no answer that holds any unless
// told to: the first time a job was red, `check` printed gh's refusal in place of each log, and the agent would have
// been told that the lint and a job of the tests failed and nothing of which test or which line.
#[test]
fn check_reads_the_logs_of_the_other_jobs_with_the_escapes_that_colour_them() {
    let text = workflow("ci.yml");
    let check = under(&block(&text, "jobs"), "check").join("\n");
    let reads: Vec<&str> = (check.lines())
        .filter(|l| l.contains("gh api") && l.contains("/logs"))
        .collect();
    assert_eq!(reads.len(), 1, "{reads:?}");
    assert!(reads[0].contains("gh api --allow-escape-sequences "), "{reads:?}");
}

/// The jobs of ci.yml that name their results, by what each does with them.
#[derive(Clone, Copy, Debug)]
enum Names {
    /// A third of the Rust tests: nextest's results are moved to their name.
    Moves,
    /// The page's tests: vitest writes them there.
    Writes,
    /// The page's other checks: results only where a step failed.
    Fails,
    /// The design tree's checks: a suite of the job's name, with an error where a step failed.
    Tree,
}

/// The script of ci.yml that names the results of a job of this kind and says a step of it that failed.
fn naming_script(kind: Names) -> String {
    let found: Vec<String> = (scripts(&workflow("ci.yml")).into_iter())
        .map(|(_, script)| script)
        .filter(|script| script.contains("of the job failed"))
        .filter(|script| {
            let (moves, tree, tested) = (
                script.contains(r#"mv "$junit" "$RESULTS""#),
                script.contains(r#"<testsuite name="%s"/>"#),
                script.contains("TESTED"),
            );
            match kind {
                Names::Moves => moves,
                Names::Tree => tree,
                Names::Writes => tested && !moves,
                Names::Fails => !moves && !tree && !tested,
            }
        })
        .collect();
    assert_eq!(found.len(), 1, "scripts of ci.yml that name a job's results: {kind:?}");
    found.into_iter().next().expect("one")
}

// A job of tests writes its results in a file named by it, and says in it a step of its own that failed as a
// testsuite of the job's name with an error naming the step, as a unit not built is: where its tests failed and wrote
// their results, nothing is added; where a step before or beside them failed, it is named, whether the tests wrote
// their results or not; where the tests' own step failed and wrote nothing, it is named. So the check before a merge
// tells a job red for something else than its tests from one whose tests failed. Run under bash: a few milliseconds.
#[test]
fn a_job_of_tests_names_its_results_by_itself_and_says_a_step_of_its_own_that_failed() {
    let (test, page) = (naming_script(Names::Moves), naming_script(Names::Writes));
    let junit =
        r#"<testsuites><testsuite name="carry"><testcase name="t"><failure/></testcase></testsuite></testsuites>"#;
    let steps = |failed: Option<&str>| {
        ["Check out the commit", "Check the version of nextest", "Build the page"]
            .map(|step| format!("{} {step}", if failed == Some(step) { "failure" } else { "success" }))
            .join("\n")
    };
    let job = "test 2 of 3";
    for (failed, tested, written, says) in [
        (None, "failure", true, None),
        (
            Some("Check the version of nextest"),
            "skipped",
            false,
            Some("Check the version of nextest"),
        ),
        (Some("Build the page"), "skipped", false, Some("Build the page")),
        (None, "failure", false, Some("Test")),
    ] {
        let dir = tempfile::tempdir().unwrap();
        if written {
            std::fs::create_dir_all(dir.path().join("target/nextest/ci")).unwrap();
            std::fs::write(dir.path().join("target/nextest/ci/junit.xml"), junit).unwrap();
        }
        let results = dir.path().join("temp").join(format!("{job}.xml"));
        // The runner's temporary directory is there when a job begins.
        std::fs::create_dir_all(results.parent().unwrap()).unwrap();
        let env = [
            ("JOB", job),
            ("RESULTS", results.to_str().unwrap()),
            ("TESTED", &format!("{tested} Test")),
            ("STEPS", &steps(failed)),
        ];
        run_script(&test, dir.path(), &env);
        let named = read(&results);
        assert_eq!(named.contains("<failure/>"), written, "{named}");
        let error = says.map(|step| {
            format!(r#"<testsuite name="{job}"><error message="the step {step} of the job failed"/></testsuite>"#)
        });
        assert_eq!(named.contains("<error"), error.is_some(), "{named}");
        if let Some(error) = error {
            assert!(named.contains(&error), "{named}");
        }
    }
    // The results of an earlier run, which `target/` keeps between runs, are never taken for this run's: where a step
    // before the tests failed, and the tests did not run, the results hold the error naming that step, and nothing of
    // what was left (a `junit.xml` a cancelled run did not move, say).
    let stale =
        r#"<testsuites><testsuite name="carry"><testcase name="left_by_an_earlier_run"/></testsuite></testsuites>"#;
    let left = |dir: &Path| {
        std::fs::create_dir_all(dir.join("target/nextest/ci")).unwrap();
        std::fs::write(dir.join("target/nextest/ci/junit.xml"), stale).unwrap();
    };
    let said_by = |dir: &Path, tested: &str, failed: Option<&str>| {
        let results = dir.join("temp").join(format!("{job}.xml"));
        std::fs::create_dir_all(results.parent().unwrap()).unwrap();
        let env = [
            ("JOB", job),
            ("RESULTS", results.to_str().unwrap()),
            ("TESTED", tested),
            ("STEPS", &steps(failed)),
        ];
        run_script(&test, dir, &env);
        read(&results)
    };
    let error = |step: &str| {
        format!(r#"<testsuite name="{job}"><error message="the step {step} of the job failed"/></testsuite>"#)
    };
    // A step before the tests failed.
    let dir = tempfile::tempdir().unwrap();
    left(dir.path());
    let named = said_by(dir.path(), "skipped Test", Some("Build the page"));
    assert!(
        named.contains(&error("Build the page")) && !named.contains("left_by_an_earlier_run"),
        "{named}"
    );
    // The page's tests: the tests failed and vitest wrote their results, which is all the file holds; the tests step
    // failed and wrote nothing, or a step before it failed, which is said in a file of the job's own.
    let steps = |failed: Option<&str>| {
        [
            "Check out the commit",
            "Install the page's dependencies",
            "Chromium for the page's browser tests",
        ]
        .map(|step| format!("{} {step}", if failed == Some(step) { "failure" } else { "success" }))
        .join("\n")
    };
    let vitest = r#"<testsuites><testsuite name="src/a.test.ts"><testcase name="a"><failure/></testcase></testsuite></testsuites>"#;
    for (tested, written, failed, says) in [
        ("failure Test the page", true, None, None),
        ("failure Test the page", false, None, Some("Test the page")),
        (
            "skipped Test the page",
            false,
            Some("Chromium for the page's browser tests"),
            Some("Chromium for the page's browser tests"),
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("page tests.xml");
        if written {
            std::fs::write(&file, vitest).unwrap();
        }
        let results = file.display().to_string();
        let env = [
            ("JOB", "page tests"),
            ("RESULTS", results.as_str()),
            ("TESTED", tested),
            ("STEPS", &steps(failed)),
        ];
        run_script(&page, dir.path(), &env);
        let named = read(&file);
        assert_eq!(named.contains("<failure/>"), written, "{named}");
        let error = says.map(|step| {
            format!(r#"<testsuite name="page tests"><error message="the step {step} of the job failed"/></testsuite>"#)
        });
        assert_eq!(named.contains("<error"), error.is_some(), "{named}");
        if let Some(error) = error {
            assert!(named.contains(&error), "{named}");
        }
    }
}

// The jobs that run no tests of their own name their results by what they found. `page` writes results only where a
// step of its own failed, which it says as a suite of its name with an error naming the step. `tree` always writes a
// file, so that a run of only the design tree has results the engine reads: a suite of its name with no test in it where
// its steps passed, and an error naming the first step that failed where one did, which the engine reads as a unit not
// built and not as a test that failed, which it would run again by name. Run under bash: a few milliseconds.
#[test]
fn the_jobs_that_run_no_tests_say_a_step_that_failed_and_the_tree_job_always_has_results() {
    let (page, tree) = (naming_script(Names::Fails), naming_script(Names::Tree));
    let steps = |failed: Option<&str>| {
        [
            "Check out the commit",
            "Citations of the documents",
            "Test the citations' check",
        ]
        .map(|step| format!("{} {step}", if failed == Some(step) { "failure" } else { "success" }))
        .join("\n")
    };
    let named = |script: &str, job: &str, failed: Option<&str>| {
        let dir = tempfile::tempdir().unwrap();
        // The runner's temporary directory is there when a job begins, and holds nothing of an earlier run's.
        let file = dir.path().join("temp").join(format!("{job}.xml"));
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let results = file.display().to_string();
        let env = [("JOB", job), ("RESULTS", results.as_str()), ("STEPS", &steps(failed))];
        run_script(script, dir.path(), &env);
        file.exists().then(|| read(&file))
    };
    assert_eq!(
        named(&tree, "tree", None).as_deref(),
        Some("<testsuites><testsuite name=\"tree\"/></testsuites>\n"),
        "the tree job's steps passed"
    );
    for failed in [
        "Check out the commit",
        "Citations of the documents",
        "Test the citations' check",
    ] {
        let error = format!(
            "<testsuites><testsuite name=\"tree\"><error message=\"the step {failed} of the job failed\"/></testsuite></testsuites>\n"
        );
        assert_eq!(named(&tree, "tree", Some(failed)), Some(error.clone()), "{failed}");
        assert_eq!(
            named(&page, "page", Some(failed)),
            Some(error.replace("\"tree\"", "\"page\"")),
            "{failed}"
        );
    }
    assert_eq!(
        named(&page, "page", None),
        None,
        "the page's steps passed: nothing to say"
    );
}

// The build of the tests of a job fails, and `target/` holds a `junit.xml` that an earlier run left (a cancelled run's,
// which its `Name the results` did not move): the tests step takes it away as it begins, so that the results the job
// names hold the error naming `Test` and nothing of the earlier run. The step of ci.yml that runs the tests is run
// with a `cargo` that builds nothing and exits 101, as a build that fails does. A few milliseconds under bash.
#[test]
fn a_build_of_the_tests_that_fails_leaves_no_results_of_an_earlier_run_to_be_uploaded() {
    let tests: Vec<String> = (scripts(&workflow("ci.yml")).into_iter())
        .map(|(_, script)| script)
        .filter(|script| script.contains("cargo nextest run"))
        .collect();
    assert_eq!(tests.len(), 1, "scripts of ci.yml that run the tests: {}", tests.len());
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("target/nextest/ci")).unwrap();
    std::fs::write(
        dir.path().join("target/nextest/ci/junit.xml"),
        r#"<testsuites><testsuite name="carry"><testcase name="left_by_an_earlier_run"/></testsuite></testsuites>"#,
    )
    .unwrap();
    let cargo = dir.path().join("cargo.sh");
    std::fs::write(&cargo, "cargo() { return 101; }\n").unwrap();
    let env = [("BASH_ENV", cargo.to_str().unwrap()), ("PARTITION", "2")];
    let (ok, said) = try_script(&tests[0], dir.path(), &env);
    assert!(!ok, "the build fails the step: {said}");
    let job = "test 2 of 3";
    let results = dir.path().join("temp").join(format!("{job}.xml"));
    std::fs::create_dir_all(results.parent().unwrap()).unwrap();
    let steps = ["Check out the commit", "Check the version of nextest", "Build the page"]
        .map(|step| format!("success {step}"))
        .join("\n");
    let env = [
        ("JOB", job),
        ("RESULTS", results.to_str().unwrap()),
        ("TESTED", "failure Test"),
        ("STEPS", steps.as_str()),
    ];
    run_script(&naming_script(Names::Moves), dir.path(), &env);
    let named = read(&results);
    assert!(
        named.contains(&format!(
            r#"<testsuite name="{job}"><error message="the step Test of the job failed"/></testsuite>"#
        )) && !named.contains("left_by_an_earlier_run"),
        "{named}"
    );
}

// A part of a script that no green run reaches is run for the first time when something has failed. The first time a
// job was red, the step of `check` that copies the failed jobs' logs ended in a syntax error of its last lines, where
// an apostrophe inside `${names:-...}` began a quoted string: `check` was red, but by the shell's error and without the
// line that names the jobs. So bash is given every script of the workflows to parse, which reads all of one and runs
// none of it; each with the number of scripts it has, so that a reading that finds fewer is seen.
#[test]
fn every_script_of_the_workflows_parses() {
    for (file, least) in [("ci.yml", 6), ("fast.yml", 6), ("measure.yml", 3)] {
        let found = scripts(&workflow(file));
        assert!(found.len() >= least, "{file}: only {} scripts found", found.len());
        // The script this test was written for is among them.
        let gathers = found.iter().any(|(_, script)| script.contains("Did not pass:"));
        assert!(
            gathers || file != "ci.yml",
            "{file}: the script of check that gathers is not found"
        );
        for (line, script) in found {
            let mut bash = Command::new("bash")
                .arg("-n")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .expect("bash starts");
            let mut stdin = bash.stdin.take().expect("its input is piped");
            stdin.write_all(script.as_bytes()).expect("the script is written");
            drop(stdin);
            let ended = bash.wait_with_output().expect("bash ends");
            let said = String::from_utf8_lossy(&ended.stderr);
            assert!(ended.status.success(), "{file}:{line}: {said}");
        }
    }
}

/// The measure workflow the repository's declaration names, as the engine reads that declaration.
fn declared_measure() -> flow::agents::CiMeasure {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let agents = flow::agents::read(&root.join("flow")).expect("what is written beside the flows is read");
    let text = std::fs::read_to_string(root.join(flow::agents::DECLARATION)).expect("the declaration is read");
    let declaration = flow::agents::Declaration::read(&text, flow::agents::DECLARATION, &agents)
        .expect("the repository's declaration is one");
    (declaration.ci.expect("the project declares its workflows"))
        .measure
        .expect("the project declares a measure workflow")
}

// The measure workflow runs by hand and by nothing else, as the fast one does: it runs on the owner's machine, and a
// trigger by a push or a pull request would run anyone's code there. It takes the inputs the declaration names, names
// its run by the call, has no concurrency group, since each run is a request an agent waits for, and runs on the
// declared machine, where its job stops at the minutes the declaration says a run may take: the wait for the machine's
// lock counts in them, and a job with no limit of its own would hold the machine for the six hours GitHub allows. It
// uploads in the artifact the declaration names. Under a millisecond alone; no other test holds that the file and the
// declaration say the same.
#[test]
fn the_measure_workflow_runs_by_hand_on_the_declared_machine_and_stops_when_the_declaration_says() {
    let measure = declared_measure();
    let text = workflow(&measure.workflow);
    let on = block(&text, "on");
    assert_eq!(children(&on), ["workflow_dispatch"]);
    let inputs = children(&under(&under(&on, "workflow_dispatch"), "inputs")[1..]);
    for input in [
        &measure.commit_input,
        &measure.base_input,
        &measure.call_input,
        &measure.measure_input,
    ] {
        assert!(inputs.contains(input), "no input {input} in {inputs:?}");
    }
    let run_name = text.lines().find(|l| l.starts_with("run-name:")).unwrap_or_default();
    assert!(
        run_name.contains(&format!("inputs.{}", measure.call_input)),
        "{run_name:?}"
    );
    assert!(block(&text, "concurrency").is_empty(), "a concurrency group");
    let job = under(&block(&text, "jobs"), "measure").join("\n");
    assert!(
        job.contains(&format!("runs-on: [self-hosted, {}]", measure.runner)),
        "{job}"
    );
    let minutes: u64 = (measure.too_long.strip_suffix('m'))
        .and_then(|m| m.parse().ok())
        .unwrap_or_else(|| panic!("too_long {:?} is not in minutes", measure.too_long));
    assert!(job.contains(&format!("timeout-minutes: {minutes}\n")), "{job}");
    assert!(text.contains(&format!("name: {}\n", measure.results)), "{text}");
}

/// A repository of two commits, as the measure workflow's job has it checked out at the commit it times: the commit the
/// work began from, with the measurement `seen`, and the work's, which changes the file `version` the measurements write
/// and adds the measurement `added`. Both write the `version` of the checkout they run in to a file of the runner's
/// temporary directory named as they are, so that what is written says in which checkout each timing ran.
fn two_commits(dir: &Path) -> (PathBuf, String, String) {
    std::fs::create_dir_all(dir.join("measure")).expect("a directory");
    let git = |args: &[&str]| {
        let ended = Command::new("git")
            .args(["-c", "user.name=Someone", "-c", "user.email=someone@example.com"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git starts");
        let said = String::from_utf8_lossy(&ended.stderr);
        assert!(ended.status.success(), "git {args:?}: {said}");
        String::from_utf8_lossy(&ended.stdout).trim().to_string()
    };
    git(&["init", "-q", "-b", "main"]);
    let writes = |name: &str| format!("cat version >> \"$RUNNER_TEMP/{name}\"\n");
    std::fs::write(dir.join("measure/seen"), writes("seen")).expect("a measurement");
    std::fs::write(dir.join("version"), "base\n").expect("the version");
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "Base"]);
    let base = git(&["rev-parse", "HEAD"]);
    std::fs::write(dir.join("measure/added"), writes("added")).expect("a measurement");
    std::fs::write(dir.join("version"), "candidate\n").expect("the version");
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "Candidate"]);
    let candidate = git(&["rev-parse", "HEAD"]);
    (dir.to_path_buf(), base, candidate)
}

// The step that times the measurements, run as GitHub runs it on a repository of two commits, with the clock as a
// function that gives every timing a duration of the test's choosing. Each name is timed in the three checkouts, in
// pairs whose order rotates, by the work's own file of that name in each checkout, so that a measurement the work adds is
// timed in the checkout that lacks it; and it writes the suites the engine reads, each side's timings in the order they
// were made and each side's median, which is computed here again from the durations and the order, apart from the shell,
// for an even number of pairs and an odd one, the workflow's own. The step that checks the results are whole passes on
// them, and is red for a timing lost and for one that is not above zero. About half a second alone (twenty-four and
// thirty timings of a script that does nothing, two checkouts, and a lock): the only run of the workflow's shell before
// it is on main, where the first run of a script that no test has run would otherwise be an agent's.
#[test]
fn the_measure_workflow_times_each_name_in_three_checkouts_and_reports_the_suites_the_engine_reads() {
    let timing = script_of("measure.yml", "flock 9");
    let whole = script_of("measure.yml", "timings for");
    let dir = tempfile::tempdir().expect("a directory");
    let (work, base, candidate) = two_commits(&dir.path().join("work"));
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).expect("a directory");
    // The time of day, as a function that bash reads before the script (`BASH_ENV`): each timing begins a hundred
    // seconds after the one before it, and ends the number of seconds that the file `durations` has on its line.
    let clock = dir.path().join("clock.sh");
    std::fs::write(
        &clock,
        r#"date() {
  local n j d
  n=$(cat "$RUNNER_TEMP/clock" 2>/dev/null || echo 0)
  echo $((n + 1)) > "$RUNNER_TEMP/clock"
  j=$((n / 2))
  if [ $((n % 2)) -eq 0 ]; then
    echo "$((j * 100)).000000000"
  else
    d=$(sed -n "$((j + 1))p" "$RUNNER_TEMP/durations")
    echo "$((j * 100 + d)).000000000"
  fi
}
"#,
    )
    .expect("the clock");
    // The first of the two names is a measurement the work's base has too, the second one the work adds.
    let names = ["seen", "added"];
    // Which side each timing of a name was, counting from the first: the sides rotate by a place each pair.
    let sides = ["base", "twin", "candidate"];
    let side_of = |p: usize| sides[(p / 3 + p % 3) % 3];
    for pairs in [4usize, 5] {
        let temp = dir.path().join(format!("temp-{pairs}"));
        let count = pairs.to_string();
        std::fs::create_dir_all(&temp).expect("a directory");
        let durations: Vec<usize> = (0..names.len() * pairs * 3).map(|j| (j * 7) % 11 + 1).collect();
        let lines: String = durations.iter().map(|d| format!("{d}\n")).collect();
        std::fs::write(temp.join("durations"), lines).expect("the durations");
        let env = [
            ("RUNNER_TEMP", temp.to_str().expect("a path")),
            ("GITHUB_WORKSPACE", work.to_str().expect("a path")),
            ("HOME", home.to_str().expect("a path")),
            ("PAIRS", count.as_str()),
            ("COMMIT", candidate.as_str()),
            ("BASE", base.as_str()),
            ("MEASURE", "seen\nadded\n"),
            ("BASH_ENV", clock.to_str().expect("a path")),
        ];
        run_script(&timing, &work, &env);

        let mut expected = String::from("<testsuites>\n");
        let mut medians = String::new();
        for side in sides {
            expected += &format!("  <testsuite name=\"{side}\">\n");
            medians += &format!("  <testsuite name=\"median {side}\">\n");
            for (n, name) in names.iter().enumerate() {
                let mut of_side: Vec<usize> = Vec::new();
                for p in (0..pairs * 3).filter(|p| side_of(*p) == side) {
                    let d = durations[n * pairs * 3 + p];
                    expected += &format!("    <testcase name=\"{name}\" time=\"{d}.000000\"/>\n");
                    of_side.push(d);
                }
                of_side.sort();
                let middle = of_side.len() / 2;
                let median = if of_side.len() % 2 == 1 {
                    of_side[middle] as f64
                } else {
                    (of_side[middle - 1] + of_side[middle]) as f64 / 2.0
                };
                medians += &format!("    <testcase name=\"{name}\" time=\"{median:.6}\"/>\n");
            }
            expected += "  </testsuite>\n";
            medians += "  </testsuite>\n";
        }
        expected += &medians;
        expected += "</testsuites>\n";
        let results = temp.join("measurement/xml/measure.xml");
        assert_eq!(read(&results), expected, "{pairs} pairs");

        // The checkout each timing ran in: the base's and its twin's hold the version of the commit the work began from.
        let ran_in: String = (0..pairs * 3)
            .map(|p| {
                if side_of(p) == "candidate" {
                    "candidate\n"
                } else {
                    "base\n"
                }
            })
            .collect();
        for name in names {
            assert_eq!(read(&temp.join(name)), ran_in, "{name}, {pairs} pairs");
        }

        // The results are whole; one timing lost, or one that is not above zero, is not.
        let env = [
            ("RUNNER_TEMP", temp.to_str().expect("a path")),
            ("PAIRS", count.as_str()),
        ];
        run_script(&whole, &work, &env);
        let kept: Vec<&str> = expected.lines().collect();
        let timing_of = |name: &str| {
            kept.iter()
                .position(|l| l.contains(&format!("<testcase name=\"{name}\"")))
        };
        let mut lost = kept.clone();
        lost.remove(timing_of("added").expect("a timing of the second name"));
        std::fs::write(&results, lost.join("\n") + "\n").expect("the results");
        let (ok, said) = try_script(&whole, &work, &env);
        let total = names.len() * (3 * pairs + 3);
        let wrong = format!("{} timings for {} measurements, not {total}", total - 1, names.len());
        assert!(!ok && said.contains(&wrong), "{said}");
        let mut zero = kept.clone();
        zero[timing_of("seen").expect("a timing of the first name")] =
            "    <testcase name=\"seen\" time=\"0.000000\"/>";
        std::fs::write(&results, zero.join("\n") + "\n").expect("the results");
        let (ok, said) = try_script(&whole, &work, &env);
        assert!(!ok && said.contains("a timing is not above zero"), "{said}");
    }
}

// What the step that times the measurements refuses before it times anything: a name that is no measurement of the
// commit, one the shell would read as more than a name, no name at all, and a base that is no commit. Each says why in
// its own words, and none leaves a checkout of the base behind. Milliseconds alone; the refusals of the one script,
// which the test above does not reach.
#[test]
fn the_measure_workflow_times_nothing_for_a_name_that_is_no_measurement_or_a_base_that_is_no_commit() {
    let timing = script_of("measure.yml", "flock 9");
    let dir = tempfile::tempdir().expect("a directory");
    let (work, base, candidate) = two_commits(&dir.path().join("work"));
    let temp = dir.path().join("temp");
    std::fs::create_dir_all(&temp).expect("a directory");
    let refused = |measure: &str, base: &str| {
        let env = [
            ("RUNNER_TEMP", temp.to_str().expect("a path")),
            ("GITHUB_WORKSPACE", work.to_str().expect("a path")),
            ("HOME", dir.path().to_str().expect("a path")),
            ("PAIRS", "1"),
            ("COMMIT", candidate.as_str()),
            ("BASE", base),
            ("MEASURE", measure),
        ];
        let (ok, said) = try_script(&timing, &work, &env);
        assert!(!ok, "{measure:?}: {said}");
        said
    };
    assert!(refused("nothere\n", &base).contains("no measurement is named nothere"));
    assert!(refused("a;b\n", &base).contains("a;b is not the name of a measurement"));
    assert!(refused("\n  \n", &base).contains("no measurement is named"));
    assert!(refused("seen\n", "0123abc").contains("0123abc is no commit of this repository"));
    assert!(!temp.join("base").exists() && !temp.join("twin").exists());
}
