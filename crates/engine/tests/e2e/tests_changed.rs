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
