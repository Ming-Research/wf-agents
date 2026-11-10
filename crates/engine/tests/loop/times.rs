//! When each thing happened, and what each round took: the work keeps both, the time as the driver gives it.

use super::*;

/// Every place the records hold, in no order.
fn places(work: &Work) -> Vec<Seq> {
    let r = &work.records;
    (r.started.iter().map(|e| e.at))
        .chain(r.rounds.iter().map(|e| e.at))
        .chain(r.returned.iter().map(|e| e.at))
        .chain(r.called.iter().map(|e| e.at))
        .chain(r.done.iter().map(|e| e.at))
        .chain(r.finished.iter().map(|e| e.at))
        .chain(r.abandoned.iter().map(|e| e.at))
        .chain(r.written.iter().map(|e| e.at))
        .chain(r.let_go.iter().map(|e| e.at))
        .chain(r.pauses.iter().map(|e| e.at))
        .chain(r.cancelled)
        .collect()
}

#[test]
fn every_entry_one_event_adds_has_that_events_time_and_a_later_events_entries_the_later_time() {
    let mut w = W::new();
    let mut seen: Vec<Seq> = Vec::new();
    let events = vec![
        wrote("request", Outsider::Person, true, None),
        Event::Returned {
            run: RunId(1),
            came: hands_in(&[("text", None)]),
            spent: spent(1),
        },
        wrote("opinions", Outsider::Person, true, None),
    ];
    for event in events {
        let now = Time(w.clock);
        w.on(event);
        let added: Vec<Seq> = places(&w.work).into_iter().filter(|p| !seen.contains(p)).collect();
        assert!(!added.is_empty(), "the event added no entry");
        for place in &added {
            assert_eq!(w.work.time_of(*place), Some(now), "the entry at {place:?}");
        }
        seen.extend(added);
    }
    assert_eq!(w.work.moments.len(), 3);
}

#[test]
fn a_refused_event_adds_no_moment_and_leaves_the_work_as_it_was() {
    let (mut w, ..) = W::requested();
    let moments = w.work.moments.clone();
    w.refused(wrote("text", Outsider::Person, true, None));
    assert_eq!(w.work.moments, moments);
}

#[test]
fn an_event_that_adds_no_entry_adds_no_moment() {
    let (mut w, ..) = W::requested();
    w.on(Event::Cancelled);
    let before = w.work.clone();
    // A second cancel, and a look at a work that has ended, add nothing.
    w.on(Event::Cancelled);
    w.on(Event::Look);
    assert_eq!(w.work, before);
}

#[test]
fn the_time_of_a_place_is_that_of_the_event_whose_entries_it_is_among() {
    let mut w = W::new();
    let first = Time(w.clock);
    w.on(wrote("request", Outsider::Person, true, None));
    let second = Time(w.clock);
    let write = w.work.records.started[0].run;
    w.returns(write, hands_in(&[("text", None)]));
    let finished = w.last_finished();
    let judge = w.work.records.started[1].at;
    assert_eq!(w.work.time_of(Seq(0)), None);
    assert_eq!(w.work.time_of(Seq(1)), Some(first));
    // The finish and the judge's start are among the second event's entries.
    assert_eq!(w.work.time_of(finished), Some(second));
    assert_eq!(w.work.time_of(judge), Some(second));
    assert_eq!(w.work.time_of(w.work.records.written[0].at), Some(first));
}

#[test]
fn what_each_round_took_is_kept_under_its_run_and_number() {
    let (mut w, write, _) = W::requested();
    // Asks for a call, is sent back, then hands in.
    let call = calls_of(&w.returns_spending(write, Came::Asks(vec![tool("a")]), spent(1)))[0].0;
    w.done(call);
    w.returns_spending(write, wanting(), spent(2));
    w.returns_spending(write, hands_in(&[("text", None)]), spent(3));
    let kept: Vec<_> = (w.work.spent.iter())
        .filter(|((run, _), _)| *run == write)
        .map(|(k, v)| (*k, *v))
        .collect();
    assert_eq!(
        kept,
        vec![((write, 1), spent(1)), ((write, 2), spent(2)), ((write, 3), spent(3))]
    );
}

#[test]
fn what_the_stopped_round_of_an_abandoned_run_took_is_kept() {
    let (mut w, write, _) = W::requested();
    w.on(Event::Cancelled);
    w.returns_spending(write, hands_in(&[("text", None)]), spent(5));
    assert_eq!(w.work.spent.get(&(write, 1)), Some(&spent(5)));
}
