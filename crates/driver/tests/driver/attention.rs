//! What needs whoever plans the work since a mark (`driver::attention`): a work that ended, one that rests for a person,
//! and one that waits for items none of which is going on, each once from the mark after it came to stand so, since the
//! time it did; never a work going on; the same answer for the same mark, a restart between or not; at most a number
//! of lines, the rest coming again.

use driver::attention::{Attention, Heed, Idle, LINES, Line, Mark, Refused, Stuck};
use rusqlite::{Connection, params};

use super::*;

/// The heeds of the answer's lines, in their order.
fn heeds(attention: &Attention) -> Vec<Heed> {
    attention.lines.iter().map(|line| line.heed.clone()).collect()
}

/// What needs the planner since the mark, which the driver must answer.
fn since(h: &H, mark: Option<&Mark>) -> Attention {
    h.d().attention(mark).expect("the driver answers")
}

/// The work's round under way asks the owner a question; gives the time it did.
fn asks_the_owner(h: &mut H, work: WorkNumber) -> Time {
    h.returns(work, Came::asks(vec![question(Who::Owner)]));
    Time(h.clock)
}

#[test]
fn a_question_for_the_owner_comes_once_since_it_was_asked_and_a_restart_adds_only_its_pause_of_everything() {
    let mut h = H::new();
    let (_, w) = h.add(new_item("a", &[], "plain"));
    h.let_on(w);
    let first = since(&h, None);
    assert_eq!(heeds(&first), vec![], "a work under way is something for the planner");
    let asked = asks_the_owner(&mut h, w);
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
        h.on(work, Event::Paused { by, why });
        // Its round under way, the work goes on until the round ends.
        let heeds = heeds(&since(&h, Some(&mark)));
        assert!(!heeds.contains(&Heed::ForAPerson { work }), "{heeds:?}");
        h.hands_in(work, &[("text", None)]);
        assert_eq!(h.decision(work), Decision::Paused);
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
fn an_item_cancelled_leaves_the_work_waiting_for_it_idle_for_good() {
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
    assert_eq!(
        heeds(&since(&h, Some(&mark))),
        vec![
            Heed::Idle {
                work: wb,
                items: vec![Idle {
                    item: a,
                    why: Stuck::Cancelled
                }],
            },
            Heed::Ended {
                work: wa,
                end: End::Cancelled
            },
        ],
        "what rests for nobody comes before what ended"
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
fn a_work_left_idle_by_the_merge_of_one_item_it_waited_for_is_a_line_when_the_other_was_cancelled_long_before() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "short"));
    let (b, wb) = h.add(new_item("b", &[], "short"));
    h.on(wb, Event::Cancelled { why: None });
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
                        why: Stuck::Cancelled
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
    asks_the_owner(&mut h, wc);
    asks_the_owner(&mut h, wb);
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
        asks_the_owner(&mut h, *w);
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
    asks_the_owner(&mut h, wb);
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
    asks_the_owner(&mut h, wa);
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
        inputs = ["start", "request"]
        outputs = ["text"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        requires = { test = "may_merge" }
        inputs = ["text"]
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
