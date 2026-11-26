//! What needs whoever plans the work since a mark (`driver::attention`): a work that ended, one that waits on the owner,
//! one that is stalled, one that waits for items none of which is going on, and one the owner paused for items that
//! have all ended, each once from the mark after it came to stand so, since the time it did; never a work going on; the
//! same answer for the same mark, a restart between or not; at most a number of lines, the rest coming again.

use driver::attention::{Attention, Heed, Idle, LINES, Line, Mark, Refused, Stuck};
use driver::{Asked, NewItem};
use rusqlite::{Connection, params};
use store::ItemChange;

use super::*;

/// The heeds of the answer's lines, in their order.
fn heeds(attention: &Attention) -> Vec<Heed> {
    attention.lines.iter().map(|line| line.heed.clone()).collect()
}

/// What needs the planner since the mark, which the driver must answer.
fn since(h: &H, mark: Option<&Mark>) -> Attention {
    h.d().attention(mark).expect("the driver answers")
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
    assert_eq!(answer.left_out, 0);
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
    // A mark is a place in the records: once the pause is lifted, the driver gives the same answer to each mark.
    h.lift_the_starts_pause();
    assert_eq!(since(&h, Some(&first.mark)), answer);
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

#[test]
fn lines_past_the_most_are_counted_and_come_again_at_the_next_mark() {
    let mut h = H::new();
    let works: Vec<WorkNumber> = (0..LINES + 3)
        .map(|i| h.add(new_item(&format!("item {i}"), &[], "plain")).1)
        .collect();
    for w in &works {
        h.let_on(*w);
        calls_the_owner(&mut h, *w);
    }
    let first = since(&h, None);
    let shown = |works: &[WorkNumber]| -> Vec<Heed> { works.iter().map(|w| Heed::ForAPerson { work: *w }).collect() };
    assert_eq!(heeds(&first), shown(&works[..LINES]));
    assert_eq!(first.left_out, 3);
    let rest = since(&h, Some(&first.mark));
    assert_eq!(heeds(&rest), shown(&works[LINES..]));
    assert_eq!(rest.left_out, 0);
    assert_eq!(heeds(&since(&h, Some(&rest.mark))), vec![]);
}

#[test]
fn a_mark_is_written_and_read_back_and_one_not_of_these_records_is_refused() {
    let mut h = H::new();
    let (_, wa) = h.add(new_item("a", &[], "plain"));
    let (_, wb) = h.add(new_item("b", &[], "plain"));
    h.let_on(wa);
    h.through(wa);
    let mark = since(&h, None).mark;
    let tick = h.d().work(wb).expect("the work is there").work.tick();
    assert_eq!(
        mark.to_string(),
        format!("2,2.{}", tick.0),
        "a work that ended is left out of the mark"
    );
    assert_eq!(Mark::read(&mark.to_string()), Ok(mark.clone()));
    for unreadable in ["", "x", "2,", "2,1", "2,1.x", "2,3.1", "2,1.1,1.2", "-1"] {
        assert!(Mark::read(unreadable).is_err(), "{unreadable:?} was read as a mark");
    }
    let refused = |text: &str| h.d().attention(Some(&Mark::read(text).expect("the mark reads")));
    assert_eq!(refused("3"), Err(Refused::NotOfTheseRecords { work: WorkNumber(3) }));
    assert_eq!(
        refused(&format!("2,2.{}", tick.0 + 1)),
        Err(Refused::NotOfTheseRecords { work: wb })
    );
    // A work left out of a mark had ended, and the work b has not.
    assert_eq!(refused("2"), Err(Refused::NotOfTheseRecords { work: wb }));
    assert!(refused("1").is_ok(), "a mark from before the second item was added");
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
    let flow = r#"
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
    compiled(flow, &[("write", write), ("check", check), ("merge", merge)])
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

// The check before a merge found a test failing on the head of main: the project's main is red, which the planner is
// told once for each time it came to be so, at rank 0 ahead of every work, with the works that wait for it; main moving
// on makes it not known to be red, and where the check finds it red on the new head too, with no pass between, that is
// the red already given; a pass on main ends it, and a failure after that is a new line. Under a second alone, a store
// on disk; no other test holds the line, its mark and what makes it new.
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
    assert_eq!(answer.left_out, 0);
    assert!(
        answer.mark.to_string().ends_with(&format!(",r.{}.{PROJECT}", read.0)),
        "{}",
        answer.mark
    );
    assert_eq!(heeds(&since(&h, Some(&answer.mark))), vec![], "it is given once");
    assert_eq!(
        since(&h, None).lines,
        answer.lines,
        "with no mark, what stands now is given"
    );
    // Main moves on: what it is, is not known until a check reads it, so it is not said to be red, and what was read is
    // kept in the mark.
    let red_in_mark = format!(",r.{}.{PROJECT}", read.0);
    h.outside("main", "Main is at m2.");
    let moved = since(&h, Some(&answer.mark));
    assert_eq!(heeds(&moved), vec![]);
    assert!(moved.mark.to_string().ends_with(&red_in_mark), "{}", moved.mark);
    // The check reads the test failing on the new head as well: it is the red that was given, and is not given again.
    assert_eq!(h.round(w).1, "check");
    finds_main_red(&mut h, w, "run/1/round/2", "own2", "m2");
    let again = since(&h, Some(&moved.mark));
    assert_eq!(heeds(&again), vec![], "no test passed between, so it is the same red");
    assert!(again.mark.to_string().ends_with(&red_in_mark), "{}", again.mark);
    // The test passes on the next head of main: the red is over, and the mark no longer holds it.
    h.outside("main", "Main is at m3.");
    assert_eq!(h.round(w).1, "check");
    checks(&mut h, w, "run/1/round/3", "own3", "m3", false);
    let over = since(&h, Some(&again.mark));
    assert_eq!(heeds(&over), vec![]);
    assert!(!over.mark.to_string().contains(",r."), "{}", over.mark);
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

// A red main is a line ahead of every work's, so a work's line is left out before it; the red of a project whose line is
// left out for the most lines of an answer comes again with the next mark, as a work's does.
#[test]
fn a_red_main_comes_before_the_works_and_one_left_out_for_the_most_lines_comes_again() {
    let mut h = H::with(vec![checked()]);
    let projects: Vec<String> = (0..LINES + 2).map(|i| format!("p{i:02}")).collect();
    for (i, project) in projects.iter().enumerate() {
        let (_, w) = h.add(NewItem {
            project: project.clone(),
            ..new_item(&format!("item {i}"), &[], "checked")
        });
        h.outside_of(project, "main", "Main is at m1.");
        comes_to_its_check(&mut h, w);
        finds_main_red(&mut h, w, &format!("run/{i}/round/1"), "own", "m1");
    }
    let (_, last) = h.add(new_item("late", &[], "checked"));
    h.let_on(last);
    calls_the_owner(&mut h, last);
    let first = since(&h, None);
    let reds = |attention: &Attention| -> Vec<String> {
        (attention.lines.iter())
            .filter_map(|line| match &line.heed {
                Heed::MainRed { project, .. } => Some(project.clone()),
                _ => None,
            })
            .collect()
    };
    assert_eq!(first.lines.len(), LINES);
    assert_eq!(reds(&first).len(), LINES, "the works that wait are after the reds");
    assert_eq!(first.left_out, 3, "two reds and the call for the owner are left out");
    let rest = since(&h, Some(&first.mark));
    assert_eq!(reds(&rest).len(), 2, "the reds left out come again");
    assert_eq!(rest.lines.len(), 3);
    let mut given = reds(&first);
    given.extend(reds(&rest));
    assert_eq!(given, projects, "each red once, in the order the reds began");
    assert_eq!(heeds(&since(&h, Some(&rest.mark))), vec![]);
}

// A mark holds the time each project's red began, which tells that red from a later one: written as `r.`, the time and
// the project's name, a `%` and a `,` in the name written as `%25` and `%2C`, and read back as it was written.
#[test]
fn a_mark_holds_the_red_of_each_project_and_one_that_is_not_so_written_is_refused() {
    for text in [
        "3,r.7.Firn-wf",
        "3,e.1,r.7.a.b,r.8.c",
        "3,1.4,r.7.a%2Cb%25",
        "0,r.7.%252C",
    ] {
        let mark = Mark::read(text).expect("the mark reads");
        assert_eq!(mark.to_string(), text);
    }
    for unreadable in [
        "3,r",
        "3,r.7",
        "3,r.7.",
        "3,r.x.a",
        "3,r.-1.a",
        "3,r.7.a,r.8.a",
        "3,r.7.a%",
        "3,r.7.a%41",
        "3,r.7.a%2",
        "3,r.7.a,b",
    ] {
        assert!(Mark::read(unreadable).is_err(), "{unreadable:?} was read as a mark");
    }
    // A project that is not red now, named by a mark, is no fault of the records.
    let h = H::new();
    assert_eq!(
        heeds(&since(&h, Some(&Mark::read("0,r.7.gone").expect("it reads")))),
        vec![]
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
    assert_eq!(heeds(&since(&h, Some(&answer.mark))), vec![], "the wait is given once");
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
    assert_eq!(Mark::read(&answer.mark.to_string()), Ok(answer.mark.clone()));
    assert_eq!(heeds(&since(&h, Some(&answer.mark))), vec![], "the pause is given once");
    h.pause_everything(false);
    h.pause_everything(true);
    assert_eq!(
        heeds(&since(&h, Some(&answer.mark))),
        vec![Heed::Everything { by: Pauser::Owner }],
        "a later pause is given again"
    );
}

/// The heeds of the answer's lines that say a pause is over.
fn over(attention: &Attention) -> Vec<Heed> {
    (attention.lines.iter())
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
    assert_eq!(over(&since(&h, Some(&both.mark))), vec![], "it is given once");
    assert_eq!(
        over(&since(&h, None)).len(),
        1,
        "with no mark, what stands now is given"
    );
    h.let_on(wc);
    assert_eq!(over(&since(&h, None)), vec![], "let on, its pause waits for nothing");
}

// A pause that is over is given again to the same mark after the driver is opened again, and those past the most lines
// of an answer come at the next mark, each work once: the line is read from the records, and the mark counts the records.
#[test]
fn a_pause_that_is_over_is_given_again_to_the_same_mark_after_a_restart_and_those_past_the_most_come_at_the_next() {
    let mut h = H::new();
    let (a, _) = h.add(new_item("a", &[], "plain"));
    let waiting: Vec<WorkNumber> = (0..LINES + 2)
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
    let works = |attention: &Attention| -> Vec<WorkNumber> {
        over(attention)
            .into_iter()
            .filter_map(|heed| match heed {
                Heed::PauseOver { work, .. } => Some(work),
                _ => None,
            })
            .collect()
    };
    let first = since(&h, Some(&mark));
    // The cancelled item may be a line of its own beside the pauses that waited for it.
    assert_eq!(first.lines.len(), LINES);
    assert!(first.left_out >= 2, "{}", first.left_out);
    h.restart();
    let again = since(&h, Some(&mark));
    assert_eq!(
        works(&again),
        works(&first),
        "the same mark gives the same lines after a restart"
    );
    assert_eq!(again.left_out, first.left_out);
    assert_eq!(again.mark, first.mark);
    let rest = since(&h, Some(&first.mark));
    assert_eq!(rest.left_out, 0);
    let mut every: Vec<WorkNumber> = works(&first);
    every.extend(works(&rest));
    every.sort();
    assert_eq!(
        every, waiting,
        "each work is given once, those past the most at the next mark"
    );
    assert_eq!(over(&since(&h, Some(&rest.mark))), vec![]);
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
