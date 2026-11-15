//! A stopped driver: once its store failed, every later call is refused as stopped with nothing kept, it shows nothing,
//! and it says why; what it kept before is given again once it is opened anew.

use super::*;

/// The fault of the store of the tests, as the driver gives it.
fn fault() -> Fault {
    Fault::Store(failure())
}

/// Every call of a stopped driver is refused, keeping nothing, and it shows nothing.
fn refuses_everything(h: &mut H, work: WorkNumber) {
    let stopped = Failed::Stopped {
        fault: fault(),
        kept: false,
    };
    let before = dump(&h.db.path);
    let now = h.now();
    let d = h.dm();
    assert_eq!(d.happened(now, work, Event::Look), Err(stopped.clone()));
    assert_eq!(d.happened(now, work, Event::Resumed), Err(stopped.clone()));
    assert_eq!(d.happened(now, WorkNumber(99), Event::Look), Err(stopped.clone()));
    let bad = Event::LetGo { cell: "nowhere".into() };
    assert_eq!(d.happened(now, work, bad), Err(stopped.clone()));
    assert_eq!(d.add(now, new_item("x", &[], "judged")), Err(stopped.clone()));
    assert_eq!(d.add(now, new_item("x", &[], "nowhere")), Err(stopped));
    assert_eq!(d.fault(), Some(&fault()));
    assert_eq!(d.pending(), vec![]);
    assert_eq!(d.works().count(), 0);
    assert!(d.work(work).is_none());
    assert!(d.view(work).is_none());
    assert_eq!(dump(&h.db.path), before, "a stopped driver changed the store");
}

#[test]
fn a_save_that_fails_stops_the_driver_for_that_call_and_every_later_one() {
    let mut h = H::new();
    let (_, w) = h.add(new_item("a", &[], "judged"));
    h.let_on(w);
    let (run, ..) = h.round(w);
    h.arm(Plan::Fail {
        op: Some(Op::Save),
        n: 1,
        on: false,
    });
    let now = h.now();
    let spent = h.spent();
    let came = hands_in(&[("text", None)]);
    let got = h.dm().happened(now, w, Event::Returned { run, came, spent });
    assert_eq!(
        got,
        Err(Failed::Stopped {
            fault: fault(),
            kept: false
        })
    );
    // The store would no longer fail; the driver is stopped all the same.
    h.arm(Plan::Never);
    refuses_everything(&mut h, w);
    // Opened anew, it gives again the round whose end was not kept.
    let todos = h.reopen();
    assert_eq!(rounds(&todos), [(w, "write".to_string(), 1)]);
}

#[test]
fn an_add_that_fails_stops_the_driver_and_keeps_nothing() {
    let mut h = H::new();
    let (_, w) = h.add(new_item("a", &[], "judged"));
    h.arm(Plan::Fail {
        op: Some(Op::Add),
        n: 1,
        on: false,
    });
    let now = h.now();
    let got = h.dm().add(now, new_item("b", &[], "short"));
    assert_eq!(
        got,
        Err(Failed::Stopped {
            fault: fault(),
            kept: false
        })
    );
    refuses_everything(&mut h, w);
    h.reopen();
    assert_eq!(h.d().works().count(), 1);
}

#[test]
fn a_store_that_fails_from_some_call_on_stops_the_driver_at_the_first() {
    let mut h = H::new();
    let (_, w) = h.add(new_item("a", &[], "judged"));
    h.arm(Plan::Fail {
        op: None,
        n: 1,
        on: true,
    });
    let now = h.now();
    let got = h.dm().happened(now, w, Event::Resumed);
    assert_eq!(
        got,
        Err(Failed::Stopped {
            fault: fault(),
            kept: false
        })
    );
    refuses_everything(&mut h, w);
    // What was not kept did not happen: opened anew, the work is still paused, and was never started.
    h.reopen();
    assert!(decide::paused(&h.d().work(w).expect("the work is there").work.records));
    assert!(matches!(h.decision(w), Decision::Stalled { .. }), "{:?}", h.decision(w));
    assert_eq!(h.d().pending(), vec![]);
}
