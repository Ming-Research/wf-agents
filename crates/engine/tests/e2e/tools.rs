//! Tools: the tool cell, the measurement workflow, the fast CI's dispatch for ci_before and the tools'
//! descriptions.

use super::*;

// A query an agent asks for runs in the tool cell, and the tool cell's return, the rows included, fits the schema the
// compiled flow gives cell `X1`: the engine checks its own tool cell's return against it and pauses the work when it
// does not, so a result the flow does not describe fails here.
#[test]
fn a_query_runs_in_the_tool_cell_and_its_return_fits_the_flow() {
    let s = setup("query");
    let mut agents = scripts();
    let implement = agents.remove("I1").unwrap();
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            if n == 1 {
                let tools = json!([{ "tool": "query", "sql": "SELECT id, title FROM items" }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            implement(r, n - 1)
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let ran = |e: &Engine| -> Option<String> {
        e.db.conn()
            .query_row(
                "SELECT r.output FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.item = ?1 AND r.cell = 'X1' AND r.transition IS NOT NULL ORDER BY r.work, r.n LIMIT 1",
                [item],
                |r| r.get(0),
            )
            .ok()
    };
    for _ in 0..60 {
        if ran(&e).is_some() || state(&e, item).is_some_and(|(_, _, paused)| paused.is_some()) {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    let (work, _, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    let output: Value = serde_json::from_str(&ran(&e).expect(&log)).unwrap();
    let result = &output["results"][0];
    assert_eq!(result["tool"], "query", "{output}");
    let rows = result["ran"]["rows"].as_array().cloned().unwrap_or_default();
    assert!(rows.iter().any(|r| r["title"] == "Add the feature"), "{output}");
    let w = e.work(work).unwrap();
    let g = e.graph_of(&w).unwrap();
    let schema = g.cell("X1").and_then(|c| c.schema.clone()).unwrap();
    assert!(flow::valid(&schema, &output), "{output}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A tool that fails for want of the network, while the engine finds the network down, is no failure of the work's:
// no question goes to the architect, every work waits for the network, and the cell runs its tools again once it is
// back.
#[test]
fn a_tool_without_the_network_waits_for_it() {
    let s = setup("tool-offline");
    let up = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let seen = up.clone();
    let hooks = Hooks {
        network: Some(Arc::new(move || seen.load(std::sync::atomic::Ordering::SeqCst))),
        ..Hooks::default()
    };
    let (mut e, said, item) = start_with(&s, scripts(), hooks);
    for _ in 0..40 {
        if state(&e, item).is_some_and(|x| x.1 == "X1") {
            break;
        }
        e.tick().unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(state(&e, item).map(|x| x.1).as_deref(), Some("X1"), "{log}");
    // The network goes down: the push cannot reach the remote.
    let away = s.root.join("origin-away");
    std::fs::rename(&s.origin, &away).unwrap();
    up.store(false, std::sync::atomic::Ordering::SeqCst);
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(at, "X1", "{log}");
    assert_eq!(paused, None, "{log}");
    assert_eq!(e.halt().map(|h| h.kind).as_deref(), Some("offline"), "{log}");
    let asked: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM architect_matters WHERE work IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(asked, 0, "{log}");
    // Back: the cell runs its tools again and the work goes on.
    std::fs::rename(&away, &s.origin).unwrap();
    up.store(true, std::sync::atomic::Ordering::SeqCst);
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "X1", "{log}");
    assert!(failed_kinds(&e).contains(&"offline".to_string()), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The branches there are on the fake GitHub's remote.
fn remote_branches(s: &Setup) -> Vec<String> {
    git(
        &s.root,
        &[
            "--git-dir",
            &s.origin.to_string_lossy(),
            "for-each-ref",
            "--format=%(refname)",
            "refs/heads",
        ],
    )
    .lines()
    .map(str::to_string)
    .collect()
}

/// Lists a workflow in the project's configuration for the measure tool, as `[[ci.measure]]`.
fn lists_a_measurement_workflow(s: &Setup) {
    let project = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    write(
        &s.data,
        "projects/p.toml",
        &format!(
            "{project}\n[[ci.measure]]\nworkflow = \"measure.yml\"\ninputs = [\"command\", \"base\"]\n\
             artifact = \"measurement\"\n"
        ),
    );
    git(&s.data, &["commit", "-q", "-am", "a measurement workflow"]);
}

/// A request for the measure tool, with the words the flow asks for beside what it runs.
fn measure_request(head: &str, workflow: &str, inputs: &str) -> Value {
    json!({ "tool": "measure", "head": head, "workflow": workflow, "inputs": inputs, "minutes": 1,
            "question": "How long does it take?", "compare": "The head with the one before it.",
            "decide": "Slower than before is a regression." })
}

// The measure tool runs a workflow the project lists, by hand, on a temporary ref that holds the code handed in as a
// commit of its own, and not on the work's branch; it waits, keeps the run's logs and artifact under the data directory
// and takes the ref away. A workflow that ends red has run, but not to the end. A request for a workflow the project
// does not list, for an input the workflow does not take, or for a head that is not the worktree's is refused, says
// what the project does list, runs nothing and leaves no ref. Every request is a row of tool_runs, which the view of the
// work's measurements reads, and the tool cell's return holds a result for each, which passed the check against the
// flow.
#[test]
fn the_measure_tool_runs_a_listed_workflow_on_a_temporary_ref() {
    let s = setup("measure");
    lists_a_measurement_workflow(&s);
    let mut agents = scripts();
    agents.insert(
        "I1",
        Box::new(|r: &Request, _: usize| {
            let wt = &r.worktree;
            write(wt, "src/lib.txt", "old\nfeature\n");
            git(wt, &["add", "-A"]);
            git(wt, &["commit", "-q", "-m", "Add the feature"]);
            let head = git(wt, &["rev-parse", "HEAD"]);
            let before = git(wt, &["rev-parse", "HEAD~1"]);
            let tools = json!([
                measure_request(&head, "measure.yml", "command=true\nbase=main"),
                measure_request(&head, "measure.yml", "command=red"),
                measure_request(&head, "other.yml", ""),
                measure_request(&head, "measure.yml", "pairs=3"),
                measure_request(&before, "measure.yml", "command=true"),
            ]);
            (vec![("outcome", "tools")], vec![("outcome.tools", tools)])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (work, _, _) = state(&e, item).unwrap();
    let head = git(&worktree_of(&e, work), &["rev-parse", "HEAD"]);
    assert_eq!(ended_at(&e, item, "X1"), [(Some("ran".to_string()), None)], "{log}");
    let runs = tool_runs(&e, item, "measure");
    assert_eq!(runs.len(), 5, "{runs:?}\n{log}");
    assert!(runs.iter().all(|r| r.0 == 1), "{runs:?}");
    let verdicts: Vec<(&str, Option<&str>)> = runs
        .iter()
        .map(|r| (r.1["push"]["verdict"].as_str().unwrap(), r.1["push"]["ok"].as_str()))
        .collect();
    assert_eq!(
        verdicts,
        [
            ("ran", Some("ran")),
            ("ran", Some("not_run")),
            ("refused", None),
            ("refused", None),
            ("refused", None)
        ],
        "{runs:?}"
    );
    // The two runs: the head handed in, a link to the run, and the files kept, which the row of tool_runs lists.
    let by_hand = s.by_hand.lock().unwrap().clone();
    assert_eq!(by_hand.len(), 2, "{by_hand:?}");
    let ran = &runs[0].1["push"];
    let commit = by_hand[0]
        .commit
        .clone()
        .expect("the temporary ref was on the remote when the workflow was run");
    assert_eq!(ran["head"], head);
    assert_eq!(ran["run"], format!("https://example.invalid/measure.yml/{commit}"));
    let files: Vec<String> = serde_json::from_value(ran["files"].clone()).unwrap();
    assert_eq!(files.len(), 2, "{files:?}");
    assert_eq!(runs[0].2, files);
    assert!(files[0].ends_with("-measure/job-1-check.log"), "{files:?}");
    assert_eq!(
        std::fs::read_to_string(&files[0]).unwrap(),
        format!("the log of https://example.invalid/measure.yml/{commit}")
    );
    assert!(
        files[1].ends_with("-measure/artifact-measurement/pairs.tsv"),
        "{files:?}"
    );
    assert_eq!(std::fs::read_to_string(&files[1]).unwrap(), "1\t2\t3\n");
    assert!(files.iter().all(|f| f.contains("tool-output")), "{files:?}");
    // A workflow that ended red has run, and its log is kept.
    assert_eq!(runs[1].1["push"]["files"].as_array().map(Vec::len), Some(2), "{runs:?}");
    // The workflow ran on a temporary ref, by hand, with the inputs given, on a commit of its own that holds the head's
    // files; another request for the same head is another commit.
    for d in &by_hand {
        assert_eq!(d.workflow, "measure.yml");
        assert!(d.branch.starts_with("wf-agents-measure/"), "{d:?}");
    }
    assert_eq!(
        by_hand[0].inputs,
        [
            ("command".to_string(), "true".to_string()),
            ("base".to_string(), "main".to_string())
        ]
    );
    assert_ne!(by_hand[0].commit, by_hand[1].commit);
    let wt = worktree_of(&e, work);
    assert_ne!(commit, head);
    assert_eq!(git(&wt, &["rev-parse", &format!("{commit}^")]), head);
    assert_eq!(
        git(&wt, &["rev-parse", &format!("{commit}^{{tree}}")]),
        git(&wt, &["rev-parse", &format!("{head}^{{tree}}")])
    );
    // Nothing is left on the remote but main: no temporary ref, and the work's branch was not pushed.
    assert_eq!(remote_branches(&s), ["refs/heads/main"], "{log}");
    // The refusals say why, and what the project lists.
    let why = |i: usize| runs[i].1["push"]["why"].as_str().unwrap().to_string();
    assert!(
        why(2).contains("\"other.yml\"") && why(2).ends_with("they are: measure.yml"),
        "{}",
        why(2)
    );
    assert!(
        why(3).contains("\"pairs\"") && why(3).ends_with("it takes: command, base"),
        "{}",
        why(3)
    );
    assert!(why(4).starts_with("The head is not the one handed in"), "{}", why(4));
    // The view of the work's measurements reads every request, the runs and the refusals.
    let (all, ran_to_the_end): (i64, i64) =
        e.db.conn()
            .query_row(
                "SELECT count(*), count(*) FILTER (WHERE json_extract(result, '$.push.ok') = 'ran')
                 FROM work_measurements WHERE work = ?1",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!((all, ran_to_the_end), (5, 1));
    let _ = std::fs::remove_dir_all(&s.root);
}

// A measurement that cannot be started is a failed tool, tried again only while the network is up; it leaves no
// temporary ref on the remote, and its attempt is a row of tool_runs.
#[test]
fn a_measurement_that_cannot_start_leaves_no_temporary_ref() {
    let s = setup("measure-down");
    lists_a_measurement_workflow(&s);
    let mut agents = scripts();
    agents.insert(
        "I1",
        Box::new(|r: &Request, _: usize| {
            let wt = &r.worktree;
            write(wt, "src/lib.txt", "old\nfeature\n");
            git(wt, &["add", "-A"]);
            git(wt, &["commit", "-q", "-m", "Add the feature"]);
            let head = git(wt, &["rev-parse", "HEAD"]);
            let tools = json!([measure_request(&head, "measure.yml", "command=down")]);
            (vec![("outcome", "tools")], vec![("outcome.tools", tools)])
        }),
    );
    let up = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let seen = up.clone();
    let hooks = Hooks {
        network: Some(Arc::new(move || seen.load(std::sync::atomic::Ordering::SeqCst))),
        ..Hooks::default()
    };
    let (mut e, said, item) = start_with(&s, agents, hooks);
    step_to(&mut e, item, "X1");
    up.store(false, std::sync::atomic::Ordering::SeqCst);
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let runs = tool_runs(&e, item, "measure");
    assert_eq!(runs.len(), 1, "{runs:?}\n{log}");
    assert_eq!(runs[0].1["failed"]["kind"], "network_or_api", "{runs:?}");
    // The workflow was asked for on a ref that was on the remote then, and is not now.
    let by_hand = s.by_hand.lock().unwrap().clone();
    assert_eq!(by_hand.len(), 1, "{by_hand:?}");
    assert!(by_hand[0].commit.is_some(), "{by_hand:?}");
    assert_eq!(remote_branches(&s), ["refs/heads/main"], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A request GitHub refuses to run (an input the workflow requires is missing) is a refusal the engineer sees with what
// GitHub said, not a failed tool tried three times; a run that does not finish in time is a measurement that did not run
// to the end, not tried again to wait as long again; and a wait that fails to reach GitHub is tried again by waiting
// for the same run, not by starting another measurement. None leaves a temporary ref.
#[test]
fn a_measurement_is_tried_again_only_by_waiting_for_the_run_it_started() {
    let s = setup("measure-retry");
    lists_a_measurement_workflow(&s);
    let mut agents = scripts();
    agents.insert(
        "I1",
        Box::new(|r: &Request, _: usize| {
            let wt = &r.worktree;
            write(wt, "src/lib.txt", "old\nfeature\n");
            git(wt, &["add", "-A"]);
            git(wt, &["commit", "-q", "-m", "Add the feature"]);
            let head = git(wt, &["rev-parse", "HEAD"]);
            let tools = json!([
                measure_request(&head, "measure.yml", "command=reject"),
                measure_request(&head, "measure.yml", "command=timeout"),
                measure_request(&head, "measure.yml", "command=flaky"),
            ]);
            (vec![("outcome", "tools")], vec![("outcome.tools", tools)])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_at(&e, item, "X1"), [(Some("ran".to_string()), None)], "{log}");
    let runs = tool_runs(&e, item, "measure");
    let attempts: Vec<i64> = runs.iter().map(|r| r.0).collect();
    assert_eq!(attempts, [1, 1, 1, 2], "{runs:?}\n{log}");
    // The refusal says what GitHub said.
    assert_eq!(runs[0].1["push"]["verdict"], "refused", "{runs:?}");
    let why = runs[0].1["push"]["why"].as_str().unwrap();
    assert!(why.contains("HTTP 422: Required input 'base' not provided"), "{why}");
    // The run that did not finish in time has run, not to the end, and says why.
    let timed_out = &runs[1].1["push"];
    assert_eq!(
        (&timed_out["verdict"], &timed_out["ok"]),
        (&json!("ran"), &json!("not_run")),
        "{runs:?}"
    );
    assert!(
        timed_out["note"]
            .as_str()
            .unwrap()
            .contains("did not finish in 10800 s"),
        "{runs:?}"
    );
    // The wait that failed is a failed attempt, and the next one has the run of the same measurement.
    assert_eq!(runs[2].1["failed"]["kind"], "network_or_api", "{runs:?}");
    assert_eq!(runs[3].1["push"]["ok"], "ran", "{runs:?}");
    // One workflow was started for each request, none for the try again.
    let by_hand = s.by_hand.lock().unwrap().clone();
    assert_eq!(by_hand.len(), 3, "{by_hand:?}");
    assert_eq!(by_hand[2].waits, 2, "{by_hand:?}");
    assert_eq!(remote_branches(&s), ["refs/heads/main"], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A run that never got started is a measurement that did not run to the end and is not tried again, and a start that
// failed once before GitHub was reached is tried again with the same temporary commit, so that no second workflow is
// started on another one.
#[test]
fn a_measurement_that_never_started_is_not_tried_again_and_a_start_that_failed_once_is_made() {
    let s = setup("measure-edges");
    lists_a_measurement_workflow(&s);
    let mut agents = scripts();
    agents.insert(
        "I1",
        Box::new(|r: &Request, _: usize| {
            let wt = &r.worktree;
            write(wt, "src/lib.txt", "old\nfeature\n");
            git(wt, &["add", "-A"]);
            git(wt, &["commit", "-q", "-m", "Add the feature"]);
            let head = git(wt, &["rev-parse", "HEAD"]);
            let tools = json!([
                measure_request(&head, "measure.yml", "command=nostart"),
                measure_request(&head, "measure.yml", "command=flaky_dispatch"),
            ]);
            (vec![("outcome", "tools")], vec![("outcome.tools", tools)])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_at(&e, item, "X1"), [(Some("ran".to_string()), None)], "{log}");
    let runs = tool_runs(&e, item, "measure");
    let attempts: Vec<i64> = runs.iter().map(|r| r.0).collect();
    assert_eq!(attempts, [1, 1, 2], "{runs:?}\n{log}");
    // A run that never started has run, not to the end, and the note says it did not start.
    let nostart = &runs[0].1["push"];
    assert_eq!(
        (&nostart["verdict"], &nostart["ok"]),
        (&json!("ran"), &json!("not_run")),
        "{runs:?}"
    );
    assert!(
        nostart["note"].as_str().is_some_and(|n| n.contains("no run of")),
        "{runs:?}"
    );
    // The start that failed once failed as an attempt, and the next attempt ran the measurement.
    assert_eq!(runs[1].1["failed"]["kind"], "network_or_api", "{runs:?}");
    assert_eq!(runs[2].1["push"]["ok"], "ran", "{runs:?}");
    // The workflow was asked for twice for the last request, on the same branch and the same commit.
    let by_hand = s.by_hand.lock().unwrap().clone();
    assert_eq!(by_hand.len(), 3, "{by_hand:?}");
    assert_eq!(by_hand[1].branch, by_hand[2].branch, "{by_hand:?}");
    assert!(by_hand[1].commit.is_some(), "{by_hand:?}");
    assert_eq!(by_hand[1].commit, by_hand[2].commit, "{by_hand:?}");
    assert_eq!(remote_branches(&s), ["refs/heads/main"], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A run with no job (GitHub gives one for a workflow file it cannot read) has run, not to the end, and has no run to link
// to and no log to keep; the note is all the engineer has to read, and it says why there is nothing.
#[test]
fn a_run_with_no_job_says_why_it_left_nothing() {
    let s = setup("measure-nojobs");
    lists_a_measurement_workflow(&s);
    let mut agents = scripts();
    agents.insert(
        "I1",
        Box::new(|r: &Request, _: usize| {
            let wt = &r.worktree;
            write(wt, "src/lib.txt", "old\nfeature\n");
            git(wt, &["add", "-A"]);
            git(wt, &["commit", "-q", "-m", "Add the feature"]);
            let head = git(wt, &["rev-parse", "HEAD"]);
            let tools = json!([measure_request(&head, "measure.yml", "command=nojobs")]);
            (vec![("outcome", "tools")], vec![("outcome.tools", tools)])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let runs = tool_runs(&e, item, "measure");
    assert_eq!(runs.len(), 1, "{runs:?}\n{log}");
    let nojobs = &runs[0].1["push"];
    assert_eq!(
        (&nojobs["verdict"], &nojobs["ok"]),
        (&json!("ran"), &json!("not_run")),
        "{runs:?}"
    );
    assert_eq!(nojobs["files"], json!([]), "{runs:?}");
    assert!(
        nojobs["note"].as_str().is_some_and(|n| !n.trim().is_empty()),
        "a run with no job gave neither a run to link to, a file, nor a note: {nojobs}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A workflow GitHub refuses to run for ci_before, the tool that runs the fast CI on the tests alone on the code before
// the change, is what it is for measure: a refusal its engineer reads, not a failed tool of a kind the flow does not
// name, tried three times. Its reference is the contract of the failure kind `rejected` (it is "never given as a failed
// tool") and the kinds of failure the flow's cell X1 lists.
#[test]
fn a_dispatch_github_refuses_for_ci_before_is_a_refusal_and_not_a_failed_tool() {
    let s = setup("ci-before-rejected");
    let project = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    write(
        &s.data,
        "projects/p.toml",
        &format!("{project}tests_input = \"command\"\n"),
    );
    git(&s.data, &["commit", "-q", "-am", "the fast CI takes the tests it runs"]);
    let mut agents = scripts();
    agents.insert(
        "I1",
        Box::new(|r: &Request, _: usize| {
            let wt = &r.worktree;
            write(wt, "src/lib.txt", "old\nfeature\n");
            write(wt, "tests/feature.txt", "feature\n");
            git(wt, &["add", "-A"]);
            git(wt, &["commit", "-q", "-m", "Add the feature"]);
            let head = git(wt, &["rev-parse", "HEAD"]);
            let tools = json!([
                { "tool": "ci", "head": head, "tests": ["feature"], "pr": { "title": "Add the feature", "body": "It adds it." } },
                { "tool": "ci_before", "head": head, "files": ["tests/feature.txt"], "tests": ["reject"] },
            ]);
            (vec![("outcome", "tools")], vec![("outcome.tools", tools)])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let runs = tool_runs(&e, item, "ci_before");
    let listed = [
        "network_or_api",
        "ci_did_not_start",
        "timed_out",
        "machine_down",
        "other",
    ];
    for r in &runs {
        if let Some(kind) = r.1["failed"]["kind"].as_str() {
            assert!(
                listed.contains(&kind),
                "a failed tool of the kind {kind}, which the flow does not name: {runs:?}"
            );
        }
    }
    // Asked for once and not tried again, and the engineer is told what GitHub said.
    assert_eq!(runs.len(), 1, "{runs:?}\n{log}");
    assert_eq!(runs[0].1["ran"]["verdict"], "refused", "{runs:?}\n{log}");
    assert!(
        runs[0].1["ran"]["why"].as_str().is_some_and(|w| w.contains("HTTP 422")),
        "{runs:?}"
    );
    assert_eq!(s.by_hand.lock().unwrap().len(), 1);
    // The temporary branch of the tests is gone; the work's own branch, which ci pushed, may be there.
    let branches = remote_branches(&s);
    assert!(
        branches.iter().all(|b| !b.contains("wf-agents-before")),
        "{branches:?}\n{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A project that lists no measurement workflow is told so, and what to answer instead; nothing runs, the request is a
// row of tool_runs but not a run of a measurement, so an answer of measured is sent back and one of not_measured stands.
#[test]
fn a_project_that_lists_no_measurement_workflow_is_told_to_answer_not_measured() {
    let s = setup("measure-none");
    let mut agents = designing();
    agents.insert(
        "D1",
        Box::new(|r: &Request, n: usize| match n {
            1 => {
                let head = git(&r.worktree, &["rev-parse", "HEAD"]);
                let tools = json!([measure_request(&head, "measure.yml", "command=true")]);
                (vec![("outcome", "tools")], vec![("outcome.tools", tools)])
            }
            2 => (vec![("outcome.measure", "measured")], Vec::new()),
            _ => (vec![("outcome.measure", "not_measured")], Vec::new()),
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    for _ in 0..40 {
        if ended_at(&e, item, "D1").len() >= 3 {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    let runs = tool_runs(&e, item, "measure");
    assert_eq!(runs.len(), 1, "{runs:?}\n{log}");
    assert_eq!(runs[0].1["push"]["verdict"], "refused", "{runs:?}");
    let why = runs[0].1["push"]["why"].as_str().unwrap();
    assert!(why.contains("ci.measure") && why.contains("not_measured"), "{why}");
    assert!(s.by_hand.lock().unwrap().is_empty());
    assert_eq!(remote_branches(&s), ["refs/heads/main"], "{log}");
    // The answer of measured is sent back, saying why; the answer of not_measured is taken.
    let d1 = ended_at(&e, item, "D1");
    assert_eq!(d1.len(), 3, "{d1:?}\n{log}");
    assert_eq!(d1[0], (Some("tools".to_string()), None), "{d1:?}");
    assert!(
        d1[1].0.is_none()
            && d1[1]
                .1
                .as_deref()
                .is_some_and(|why| why.contains("measured, but this work has no measure run")),
        "{d1:?}\n{log}"
    );
    assert!(d1[2].0.is_some() && d1[2].1.is_none(), "{d1:?}\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The form of a tool in the schema of the common format, as an agent is given it: the object that holds its
/// properties.
pub(super) fn tool_form(schema: &Value, tool: &str) -> Option<Value> {
    match schema {
        Value::Object(o) => {
            if o.get("properties")
                .and_then(|p| p.get("tool"))
                .is_some_and(|t| t["const"] == tool)
            {
                return Some(schema.clone());
            }
            o.values().find_map(|v| tool_form(v, tool))
        }
        Value::Array(xs) => xs.iter().find_map(|v| tool_form(v, tool)),
        _ => None,
    }
}

/// What each of the cells that ask for the tools is told of the gh tool and of the measure tool in the common format its
/// session is given for the project: the cell, the gh tool's description and the description of the measure tool's input
/// that names the workflow. The cells are the research (`R1`), the design (`D1`, `D2`) and the implementation (`I1`).
fn tool_descriptions(s: &Setup) -> Vec<(&'static str, String, String)> {
    let (e, _) = engine(s, scripts());
    let flow = e.flow(e.flow_now).unwrap();
    let g = flow.graphs.iter().find(|g| g.name == engine::ENGINEER).unwrap();
    ["R1", "D1", "D2", "I1"]
        .into_iter()
        .map(|cell| {
            let parts = engine::message::line_parts(&e, g, g.cell(cell).unwrap(), "p");
            let common = parts
                .iter()
                .find(|(address, _)| address == "format:common")
                .and_then(|(_, text)| text.as_str())
                .unwrap();
            // The schema is the last part of the text, which holds no blank line.
            let schema: Value = serde_json::from_str(common.rsplit("\n\n").next().unwrap()).unwrap();
            let gh = tool_form(&schema, "gh").unwrap_or_else(|| panic!("{cell}: no gh tool"));
            let measure = tool_form(&schema, "measure").unwrap_or_else(|| panic!("{cell}: no measure tool"));
            (
                cell,
                gh["properties"]["tool"]["description"].as_str().unwrap().to_string(),
                measure["properties"]["workflow"]["description"]
                    .as_str()
                    .unwrap()
                    .to_string(),
            )
        })
        .collect()
}

// What the cells that research, design and implement are told of the gh tool and of the measure tool holds the values
// of the settings they name, as the installation and the project set them now: the allow list of gh, and the
// workflows the project lists, which read as none where it lists none; a change of either shows in the description
// given next. A design that does not know the project lists no measurement workflow would lock acceptance conditions
// that need one.
#[test]
fn the_tools_are_described_with_the_settings_in_force() {
    let s = setup("tool-settings");
    let told = tool_descriptions(&s);
    assert_eq!(told.len(), 4);
    for (cell, gh, workflow) in &told {
        assert!(gh.contains("`gh.allow` ([\"pr view\"])"), "{cell}: {gh}");
        for copied in ["pr list", "repo view", "release view", "search"] {
            assert!(!gh.contains(copied), "{cell}: {copied}: {gh}");
        }
        assert!(workflow.contains("`project.ci.measure` (none)"), "{cell}: {workflow}");
    }
    // The installation allows more, and the project lists a workflow.
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(
        &s.data,
        "install.toml",
        &install.replace("allow = [\"pr view\"]", "allow = [\"pr view\", \"run list\"]"),
    );
    lists_a_measurement_workflow(&s);
    for (cell, gh, workflow) in &tool_descriptions(&s) {
        assert!(gh.contains("`gh.allow` ([\"pr view\",\"run list\"])"), "{cell}: {gh}");
        assert!(
            workflow.contains(
                "`project.ci.measure` ([{\"workflow\":\"measure.yml\",\"inputs\":[\"command\",\"base\"],\
                 \"artifact\":\"measurement\"}])"
            ),
            "{cell}: {workflow}"
        );
    }
    let _ = std::fs::remove_dir_all(&s.root);
}
