//! The checks the tool `ci` runs on a head before it pushes it (`[[ci.push_checks]]`): what a check that does not pass
//! keeps from happening, what the result tells the engineer, and what is recorded. The commands are `sh -c` stubs that
//! read the worktree, not rustfmt or tsc.

use super::*;

/// Puts `tables` at the end of the project's file, which ends in its `[ci]` table, and commits it: the engine reads the
/// configuration from its last commit.
fn lists_push_checks(s: &Setup, tables: &str) {
    let project = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    write(&s.data, "projects/p.toml", &format!("{project}\n{tables}"));
    git(&s.data, &["commit", "-q", "-am", "the checks run before a push"]);
}

/// The agents as `scripts` makes them, but for the implementation: its first round commits the feature and asks for ci,
/// its second commits `src/formatted.txt`, which the stub checks look for, and asks for ci again; it then hands in as
/// it did.
fn formatting_late() -> BTreeMap<&'static str, Script> {
    let mut agents = scripts();
    let implement = agents.remove("I1").unwrap();
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            let wt = &r.worktree;
            match n {
                1 => {
                    write(wt, "src/lib.txt", "old\nfeature\n");
                    git(wt, &["add", "-A"]);
                    git(wt, &["commit", "-q", "-m", "Add the feature"]);
                }
                2 => {
                    write(wt, "src/formatted.txt", "yes\n");
                    git(wt, &["add", "-A"]);
                    git(wt, &["commit", "-q", "-m", "Format it"]);
                }
                _ => return implement(r, n),
            }
            let head = git(wt, &["rev-parse", "HEAD"]);
            let tools = json!([{ "tool": "ci", "head": head, "tests": ["feature"] }]);
            (vec![("outcome", "tools")], vec![("outcome.tools", tools)])
        }),
    );
    agents
}

/// The output of the tool cell's `nth` round that ended (from 0), which the engine checked against the flow before it
/// took it.
fn tool_cell_output(e: &Engine, item: i64, nth: i64) -> Value {
    let text: String =
        e.db.conn()
            .query_row(
                "SELECT r.output FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.item = ?1 AND r.cell = 'X1' AND r.transition IS NOT NULL
                 ORDER BY r.work, r.n LIMIT 1 OFFSET ?2",
                params![item, nth],
                |r| r.get(0),
            )
            .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// What the work's row holds in a column.
fn work_column(e: &Engine, work: i64, column: &str) -> Option<String> {
    e.db.conn()
        .query_row(&format!("SELECT {column} FROM works WHERE id = ?1"), [work], |r| {
            r.get(0)
        })
        .unwrap()
}

// Every check the project lists runs on the head before ci pushes it, in the order listed and whatever an earlier one
// found; one that does not pass (an exit code but 0, or running past its time limit) keeps the head, the pull request
// and the CI untouched, and the result tells each check with the exit code and output of those that failed. Each run is
// a row of tool_runs under `push_check`, and the call that was kept from pushing has no row of ci. The same head with
// the file the checks look for pushes as it does with none listed, after the checks have run again. Both results fit
// the flow.
#[test]
fn a_check_that_does_not_pass_keeps_the_head_from_being_pushed_and_all_of_them_run() {
    let s = setup("push-checks");
    let ran = s.root.join("ran.txt");
    lists_push_checks(
        &s,
        &format!(
            "[[ci.push_checks]]\nname = \"format\"\ncommand = \"sh -c 'echo format >> {ran}; if test -f \
             src/formatted.txt; then exit 0; fi; echo Diff in src/lib.txt >&2; echo not formatted; exit 1'\"\n\
             [[ci.push_checks]]\nname = \"types\"\ncommand = \"sh -c 'echo types >> {ran}'\"\n\
             [[ci.push_checks]]\nname = \"slow\"\ntimeout = 1\ncommand = \"sh -c 'echo slow >> {ran}; if test -f \
             src/formatted.txt; then exit 0; fi; echo started; sleep 120'\"\n",
            ran = ran.display()
        ),
    );
    let (mut e, said, item) = start(&s, formatting_late());
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (work, _, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_eq!(ended_at(&e, item, "X1"), [(Some("ran".to_string()), None)], "{log}");
    let wt = worktree_of(&e, work);
    let first_head = git(&wt, &["rev-parse", "HEAD"]);
    // The first head: the format check failed with its exit code and what it wrote, the types check passed, the slow
    // check was stopped for its time; all three ran, in the order listed.
    let output = tool_cell_output(&e, item, 0);
    let push = &output["results"][0]["push"];
    assert_eq!(push["verdict"], "checks_failed", "{output}\n{log}");
    assert_eq!(push["head"], first_head);
    let checks = push["push_checks"].as_array().unwrap();
    let told: Vec<(&str, &str)> = checks
        .iter()
        .map(|c| (c["name"].as_str().unwrap(), c["verdict"].as_str().unwrap()))
        .collect();
    assert_eq!(
        told,
        [("format", "failed"), ("types", "passed"), ("slow", "failed")],
        "{output}"
    );
    assert_eq!(checks[0]["exit"], 1);
    assert_eq!(checks[0]["timed_out"], false);
    assert_eq!(checks[0]["output"], "Diff in src/lib.txt\nnot formatted\n");
    assert!(
        checks[1].get("exit").is_none() && checks[1].get("output").is_none(),
        "{output}"
    );
    assert!(checks[2].get("exit").is_none(), "{output}");
    assert_eq!(
        (&checks[2]["timed_out"], &checks[2]["output"]),
        (&json!(true), &json!("started\n"))
    );
    assert!(
        checks[0]["command"]
            .as_str()
            .unwrap()
            .starts_with("sh -c 'echo format >> "),
        "{output}"
    );
    assert_eq!(std::fs::read_to_string(&ran).unwrap(), "format\ntypes\nslow\n");
    // Each run is a row of tool_runs, with the result the engineer was given; the ci call was kept from pushing has no
    // row, and nothing was pushed: no branch, no pull request, no push recorded, and nothing the checks wrote in the
    // worktree.
    let runs = tool_runs(&e, item, "push_check");
    assert!(runs.iter().all(|r| r.0 == 1), "{runs:?}");
    assert_eq!(runs.iter().map(|r| r.1.clone()).collect::<Vec<_>>(), *checks);
    assert!(tool_runs(&e, item, "ci").is_empty(), "{log}");
    assert_eq!(tools::remote_branches(&s), ["refs/heads/main"], "{log}");
    assert!(s.prs.lock().unwrap().is_empty());
    assert_eq!(work_column(&e, work, "pushed_head"), None);
    assert_eq!(git(&wt, &["status", "--porcelain"]), "");
    assert!(dependency_sources::fits_the_flow(&mut e, work, &output), "{output}");
    // The second head holds the file: every check passes, and the head is pushed as it is with no check listed.
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        ended_at(&e, item, "X1"),
        [(Some("ran".to_string()), None), (Some("ran".to_string()), None)],
        "{log}"
    );
    let second_head = git(&wt, &["rev-parse", "HEAD"]);
    assert_ne!(first_head, second_head);
    let runs = tool_runs(&e, item, "push_check");
    assert_eq!(runs.len(), 6, "{runs:?}\n{log}");
    assert!(runs[3..].iter().all(|r| r.1["verdict"] == "passed"), "{:?}", &runs[3..]);
    assert_eq!(
        std::fs::read_to_string(&ran).unwrap(),
        "format\ntypes\nslow\nformat\ntypes\nslow\n"
    );
    let pushed = tool_runs(&e, item, "ci");
    assert_eq!(pushed.len(), 1, "{pushed:?}\n{log}");
    assert_eq!(pushed[0].1["push"]["verdict"], "pushed", "{pushed:?}");
    assert_eq!(pushed[0].1["push"]["head"], second_head);
    let branch = work_column(&e, work, "branch").unwrap();
    assert_eq!(
        tools::remote_branches(&s),
        ["refs/heads/main".to_string(), format!("refs/heads/{branch}")],
        "{log}"
    );
    assert_eq!(s.prs.lock().unwrap().len(), 1);
    let second = tool_cell_output(&e, item, 1);
    assert!(dependency_sources::fits_the_flow(&mut e, work, &second), "{second}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The cells that ask for ci are told the checks the project lists before a push, as the setting stands: `none` where it
// lists none, the list where it lists some. An engineer that does not know a check runs would take the refusal it gives
// for a fault of ci.
#[test]
fn the_ci_tool_is_described_with_the_checks_the_project_lists() {
    let s = setup("push-checks-described");
    let described = |s: &Setup| -> String {
        let (e, _) = engine(s, scripts());
        let flow = e.flow(e.flow_now).unwrap();
        let g = flow.graphs.iter().find(|g| g.name == engine::ENGINEER).unwrap();
        let parts = engine::message::line_parts(&e, g, g.cell("I1").unwrap(), "p");
        let common = parts
            .iter()
            .find(|(address, _)| address == "format:common")
            .and_then(|(_, text)| text.as_str())
            .unwrap();
        // The schema is the last part of the text, which holds no blank line.
        let schema: Value = serde_json::from_str(common.rsplit("\n\n").next().unwrap()).unwrap();
        let form = tools::tool_form(&schema, "ci").expect("the common format has the tool");
        form["properties"]["tool"]["description"].as_str().unwrap().to_string()
    };
    let none = described(&s);
    assert!(none.contains("`project.ci.push_checks` (none)"), "{none}");
    lists_push_checks(
        &s,
        "[[ci.push_checks]]\nname = \"format\"\ncommand = \"cargo fmt --all --check\"\n",
    );
    let told = described(&s);
    assert!(
        told.contains(r#"`project.ci.push_checks` ([{"name":"format","command":"cargo fmt --all --check"}])"#),
        "{told}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// With no check listed no command runs and ci pushes as it did: nothing is recorded under `push_check`.
#[test]
fn with_no_check_listed_no_command_runs_and_the_head_is_pushed() {
    let s = setup("push-checks-none");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(tool_runs(&e, item, "push_check").is_empty(), "{log}");
    let pushed = tool_runs(&e, item, "ci");
    assert_eq!(pushed.len(), 1, "{pushed:?}\n{log}");
    assert_eq!(pushed[0].1["push"]["verdict"], "pushed", "{pushed:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A check whose command cannot be started is a fault of the configuration, not a verdict on the head: the tool fails,
// each attempt is a row of tool_runs with the reason, and nothing is pushed.
#[test]
fn a_check_that_cannot_be_started_is_a_failed_tool_and_nothing_is_pushed() {
    let s = setup("push-checks-missing");
    lists_push_checks(
        &s,
        "[[ci.push_checks]]\nname = \"format\"\ncommand = \"wf-agents-no-such-program --check\"\n",
    );
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let runs = tool_runs(&e, item, "push_check");
    assert_eq!(
        runs.iter().map(|r| r.0).collect::<Vec<_>>(),
        [1, 2, 3],
        "{runs:?}\n{log}"
    );
    for (_, result, _) in &runs {
        assert_eq!(result["failed"]["kind"], "other", "{runs:?}");
        let what = result["failed"]["what"].as_str().unwrap();
        assert!(what.starts_with("wf-agents-no-such-program: "), "{what}");
    }
    assert!(tool_runs(&e, item, "ci").is_empty(), "{log}");
    assert_eq!(tools::remote_branches(&s), ["refs/heads/main"], "{log}");
    assert!(s.prs.lock().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&s.root);
}
