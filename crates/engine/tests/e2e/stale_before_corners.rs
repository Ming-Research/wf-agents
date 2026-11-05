//! Corners of the note by which ci tells a result of ci_before gone stale: the order of the tools in a call, a change
//! of a file the run did not put on the code before the change, and a second change of the file after the run was
//! asked again.
//!
//! The reference is the check of a return that the engine runs on the test results the engineer hands in: a result of
//! ci_before counts only while the files that run put on the code before the change are as they were. The note must
//! name no run that check would accept, and must miss none it would send back.

use super::*;

/// The test file the implementing script writes, in the test paths of the project.
const TEST_FILE: &str = "tests/feature_test.txt";

/// A file the run of ci_before never put on the code before the change.
const OTHER_FILE: &str = "src/other.txt";

/// The implementing script. Round 1 adds the feature and its test file, and asks for ci_before (first in the call) and
/// ci for it; round 2 changes only `OTHER_FILE`; round 3 changes the test file; round 4 asks ci_before again as the
/// note of round 3 says; round 5 changes `OTHER_FILE` again and pushes; later rounds hand in the work.
fn implementing() -> Script {
    let base = scripts().remove("I1").unwrap();
    Box::new(move |r: &Request, n: usize| {
        let wt = &r.worktree;
        // The first commit is the one the default script takes for the feature's: it adds nothing of its own after it.
        let commit = |file: &str, text: &str| {
            write(wt, file, text);
            git(wt, &["add", "-A"]);
            let message = if n == 1 { "Add the feature" } else { "Change a file" };
            git(wt, &["commit", "-q", "-m", message]);
            git(wt, &["rev-parse", "HEAD"])
        };
        let pr = json!({ "title": "Add the feature", "body": "It adds it." });
        let before = |head: &str, files: Value, tests: Value| json!({ "tool": "ci_before", "head": head, "files": files, "tests": tests });
        let ci = |head: &str| json!({ "tool": "ci", "head": head, "tests": ["feature"] });
        let ask = |tools: Value| (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
        match n {
            1 => {
                write(wt, "src/lib.txt", "old\nfeature\n");
                let head = commit(TEST_FILE, "one\n");
                let mut first = ci(&head);
                first["pr"] = pr;
                // ci_before is listed before the ci that pushes the head it reads from.
                ask(json!([before(&head, json!([TEST_FILE]), json!(["feature"])), first]))
            }
            2 => ask(json!([ci(&commit(OTHER_FILE, "one\n"))])),
            3 => ask(json!([ci(&commit(TEST_FILE, "two\n"))])),
            4 => {
                let results = given_value(&r.prompt, "\u{b7}X1\u{b7}results").expect(&r.prompt);
                let stale = &results[0]["push"]["ci_before_stale"][0];
                let head = git(wt, &["rev-parse", "HEAD"]);
                ask(json!([before(&head, stale["files"].clone(), stale["tests"].clone())]))
            }
            5 => ask(json!([ci(&commit(OTHER_FILE, "two\n"))])),
            _ => {
                let (pick, mut set) = base(r, n);
                set.push((
                    "outcome.test_changes",
                    json!([{
                        "change": "added",
                        "test": "feature",
                        "catches": "The feature missing.",
                        "kind": "old_behaviour"
                    }]),
                ));
                (pick, set)
            }
        }
    })
}

/// What each round of the tool cell gave the work, in order: the `results` of its return.
fn tool_results(e: &Engine, item: i64) -> Vec<Vec<Value>> {
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT r.output FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.item = ?1 AND r.cell = 'X1' AND r.transition IS NOT NULL ORDER BY r.work, r.n",
            )
            .unwrap();
    st.query_map([item], |r| r.get::<_, String>(0))
        .unwrap()
        .map(|x| {
            let out: Value = serde_json::from_str(&x.unwrap()).unwrap();
            out["results"].as_array().cloned().unwrap_or_default()
        })
        .collect()
}

// Item #311: the note names exactly what the check of a return would send back, over a longer course. A ci_before listed
// before the ci of its call leaves nothing stale; a push that changes only a file the run did not read says nothing; a
// push that changes the test file again says so, as it did the first time; and once ci_before is asked again, a later
// push of another file says nothing, since the new run is the one that counts. The return that follows is not sent back.
#[test]
fn the_note_follows_the_files_of_the_latest_run_over_several_pushes() {
    let s = setup("stale-before-corners");
    let mut agents = scripts();
    agents.insert("I1", implementing());
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "T3");
    let (work, _, _) = state(&e, item).unwrap();
    let log = said.lock().unwrap().join("\n");
    let calls = tool_results(&e, item);
    assert_eq!(calls.len(), 5, "{calls:?}\n{log}");
    // The first call listed ci_before before ci; each ran, and the run of ci_before leaves nothing stale. The order of
    // the results is not the point, so each is found by its tool.
    let of = |tool: &str| {
        calls[0]
            .iter()
            .find(|v| v["tool"] == tool)
            .unwrap_or_else(|| panic!("no {tool}: {calls:?}"))
    };
    assert_eq!(of("ci_before")["ran"]["verdict"], "ran", "{calls:?}\n{log}");
    assert_eq!(of("ci")["push"]["verdict"], "pushed", "{calls:?}\n{log}");
    assert_eq!(of("ci")["push"]["ci_before_stale"], Value::Null, "{calls:?}");
    // Only a file the run did not read changed: nothing is stale.
    assert_eq!(calls[1][0]["push"]["verdict"], "pushed", "{calls:?}");
    assert_eq!(calls[1][0]["push"]["ci_before_stale"], Value::Null, "{calls:?}");
    // The test file changed: stale, for the first run's test and its file alone.
    assert_eq!(
        calls[2][0]["push"]["ci_before_stale"],
        json!([{ "tests": ["feature"], "changed": [TEST_FILE], "files": [TEST_FILE] }]),
        "{calls:?}\n{log}"
    );
    // Asked again as the note said.
    assert_eq!(calls[3][0]["ran"]["verdict"], "ran", "{calls:?}\n{log}");
    // The newer run counts: a push of another file leaves nothing stale.
    assert_eq!(calls[4][0]["push"]["verdict"], "pushed", "{calls:?}");
    assert_eq!(calls[4][0]["push"]["ci_before_stale"], Value::Null, "{calls:?}");
    assert_eq!(sent_back(&e), 0, "{:?}\n{log}", why_sent_back(&e, work, "I1"));
    let _ = std::fs::remove_dir_all(&s.root);
}
