//! Every version of every block: who wrote it, its meaning, and its value.

use super::*;

#[test]
fn each_version_shows_who_wrote_it_a_person_the_outside_or_a_run() {
    let mut w = World::with_gate(vec![cell("check", &["main", "notes"], &["notes"])]);
    let main = w.outside_writes("main");
    let person = w.person_writes("notes");
    let at = w.run("check", &[("notes", Matters)]);
    let run = finished_by(&w, at);
    let blocks = seen(&w).blocks;
    assert_eq!(
        blocks[&BlockId::from("main")],
        vec![version(main, Writer::Outside, main, None)]
    );
    assert_eq!(
        blocks[&BlockId::from("notes")],
        vec![
            version(person, Writer::Person, person, None),
            version(at, Writer::Run(run), at, None)
        ]
    );
}

#[test]
fn a_choice_shows_its_value() {
    let mut w = World::with_gate(vec![cell("ask", &[], &["more"])]);
    let at = w.run("ask", &[("more", Value("yes"))]);
    let run = finished_by(&w, at);
    let person = w.person_sets("more", "no");
    assert_eq!(
        seen(&w).blocks[&BlockId::from("more")],
        vec![
            version(at, Writer::Run(run), at, Some("yes")),
            version(person, Writer::Person, person, Some("no"))
        ]
    );
}

#[test]
fn a_row_of_writes_said_not_to_matter_keeps_the_meaning_of_the_first() {
    let mut w = planned();
    w.person_writes("background");
    let p1 = w.run("research", &[("plan", Matters)]);
    w.person_writes("background");
    let p2 = w.run("research", &[("plan", DoesNotMatter)]);
    w.person_writes("background");
    let p3 = w.run("research", &[("plan", DoesNotMatter)]);
    let meanings: Vec<(Seq, Seq)> = (seen(&w).blocks[&BlockId::from("plan")].iter())
        .map(|v| (v.at, v.meaning))
        .collect();
    assert_eq!(meanings, vec![(p1, p1), (p2, p1), (p3, p1)]);
}

#[test]
fn a_block_no_cell_reads_is_still_listed_with_its_versions() {
    let mut w = planned();
    let at = w.person_writes("elsewhere");
    assert_eq!(
        seen(&w).blocks[&BlockId::from("elsewhere")],
        vec![version(at, Writer::Person, at, None)]
    );
}
