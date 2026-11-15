//! The repository's two CI workflows as the engine relies on them. The engine runs the workflows the project declares
//! (`ci` in flow/project.toml) by hand on the commit it gives, finds its own run by the value it gives (`call`) in the
//! run's name, and judges a run by the conclusions of its jobs and by the JUnit of its tests in the artifact
//! `test-results`; so a job renamed, or added to the wrong workflow, or a trigger changed, or JUnit left out, changes
//! what a green run means. GitHub is the reference for the workflow syntax; the comments at the head of each workflow
//! say what it must and must not hold.

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

// The full workflow keeps a job `check` that is red whenever the page, the lint or a job of the tests did not pass,
// cancelled ones included, and that makes the JUnit of those jobs one artifact.
#[test]
fn the_full_workflow_keeps_the_checks_a_merge_waits_for() {
    let text = workflow("ci.yml");
    let jobs = block(&text, "jobs");
    let names = children(&jobs);
    for want in ["page", "lint", "test", "check"] {
        assert!(names.iter().any(|n| n == want), "no job {want} in {names:?}");
    }
    let check = under(&jobs, "check").join("\n");
    assert!(check.contains("needs: [page, lint, test]"), "{check}");
    assert!(check.contains("if: always()"), "{check}");
    assert!(check.contains(r#"[ "$r" = success ] || red=true"#), "{check}");
    for (job, lines) in [("page", under(&jobs, "page")), ("check", under(&jobs, "check"))] {
        assert!(
            !lines.iter().any(|l| l.starts_with("    name:")),
            "the job {job} is given another name, which the engine would not find"
        );
    }
}

// The full workflow is run by hand by the engine, with the commit to test and a value only its request knows, and still
// runs on each pull request into `main` and each push to `main`, and on no other push, and on none to a branch of the
// name `main` once had.
#[test]
fn the_full_workflow_runs_by_hand_and_on_the_pull_requests_and_pushes_of_main() {
    let text = workflow("ci.yml");
    let on = block(&text, "on");
    assert_eq!(children(&on), ["workflow_dispatch", "pull_request", "push"]);
    for event in ["pull_request", "push"] {
        let branches: Vec<&str> = (under(&on, event).iter())
            .filter_map(|l| l.trim().strip_prefix("- "))
            .collect();
        assert_eq!(branches, ["main"], "the branches of {event}");
    }
    assert!(
        !text.contains("refs/heads/next") && !text.lines().any(|l| l.trim() == "- next"),
        "the branch `next` is no part of the workflow"
    );
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

// What a run uploads for the engine is JUnit, all of it in the one artifact `test-results`: the full workflow's jobs
// each upload theirs in a file named by the job, nextest's of each third of the Rust tests and vitest's of the page,
// and the job `check` makes them one; the fast workflow uploads its own.
#[test]
fn every_test_of_a_run_is_reported_as_junit_in_the_artifact_test_results() {
    let text = workflow("ci.yml");
    let jobs = block(&text, "jobs");
    let page = under(&jobs, "page").join("\n");
    assert!(page.contains("--reporter=junit") && page.contains("page.xml"), "{page}");
    assert!(page.contains("name: junit-page"), "{page}");
    let test = under(&jobs, "test").join("\n");
    assert!(
        test.contains("name: test ${{ matrix.partition }} of 3")
            && test.contains("JOB: test ${{ matrix.partition }} of 3")
            && test.contains("path: target/nextest/ci/test ${{ matrix.partition }} of 3.xml")
            && test.contains("name: junit-${{ matrix.partition }}"),
        "{test}"
    );
    assert!(page.contains("JOB: page"), "{page}");
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

/// The one script of fast.yml that holds `marker`.
fn fast_script(marker: &str) -> String {
    let mut found: Vec<String> = (scripts(&workflow("fast.yml")).into_iter())
        .map(|(_, script)| script)
        .filter(|script| script.contains(marker))
        .collect();
    assert_eq!(
        found.len(),
        1,
        "scripts of fast.yml that hold {marker:?}: {}",
        found.len()
    );
    found.remove(0)
}

/// Runs a script of fast.yml under bash in `dir`, with the environment a step is given, and returns what it printed.
fn run_script(script: &str, dir: &Path, env: &[(&str, &str)]) -> String {
    let ended = Command::new("bash")
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .envs(env.iter().copied())
        .output()
        .expect("bash starts");
    let said = String::from_utf8_lossy(&ended.stderr);
    assert!(ended.status.success(), "{said}");
    String::from_utf8_lossy(&ended.stdout).into_owned()
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
    // and each call kept.
    let stub = dir.path().join("stub.sh");
    std::fs::write(
        &stub,
        r#"npx() {
  echo "$*" >> "$STUB_DIR/calls"
  case "$*" in
    *"vitest list --project unit"*) sed "s|@ROOT@|$PWD|g" "$STUB_DIR/unit.json" ;;
    *"vitest list --project browser"*) sed "s|@ROOT@|$PWD|g" "$STUB_DIR/browser.json" ;;
  esac
}
"#,
    )
    .expect("the stub");
    let names = temp.join("names");
    let out = dir.path().join("output");
    let run = |page: &str, rust: &str, rust_ran: bool| {
        for file in ["calls", "output"] {
            std::fs::write(dir.path().join(file), "").expect("a file");
        }
        std::fs::write(names.join("page"), page).expect("the names");
        std::fs::write(names.join("rust"), rust).expect("the names");
        let env = [
            ("BASH_ENV", stub.to_str().expect("a path")),
            ("STUB_DIR", dir.path().to_str().expect("a path")),
            ("RUNNER_TEMP", temp.to_str().expect("a path")),
            ("GITHUB_OUTPUT", out.to_str().expect("a path")),
            ("RUST", if rust_ran { "true" } else { "false" }),
        ];
        run_script(&script, &client, &env)
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
}

// What `check` gives of a job that did not pass is that job's own log, which the engine then reads in `check`'s. The
// log of a job holds the escapes that colour cargo's output, and gh gives no answer that holds any unless told to: the
// first time a job was red, `check` printed gh's refusal in place of each log, and the agent would have been told that
// the lint and a job of the tests failed and nothing of which test or which line.
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

/// The script of ci.yml that names the results of a job and says a step of it that failed: of the job of a third of
/// the Rust tests, which moves nextest's results to their name, or of the page's, which vitest writes there.
fn naming_script(moves: bool) -> String {
    let found: Vec<String> = (scripts(&workflow("ci.yml")).into_iter())
        .map(|(_, script)| script)
        .filter(|script| script.contains("of the job failed") && script.contains(r#"mv "$junit" "$RESULTS""#) == moves)
        .collect();
    assert_eq!(
        found.len(),
        1,
        "scripts of ci.yml that name a job's results, moving them: {moves}"
    );
    found.into_iter().next().expect("one")
}

// A job of tests writes its results in a file named by it, and says in it a step of its own that failed as a
// testsuite of the job's name with an error naming the step, as a unit not built is: where its tests failed and wrote
// their results, nothing is added; where a step before or beside them failed, it is named, whether the tests wrote
// their results or not; where the tests' own step failed and wrote nothing, it is named. So the check before a merge
// tells a job red for something else than its tests from one whose tests failed. Run under bash: a few milliseconds.
#[test]
fn a_job_of_tests_names_its_results_by_itself_and_says_a_step_of_its_own_that_failed() {
    let (test, page) = (naming_script(true), naming_script(false));
    let junit =
        r#"<testsuites><testsuite name="carry"><testcase name="t"><failure/></testcase></testsuite></testsuites>"#;
    let steps = |failed: Option<&str>| {
        ["Check out the commit", "Check the version of nextest", "Build the page"]
            .map(|step| format!("{} {step}", if failed == Some(step) { "failure" } else { "success" }))
            .join("\n")
    };
    let job = "test 2 of 3";
    let results = format!("target/nextest/ci/{job}.xml");
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
        let env = [
            ("JOB", job),
            ("RESULTS", results.as_str()),
            ("TESTED", &format!("{tested} Test")),
            ("STEPS", &steps(failed)),
        ];
        run_script(&test, dir.path(), &env);
        let named = read(&dir.path().join(&results));
        assert_eq!(named.contains("<failure/>"), written, "{named}");
        let error = says.map(|step| {
            format!(r#"<testsuite name="{job}"><error message="the step {step} of the job failed"/></testsuite>"#)
        });
        assert_eq!(named.contains("<error"), error.is_some(), "{named}");
        if let Some(error) = error {
            assert!(named.contains(&error), "{named}");
        }
    }
    // The page's job: its tests passed and its build after them failed, which is said in the file the tests wrote.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("page.xml");
    std::fs::write(
        &file,
        r#"<testsuites><testsuite name="src/a.test.ts"><testcase name="a"/></testsuite></testsuites>"#,
    )
    .unwrap();
    let results = file.display().to_string();
    let steps = "success Lint the page\nfailure Build the page".to_string();
    let env = [
        ("JOB", "page"),
        ("RESULTS", results.as_str()),
        ("TESTED", "success Test the page"),
        ("STEPS", steps.as_str()),
    ];
    run_script(&page, dir.path(), &env);
    let named = read(&file);
    assert!(
        named.contains(r#"<testcase name="a"/></testsuite><testsuite name="page"><error message="the step Build the page of the job failed"/></testsuite></testsuites>"#),
        "{named}"
    );
}

// A part of a script that no green run reaches is run for the first time when something has failed. The first time a
// job was red, the step of `check` that copies the failed jobs' logs ended in a syntax error of its last lines, where
// an apostrophe inside `${names:-...}` began a quoted string: `check` was red, but by the shell's error and without the
// line that names the jobs. So bash is given every script of both workflows to parse, which reads all of one and runs
// none of it.
#[test]
fn every_script_of_the_workflows_parses() {
    for file in ["ci.yml", "fast.yml"] {
        let found = scripts(&workflow(file));
        assert!(found.len() > 5, "{file}: only {} scripts found", found.len());
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
