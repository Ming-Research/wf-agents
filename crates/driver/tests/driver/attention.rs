//! What needs whoever plans the work (`driver::attention`): a work that ended, one that waits on the owner, one that is
//! stalled, one that waits for items none of which is going on, and one the owner paused for items that have all ended,
//! each since the time it came to stand so; never a work going on; the same lines, a restart between or not. The
//! interface gives a line as new where it came to stand at or after the time it last read them, as these tests read it.

use driver::attention::{Heed, Idle, Line, Stuck};
use driver::{Asked, NewItem};
use rusqlite::{Connection, params};
use store::ItemChange;

use super::*;

/// What the planner is given at a read, as the interface reads the driver's lines: with the time of a read before, each
/// line that came to stand at or after it; with none, each line that stands but the works that ended. With the time of
/// this read, after everything that happened so far and before anything to come, from which the next read gives.
#[derive(Debug)]
struct Answer {
    lines: Vec<Line>,
    mark: Time,
}

/// The heeds of the answer's lines, in their order.
fn heeds(answer: &Answer) -> Vec<Heed> {
    answer.lines.iter().map(|line| line.heed.clone()).collect()
}

/// What needs the planner since the read at the mark, of the lines the driver must give.
fn since(h: &H, mark: Option<&Time>) -> Answer {
    let lines = h.d().attention().expect("the driver answers");
    let lines = (lines.into_iter())
        .filter(|line| match mark {
            None => !matches!(line.heed, Heed::Ended { .. }),
            Some(mark) => line.since >= *mark,
        })
        .collect();
    Answer {
        lines,
        mark: Time(h.clock + 1),
    }
}

/// The work's run reaches its limit of returns and calls the owner; gives the time it did.
fn calls_the_owner(h: &mut H, work: WorkNumber) -> Time {
    h.at_limit(work);
    Time(h.clock)
}

#[test]
fn a_call_for_the_owner_comes_once_since_it_was_made_and_a_restart_adds_only_its_pause_of_everything() {
    let mut h = H::new();
    let (_, w) = h.add(new_item("a", &[], "plain"));
    h.let_on(w);
    let first = since(&h, None);
    assert_eq!(heeds(&first), vec![], "a work under way is something for the planner");
    let asked = calls_the_owner(&mut h, w);
    let answer = since(&h, Some(&first.mark));
    assert_eq!(
        answer.lines,
        vec![Line {
            heed: Heed::ForAPerson { work: w },
            since: asked,
        }]
    );
    let again = since(&h, Some(&answer.mark));
    assert_eq!(
        heeds(&again),
        vec![],
        "what was given is given again from the mark after it"
    );
    // Opened again on a work going on, the driver paused everything until the owner lifts it: that is given first.
    h.reopen();
    let held = Line {
        heed: Heed::Everything { by: Pauser::Engine },
        since: Time(h.clock),
    };
    let mut lines = vec![held];
    lines.extend(answer.lines.clone());
    assert_eq!(since(&h, Some(&first.mark)).lines, lines);
    // The lines are read from the records: once the pause is lifted, the driver gives the same lines to each mark.
    h.lift_the_starts_pause();
    assert_eq!(since(&h, Some(&first.mark)).lines, answer.lines);
    assert_eq!(heeds(&since(&h, Some(&answer.mark))), vec![]);
    assert_eq!(
        since(&h, None).lines,
        answer.lines,
        "with no mark, what stands now is given"
    );
}

#[test]
fn a_work_going_on_from_cell_to_cell_is_nothing_and_its_merge_is_a_line() {
    let mut h = H::new();
    let (_, w) = h.add(new_item("a", &[], "judged"));
    let mut mark = since(&h, None).mark;
    h.let_on(w);
    for _ in 0..20 {
        let answer = since(&h, Some(&mark));
        assert_eq!(heeds(&answer), vec![], "a work going on is something for the planner");
        mark = answer.mark;
        let (_, cell, ..) = h.round(w);
        let kept = h.d().work(w).expect("the work is there");
        let came = proper(kept.flow, &cell);
        h.returns(w, came);
        if h.decision(w) == Decision::Ended(End::Merged) {
            break;
        }
    }
    let merged = Time(h.clock);
    assert_eq!(
        since(&h, Some(&mark)).lines,
        vec![Line {
            heed: Heed::Ended {
                work: w,
                end: End::Merged
            },
            since: merged,
        }]
    );
    assert_eq!(
        heeds(&since(&h, None)),
        vec![],
        "with no mark, a work that ended is taken as read"
    );
}

#[test]
fn the_engine_pausing_a_work_for_its_reason_is_a_line_and_the_owner_pausing_one_is_not() {
    let mut h = H::new();
    let (_, by_engine) = h.add(new_item("a", &[], "plain"));
    let (_, by_owner) = h.add(new_item("b", &[], "plain"));
    let mark = since(&h, None).mark;
    let mut rested = Vec::new();
    for (work, by, why) in [
        (by_engine, Pauser::Engine, Some("it spent more than it may".to_string())),
        (by_owner, Pauser::Owner, None),
    ] {
        h.let_on(work);
        h.on(
            work,
            Event::Paused {
                by,
                why,
                waits_for: Vec::new(),
            },
        );
        // Its round under way, the work goes on until the round ends.
        let heeds = heeds(&since(&h, Some(&mark)));
        assert!(!heeds.contains(&Heed::ForAPerson { work }), "{heeds:?}");
        h.hands_in(work, &[("text", None)]);
        assert!(matches!(h.decision(work), Decision::Paused { .. }));
        rested.push(Time(h.clock));
    }
    assert_eq!(
        since(&h, Some(&mark)).lines,
        vec![Line {
            heed: Heed::ForAPerson { work: by_engine },
            since: rested[0],
        }]
    );
}

#[test]
fn a_work_that_stalls_is_a_line_until_what_runs_a_cell_moves_it_on() {
    let mut h = H::new();
    let (_, w) = h.add(new_item("a", &[], "plain"));
    let mark = since(&h, None).mark;
    h.let_on(w);
    // The writer deals with what ran it and hands in nothing: nothing runs the merge, and nothing will.
    h.hands_in(w, &[]);
    let stalled = Time(h.clock);
    let at_the_merge = Decision::Stalled {
        at: "merge".into(),
        on: Vec::new(),
        by: vec![decide::Stalling {
            cell: "write".into(),
            why: decide::Unrun::NothingNew,
        }],
    };
    assert_eq!(h.decision(w), at_the_merge);
    let kept = h.d().work(w).expect("the work is there");
    assert!(!decide::paused(&kept.work.records), "nothing pauses a stalled work");
    let answer = since(&h, Some(&mark));
    // Not the owner's to end: a line of its own, which comes with the faults.
    assert_eq!(
        answer.lines,
        vec![Line {
            heed: Heed::Stalled { work: w },
            since: stalled,
        }]
    );
    // The owner writes a request: the writer runs, and the work stands so no longer.
    h.on(w, wrote("request", Outsider::Person, None));
    assert!(matches!(h.decision(w), Decision::Busy { .. }), "{:?}", h.decision(w));
    assert_eq!(heeds(&since(&h, Some(&answer.mark))), vec![]);
}

#[test]
fn a_run_at_its_limit_is_a_line() {
    let mut h = H::new();
    let (_, w) = h.add(new_item("a", &[], "plain"));
    let mark = since(&h, None).mark;
    h.let_on(w);
    // What the cell may not write is sent back, until the run reaches its limit and waits for the owner.
    for _ in 0..5 {
        h.hands_in(w, &[("merged", None)]);
        if matches!(h.decision(w), Decision::Wait { .. }) {
            break;
        }
    }
    let Decision::Wait { on, .. } = h.decision(w) else {
        panic!("the run never reached its limit");
    };
    assert!(
        matches!(
            on[..],
            [Waited::Call {
                on: Some(Who::Owner),
                ..
            }]
        ),
        "{on:?}"
    );
    assert_eq!(
        since(&h, Some(&mark)).lines,
        vec![Line {
            heed: Heed::ForAPerson { work: w },
            since: Time(h.clock),
        }]
    );
}

#[test]
fn a_work_waiting_for_an_item_nobody_lets_on_is_idle_and_again_when_that_item_stops_again() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    // Added paused, the work waits for the item from the start: the walk reaches the precondition before the pause.
    let (_, wb) = h.add(new_item("b", &[a], "judged"));
    let waits = Time(h.clock);
    h.let_on(wb);
    assert_eq!(h.decision(wb), waits_for_dependencies("write"));
    let first = since(&h, None);
    assert_eq!(
        first.lines,
        vec![Line {
            heed: Heed::Idle {
                work: wb,
                items: vec![Idle {
                    item: a,
                    why: Stuck::Paused
                }],
            },
            since: waits,
        }]
    );
    h.let_on(wa);
    let going = since(&h, Some(&first.mark));
    assert_eq!(heeds(&going), vec![], "a work waiting for an item going on is idle");
    h.on(
        wa,
        Event::Paused {
            by: Pauser::Owner,
            why: None,
            waits_for: Vec::new(),
        },
    );
    // Its round under way, the item goes on until the round ends.
    assert_eq!(heeds(&since(&h, Some(&going.mark))), vec![]);
    h.hands_in(wa, &[("plan", None)]);
    let stopped = Time(h.clock);
    let again = since(&h, Some(&going.mark));
    assert_eq!(
        again.lines,
        vec![Line {
            heed: Heed::Idle {
                work: wb,
                items: vec![Idle {
                    item: a,
                    why: Stuck::Paused
                }],
            },
            since: stopped,
        }]
    );
    assert_eq!(heeds(&since(&h, Some(&again.mark))), vec![]);
}

#[test]
fn a_started_work_waiting_where_it_stands_for_an_item_it_came_to_depend_on_is_idle_as_one_not_begun_is() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (b, wb) = h.add(new_item("b", &[], "judged"));
    h.let_on(wb);
    assert_eq!(h.round(wb).1, "write");
    assert!(h.change(b, ItemChange::DependsOn(a)).0);
    // Its round ends as it would, the work waits at the cell it would run, and nobody lets the item on.
    let kept = h.d().work(wb).expect("the work is there");
    let came = proper(kept.flow, "write");
    h.returns(wb, came);
    assert_eq!(h.decision(wb), waits_for_dependencies("judge"));
    assert_eq!(
        heeds(&since(&h, None)),
        vec![Heed::Idle {
            work: wb,
            items: vec![Idle {
                item: a,
                why: Stuck::Paused
            }],
        }]
    );
    // The item goes on, and the work waiting for it is idle no more.
    h.let_on(wa);
    assert_eq!(heeds(&since(&h, None)), vec![]);
}

#[test]
fn a_work_waiting_for_an_item_that_waits_in_turn_for_one_nobody_lets_on_is_idle_too() {
    let mut h = H::new();
    let (a, _) = h.add(new_item("a", &[], "short"));
    let (b, wb) = h.add(new_item("b", &[a], "short"));
    let (_, wc) = h.add(new_item("c", &[b], "judged"));
    h.let_on(wb);
    h.let_on(wc);
    let answer = since(&h, None);
    assert_eq!(
        heeds(&answer),
        vec![
            Heed::Idle {
                work: wb,
                items: vec![Idle {
                    item: a,
                    why: Stuck::Paused
                }],
            },
            Heed::Idle {
                work: wc,
                items: vec![Idle {
                    item: b,
                    why: Stuck::Idle
                }],
            },
        ]
    );
}

#[test]
fn an_item_cancelled_lets_the_work_waiting_for_it_go_on_and_is_a_line_as_it_ends() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (_, wb) = h.add(new_item("b", &[a], "judged"));
    h.let_on(wa);
    h.let_on(wb);
    let mark = since(&h, None).mark;
    let (run, ..) = h.round(wa);
    h.on(wa, Event::Cancelled { why: None });
    // The round under way is stopped, and ends.
    let spent = h.spent();
    h.on(
        wa,
        Event::Returned {
            run,
            came: hands_in(&[("plan", None)]),
            spent,
        },
    );
    assert_eq!(h.decision(wa), Decision::Ended(End::Cancelled));
    // b waits for a cancelled item no more than for a merged one: it goes on, and is nothing for the planner.
    assert!(matches!(h.decision(wb), Decision::Busy { .. }), "{:?}", h.decision(wb));
    assert_eq!(
        heeds(&since(&h, Some(&mark))),
        vec![Heed::Ended {
            work: wa,
            end: End::Cancelled
        }]
    );
}

#[test]
fn a_work_left_idle_by_the_merge_of_one_item_it_waited_for_is_a_line_since_that_merge() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    // Added paused and never let on, so that it stands still from before the mark.
    let (b, _) = h.add(new_item("b", &[], "short"));
    let (_, ww) = h.add(new_item("w", &[a, b], "judged"));
    h.let_on(wa);
    h.let_on(ww);
    let mark = since(&h, None).mark;
    h.through(wa);
    // The merge is the call before the last: the last let go of what the merged work held.
    let merged = Time(h.clock - 1000);
    assert_eq!(
        since(&h, Some(&mark)).lines,
        vec![
            Line {
                heed: Heed::Idle {
                    work: ww,
                    items: vec![Idle {
                        item: b,
                        why: Stuck::Paused
                    }],
                },
                since: merged,
            },
            Line {
                heed: Heed::Ended {
                    work: wa,
                    end: End::Merged
                },
                since: merged,
            },
        ]
    );
}

#[test]
fn a_work_left_idle_by_the_cancel_of_one_item_it_waited_for_is_a_line_since_that_cancel() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    // Added paused and never let on, so that it stands still from before the mark.
    let (b, _) = h.add(new_item("b", &[], "short"));
    let (_, ww) = h.add(new_item("w", &[a, b], "judged"));
    h.let_on(wa);
    h.let_on(ww);
    let mark = since(&h, None).mark;
    // a is cancelled with its round under way: its run is abandoned as it is cancelled, and the work has ended then,
    // though the round it stops ends after.
    let (run, ..) = h.round(wa);
    h.changes(vec![(a, Asked::Cancel)]).expect("a is cancelled");
    let cancelled = Time(h.clock);
    let spent = h.spent();
    let came = hands_in(&[("plan", None)]);
    let todos = h.on(wa, Event::Returned { run, came, spent });
    h.released(todos);
    // Left waiting for b alone, which stands still, w is idle since a was cancelled, not since b stood still.
    let lines = since(&h, Some(&mark)).lines;
    assert_eq!(
        lines[0],
        Line {
            heed: Heed::Idle {
                work: ww,
                items: vec![Idle {
                    item: b,
                    why: Stuck::Paused
                }],
            },
            since: cancelled,
        }
    );
    assert_eq!(
        lines[1].heed,
        Heed::Ended {
            work: wa,
            end: End::Cancelled
        }
    );
    assert_eq!(lines.len(), 2, "{lines:?}");
}

#[test]
fn what_rests_for_a_person_comes_first_the_longest_waiting_first() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "plain"));
    let (_, wb) = h.add(new_item("b", &[], "plain"));
    let (_, wc) = h.add(new_item("c", &[], "plain"));
    let (_, wd) = h.add(new_item("d", &[a], "short"));
    let mark = since(&h, None).mark;
    for w in [wa, wb, wc, wd] {
        h.let_on(w);
    }
    h.hands_in(wa, &[("text", None)]);
    h.hands_in(wa, &[("merged", None)]);
    calls_the_owner(&mut h, wc);
    calls_the_owner(&mut h, wb);
    assert_eq!(
        heeds(&since(&h, Some(&mark))),
        vec![
            Heed::ForAPerson { work: wc },
            Heed::ForAPerson { work: wb },
            Heed::Ended {
                work: wa,
                end: End::Merged
            },
        ]
    );
}

/// Makes the records of the work faulty behind the driver's back, an entry at the place of its first, and opens the
/// driver again on them, the owner lifting at once the pause of everything it makes as it opens.
fn at_fault(h: &mut H, work: WorkNumber) {
    let conn = Connection::open(&h.db.path).expect("the database opens");
    let changed = conn
        .execute(
            "INSERT INTO let_go (work, at, cell) VALUES (?1, 1, 'write')",
            params![work.0],
        )
        .expect("an entry is added behind the driver's back");
    assert_eq!(changed, 1);
    h.faulty.insert(work);
    h.restart();
    assert!(matches!(h.decision(work), Decision::Fault(_)), "{:?}", h.decision(work));
}

#[test]
fn a_work_whose_records_are_at_fault_is_a_line_first() {
    let mut h = H::new();
    let (_, wa) = h.add(new_item("a", &[], "plain"));
    let added = Time(h.clock);
    let (_, wb) = h.add(new_item("b", &[], "plain"));
    h.let_on(wb);
    calls_the_owner(&mut h, wb);
    at_fault(&mut h, wa);
    assert_eq!(
        heeds(&since(&h, None)),
        vec![Heed::Fault { work: wa }, Heed::ForAPerson { work: wb }]
    );
    assert_eq!(since(&h, None).lines[0].since, added);
}

#[test]
fn a_work_waiting_for_an_item_whose_records_are_at_fault_is_idle() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "plain"));
    let (_, wb) = h.add(new_item("b", &[a], "judged"));
    h.let_on(wb);
    at_fault(&mut h, wa);
    assert_eq!(
        heeds(&since(&h, None)),
        vec![
            Heed::Fault { work: wa },
            Heed::Idle {
                work: wb,
                items: vec![Idle {
                    item: a,
                    why: Stuck::Fault
                }],
            },
        ]
    );
}

#[test]
fn a_work_waiting_for_an_item_that_rests_for_a_person_is_idle() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "plain"));
    let (_, wb) = h.add(new_item("b", &[a], "judged"));
    h.let_on(wa);
    h.let_on(wb);
    calls_the_owner(&mut h, wa);
    assert_eq!(
        heeds(&since(&h, None)),
        vec![
            Heed::ForAPerson { work: wa },
            Heed::Idle {
                work: wb,
                items: vec![Idle {
                    item: a,
                    why: Stuck::ForAPerson
                }],
            },
        ]
    );
}

#[test]
fn a_work_waiting_for_a_precondition_the_owner_makes_hold_is_a_line() {
    let flow = r#"
        name = "gated"
        order = ["write", "merge"]
        [limits]
        runs = 2
        returns = 2
        calls = 3
        [lines.main]
        [blocks.request]
        people = true
        [blocks.text]
        [blocks.go]
        values = ["yes", "no"]
        people = true
        [blocks.merged]
        shown = true
        [blocks.start]
        outside = "start"
        writes = { text = "The owner started the work." }
        [tests.may_merge]
        says = "The owner let the work merge"
        kind = "all"
        reads = ["go"]
        is = "yes"
    "#;
    let write = r#"
        title = "Write"
        kind = "agent"
        line = "main"
        task = "Write the text the request asks for."
        effort = "high"
        watches = ["start", "request"]
        outputs = ["text"]

        [kinds]
        start = "last_and_latest"
        request = "last_and_latest"
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        requires = { test = "may_merge" }
        watches = ["text"]
        outputs = ["merged"]
    "#;
    let mut h = H::with(vec![compiled(flow, &[("write", write), ("merge", merge)])]);
    let (_, w) = h.add(new_item("a", &[], "gated"));
    h.let_on(w);
    let mark = since(&h, None).mark;
    h.hands_in(w, &[("text", None)]);
    let Decision::Wait { on, .. } = h.decision(w) else {
        panic!("the work does not wait: {:?}", h.decision(w));
    };
    assert!(
        matches!(&on[..], [Waited::Precondition { by, .. }] if by == &vec![decide::Maker::Owner]),
        "{on:?}"
    );
    assert_eq!(
        since(&h, Some(&mark)).lines,
        vec![Line {
            heed: Heed::ForAPerson { work: w },
            since: Time(h.clock),
        }]
    );
}

/// A flow whose check, a cell that reads main, writes `checked`, which the merge waits to be yes.
fn checked() -> flow::Compiled {
    checked_flow(false)
}

/// The same flow, its writer waiting for the items its item depends on to have merged where `after_items`.
fn checked_flow(after_items: bool) -> flow::Compiled {
    let mut flow = r#"
        name = "checked"
        order = ["write", "check", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.main]
        [blocks.request]
        people = true
        [blocks.text]
        [blocks.checked]
        values = ["yes", "no"]
        [blocks.main]
        outside = "main"
        [blocks.merged]
        shown = true
        [blocks.start]
        outside = "start"
        writes = { text = "The owner started the work." }
        [tests.passed]
        says = "The check passed"
        kind = "all"
        reads = ["checked"]
        is = "yes"
    "#
    .to_string();
    let mut write = r#"
        title = "Write"
        kind = "agent"
        line = "main"
        task = "Write the text the request asks for."
        effort = "high"
        watches = ["start", "request"]
        outputs = ["text"]

        [kinds]
        start = "last_and_latest"
        request = "last_and_latest"
    "#
    .to_string();
    if after_items {
        flow.push_str(
            r#"
        [blocks.deps]
        values = ["waiting", "merged"]
        outside = "dependencies"
        writes = { merged = "Every item this one depends on has merged.", waiting = "Waits for the items {items} to merge.", between = ", " }
        [tests.ready]
        says = "Every item this one depends on has merged"
        kind = "all"
        reads = ["deps"]
        is = "merged"
    "#,
        );
        write = write.replace(
            "effort = \"high\"",
            "effort = \"high\"\n        requires = { test = \"ready\" }",
        );
    }
    let check = r#"
        title = "Check"
        kind = "agent"
        line = "main"
        task = "Check the text against main."
        effort = "high"
        watches = ["text", "main"]
        outputs = ["checked"]

        [kinds]
        text = "last_and_latest"
        main = "latest"
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        requires = { test = "passed" }
        watches = ["text", "checked"]
        outputs = ["merged"]
    "#;
    compiled(&flow, &[("write", write.as_str()), ("check", check), ("merge", merge)])
}

// A work whose check says that what it wrote waits for main waits for main, no one's to answer, and is no line for
// whoever plans the work; the same check not saying so leaves the work stalled, which is. Well under a second alone, a
// store on disk; no other test holds what a wait for main is to the one who plans.
#[test]
fn a_work_waiting_for_main_to_move_is_no_line_and_one_stalled_is() {
    for waits in [true, false] {
        let mut h = H::with(vec![checked()]);
        let (_, w) = h.add(new_item("a", &[], "checked"));
        let mark = since(&h, None).mark;
        h.let_on(w);
        h.hands_in(w, &[("text", None)]);
        let held = hands_in(&[("checked", Some("no"))]);
        let came = Came {
            waits_for: waits.then(|| "main".into()),
            ..held
        };
        h.returns(w, came);
        let answer = since(&h, Some(&mark));
        if waits {
            assert!(
                matches!(&h.decision(w), Decision::Wait { on, .. } if matches!(on[..], [Waited::Outside { .. }])),
                "{:?}",
                h.decision(w)
            );
            assert_eq!(heeds(&answer), Vec::<Heed>::new(), "waiting on no one");
        } else {
            assert!(matches!(h.decision(w), Decision::Stalled { .. }), "{:?}", h.decision(w));
            assert_eq!(heeds(&answer), [Heed::Stalled { work: w }]);
        }
    }
}

/// What a run of the CI said of the test `carry`/`t`, which the check of a work asked for in its round.
fn read_test(h: &mut H, work: WorkNumber, asking: &str, source: work::Source, commit: &str, passed: bool) {
    let reading = work::timed::Reading {
        asking: asking.to_string(),
        source,
        commit: commit.to_string(),
        workflow: "ci.yml".to_string(),
        jobs: Vec::new(),
        tests: vec![work::timed::Test {
            unit: "carry".to_string(),
            name: "t".to_string(),
            passed,
            millis: 10,
        }],
    };
    h.on(work, Event::Timed { reading });
}

/// The work, let on, writes its text, and its check begins.
fn comes_to_its_check(h: &mut H, work: WorkNumber) {
    h.let_on(work);
    h.hands_in(work, &[("text", None)]);
    assert_eq!(h.round(work).1, "check");
}

/// The check of the work, in its round asked as `asking`, finds the test failing on the commit `own`, and failing on the
/// head of main `head` where `fails_on_main`, passing there where not; the work waits for main as the check says. Gives
/// when the run on main was read.
fn checks(h: &mut H, work: WorkNumber, asking: &str, own: &str, head: &str, fails_on_main: bool) -> Time {
    read_test(h, work, asking, work::Source::Full, own, false);
    read_test(h, work, asking, work::Source::Fast, own, false);
    read_test(h, work, asking, work::Source::Fast, head, !fails_on_main);
    let read = Time(h.clock);
    let held = hands_in(&[("checked", Some("no"))]);
    h.returns(
        work,
        Came {
            waits_for: Some("main".into()),
            ..held
        },
    );
    read
}

/// The check of the work finds the test failing on the commit `own` and on the head of main `head`, so that the work
/// waits for main; gives when the failure on main was read.
fn finds_main_red(h: &mut H, work: WorkNumber, asking: &str, own: &str, head: &str) -> Time {
    checks(h, work, asking, own, head, true)
}

fn red_tests(head: &str) -> Vec<driver::red_main::RedTest> {
    vec![driver::red_main::RedTest {
        unit: "carry".to_string(),
        name: "t".to_string(),
        head: head.to_string(),
    }]
}

// The check before a merge found a test failing on the head of main: the project's main is red, a line since it came to
// be so, at rank 0 ahead of every work, with the works that wait for it; main moving on makes it not known to be red,
// and where the check finds it red on the new head too, with no pass between, that is the red already given, since the
// same time; a pass on main ends it, and a failure after that is a new line. Under a second alone, a store on disk; no
// other test holds the line and what makes it new.
#[test]
fn a_red_main_is_a_line_once_for_each_time_it_came_to_be_and_a_pass_on_main_ends_it() {
    let mut h = H::with(vec![checked()]);
    let (_, w) = h.add(new_item("a", &[], "checked"));
    h.outside("main", "Main is at m1.");
    comes_to_its_check(&mut h, w);
    let first = since(&h, None);
    assert_eq!(heeds(&first), vec![], "main is not red before a check reads it");
    let read = finds_main_red(&mut h, w, "run/1/round/1", "own1", "m1");
    let answer = since(&h, Some(&first.mark));
    assert_eq!(
        answer.lines,
        vec![Line {
            heed: Heed::MainRed {
                project: PROJECT.to_string(),
                tests: red_tests("m1"),
                works: vec![w],
            },
            since: read,
        }]
    );
    assert_eq!(heeds(&since(&h, Some(&answer.mark))), vec![], "it is new once");
    assert_eq!(
        since(&h, None).lines,
        answer.lines,
        "with no mark, what stands now is given"
    );
    // Main moves on: what it is, is not known until a check reads it, so it is not said to be red.
    h.outside("main", "Main is at m2.");
    let moved = since(&h, Some(&answer.mark));
    assert_eq!(heeds(&moved), vec![]);
    assert_eq!(since(&h, None).lines, vec![], "main is not known to be red");
    // The check reads the test failing on the new head as well: it is the red that was given, since the same time, and
    // is not new.
    assert_eq!(h.round(w).1, "check");
    finds_main_red(&mut h, w, "run/1/round/2", "own2", "m2");
    let again = since(&h, Some(&moved.mark));
    assert_eq!(heeds(&again), vec![], "no test passed between, so it is the same red");
    assert_eq!(since(&h, None).lines.first().map(|line| line.since), Some(read));
    // The test passes on the next head of main: the red is over.
    h.outside("main", "Main is at m3.");
    assert_eq!(h.round(w).1, "check");
    checks(&mut h, w, "run/1/round/3", "own3", "m3", false);
    let over = since(&h, Some(&again.mark));
    assert_eq!(heeds(&over), vec![]);
    assert_eq!(since(&h, None).lines, vec![], "nothing is red now");
    // It fails on the head after that: main is red again, which is told again, since the time it began.
    h.outside("main", "Main is at m4.");
    assert_eq!(h.round(w).1, "check");
    let began = finds_main_red(&mut h, w, "run/1/round/4", "own4", "m4");
    assert_eq!(
        since(&h, Some(&over.mark)).lines,
        vec![Line {
            heed: Heed::MainRed {
                project: PROJECT.to_string(),
                tests: red_tests("m4"),
                works: vec![w],
            },
            since: began,
        }]
    );
}

// A work that depends on one that waits for main is not idle: the work it waits for is going on, whoever is to mend
// main, and a red main changes no work's standing. Well under a second alone; no other test holds a work that depends
// on one waiting for main.
#[test]
fn a_work_that_depends_on_one_waiting_for_main_is_not_idle() {
    let mut h = H::with(vec![checked_flow(true)]);
    let (a, wa) = h.add(new_item("a", &[], "checked"));
    let (_, wb) = h.add(new_item("b", &[a], "checked"));
    h.outside("main", "Main is at m1.");
    comes_to_its_check(&mut h, wa);
    finds_main_red(&mut h, wa, "run/1/round/1", "own1", "m1");
    h.let_on(wb);
    assert!(
        matches!(h.decision(wb), Decision::Wait { .. }),
        "the work waits for the item: {:?}",
        h.decision(wb)
    );
    assert_eq!(
        heeds(&since(&h, None)),
        vec![Heed::MainRed {
            project: PROJECT.to_string(),
            tests: red_tests("m1"),
            works: vec![wa],
        }],
        "no line says the work {wb:?} is idle, and it does not wait for main"
    );
}

// A project's main is its own: each project whose main is red is a line of its own, naming the tests that failed on its
// head and the works of that project alone that wait for it.
#[test]
fn a_red_main_is_the_projects_own() {
    let mut h = H::with(vec![checked()]);
    let (_, w) = h.add(new_item("a", &[], "checked"));
    let (_, other) = h.add(NewItem {
        project: "two".to_string(),
        ..new_item("b", &[], "checked")
    });
    h.outside("main", "Main is at m1.");
    h.outside_of("two", "main", "Main of two is at n1.");
    comes_to_its_check(&mut h, w);
    finds_main_red(&mut h, w, "run/1/round/1", "own1", "m1");
    assert_eq!(
        heeds(&since(&h, None)),
        vec![Heed::MainRed {
            project: PROJECT.to_string(),
            tests: red_tests("m1"),
            works: vec![w],
        }],
        "the other project's work has read nothing"
    );
    comes_to_its_check(&mut h, other);
    finds_main_red(&mut h, other, "run/2/round/1", "own2", "n1");
    assert_eq!(
        heeds(&since(&h, None)),
        vec![
            Heed::MainRed {
                project: PROJECT.to_string(),
                tests: red_tests("m1"),
                works: vec![w],
            },
            Heed::MainRed {
                project: "two".to_string(),
                tests: red_tests("n1"),
                works: vec![other],
            },
        ]
    );
}

#[test]
fn the_wait_every_work_shares_is_one_line_about_no_work_and_the_works_it_holds_go_on() {
    let mut h = H::new();
    let (_, a) = h.add(new_item("a", &[], "plain"));
    let (_, b) = h.add(new_item("b", &[], "plain"));
    h.let_on(a);
    h.let_on(b);
    let mark = since(&h, None).mark;
    let (run, ..) = h.round(a);
    let spent = h.spent();
    h.on(
        a,
        Event::Failed {
            run,
            why: "the account's usage limit was reached".to_string(),
            failure: decide::run::Failure::UsageLimit,
            resets: None,
            spent,
        },
    );
    let outage = h.d().outage().expect("the wait stands");
    let answer = since(&h, Some(&mark));
    assert_eq!(
        answer.lines,
        vec![Line {
            heed: Heed::Agents { outage },
            since: Time(h.clock),
        }]
    );
    assert_eq!(heeds(&since(&h, Some(&answer.mark))), vec![], "the wait is new once");
}

#[test]
fn a_work_that_ended_and_could_not_let_go_of_what_it_held_rests_for_a_person() {
    let mut h = H::new();
    let (_, w) = h.add(new_item("a", &[], "plain"));
    h.let_on(w);
    h.hands_in(w, &[("text", None)]);
    let mark = since(&h, None).mark;
    h.hands_in(w, &[("merged", None)]);
    assert_eq!(h.decision(w), Decision::Ended(End::Merged));
    h.on(
        w,
        Event::NotReleased {
            why: "the worktree is locked".to_string(),
        },
    );
    assert_eq!(
        since(&h, Some(&mark)).lines,
        vec![Line {
            heed: Heed::ForAPerson { work: w },
            since: Time(h.clock),
        }]
    );
}

#[test]
fn a_work_the_engine_paused_while_it_waits_for_a_tool_rests_for_a_person_from_the_pause() {
    let mut h = H::new();
    let (_, w) = h.add(new_item("a", &[], "plain"));
    h.let_on(w);
    h.returns(w, Came::asks(vec![tool("ci")]));
    let mark = since(&h, None).mark;
    h.on(
        w,
        Event::Paused {
            by: Pauser::Engine,
            why: Some("it spent more than it may".to_string()),
            waits_for: Vec::new(),
        },
    );
    assert!(matches!(h.decision(w), Decision::Wait { .. }), "{:?}", h.decision(w));
    assert_eq!(
        since(&h, Some(&mark)).lines,
        vec![Line {
            heed: Heed::ForAPerson { work: w },
            since: Time(h.clock),
        }]
    );
}

#[test]
fn the_owners_pause_of_everything_is_one_line_about_no_work_and_leaves_no_work_idle_for_another() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (_, wb) = h.add(new_item("b", &[a], "judged"));
    h.let_on(wa);
    h.let_on(wb);
    let mark = since(&h, None).mark;
    h.pause_everything(true);
    let paused = Time(h.clock);
    // The item waited for ends its round under way, and is held back by the pause of everything alone.
    h.hands_in(wa, &[("plan", None)]);
    let answer = since(&h, Some(&mark));
    assert_eq!(
        answer.lines,
        vec![Line {
            heed: Heed::Everything { by: Pauser::Owner },
            since: paused,
        }]
    );
    assert_eq!(heeds(&since(&h, Some(&answer.mark))), vec![], "the pause is new once");
    h.pause_everything(false);
    h.pause_everything(true);
    assert_eq!(
        heeds(&since(&h, Some(&answer.mark))),
        vec![Heed::Everything { by: Pauser::Owner }],
        "a later pause is new again"
    );
}

/// The heeds of the answer's lines that say a pause is over.
fn over(answer: &Answer) -> Vec<Heed> {
    (answer.lines.iter())
        .map(|line| line.heed.clone())
        .filter(|heed| matches!(heed, Heed::PauseOver { .. }))
        .collect()
}

// The owner's pause that waited for items is a line once each has merged or been cancelled, since the time the last did:
// not while one has not, not again from the mark after it, and not once the owner lets the work on.
#[test]
fn the_owners_pause_that_waited_for_items_is_a_line_once_each_has_ended_and_never_again_or_after_it_is_lifted() {
    let mut h = H::new();
    let (a, _) = h.add(new_item("a", &[], "plain"));
    let (b, _) = h.add(new_item("b", &[], "plain"));
    let (_, wc) = h.add(new_item("c", &[], "plain"));
    h.on(
        wc,
        Event::Paused {
            by: Pauser::Owner,
            why: Some("after a and b".to_string()),
            waits_for: vec![a.0, b.0],
        },
    );
    let first = since(&h, None);
    assert_eq!(over(&first), vec![], "nothing it waits for has ended");
    h.changes(vec![(a, Asked::Cancel)]).expect("a is cancelled");
    let one = since(&h, Some(&first.mark));
    assert_eq!(over(&one), vec![], "one of the two has ended");
    h.changes(vec![(b, Asked::Cancel)]).expect("b is cancelled");
    let ended = Time(h.clock);
    let both = since(&h, Some(&one.mark));
    assert_eq!(
        over(&both),
        vec![Heed::PauseOver {
            work: wc,
            items: vec![a, b]
        }]
    );
    let line = both
        .lines
        .iter()
        .find(|line| matches!(line.heed, Heed::PauseOver { .. }));
    assert_eq!(line.map(|line| line.since), Some(ended));
    assert_eq!(over(&since(&h, Some(&both.mark))), vec![], "it is new once");
    assert_eq!(
        over(&since(&h, None)).len(),
        1,
        "with no mark, what stands now is given"
    );
    h.let_on(wc);
    assert_eq!(over(&since(&h, None)), vec![], "let on, its pause waits for nothing");
}

// A pause that is over is given again to the same mark after the driver is opened again: the line is read from the
// records, since the time the item it waited for was cancelled.
#[test]
fn a_pause_that_is_over_is_given_again_to_the_same_mark_after_a_restart() {
    let mut h = H::new();
    let (a, _) = h.add(new_item("a", &[], "plain"));
    let waiting: Vec<WorkNumber> = (0..3)
        .map(|i| {
            let (_, work) = h.add(new_item(&format!("item {i}"), &[], "plain"));
            h.on(
                work,
                Event::Paused {
                    by: Pauser::Owner,
                    why: None,
                    waits_for: vec![a.0],
                },
            );
            work
        })
        .collect();
    let mark = since(&h, None).mark;
    h.changes(vec![(a, Asked::Cancel)]).expect("a is cancelled");
    let works = |answer: &Answer| -> Vec<WorkNumber> {
        over(answer)
            .into_iter()
            .filter_map(|heed| match heed {
                Heed::PauseOver { work, .. } => Some(work),
                _ => None,
            })
            .collect()
    };
    let first = since(&h, Some(&mark));
    assert_eq!(works(&first), waiting);
    h.restart();
    let again = since(&h, Some(&mark));
    assert_eq!(
        again.lines, first.lines,
        "the same mark gives the same lines after a restart"
    );
    assert_eq!(over(&since(&h, Some(&first.mark))), vec![]);
}

// A pause for items that have ended already is over as it is made: it is a line at once, as is a pause that names none
// but is given again, which is no line.
#[test]
fn a_pause_for_items_that_ended_already_is_a_line_at_once_and_one_that_waits_for_none_is_not() {
    let mut h = H::new();
    let (a, _) = h.add(new_item("a", &[], "plain"));
    let (_, wb) = h.add(new_item("b", &[], "plain"));
    let (_, wc) = h.add(new_item("c", &[], "plain"));
    h.changes(vec![(a, Asked::Cancel)]).expect("a is cancelled");
    let mark = since(&h, None).mark;
    h.on(
        wb,
        Event::Paused {
            by: Pauser::Owner,
            why: None,
            waits_for: vec![a.0],
        },
    );
    let paused = Time(h.clock);
    h.on(
        wc,
        Event::Paused {
            by: Pauser::Owner,
            why: Some("for a reason alone".to_string()),
            waits_for: Vec::new(),
        },
    );
    let answer = since(&h, Some(&mark));
    assert_eq!(
        over(&answer),
        vec![Heed::PauseOver {
            work: wb,
            items: vec![a]
        }]
    );
    let line = answer
        .lines
        .iter()
        .find(|line| matches!(line.heed, Heed::PauseOver { .. }));
    assert_eq!(line.map(|line| line.since), Some(paused));
}

/// What a run of the CI said of each of these tests, `carry`/the name, for the check of a work in its round.
fn read_tests(h: &mut H, work: WorkNumber, asking: &str, source: work::Source, commit: &str, tests: &[(&str, bool)]) {
    let reading = work::timed::Reading {
        asking: asking.to_string(),
        source,
        commit: commit.to_string(),
        workflow: "ci.yml".to_string(),
        jobs: Vec::new(),
        tests: (tests.iter())
            .map(|(name, passed)| work::timed::Test {
                unit: "carry".to_string(),
                name: name.to_string(),
                passed: *passed,
                millis: 10,
            })
            .collect(),
    };
    h.on(work, Event::Timed { reading });
}

/// The check of the work, in its round `asking`, reads `own_tests` on its commit `own` and `main_tests` on the head of
/// main `head`, and the work waits for main; gives when the run on main was read.
fn checks_tests(
    h: &mut H,
    work: WorkNumber,
    asking: &str,
    own: &str,
    head: &str,
    own_tests: &[(&str, bool)],
    main_tests: &[(&str, bool)],
) -> Time {
    read_tests(h, work, asking, work::Source::Full, own, own_tests);
    read_tests(h, work, asking, work::Source::Fast, own, own_tests);
    read_tests(h, work, asking, work::Source::Fast, head, main_tests);
    let read = Time(h.clock);
    let held = hands_in(&[("checked", Some("no"))]);
    h.returns(
        work,
        Came {
            waits_for: Some("main".into()),
            ..held
        },
    );
    read
}

// What `run_tests` is told of a project's main comes from the driver as the tests red on the head of main of the project
// of the work that asks, whichever work found it and whether that work has checked or not: a work of another project is
// told nothing, and once main moved on, or a pass on main came, no test is said red. Reference: the documentation of
// `Driver::red_tests` and acceptance 12 (the sentence is said only of what is red on the main of the work's project).
// Under a second alone, a store on disk; no test of the driver holds the call `run_tests` makes of it.
#[test]
fn the_tests_run_tests_is_told_are_the_red_of_the_main_of_the_project_of_the_work_that_asks() {
    let mut h = H::with(vec![checked()]);
    let (_, w) = h.add(new_item("a", &[], "checked"));
    let (_, same) = h.add(new_item("a2", &[], "checked"));
    let (_, other) = h.add(NewItem {
        project: "two".to_string(),
        ..new_item("b", &[], "checked")
    });
    h.outside("main", "Main is at m1.");
    h.outside_of("two", "main", "Main of two is at n1.");
    assert_eq!(h.d().red_tests(w), vec![], "nothing is red before a check reads it");
    comes_to_its_check(&mut h, w);
    finds_main_red(&mut h, w, "run/1/round/1", "own1", "m1");
    assert_eq!(h.d().red_tests(w), red_tests("m1"), "the work that found it");
    assert_eq!(
        h.d().red_tests(same),
        red_tests("m1"),
        "another work of the project that has not checked"
    );
    assert_eq!(h.d().red_tests(other), vec![], "a work of another project");
    // Main moves on: no run on the new head says whether the test is red, so none is said to be.
    h.outside("main", "Main is at m2.");
    assert_eq!(h.d().red_tests(same), vec![]);
    // A run on the new head where it passes: still none.
    assert_eq!(h.round(w).1, "check");
    checks(&mut h, w, "run/1/round/2", "own2", "m2", false);
    assert_eq!(h.d().red_tests(same), vec![]);
}

// The red of a project's main stands when the work that waits for it is paused by the owner and then cancelled, while
// another work of the project has not ended: what is red on main does not depend on any work waiting for it. Reference:
// acceptance 4 (the engineer's driver test of it has no real pause or cancel). Under a second alone, a store on disk.
#[test]
fn a_red_main_stands_when_the_work_waiting_for_it_is_paused_and_then_cancelled() {
    let mut h = H::with(vec![checked()]);
    let (_, w) = h.add(new_item("a", &[], "checked"));
    let (_, other) = h.add(new_item("b", &[], "checked"));
    h.outside("main", "Main is at m1.");
    comes_to_its_check(&mut h, w);
    finds_main_red(&mut h, w, "run/1/round/1", "own1", "m1");
    let is_red = |h: &H| -> bool {
        (since(h, None).lines.iter()).any(|line| {
            matches!(&line.heed, Heed::MainRed { project, tests, .. } if project == PROJECT && *tests == red_tests("m1"))
        })
    };
    assert!(is_red(&h));
    h.on(
        w,
        Event::Paused {
            by: Pauser::Owner,
            why: None,
            waits_for: Vec::new(),
        },
    );
    assert!(is_red(&h), "the work that waits for main is paused");
    h.on(w, Event::Cancelled { why: None });
    assert!(
        is_red(&h),
        "the work that found it is cancelled, and another of the project has not ended"
    );
    assert_eq!(h.d().red_tests(other), red_tests("m1"));
}

// While one test of a project's main stays red and another that was red passes, the project's main has been red the whole
// time: it is not a red that came to be anew, and is not given again. Reference: acceptance 3 (the same red is given
// once, and again only after a pass and a new failure) and the documentation of `Heed::MainRed` (once for each time it
// came to be so). Under a second alone, a store on disk.
#[test]
fn a_red_main_is_not_given_again_when_one_of_its_failing_tests_passes_and_another_goes_on_failing() {
    let mut h = H::with(vec![checked()]);
    let (_, w) = h.add(new_item("a", &[], "checked"));
    h.outside("main", "Main is at m1.");
    comes_to_its_check(&mut h, w);
    let first = since(&h, None);
    // `a` fails on main's head m1: main is red since then.
    checks_tests(
        &mut h,
        w,
        "run/1/round/1",
        "own1",
        "m1",
        &[("a", false)],
        &[("a", false)],
    );
    let given = since(&h, Some(&first.mark));
    assert_eq!(given.lines.len(), 1);
    // `b` fails on the next head as well as `a`: the same red, which is no new line.
    h.outside("main", "Main is at m2.");
    checks_tests(
        &mut h,
        w,
        "run/1/round/2",
        "own2",
        "m2",
        &[("a", false), ("b", false)],
        &[("a", false), ("b", false)],
    );
    let second = since(&h, Some(&given.mark));
    assert_eq!(heeds(&second), vec![], "a second test failing is no new red");
    // `a` passes on the next head, `b` still fails: main has not been green, and the red is not told again.
    h.outside("main", "Main is at m3.");
    checks_tests(
        &mut h,
        w,
        "run/1/round/3",
        "own3",
        "m3",
        &[("a", false), ("b", false)],
        &[("a", true), ("b", false)],
    );
    let third = since(&h, Some(&second.mark));
    assert_eq!(
        heeds(&third),
        vec![],
        "main was red all along; one of its tests passing begins no new red"
    );
}

// While no work of a project has not ended, nothing says whether its main is red: a work that comes to the same head
// finds the same red, since the same time, which is not new. Reference:
// acceptance 3 (the same red is given once) and acceptance 5 (no current fact where no work has not ended). Under a
// second alone, a store on disk.
#[test]
fn a_red_main_is_not_given_again_when_its_works_end_and_another_comes_to_the_same_head() {
    let mut h = H::with(vec![checked()]);
    let (_, w) = h.add(new_item("a", &[], "checked"));
    h.outside("main", "Main is at m1.");
    comes_to_its_check(&mut h, w);
    let first = since(&h, None);
    finds_main_red(&mut h, w, "run/1/round/1", "own1", "m1");
    let given = since(&h, Some(&first.mark));
    assert_eq!(given.lines.len(), 1);
    h.on(w, Event::Cancelled { why: None });
    let alone = since(&h, Some(&given.mark));
    let reds = |answer: &Answer| {
        (heeds(answer).into_iter())
            .filter(|heed| matches!(heed, Heed::MainRed { .. }))
            .count()
    };
    assert_eq!(reds(&alone), 0, "no work says what main is: there is no red to give");
    h.add(new_item("b", &[], "checked"));
    h.outside("main", "Main is at m1.");
    assert_eq!(
        reds(&since(&h, Some(&alone.mark))),
        0,
        "the same red, found again, is not new"
    );
}
