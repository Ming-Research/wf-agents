//! What adversarial testing is given of the tests the engineer changed.

use super::*;

// Item #292: adversarial testing is given the project's test paths, as its configuration writes them, and the tests the
// engineer changed with the test files the work changed, from the engine; it is no longer given the added and extended
// test changes of the latest return of each cell.
#[test]
fn adversarial_testing_is_given_the_test_paths_and_the_tests_the_engineer_changed() {
    let s = setup("tests-changed");
    let mut agents = scripts();
    agents.insert("T3", adversarial());
    // Implementing changes a test file besides the source, in the commit it asks for ci with.
    let implementing = scripts().remove("I1").unwrap();
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            write(&r.worktree, "tests/new_test.txt", "a test\n");
            implementing(r, n)
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "T1");
    let (work, _, _) = state(&e, item).unwrap();
    e.step_work(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    let m = message_at(&e, work, "T1", false);
    // The test paths of the project, whole: every cell of the line reads the setting, so the line's session is given it
    // in its system prompt, and the round lists it among its inputs by its address.
    let flow = e.flow(e.flow_now).unwrap();
    let g = flow.graphs.iter().find(|g| g.name == engine::ENGINEER).unwrap();
    let parts = engine::message::line_parts(&e, g, g.cell("T1").unwrap(), "p");
    let paths = parts.iter().find(|(address, _)| address == "config:test_paths");
    assert_eq!(paths.map(|(_, v)| v), Some(&json!(["tests/"])), "{parts:?}");
    // The tests the engineer changed: the engineer wrote no row, and the work changed the one test file.
    let changed = given_value(&m, "\u{b7}engine\u{b7}tests_changed").expect(&m);
    assert_eq!(changed["tests"], json!([]), "{changed}");
    assert_eq!(changed["weakened"], json!([]), "{changed}");
    assert_eq!(changed["test_files"], json!(["tests/new_test.txt"]), "{changed}");
    // Neither input reads the test changes of the latest return of a cell any longer.
    let from = work_from(&m);
    assert!(from.iter().any(|l| l.ends_with(". config:test_paths")), "{m}\n{log}");
    assert!(from.iter().all(|l| !l.contains("test_changes[")), "{m}\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The default implementing script, writing the test file `file` besides the source, and handing in `rows` as its test
/// changes in the round that ends: the round that asks for ci hands in none.
fn implementing_with(file: &'static str, rows: Value) -> Script {
    let base = scripts().remove("I1").unwrap();
    Box::new(move |r: &Request, n: usize| {
        write(&r.worktree, file, "a test\n");
        let (pick, mut set) = base(r, n);
        if !pick
            .iter()
            .any(|(field, value)| *field == "outcome" && *value == "tools")
        {
            set.push(("outcome.test_changes", rows.clone()));
        }
        (pick, set)
    })
}

/// The name of the last round of the work at `cell`.
fn last_round(e: &Engine, work: i64, cell: &str) -> String {
    let n: i64 =
        e.db.conn()
            .query_row(
                "SELECT max(n) FROM rounds WHERE work = ?1 AND cell = ?2",
                db::rusqlite::params![work, cell],
                |r| r.get(0),
            )
            .unwrap();
    format!("R{work}-{n}")
}

// Item #292: the test changes of a real return of implementing, as the engine stored it, give the tests the engineer
// changed in the order the rows were written: a test removed and added again is a test again with no removal left, a
// test added and removed is gone from the tests and said among those weakened, and a skipped, relaxed or removed test
// the engineer never added is only said. The reference is the order of the rows in the return and the meaning of the
// five kinds of change the flow gives them.
#[test]
fn the_rows_of_a_stored_return_make_the_tests_and_the_weakened_tests_in_the_order_written() {
    let s = setup("tests-changed-rows");
    let new = |test: &str| json!({ "change": "added", "test": test, "catches": "c", "kind": "new_behaviour" });
    let reason = |change: &str, test: &str| json!({ "change": change, "test": test, "reason": "technical" });
    let rows = json!([
        new("alpha"),
        { "change": "added", "test": "beta", "catches": "c", "kind": "old_behaviour" },
        reason("removed", "beta"),
        { "change": "extended", "test": "gamma", "catches": "more", "kind": "new_behaviour" },
        reason("skipped", "gamma"),
        reason("relaxed", "delta"),
        reason("removed", "alpha"),
        new("alpha"),
    ]);
    let mut agents = scripts();
    agents.insert("T3", adversarial());
    agents.insert("I1", implementing_with("tests/rows_test.txt", rows));
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "T1");
    let (work, _, _) = state(&e, item).unwrap();
    e.step_work(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    let m = message_at(&e, work, "T1", false);
    let changed = given_value(&m, "\u{b7}engine\u{b7}tests_changed").expect(&m);
    let round = last_round(&e, work, "I1");
    let tests: Vec<(&str, &str, &str)> = changed["tests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["test"].as_str().unwrap(),
                r["change"].as_str().unwrap(),
                r["round"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        tests,
        [
            ("gamma", "extended", round.as_str()),
            ("alpha", "added", round.as_str())
        ],
        "{changed}\n{log}"
    );
    let weakened: Vec<(&str, &str)> = changed["weakened"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["change"].as_str().unwrap(), r["test"].as_str().unwrap()))
        .collect();
    assert_eq!(
        weakened,
        [("removed", "beta"), ("skipped", "gamma"), ("relaxed", "delta")],
        "{changed}\n{log}"
    );
    assert_eq!(changed["weakened"][0]["reason"], "technical", "{changed}");
    assert_eq!(changed["tests"][1]["kind"], "new_behaviour", "{changed}");
    assert_eq!(changed["test_files"], json!(["tests/rows_test.txt"]), "{changed}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #292: a project whose test paths are an empty list is told so (an empty list is a setting with nothing in it, not
// a missing one), and with no row and no test file under no path there is nothing to give of the tests the engineer
// changed: the input is left out, not given empty. The reference is the project's configuration, which says no path.
#[test]
fn a_project_with_no_test_paths_is_given_an_empty_list_and_no_tests_changed() {
    let s = setup("tests-changed-no-paths");
    let file = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    assert!(file.contains("test_paths = [\"tests/\"]"), "{file}");
    write(
        &s.data,
        "projects/p.toml",
        &file.replacen("test_paths = [\"tests/\"]", "test_paths = []", 1),
    );
    git(&s.data, &["commit", "-q", "-am", "no test paths"]);
    let mut agents = scripts();
    agents.insert("T3", adversarial());
    agents.insert("I1", implementing_with("tests/new_test.txt", json!([])));
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "T1");
    let (work, _, _) = state(&e, item).unwrap();
    e.step_work(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    let m = message_at(&e, work, "T1", false);
    let flow = e.flow(e.flow_now).unwrap();
    let g = flow.graphs.iter().find(|g| g.name == engine::ENGINEER).unwrap();
    let parts = engine::message::line_parts(&e, g, g.cell("T1").unwrap(), "p");
    let paths = parts.iter().find(|(address, _)| address == "config:test_paths");
    assert_eq!(paths.map(|(_, v)| v), Some(&json!([])), "{parts:?}");
    assert!(
        work_from(&m).iter().any(|l| l.ends_with(". config:test_paths")),
        "{m}\n{log}"
    );
    assert_eq!(given_value(&m, "\u{b7}engine\u{b7}tests_changed"), None, "{m}\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
