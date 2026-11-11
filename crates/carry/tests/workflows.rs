//! The repository's two CI workflows as the engine relies on them. The engine reads the checks of a commit by the names
//! of their jobs (`github.checks`, `page` and `check` in this repository), and runs the fast workflow by hand on the
//! commit it gives (`github.fast`); so a job renamed, or added to the wrong workflow, or a trigger changed, changes what
//! a green run means. GitHub is the reference for the workflow syntax; the comments at the head of each workflow say
//! what it must and must not hold.

use std::io::Write;
use std::path::PathBuf;
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
        if rest.trim() != "|" {
            out.push((at + 1, rest.trim().to_string()));
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

// The full workflow keeps the two checks the engine waits for, `page` and `check`, and `check` is red whenever the lint
// or a job of the tests did not pass, cancelled ones included: a merge waits for that one job, not for the jobs it
// gathers, so that how the tests are split can change without changing what the engine is told to wait for.
#[test]
fn the_full_workflow_keeps_the_checks_a_merge_waits_for() {
    let text = workflow("ci.yml");
    let jobs = block(&text, "jobs");
    let names = children(&jobs);
    for want in ["page", "lint", "test", "check"] {
        assert!(names.iter().any(|n| n == want), "no job {want} in {names:?}");
    }
    let check = under(&jobs, "check").join("\n");
    assert!(check.contains("needs: [lint, test]"), "{check}");
    assert!(check.contains("if: always()"), "{check}");
    assert!(check.contains(r#"[ "$r" = success ] || red=true"#), "{check}");
    for (job, lines) in [("page", under(&jobs, "page")), ("check", under(&jobs, "check"))] {
        assert!(
            !lines.iter().any(|l| l.trim().starts_with("name:")),
            "the job {job} is given another name, which the engine would not find"
        );
    }
}

// The full workflow runs on each pull request into `next` and each push to `next`, and on no other push: a branch of
// the code before a change (`wf-agents-before/`) is run by the fast workflow, by hand, with only the tests named, and a
// whole run of this one beside it would take GitHub's machines for nothing.
#[test]
fn the_full_workflow_runs_on_the_pull_requests_and_pushes_of_next() {
    let text = workflow("ci.yml");
    let on = block(&text, "on");
    assert_eq!(children(&on), ["pull_request", "push"]);
    for event in ["pull_request", "push"] {
        let branches: Vec<&str> = (under(&on, event).iter())
            .filter_map(|l| l.trim().strip_prefix("- "))
            .collect();
        assert_eq!(branches, ["next"], "the branches of {event}");
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
