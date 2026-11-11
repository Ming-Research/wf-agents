//! The versions written blank, which the deciding code reads as saying nothing, load back as the engine recorded them:
//! the store tells them from the content it keeps.

use super::*;

#[test]
fn a_version_written_blank_by_a_person_or_a_run_loads_back_as_blank() {
    let mut d = Driver::saved();
    // The driver saves and loads back after each event, and compares.
    d.on(Event::Wrote {
        block: "request".into(),
        by: Outsider::Person,
        content: " \n".to_string(),
        matters: true,
        value: None,
    });
    let mut text = handed("text", None);
    text.content = String::new();
    d.end(Came::HandsIn(vec![text]));
    let r = &d.work.records;
    let blank: Vec<&str> = r.blank.iter().map(|(block, _)| block.0.as_str()).collect();
    assert_eq!(blank, ["request", "text"]);
}
