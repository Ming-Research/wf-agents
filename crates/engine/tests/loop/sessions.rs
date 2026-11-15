//! A session of a line that began is recorded at its place, as what happened to the work; what someone outside the
//! work adds as it takes something is taken before the walk decides; and a gate that holds a line, as a keep-warm ping
//! of its session under way does, holds the rounds of that line alone.

use decide::Room;
use engine::{Gate, Session, Why, handle_then, handle_with};

use super::*;

#[test]
fn a_new_session_of_a_line_is_recorded_at_its_place() {
    let (mut w, ..) = W::requested();
    let effects = w.on(Event::Anew {
        line: "other".to_string(),
        why: Why::Owner,
    });
    assert_eq!(effects, [], "the round under way goes on");
    let at = w.work.tick();
    assert_eq!(
        w.work.sessions,
        [Session {
            at: Seq(at.0 - 1),
            line: "other".to_string(),
            why: Why::Owner,
        }]
    );
}

#[test]
fn a_new_session_of_a_line_the_flow_has_no_cell_on_is_refused() {
    let (mut w, ..) = W::requested();
    let why = w.refused(Event::Anew {
        line: "elsewhere".to_string(),
        why: Why::Owner,
    });
    assert_eq!(why, Refused::NoSuchLine("elsewhere".to_string()));
}

#[test]
fn each_try_of_the_round_under_way_is_recorded_with_what_it_ran_on() {
    let (mut w, run, _) = W::requested();
    let on = |model: &str| engine::RanOn {
        level: "1".to_string(),
        model: model.to_string(),
        effort: None,
        account: None,
    };
    let ran = |attempt, model| Event::Ran {
        run,
        attempt,
        on: on(model),
    };
    assert_eq!(w.on(ran(1, "a")), []);
    w.on(ran(2, "b"));
    let recorded: Vec<(usize, usize, &str)> = (w.work.ran.iter())
        .map(|r| (r.round, r.attempt, r.on.model.as_str()))
        .collect();
    assert_eq!(recorded, [(1, 1, "a"), (1, 2, "b")]);
    w.returns(run, hands_in(&[("text", None)]));
    let why = w.refused(ran(1, "c"));
    assert_eq!(why, Refused::NoRoundUnderWay(run), "no round of the run is under way");
}

#[test]
fn what_follows_from_outside_is_taken_before_the_walk_decides() {
    let (mut w, run, _) = W::requested();
    // As the writer's round ends, its run finishes, and then the outside writes `main`, which the writer reads: the
    // walk runs the writer again for it, where without the write the judge would run.
    let then = |_: &Compiled, _: &Work| vec![wrote("main", Outsider::Outside, true, None)];
    let now = w.tick();
    let came = hands_in(&[("text", None)]);
    let effects = handle_then(
        &w.flow,
        &mut w.work,
        now,
        Event::Returned {
            run,
            came,
            spent: spent(1),
        },
        Room::FREE,
        Gate::OPEN,
        &then,
    )
    .expect("taken");
    assert_eq!(w.work.records.written.last().map(|w| w.block.as_str()), Some("main"));
    let (next, cell, number, given) = round(&effects);
    assert_eq!((cell.as_str(), number, given), ("write", 1, Given::Task));
    assert_ne!(next, run, "a new run of the writer");
    assert!(
        w.work.records.finished.iter().any(|f| f.run == run),
        "the run finished first"
    );
}

// While a keep-warm ping of a line's session is under way the gate holds that line's rounds, and no other's: the
// judge's round, of another line, begins beside it, and the writer's, of the line pinged, waits and begins once the gate
// no longer holds its line. Well under a millisecond alone, the work in memory; no other test of the loop holds a gate
// of lines.
#[test]
fn a_gate_that_holds_a_line_holds_the_rounds_of_its_cells_and_no_others() {
    let (mut w, write, _) = W::requested();
    let pinged = Gate {
        lines: ["main".to_string()].into(),
        ..Gate::OPEN
    };
    let held = |w: &mut W, event| {
        let now = w.tick();
        handle_with(&w.flow, &mut w.work, now, event, pinged.clone()).expect("taken")
    };
    let returned = Event::Returned {
        run: write,
        came: hands_in(&[("text", None)]),
        spent: spent(1),
    };
    let (judge, cell, ..) = round(&held(&mut w, returned));
    assert_eq!(cell, "judge", "a round of another line is held");
    // The judge's opinions run the writer again: its run does not start while the gate holds its line.
    let effects = held(
        &mut w,
        Event::Returned {
            run: judge,
            came: hands_in(&[("opinions", None), ("verdict", Some("no"))]),
            spent: spent(2),
        },
    );
    assert_eq!(effects, [], "a round of the line held began");
    let writes = (w.work.records.started.iter())
        .filter(|s| s.cell.as_str() == "write")
        .count();
    assert_eq!(writes, 1, "a run of the line held started");
    let (_, cell, ..) = round(&w.on(Event::Look));
    assert_eq!(cell, "write", "the writer runs once its line is no longer held");
}
