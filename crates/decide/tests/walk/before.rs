//! The records as they stood before a place: what was decided then is decided again from them, so they hold every
//! entry of every kind at an earlier place, and none at it or after.

use std::collections::BTreeSet;

use decide::{Called, Finished, LetGo, Outsider, Pause, Records, Started};

use super::*;

/// Records with an entry of every kind, each at a place of its own, from 1 to [`LAST`]: a run that finished, its
/// return naming what it dealt with, after a version was delivered to it and a call it made had its result; one that
/// was abandoned; a person's write and the outside's; a cell let go; a pause and a letting on; a cancel; and the
/// release of what the work held.
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
                at: Seq(12),
                because: vec![Cause::NoRecord],
                basis: BTreeMap::new(),
            },
        ],
        rounds: vec![Round { run: a, at: Seq(3) }, Round { run: a, at: Seq(7) }],
        returned: vec![Returned { run: a, at: Seq(4) }, Returned { run: a, at: Seq(9) }],
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
            at: Seq(10),
            writes: vec![Write {
                block: "q".into(),
                meaning_unchanged: true,
                value: None,
            }],
            dealt_with: Some(vec!["p".into()]),
        }],
        abandoned: vec![Abandoned { run: b, at: Seq(13) }],
        delivered: vec![Delivered {
            run: a,
            block: "p".into(),
            version: Seq(1),
            at: Seq(8),
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
                block: "main".into(),
                by: Outsider::Outside,
                matters: true,
                value: None,
                at: Seq(11),
            },
        ],
        let_go: vec![LetGo {
            cell: "b".into(),
            at: Seq(14),
        }],
        pauses: vec![
            Pause {
                paused: true,
                at: Seq(15),
                by: Pauser::Owner,
                why: None,
            },
            Pause {
                paused: false,
                at: Seq(16),
                by: Pauser::Owner,
                why: None,
            },
        ],
        cancelled: Some(Seq(17)),
        released: Some(Seq(LAST)),
    }
}

/// The place of the last entry of [`every_kind`].
const LAST: u64 = 18;

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
        .chain(of("delivered", &r.delivered, |e| e.at))
        .chain(of("written", &r.written, |e| e.at))
        .chain(of("let_go", &r.let_go, |e| e.at))
        .chain(of("pauses", &r.pauses, |e| e.at))
        .chain(r.cancelled.map(|at| ("cancelled", at)))
        .chain(r.released.map(|at| ("released", at)))
        .collect()
}

#[test]
fn the_records_before_a_place_hold_every_entry_of_every_kind_at_an_earlier_place_and_none_at_it_or_after() {
    let r = every_kind();
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
        assert!(before.delivered.iter().all(|d| r.delivered.contains(d)));
    }
}
