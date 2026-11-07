//! What the reviews are given of the CI on the head they review.

use super::*;

/// Implementing that runs ci three times before it hands in: once on a first commit, then twice on the commit it hands
/// in, the second time naming one test more, so that each run on the head it hands in can be told by what it named.
fn implementing_with_three_runs() -> Script {
    Box::new(|r: &Request, n: usize| {
        let wt = &r.worktree;
        let ci = |tests: &[&str]| {
            let head = git(wt, &["rev-parse", "HEAD"]);
            let tools = json!([{ "tool": "ci", "head": head, "tests": tests, "pr": { "title": "Add the feature", "body": "It adds it." } }]);
            (vec![("outcome", "tools")], vec![("outcome.tools", tools)])
        };
        match n {
            1 => {
                write(wt, "src/lib.txt", "old\nfeature\n");
                git(wt, &["add", "-A"]);
                git(wt, &["commit", "-q", "-m", "Add the feature"]);
                ci(&["feature"])
            }
            2 => {
                write(wt, "src/more.txt", "more\n");
                git(wt, &["add", "-A"]);
                git(wt, &["commit", "-q", "-m", "Add more of the feature"]);
                ci(&["feature"])
            }
            3 => ci(&["feature", "regression"]),
            _ => {
                let head = git(wt, &["rev-parse", "HEAD"]);
                (
                    vec![("outcome.departs", "no")],
                    vec![
                        ("outcome.head", json!(head)),
                        (
                            "outcome.conditions",
                            json!([{ "how": "tested", "id": "A1", "tests": ["feature"] }]),
                        ),
                    ],
                )
            }
        }
    })
}

// The first review is given the CI on the head it reviews: implementing ran ci once on a first commit and twice on the
// head it handed in, and the review is given the two runs on that head, the later first, each with the tests it named,
// and not the run on the commit before. The details review goes on in the same session and is referred to the same
// input, by its address, not given it again. It takes about a second on the fast CI, as the other works of these tests
// that run to the reviews, and is the one test of the input `head_ci`.
#[test]
fn the_reviews_are_given_the_ci_on_the_head_they_review() {
    let s = setup("head-ci");
    let mut agents = scripts();
    agents.insert("I1", implementing_with_three_runs());
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "T3");
    let (work, _, _) = state(&e, item).unwrap();
    e.step_work(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    let head: String =
        e.db.conn()
            .query_row("SELECT pushed_head FROM works WHERE id = ?1", [work], |r| r.get(0))
            .unwrap();
    assert_eq!(tool_runs(&e, item, "ci").len(), 3, "{log}");
    let m = message_at(&e, work, "T2", false);
    let ci = given_value(&m, "\u{b7}engine\u{b7}head_ci").expect(&m);
    let runs = ci.as_array().expect(&m);
    assert_eq!(runs.len(), 2, "{ci}\n{log}");
    let named = |run: &Value| -> Vec<String> {
        run["tests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["test"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(named(&runs[0]), ["feature", "regression"], "{ci}");
    assert_eq!(named(&runs[1]), ["feature"], "{ci}");
    assert!(runs.iter().all(|r| r["head"] == json!(head)), "{ci}");
    // The details review names the first review's version of it among what it works from, and is not given it again.
    let address = work_from(&m)
        .iter()
        .find_map(|l| {
            l.split(". ")
                .nth(1)
                .filter(|a| a.ends_with("\u{b7}engine\u{b7}head_ci"))
        })
        .map(str::to_string)
        .expect(&m);
    let details = input_at(&e, work, "T3", false);
    let text = details["message"].as_str().unwrap_or_default();
    assert!(work_from(text).iter().any(|l| l.ends_with(&address)), "{text}");
    assert_eq!(given(&details, &address), Some(false), "{details}");
    let _ = std::fs::remove_dir_all(&s.root);
}
