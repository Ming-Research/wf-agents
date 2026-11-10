//! One thing at a time: what `happened` gives for the work it is told of, what it refuses leaving the driver and the
//! store as they were, a work that is not there, a round under way that what arrives meanwhile does not interrupt, and
//! a pause that holds back only what would begin a round.

use decide::run::Unmet;

use super::*;

fn round_todo(work: WorkNumber, run: RunId, cell: &str, number: usize, given: Given) -> Todo {
    Todo {
        work,
        effect: Effect::Round {
            run,
            cell: cell.into(),
            number,
            given,
        },
    }
}

/// The one call given, and its run.
fn one_call(todos: &[Todo]) -> (CallId, RunId) {
    match todos {
        [
            Todo {
                effect: Effect::Call { call, run, .. },
                ..
            },
        ] => (*call, *run),
        other => panic!("not one call: {other:?}"),
    }
}

/// A work of the judged flow, let on, its writer's first round under way.
fn started() -> (H, WorkNumber) {
    let mut h = H::new();
    let (_, w) = h.add(new_item("a", &[], "judged"));
    h.let_on(w);
    (h, w)
}

#[test]
fn what_happened_to_a_work_gives_what_is_to_be_done_for_it_next() {
    let mut h = H::new();
    let (_, w) = h.add(new_item("a", &[], "judged"));
    let todos = h.let_on(w);
    let (write, ..) = h.round(w);
    assert_eq!(todos, vec![round_todo(w, write, "write", 1, Given::Task)]);
    // The writer asks for a tool: the call is made, and the work waits for the engine.
    let todos = h.returns(w, Came::Asks(vec![tool("search")]));
    let (call, run) = one_call(&todos);
    assert_eq!(run, write);
    assert_eq!(
        todos,
        vec![Todo {
            work: w,
            effect: Effect::Call {
                call,
                run: write,
                what: Call::Asked(tool("search")),
            },
        }]
    );
    assert_eq!(
        h.decision(w),
        Decision::Wait {
            at: "write".into(),
            on: vec![Waited::Call { call, on: None }],
        }
    );
    // The call has its result: the next round is given it.
    let todos = h.answer(w, call);
    assert_eq!(
        todos,
        vec![round_todo(w, write, "write", 2, Given::Results(vec![call]))]
    );
    // It hands in what it may not write: it is sent back, saying why.
    let todos = h.hands_in(w, &[("verdict", Some("yes"))]);
    let unmet = vec![Unmet::NotAnOutput {
        block: block("verdict"),
    }];
    assert_eq!(todos, vec![round_todo(w, write, "write", 3, Given::SentBack(unmet))]);
    // It hands in what the flow asks: it finishes, and the judge runs.
    let todos = h.hands_in(w, &[("text", None)]);
    let (judge, ..) = h.round(w);
    assert_ne!(judge, write);
    assert_eq!(todos, vec![round_todo(w, judge, "judge", 1, Given::Task)]);
    // The judge asks the owner: the work waits for the owner.
    let todos = h.returns(w, Came::Asks(vec![question(Who::Owner)]));
    let (call, _) = one_call(&todos);
    assert_eq!(
        h.decision(w),
        Decision::Wait {
            at: "judge".into(),
            on: vec![Waited::Call {
                call,
                on: Some(Who::Owner),
            }],
        }
    );
    let todos = h.answer(w, call);
    assert_eq!(
        todos,
        vec![round_todo(w, judge, "judge", 2, Given::Results(vec![call]))]
    );
    let todos = h.hands_in(w, &[("verdict", Some("yes"))]);
    let (merge, ..) = h.round(w);
    assert_eq!(todos, vec![round_todo(w, merge, "merge", 1, Given::Task)]);
    // The merge's act is done: the work has merged, and nothing more is to be done.
    let todos = h.hands_in(w, &[("merged", None)]);
    assert_eq!(todos, vec![]);
    assert_eq!(h.decision(w), Decision::Ended(End::Merged));
    assert_eq!(h.d().pending(), vec![]);
    // A look at a work that has ended gives nothing.
    assert_eq!(h.on(w, Event::Look), vec![]);
}

#[test]
fn what_cannot_have_happened_to_a_work_is_refused_and_leaves_the_driver_and_the_store_as_they_were() {
    let (mut h, w) = started();
    let (write, ..) = h.round(w);
    let refusals: Vec<(Event, engine::Refused)> = vec![
        (
            wrote("nowhere", Outsider::Person, None),
            engine::Refused::NoSuchBlock(block("nowhere")),
        ),
        (
            wrote("text", Outsider::Person, None),
            engine::Refused::NotTheirs {
                block: block("text"),
                by: Outsider::Person,
            },
        ),
        (
            wrote("verdict", Outsider::Person, Some("maybe")),
            engine::Refused::Unmet(Unmet::NotAValue {
                block: block("verdict"),
                value: "maybe".into(),
            }),
        ),
        (
            Event::LetGo { cell: "nowhere".into() },
            engine::Refused::NoSuchCell("nowhere".into()),
        ),
        (
            Event::Done {
                call: CallId(99),
                result: "late".into(),
            },
            engine::Refused::NoSuchCall(CallId(99)),
        ),
        (
            Event::Returned {
                run: RunId(99),
                came: Came::Asks(vec![]),
                spent: Spent::default(),
            },
            engine::Refused::NoRoundUnderWay(RunId(99)),
        ),
    ];
    for (event, why) in refusals {
        let got = h.refused(|d, now| d.happened(now, w, event).map(drop));
        assert_eq!(got, Failed::Refused(why));
    }
    // A call that has its result already.
    let todos = h.returns(w, Came::Asks(vec![tool("search")]));
    let (call, _) = one_call(&todos);
    h.answer(w, call);
    let got = h.refused(|d, now| {
        let result = "again".to_string();
        d.happened(now, w, Event::Done { call, result }).map(drop)
    });
    assert_eq!(got, Failed::Refused(engine::Refused::AlreadyDone(call)));
    // The work goes on as it was.
    assert_eq!(h.round(w).0, write);
}

#[test]
fn what_happened_to_a_work_that_is_not_there_is_refused_and_changes_nothing() {
    let (mut h, _) = started();
    let none = WorkNumber(2);
    let got = h.refused(|d, now| d.happened(now, none, Event::Look).map(drop));
    assert_eq!(got, Failed::NoSuchWork(none));
    let got = h.refused(|d, now| d.happened(now, none, Event::Cancelled).map(drop));
    assert_eq!(got, Failed::NoSuchWork(none));
}

#[test]
fn a_round_under_way_is_not_interrupted_and_what_arrived_meanwhile_decides_as_it_ends() {
    let (mut h, w) = started();
    let (first, ..) = h.round(w);
    // A person writes the request the writer reads while its round is under way: nothing is decided.
    let todos = h.on(w, wrote("request", Outsider::Person, None));
    assert_eq!(todos, vec![]);
    let request = last_written(&h, w);
    assert_eq!(h.round(w).0, first);
    // As the round ends its run is stale: it is abandoned rather than finished, and the writer runs again from the
    // request.
    let todos = h.hands_in(w, &[("text", None)]);
    let (second, ..) = h.round(w);
    assert_ne!(second, first);
    assert_eq!(todos, vec![round_todo(w, second, "write", 1, Given::Task)]);
    let kept = h.d().work(w).expect("the work is there");
    let records = &kept.work.records;
    assert!(
        records.finished.is_empty(),
        "a stale run finished: {:?}",
        records.finished
    );
    assert!(
        records.abandoned.iter().any(|x| x.run == first),
        "the stale run was not abandoned"
    );
    let started = (records.started.iter()).find(|s| s.run == second).expect("started");
    assert_eq!(started.basis.get(&block("request")), Some(&Some(request)));
}

#[test]
fn a_pause_holds_back_only_what_would_begin_a_round() {
    let (mut h, w) = started();
    let (write, ..) = h.round(w);
    assert_eq!(
        h.on(
            w,
            Event::Paused {
                by: Pauser::Owner,
                why: None
            }
        ),
        vec![]
    );
    // What the round asks for while the work is paused is still taken, and the call is made.
    let todos = h.returns(w, Came::Asks(vec![tool("search")]));
    let (call, run) = one_call(&todos);
    assert_eq!(run, write);
    // Its result is taken; the next round waits for the work to be let on.
    assert_eq!(h.answer(w, call), vec![]);
    let todos = h.let_on(w);
    assert_eq!(
        todos,
        vec![round_todo(w, write, "write", 2, Given::Results(vec![call]))]
    );
    // What the round hands in while the work is paused is taken: the run finishes, and the next cell waits.
    assert_eq!(
        h.on(
            w,
            Event::Paused {
                by: Pauser::Owner,
                why: None
            }
        ),
        vec![]
    );
    assert_eq!(h.hands_in(w, &[("text", None)]), vec![]);
    let kept = h.d().work(w).expect("the work is there");
    assert!(
        kept.work.records.finished.iter().any(|f| f.run == write),
        "the run did not finish"
    );
    assert_eq!(h.decision(w), Decision::Paused);
    let todos = h.let_on(w);
    assert_eq!(rounds(&todos), [(w, "judge".to_string(), 1)]);
}

#[test]
fn a_cancel_stops_the_round_under_way_and_nothing_of_the_work_goes_on() {
    let (mut h, w) = started();
    let (run, ..) = h.round(w);
    let todos = h.on(w, Event::Cancelled);
    assert_eq!(
        todos,
        vec![Todo {
            work: w,
            effect: Effect::Stop { run },
        }]
    );
    assert_eq!(h.decision(w), Decision::Ended(End::Cancelled));
    // The end of the stopped round gives nothing more, and nor does a person's write.
    let spent = h.spent();
    let came = Came::Asks(vec![]);
    assert_eq!(h.on(w, Event::Returned { run, came, spent }), vec![]);
    assert_eq!(h.on(w, wrote("request", Outsider::Person, None)), vec![]);
    assert_eq!(h.d().pending(), vec![]);
}
