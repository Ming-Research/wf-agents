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

// A version shows whether it holds nothing as its write recorded it, which a test of content reads, so that whoever
// shows the work reads that as the walk does and decides it by no rule of its own.
#[test]
fn a_version_that_holds_nothing_shows_so_as_its_write_recorded_it() {
    let mut w = World::with_gate(vec![cell("check", &["main", "notes"], &["notes"])]);
    w.outside_writes("main");
    let at = w.run("check", &[("notes", Nothing)]);
    let run = finished_by(&w, at);
    let nothing = VersionView {
        empty: true,
        ruled: false,
        ..version(at, Writer::Run(run), at, None)
    };
    assert_eq!(seen(&w).blocks[&BlockId::from("notes")], vec![nothing]);
}

#[test]
fn a_choice_shows_its_value() {
    let mut w = World::with_gate(vec![cell("ask", &["brief"], &["more"])]);
    w.person_writes("brief");
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
    let meanings: Vec<(Seq, Option<Seq>)> = (seen(&w).blocks[&BlockId::from("plan")].iter())
        .map(|v| (v.at, v.meaning))
        .collect();
    assert_eq!(meanings, vec![(p1, Some(p1)), (p2, Some(p1)), (p3, Some(p1))]);
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

// A version is numbered among the versions of its datum alone, counted from one in the order they were written,
// whatever else the work wrote between them, and is found again by its number; a group a cell takes as one is numbered
// as one datum, each place that wrote any of its blocks one version of it (decision
// a-version-is-numbered-within-its-datum).
#[test]
fn a_version_is_numbered_among_its_datum_s_versions_and_found_again_by_its_number() {
    let mut w = planned();
    let b1 = w.person_writes("background");
    let p1 = w.run("research", &[("plan", Matters)]);
    let c1 = w.run("implement", &[("code", Matters)]);
    let b2 = w.person_writes("background");
    let p2 = w.run("research", &[("plan", Matters)]);
    assert!(p2.0 > p1.0 + 2, "other entries stand between the plan's two versions");
    let r = &w.records;
    let plan = BlockId::from("plan");
    assert_eq!((r.number([&plan], p1), r.number([&plan], p2)), (Some(1), Some(2)));
    assert_eq!(r.number([&plan], b2), None, "no version of the plan was written there");
    assert_eq!(
        (
            r.place([&plan], 1),
            r.place([&plan], 2),
            r.place([&plan], 3),
            r.place([&plan], 0)
        ),
        (Some(p1), Some(p2), None, None)
    );
    assert_eq!(r.next_number([&plan]), 3);
    let both = [BlockId::from("background"), plan.clone()];
    let numbers: Vec<Option<u64>> = [b1, p1, c1, b2, p2].iter().map(|at| r.number(&both, *at)).collect();
    assert_eq!(numbers, [Some(1), Some(2), None, Some(3), Some(4)]);
}
