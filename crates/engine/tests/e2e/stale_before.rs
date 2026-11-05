//! A result of ci_before gone stale: the engineer is told by `ci`, before it hands in its test results, which results a
//! return would be sent back for, and the return is still sent back when it names a stale result.
//!
//! The reference is the check of a return that the engine runs on the test results the engineer hands in: a test's
//! result before the change counts only while the test files the run of ci_before that gave it put on the code before
//! the change are as they were (the flow's forms say so in their words, "run ci_before again").

use super::*;

/// The test file the implementing scripts write, in the test paths of the project.
const TEST_FILE: &str = "tests/feature_test.txt";

/// What the implementing script does about the result of ci_before after it changed the test file and pushed the change.
#[derive(Clone, Copy)]
enum Rerun {
    /// Reads the note of the result of ci, and asks ci_before again as the note says.
    FromNote,
    /// Asks ci_before for the changed file in the same call as ci, before any note.
    WithCi,
    /// Hands in its test results without asking again, and asks only when sent back.
    Late,
}

/// The rows of the test changes the script ends with: the feature's test, which passes on the head and before the
/// change, so that nothing is wrong with it but a result of ci_before gone stale.
fn test_rows() -> Value {
    json!([{ "change": "added", "test": "feature", "catches": "The feature missing.", "kind": "old_behaviour" }])
}

/// The implementing script: the first round adds the feature and its test file, and asks for ci and, in the same
/// call, ci_before for the test file; the second changes the test file and asks for ci (and ci_before with it, for
/// `WithCi`); the rounds after hand in the test results, or ask ci_before again, as `rerun` says.
fn implementing(rerun: Rerun) -> Script {
    let base = scripts().remove("I1").unwrap();
    Box::new(move |r: &Request, n: usize| {
        let wt = &r.worktree;
        let commit = |text: &str, message: &str| {
            write(wt, TEST_FILE, text);
            git(wt, &["add", "-A"]);
            git(wt, &["commit", "-q", "-m", message]);
            git(wt, &["rev-parse", "HEAD"])
        };
        let pr = json!({ "title": "Add the feature", "body": "It adds it." });
        let before = |head: &str, files: Value, tests: Value| json!({ "tool": "ci_before", "head": head, "files": files, "tests": tests });
        let ask = |tools: Value| (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
        let finish = || {
            let (pick, mut set) = base(r, n);
            set.push(("outcome.test_changes", test_rows()));
            (pick, set)
        };
        match (n, rerun) {
            (1, _) => {
                write(wt, "src/lib.txt", "old\nfeature\n");
                let head = commit("one\n", "Add the feature");
                ask(json!([
                    { "tool": "ci", "head": head, "tests": ["feature"], "pr": pr },
                    before(&head, json!([TEST_FILE]), json!(["feature"])),
                ]))
            }
            (2, Rerun::WithCi) => {
                let head = commit("two\n", "Change the test");
                ask(json!([
                    { "tool": "ci", "head": head, "tests": ["feature"] },
                    before(&head, json!([TEST_FILE]), json!(["feature"])),
                ]))
            }
            (2, _) => {
                let head = commit("two\n", "Change the test");
                ask(json!([{ "tool": "ci", "head": head, "tests": ["feature"] }]))
            }
            // The note is the request: its files and tests are handed in again as they are.
            (3, Rerun::FromNote) => {
                let results = given_value(&r.prompt, "\u{b7}X1\u{b7}results").expect(&r.prompt);
                let stale = &results[0]["push"]["ci_before_stale"][0];
                let head = git(wt, &["rev-parse", "HEAD"]);
                ask(json!([before(&head, stale["files"].clone(), stale["tests"].clone())]))
            }
            (4, Rerun::Late) => {
                let head = git(wt, &["rev-parse", "HEAD"]);
                ask(json!([before(&head, json!([TEST_FILE]), json!(["feature"]))]))
            }
            _ => finish(),
        }
    })
}

/// A work whose engineer runs `implementing(rerun)`, stepped until its implementation is taken and the details review
/// is next: the setup, the engine, what it said, the item and its work.
fn run(name: &str, rerun: Rerun) -> (Setup, Engine, Arc<Mutex<Vec<String>>>, i64, i64) {
    let s = setup(name);
    let mut agents = scripts();
    agents.insert("I1", implementing(rerun));
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "T3");
    let (work, _, _) = state(&e, item).unwrap();
    (s, e, said, item, work)
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

// Item #311: the engineer runs ci_before, changes the test file it read and pushes the change with ci. The result of ci
// says that the result of ci_before is stale, with the files and tests to hand in again; the engineer hands them in
// as given, and its test results are not sent back. Nothing is told of the first call, which asked for ci and ci_before
// for the same head.
#[test]
fn ci_tells_a_result_of_ci_before_gone_stale_and_the_engineer_is_not_sent_back() {
    let (s, e, said, item, work) = run("stale-before-note", Rerun::FromNote);
    let log = said.lock().unwrap().join("\n");
    let calls = tool_results(&e, item);
    assert_eq!(calls.len(), 3, "{calls:?}\n{log}");
    // The first call's ci and ci_before are of one head: nothing is stale.
    assert_eq!(calls[0][0]["tool"], "ci", "{calls:?}");
    assert_eq!(calls[0][0]["push"]["ci_before_stale"], Value::Null, "{calls:?}");
    // The second call pushed the changed test file: the first call's ci_before is stale, for its test and its file.
    assert_eq!(
        calls[1][0]["push"]["ci_before_stale"],
        json!([{ "tests": ["feature"], "changed": [TEST_FILE], "files": [TEST_FILE] }]),
        "{calls:?}\n{log}"
    );
    // The third call asked again as the note said, and ran.
    assert_eq!(calls[2][0]["tool"], "ci_before", "{calls:?}");
    assert_eq!(calls[2][0]["ran"]["verdict"], "ran", "{calls:?}\n{log}");
    // The return that followed was not sent back.
    assert_eq!(sent_back(&e), 0, "{:?}\n{log}", why_sent_back(&e, work, "I1"));
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #311: the engineer who asks for ci_before in the same call as the ci that pushes the change gets the new result
// and no note: the run of that call is the latest to report the test, so nothing is stale. This is what the note would
// have asked for, and it holds only if the note is made once every run of the call is recorded.
#[test]
fn a_ci_before_in_the_call_of_the_ci_that_pushes_the_change_leaves_nothing_stale() {
    let (s, e, said, item, work) = run("stale-before-same-call", Rerun::WithCi);
    let log = said.lock().unwrap().join("\n");
    let calls = tool_results(&e, item);
    assert_eq!(calls.len(), 2, "{calls:?}\n{log}");
    assert_eq!(calls[1][0]["tool"], "ci", "{calls:?}");
    assert_eq!(calls[1][1]["ran"]["verdict"], "ran", "{calls:?}\n{log}");
    assert_eq!(calls[1][0]["push"]["ci_before_stale"], Value::Null, "{calls:?}");
    assert_eq!(sent_back(&e), 0, "{:?}\n{log}", why_sent_back(&e, work, "I1"));
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #311: the check of a return is not relaxed: an engineer who hands in its test results with the result of
// ci_before stale, whatever it was told, is sent back with the test and the file that changed, and goes on once it has
// run ci_before again.
#[test]
fn test_results_that_rest_on_a_stale_result_of_ci_before_are_still_sent_back() {
    let (s, e, said, _, work) = run("stale-before-sent-back", Rerun::Late);
    let log = said.lock().unwrap().join("\n");
    let why = why_sent_back(&e, work, "I1");
    assert_eq!(why.len(), 1, "{why:?}\n{log}");
    assert!(
        why[0].contains("feature: its result before the change comes from")
            && why[0].contains(&format!("{TEST_FILE} changed after it"))
            && why[0].contains("run ci_before again"),
        "{why:?}"
    );
    assert_eq!(sent_back(&e), 1, "{why:?}\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
