//! A session of a line that began is recorded at its place, as what happened to the work; and what someone outside the
//! work adds as it takes something is taken before the walk decides.

use decide::Room;
use engine::{Gate, Session, Why, handle_then};

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
        Gate::Open,
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
