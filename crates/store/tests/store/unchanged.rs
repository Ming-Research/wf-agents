//! The versions that hold what the version of their block before them held, which a test of what was confirmed does
//! not ask about again, load back as the engine recorded them: the store tells them from the contents it keeps.

use super::*;

#[test]
fn a_version_written_again_the_same_by_a_person_or_a_run_loads_back_as_unchanged() {
    let mut d = Driver::saved();
    // The driver saves and loads back after each event, and compares.
    let request = |content: &str| Event::Wrote {
        block: "request".into(),
        by: Outsider::Person,
        content: content.to_string(),
        matters: true,
        value: None,
    };
    d.on(request("Greet the user."));
    d.on(request("Greet the user."));
    d.on(request("Greet the user by name."));
    d.end(Came::HandsIn(vec![handed("text", None)]));
    let r = &d.work.records;
    let unchanged: Vec<&str> = r.unchanged.iter().map(|(block, _)| block.0.as_str()).collect();
    assert_eq!(unchanged, ["request"]);
}
