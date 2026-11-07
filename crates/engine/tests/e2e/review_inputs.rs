//! What the reviews are given of the CI on the head they review.

use super::*;

// The first review is given the CI on the head it reviews: each run of ci on the head the work last pushed, as the
// tool recorded what it reported, so that it does not look the CI up. The details review goes on in the same session
// and is referred to the same input, by its address, not given it again. Takes about a second, as the other works of
// these tests that run through the reviews.
#[test]
fn the_reviews_are_given_the_ci_on_the_head_they_review() {
    let s = setup("head-ci");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "T3");
    let (work, _, _) = state(&e, item).unwrap();
    e.step_work(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    let head: String =
        e.db.conn()
            .query_row("SELECT pushed_head FROM works WHERE id = ?1", [work], |r| r.get(0))
            .unwrap();
    // What each run of ci on the pushed head reported, newest first, from the tool's own records.
    let mut runs: Vec<Value> = tool_runs(&e, item, "ci")
        .into_iter()
        .map(|(_, result, _)| result["push"].clone())
        .filter(|p| p["head"] == json!(head) && p["run"] != "none")
        .collect();
    runs.reverse();
    assert!(!runs.is_empty(), "{log}");
    let m = message_at(&e, work, "T2", false);
    let ci = given_value(&m, "\u{b7}engine\u{b7}head_ci").expect(&m);
    assert_eq!(ci, Value::Array(runs), "{m}\n{log}");
    assert_eq!(ci[0]["tests"][0]["test"], "feature", "{ci}");
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
