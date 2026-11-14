//! The records as they stood before a place: what was decided then is decided again from them, so they hold every
//! entry of every kind at an earlier place, and none at it or after.

use std::collections::BTreeSet;

use decide::{Answered, Called, Finished, LetGo, Outsider, Pause, Questioned, Records, Started};

use super::*;

/// Records with an entry of every kind, each at a place of its own, from 1 to [`LAST`], as a work can have them: a run
/// that made a call, had its result, and in its next round was told of the owner's words, written while it was under
/// way, then finished naming what it dealt with; the outside's write; one run abandoned; a cell let go; a pause and a
/// letting on; a cancel; a question the finished run asked and the owner's answer to it; the release of what the work
/// held; and a reopen.
fn every_kind() -> Records {
    let (a, b) = (RunId(1), RunId(2));
    Records {
        started: vec![
            Started {
                run: a,
                cell: "a".into(),
                at: Seq(2),
                because: vec![Cause::NoRecord],
                basis: [("p".into(), Some(Seq(1)))].into(),
            },
            Started {
                run: b,
                cell: "b".into(),
                at: Seq(13),
                because: vec![Cause::NoRecord],
                basis: BTreeMap::new(),
            },
        ],
        rounds: vec![Round { run: a, at: Seq(3) }, Round { run: a, at: Seq(7) }],
        returned: vec![Returned { run: a, at: Seq(4) }, Returned { run: a, at: Seq(10) }],
        called: vec![Called {
            call: CallId(3),
            run: a,
            on: None,
            at: Seq(5),
        }],
        done: vec![Done {
            call: CallId(3),
            at: Seq(6),
        }],
        finished: vec![Finished {
            run: a,
            at: Seq(11),
            writes: vec![Write {
                block: "q".into(),
                meaning_unchanged: true,
                value: None,
            }],
            dealt_with: Some(vec!["p".into(), "words".into()]),
        }],
        abandoned: vec![Abandoned { run: b, at: Seq(14) }],
        put_in: vec![PutIn {
            run: a,
            block: "words".into(),
            version: Seq(8),
            at: Seq(9),
        }],
        written: vec![
            Written {
                block: "p".into(),
                by: Outsider::Person,
                matters: true,
                value: None,
                at: Seq(1),
            },
            Written {
                block: "words".into(),
                by: Outsider::Person,
                matters: true,
                value: None,
                at: Seq(8),
            },
            Written {
                block: "main".into(),
                by: Outsider::Outside,
                matters: true,
                value: None,
                at: Seq(12),
            },
        ],
        let_go: vec![LetGo {
            cell: "b".into(),
            at: Seq(15),
        }],
        pauses: vec![
            Pause {
                paused: true,
                at: Seq(16),
                by: Pauser::Owner,
                why: None,
            },
            Pause {
                paused: false,
                at: Seq(17),
                by: Pauser::Owner,
                why: None,
            },
        ],
        cancels: vec![Seq(18)],
        questions: vec![Questioned {
            name: "way".to_string(),
            cell: "a".into(),
            run: a,
            engine: false,
            waits: false,
            withdrawn: false,
            at: Seq(19),
        }],
        answers: vec![Answered {
            question: "way".to_string(),
            version: Seq(19),
            option: 2,
            why: Some("The other way.".to_string()),
            at: Seq(20),
        }],
        releases: vec![Seq(21)],
        reopens: vec![Seq(LAST)],
    }
}

/// The place of the last entry of [`every_kind`].
const LAST: u64 = 22;

/// The place of every entry of the records, with the list that holds it.
fn places(r: &Records) -> BTreeSet<(&'static str, Seq)> {
    fn of<T>(name: &'static str, list: &[T], at: impl Fn(&T) -> Seq) -> impl Iterator<Item = (&'static str, Seq)> {
        list.iter().map(move |e| (name, at(e)))
    }
    (of("started", &r.started, |e| e.at))
        .chain(of("rounds", &r.rounds, |e| e.at))
        .chain(of("returned", &r.returned, |e| e.at))
        .chain(of("called", &r.called, |e| e.at))
        .chain(of("done", &r.done, |e| e.at))
        .chain(of("finished", &r.finished, |e| e.at))
        .chain(of("abandoned", &r.abandoned, |e| e.at))
        .chain(of("put_in", &r.put_in, |e| e.at))
        .chain(of("written", &r.written, |e| e.at))
        .chain(of("questions", &r.questions, |e| e.at))
        .chain(of("answers", &r.answers, |e| e.at))
        .chain(of("let_go", &r.let_go, |e| e.at))
        .chain(of("pauses", &r.pauses, |e| e.at))
        .chain(of("cancels", &r.cancels, |at| *at))
        .chain(of("reopens", &r.reopens, |at| *at))
        .chain(of("releases", &r.releases, |at| *at))
        .collect()
}

#[test]
fn the_records_before_a_place_hold_every_entry_of_every_kind_at_an_earlier_place_and_none_at_it_or_after() {
    let r = every_kind();
    // They are records a work can have.
    let mut w = World::with_gate(vec![cell("a", &["p", "words"], &["q"]), cell("b", &[], &[])]);
    w.records = r.clone();
    assert!(!matches!(w.decide(), Decision::Fault(_)), "{:?}", w.decide());
    let all = places(&r);
    assert_eq!(
        all.iter().map(|(_, at)| at.0).collect::<BTreeSet<_>>(),
        (1..=LAST).collect(),
        "each place holds one entry"
    );
    // Before the first place nothing was recorded; after the last, everything was, as it is.
    assert_eq!(r.before(Seq(1)), Records::default());
    assert_eq!(r.before(Seq(LAST + 1)), r);
    for place in 1..=LAST + 1 {
        let before = r.before(Seq(place));
        let expected: BTreeSet<_> = all.iter().filter(|(_, at)| at.0 < place).copied().collect();
        assert_eq!(places(&before), expected, "before place {place}");
        // What is kept of an entry is the entry as it was recorded.
        assert!(before.finished.iter().all(|f| r.finished.contains(f)));
        assert!(before.put_in.iter().all(|d| r.put_in.contains(d)));
    }
}
