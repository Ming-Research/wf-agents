//! What each thing that happens does to the records, and what comes back to be done.

use super::*;

#[test]
fn a_persons_write_is_kept_with_its_content_and_the_first_cell_is_given_its_task() {
    let mut w = W::new();
    let effects = w.on(wrote("request", Outsider::Person, true, None));
    let at = w.last_written();
    let written = decide::Written {
        block: "request".into(),
        by: Outsider::Person,
        matters: true,
        value: None,
        at,
    };
    assert_eq!(w.work.records.written, vec![written]);
    let content = BTreeMap::from([(("request".into(), at), "request, as written by Person".to_string())]);
    assert_eq!(w.work.contents, content);
    let run = w.work.records.started[0].run;
    let start = started_with(
        vec![Cause::NoRecord],
        &[
            ("request", Some(at)),
            ("opinions", None),
            ("main", None),
            ("text", None),
        ],
    );
    assert_eq!(effects, vec![round_effect(run, "write", 1, Given::Task)]);
    assert_eq!(start_of(&w, run), start);
}

#[test]
fn a_write_from_outside_is_kept_as_the_outside_wrote_it() {
    let mut w = W::new();
    w.on(wrote("main", Outsider::Outside, false, None));
    let at = w.last_written();
    let written = decide::Written {
        block: "main".into(),
        by: Outsider::Outside,
        matters: false,
        value: None,
        at,
    };
    assert_eq!(w.work.records.written, vec![written]);
    assert_eq!(
        w.work.contents.get(&("main".into(), at)),
        Some(&"main, as written by Outside".to_string())
    );
}

#[test]
fn a_return_that_meets_everything_finishes_the_run_keeps_each_block_under_the_finish_and_gives_the_next_cell() {
    let (mut w, write, _) = W::requested();
    let effects = w.returns(write, hands_in(&[("text", None)]));
    let at = w.last_finished();
    let finished = decide::Finished {
        run: write,
        at,
        writes: vec![decide::Write {
            block: "text".into(),
            matters: true,
            value: None,
        }],
    };
    assert_eq!(w.work.records.finished, vec![finished]);
    assert_eq!(w.work.contents.get(&("text".into(), at)), Some(&"the text".to_string()));
    let judge = w.work.records.started[1].run;
    let start = started_with(
        vec![Cause::NoRecord],
        &[("text", Some(at)), ("opinions", None), ("verdict", None)],
    );
    assert_eq!(effects, vec![round_effect(judge, "judge", 1, Given::Task)]);
    assert_eq!(start_of(&w, judge), start);
}

#[test]
fn a_return_that_does_not_meet_what_is_required_is_sent_back_to_the_same_run_with_exactly_why() {
    let (mut w, write, _) = W::requested();
    let effects = w.returns(write, wanting());
    assert_eq!(
        effects,
        vec![round_effect(write, "write", 2, Given::SentBack(wanting_unmet()))]
    );
    assert_eq!(w.work.records.finished, vec![]);
    assert!(!w.work.contents.keys().any(|(b, _)| b.as_str() == "verdict"));
}

#[test]
fn a_return_asking_for_nothing_is_sent_back_as_asking_nothing() {
    let (mut w, write, _) = W::requested();
    let effects = w.returns(write, Came::Asks(vec![]));
    assert_eq!(
        effects,
        vec![round_effect(
            write,
            "write",
            2,
            Given::SentBack(vec![Unmet::AsksNothing])
        )]
    );
}

#[test]
fn the_owner_letting_a_cell_go_on_pausing_resuming_and_cancelling_are_each_recorded() {
    let mut w = W::new();
    w.on(Event::LetGo { cell: "judge".into() });
    w.on(Event::Paused {
        by: Pauser::Owner,
        why: None,
    });
    w.on(Event::Resumed);
    w.on(Event::Cancelled);
    let r = &w.work.records;
    assert_eq!(
        r.let_go.iter().map(|l| l.cell.as_str()).collect::<Vec<_>>(),
        vec!["judge"]
    );
    assert_eq!(r.pauses.iter().map(|p| p.paused).collect::<Vec<_>>(), vec![true, false]);
    assert!(r.cancelled.is_some());
}

#[test]
fn a_paused_work_is_given_nothing_and_when_let_on_is_given_its_next_round() {
    let mut w = W::new();
    assert_eq!(
        w.on(Event::Paused {
            by: Pauser::Owner,
            why: None
        }),
        vec![]
    );
    let (effects, request) = w.person_writes("request");
    assert_eq!(effects, vec![]);
    let effects = w.on(Event::Resumed);
    let run = w.work.records.started[0].run;
    let start = started_with(
        vec![Cause::NoRecord],
        &[
            ("request", Some(request)),
            ("opinions", None),
            ("main", None),
            ("text", None),
        ],
    );
    assert_eq!(effects, vec![round_effect(run, "write", 1, Given::Task)]);
    assert_eq!(start_of(&w, run), start);
}

#[test]
fn a_write_while_a_round_is_under_way_gives_nothing_until_the_round_ends() {
    let (mut w, ..) = W::requested();
    assert_eq!(w.on(wrote("opinions", Outsider::Person, true, None)), vec![]);
}

#[test]
fn an_engine_cell_goes_through_the_same_mechanism() {
    let (mut w, write, _) = W::requested();
    w.lets_merge();
    let judge_effects = w.returns(write, hands_in(&[("text", None)]));
    let text = w.last_finished();
    let (judge, ..) = round(&judge_effects);
    let effects = w.returns(judge, hands_in(&[("verdict", Some("yes"))]));
    let verdict = w.last_finished();
    let merge = w.work.records.started[2].run;
    let start = started_with(
        vec![Cause::NoRecord],
        &[("text", Some(text)), ("verdict", Some(verdict)), ("merged", None)],
    );
    assert_eq!(effects, vec![round_effect(merge, "merge", 1, Given::Task)]);
    assert_eq!(start_of(&w, merge), start);
    assert_eq!(w.returns(merge, hands_in(&[("merged", None)])), vec![]);
    let merged = w.last_finished();
    assert_eq!(
        w.work.contents.get(&("merged".into(), merged)),
        Some(&"the merged".to_string())
    );
    assert_eq!(w.decision(), decide::Decision::Ended(decide::End::Merged));
}

#[test]
fn a_write_from_outside_said_not_to_matter_runs_nothing() {
    let (mut w, write, _) = W::requested();
    let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
    w.returns(judge, hands_in(&[]));
    let first = w.on(wrote("main", Outsider::Outside, true, None));
    let (again, ..) = round(&first);
    w.returns(again, hands_in(&[]));
    assert_eq!(w.on(wrote("main", Outsider::Outside, false, None)), vec![]);
}
