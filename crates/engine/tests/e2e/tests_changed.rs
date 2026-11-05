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
    // The test paths of the project, whole.
    assert_eq!(
        given_value(&m, "config:test_paths"),
        Some(json!(["tests/"])),
        "{m}\n{log}"
    );
    // The tests the engineer changed: the engineer wrote no row, and the work changed the one test file.
    let changed = given_value(&m, "\u{b7}engine\u{b7}tests_changed").expect(&m);
    assert_eq!(changed["tests"], json!([]), "{changed}");
    assert_eq!(changed["weakened"], json!([]), "{changed}");
    assert_eq!(changed["test_files"], json!(["tests/new_test.txt"]), "{changed}");
    // Neither input reads the test changes of the latest return of a cell any longer.
    let from = work_from(&m);
    assert!(from.iter().any(|l| l.ends_with(". config:test_paths")), "{m}");
    assert!(from.iter().all(|l| !l.contains("test_changes[")), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}
