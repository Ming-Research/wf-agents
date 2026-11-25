//! The turn of a project to check and merge (`decide::Room::turn`): where it is not the work's turn, a cell that takes it
//! waits for it before it runs and a run at rest of such a cell has no round; and the walk stands where a hold of the
//! turn goes on, and comes to a cell that takes it, as the work's flow and records say, with nothing kept beside them
//! (`keeps_turn`, `comes_to_turn`). Whether a work holds the turn, its walk having kept it at every moment since its
//! round began, is the driver's to work out over the work's moments.

use decide::run::Hold;
use decide::{Room, Test, TestKind, comes_to_turn, decide_in, hold_in, keeps_turn};

use super::*;

/// Another work of the project holds the turn, a slot free, nothing paused.
const NOT_OURS: Room = Room {
    paused: false,
    slot: true,
    turn: false,
};

/// A plan that a request runs, a check of it that takes the turn and runs again where main moved, and the merge that
/// takes it too, which is the last cell: the work ends where it finishes.
fn checking() -> World {
    let mut check = cell("check", &["plan", "main"], &["verdict"]);
    check.turn = true;
    let mut land = cell("land", &["verdict"], &["landed"]);
    land.turn = true;
    let mut w = World::new(vec![cell("plan", &["request"], &["plan"]), check, land]);
    w.person_writes("main");
    w.person_writes("request");
    w
}

/// The work with its plan made, its check to begin.
fn planned() -> World {
    let mut w = checking();
    w.run("plan", &[("plan", Matters)]);
    w
}

/// Whether the walk stands where a hold of the turn goes on, and whether it comes to a cell that takes the turn.
fn stands(w: &World) -> (bool, bool) {
    (keeps_turn(&w.flow, &w.records), comes_to_turn(&w.flow, &w.records))
}

#[test]
fn where_it_is_not_the_works_turn_a_cell_that_takes_it_waits_and_one_that_takes_none_runs() {
    let mut w = checking();
    assert!(matches!(
        decide_in(&w.flow, &w.records, NOT_OURS),
        Decision::Run { cell, .. } if cell.as_str() == "plan"
    ));
    w.run("plan", &[("plan", Matters)]);
    assert_eq!(
        decide_in(&w.flow, &w.records, NOT_OURS),
        Decision::Wait {
            at: "check".into(),
            on: vec![Waited::Turn],
        }
    );
    assert!(matches!(
        decide_in(&w.flow, &w.records, Room::FREE),
        Decision::Run { cell, .. } if cell.as_str() == "check"
    ));
    assert_eq!(decide_in(&w.flow, &w.records, Room::FREE), w.decide());
}

#[test]
fn the_pause_comes_before_the_turn_and_the_turn_before_the_slot() {
    let w = planned();
    let neither = Room {
        paused: false,
        slot: false,
        turn: false,
    };
    assert_eq!(
        decide_in(&w.flow, &w.records, neither),
        Decision::Wait {
            at: "check".into(),
            on: vec![Waited::Turn],
        }
    );
    let paused_too = Room {
        paused: true,
        ..neither
    };
    assert!(matches!(
        decide_in(&w.flow, &w.records, paused_too),
        Decision::Paused { cell } if cell.as_str() == "check"
    ));
}

#[test]
fn a_run_at_rest_of_a_cell_that_takes_the_turn_goes_on_but_has_no_round_while_it_is_not_the_works_turn() {
    let mut w = planned();
    let check = w.start("check");
    w.returns("check");
    assert_eq!(hold_in(&w.flow, &w.records, check, Room::FREE), Hold::Nothing);
    assert_eq!(hold_in(&w.flow, &w.records, check, NOT_OURS), Hold::Rounds);
    // The walk says to go on whatever the room: what the run does next is the engine's to hold back.
    assert_eq!(decide_in(&w.flow, &w.records, NOT_OURS), go_on("check", check));
    // A cell that takes no turn is not held by it.
    let mut w = checking();
    let plan = w.start("plan");
    w.returns("plan");
    assert_eq!(hold_in(&w.flow, &w.records, plan, NOT_OURS), Hold::Nothing);
}

#[test]
fn a_work_keeps_the_turn_from_its_round_of_a_cell_that_takes_it_and_comes_to_the_cell_before() {
    let mut w = checking();
    assert_eq!(stands(&w), (false, false), "its plan is next, which takes no turn");
    w.start("plan");
    assert_eq!(stands(&w), (false, false));
    w.finish("plan", &[("plan", Matters)]);
    assert_eq!(stands(&w), (false, true), "its check is next");
    w.start("check");
    assert_eq!(stands(&w), (true, false), "its round is under way");
}

#[test]
fn a_check_that_passed_keeps_the_turn_for_its_merge_until_the_merge_has_ended() {
    let mut w = planned();
    w.start("check");
    w.finish("check", &[("verdict", Matters)]);
    assert_eq!(stands(&w), (true, true), "the merge is next, in the same hold");
    w.start("land");
    assert_eq!(stands(&w), (true, false));
    w.finish("land", &[("landed", Matters)]);
    assert!(matches!(w.decide(), Decision::Ended(_)));
    assert_eq!(stands(&w), (false, false), "it has ended");
}

#[test]
fn a_check_again_after_main_moved_and_a_round_tried_again_are_one_hold_with_the_check_before() {
    let mut w = planned();
    w.start("check");
    w.finish("check", &[("verdict", Matters)]);
    w.person_writes("main");
    assert!(matches!(w.decide(), Decision::Run { cell, .. } if cell.as_str() == "check"));
    assert_eq!(
        stands(&w),
        (true, true),
        "the check is to run again, and the work still keeps the turn"
    );
    w.start("check");
    w.returns("check");
    assert_eq!(stands(&w), (true, true), "a run at rest goes on");
}

// A round that failed in passing is tried again after a wait the engine makes as a call of the run: the walk stays at
// the cell through that wait, and the work keeps the turn. Fails where the wait lets go of the turn, so that another
// work begins its check meanwhile, and this one takes the turn back as its wait ends.
#[test]
fn a_round_that_failed_in_passing_keeps_the_turn_while_the_engine_waits_to_try_it_again() {
    let mut w = planned();
    w.start("check");
    let wait = w.calls("check", None);
    assert!(matches!(w.decide(), Decision::Wait { at, .. } if at.as_str() == "check"));
    assert_eq!(
        stands(&w),
        (true, false),
        "the engine waits before it tries the round again"
    );
    w.done(wait);
    assert_eq!(stands(&w), (true, true), "the round is to be tried again");
    w.go_on("check");
    assert_eq!(stands(&w), (true, false), "the round tried again is under way");
}

// The owner's word at a limit is no wait of the engine's: the work lets go of the turn while it waits for it, as for an
// answer.
#[test]
fn a_run_that_waits_for_the_owners_word_at_a_limit_lets_go_of_the_turn() {
    let mut w = planned();
    w.start("check");
    w.calls("check", Some(Who::Owner));
    assert!(matches!(w.decide(), Decision::Wait { at, .. } if at.as_str() == "check"));
    assert_eq!(stands(&w), (false, false));
}

#[test]
fn a_work_lets_go_of_the_turn_as_its_walk_stands_anywhere_else() {
    let mut w = planned();
    w.start("check");
    w.finish("check", &[("verdict", Matters)]);
    assert_eq!(stands(&w), (true, true));
    // Sent back: the request changed, and its plan is to be made again.
    w.person_writes("request");
    assert!(matches!(w.decide(), Decision::Run { cell, .. } if cell.as_str() == "plan"));
    assert_eq!(stands(&w), (false, false));
}

#[test]
fn a_work_that_waits_for_an_answer_at_its_check_neither_keeps_the_turn_nor_comes_to_it() {
    let mut w = planned();
    w.start("check");
    w.finish("check", &[("verdict", Matters)]);
    assert_eq!(stands(&w), (true, true));
    w.questions("check", "which", true);
    assert!(matches!(w.decide(), Decision::Wait { .. }), "{:?}", w.decide());
    assert_eq!(stands(&w), (false, false));
}

#[test]
fn a_cancelled_work_neither_keeps_the_turn_nor_comes_to_it() {
    let mut w = planned();
    w.start("check");
    w.finish("check", &[("verdict", Matters)]);
    assert_eq!(stands(&w), (true, true));
    w.cancel();
    assert_eq!(stands(&w), (false, false));
}

#[test]
fn a_work_paused_while_its_round_is_under_way_keeps_the_turn_until_the_round_ends() {
    let mut w = planned();
    w.start("check");
    w.pause();
    assert_eq!(stands(&w), (true, false), "the round under way runs to its end");
    w.returns("check");
    assert!(!stands(&w).0, "its own pause held it as the round ended");
    assert!(stands(&w).1, "and it comes to the check, which its pause holds back");
}

#[test]
fn a_pause_set_and_lifted_while_the_round_ran_does_not_let_go_of_the_turn() {
    let mut w = planned();
    w.start("check");
    w.pause();
    w.resume();
    w.finish("check", &[("verdict", Matters)]);
    assert_eq!(stands(&w), (true, true));
}

#[test]
fn a_pause_after_the_round_ended_lets_go_and_lifting_it_does_not_take_the_turn_back() {
    let mut w = planned();
    w.start("check");
    w.finish("check", &[("verdict", Matters)]);
    assert_eq!(stands(&w), (true, true));
    w.pause();
    assert_eq!(stands(&w), (false, true), "its merge is next, held by its own pause");
    w.resume();
    assert_eq!(
        stands(&w),
        (false, true),
        "it comes to the merge, and keeps no turn until its next round of such a cell begins"
    );
    w.start("land");
    assert_eq!(stands(&w), (true, false));
}

#[test]
fn a_round_that_failed_and_paused_the_work_lets_go_of_the_turn_and_comes_to_it_again() {
    let mut w = planned();
    w.start("check");
    w.fails("check", "the agents' program could not run");
    assert_eq!(stands(&w), (false, true));
    w.resume();
    assert_eq!(stands(&w), (false, true));
    w.go_on("check");
    assert_eq!(stands(&w), (true, false), "the round tried again is under way");
}

#[test]
fn a_cell_the_flow_no_longer_has_takes_no_turn() {
    let mut w = planned();
    w.start("check");
    w.finish("check", &[("verdict", Matters)]);
    assert_eq!(stands(&w), (true, true));
    // The flow installed since has no check: the work that ran it holds nothing of it.
    let mut land = cell("land", &["verdict"], &["landed"]);
    land.turn = true;
    w.install(vec![cell("plan", &["request"], &["plan"]), land]);
    assert!(!stands(&w).0);
}

/// What a work held back for the items it depends on waits for.
fn held_on() -> Vec<Waited> {
    vec![Waited::Precondition {
        on: "items merged".into(),
        by: vec![Maker::Outside {
            source: "dependencies".to_string(),
        }],
    }]
}

#[test]
fn a_work_held_back_for_items_comes_to_no_turn_begins_no_check_and_lets_go_of_the_turn_between_rounds() {
    // The plan waits for the items the work depends on, which the outside says have merged; the check after it takes
    // the turn.
    let merged = Test {
        kind: TestKind::All,
        reads: vec!["dependencies".into()],
        is: Some("yes".to_string()),
    };
    let mut plan = cell("plan", &["request"], &["plan"]);
    plan.requires = vec!["items merged".into()];
    let mut check = cell("check", &["plan", "main"], &["verdict"]);
    check.turn = true;
    let mut land = cell("land", &["verdict"], &["landed"]);
    land.turn = true;
    let mut w = World::with_tests(
        vec![plan, check, land],
        BTreeMap::from([("items merged".into(), merged)]),
    );
    w.written_outside("dependencies", "dependencies");
    w.outside_sets("dependencies", "yes");
    w.person_writes("main");
    w.person_writes("request");
    w.run("plan", &[("plan", Matters)]);
    assert_eq!(
        stands(&w),
        (false, true),
        "the items have merged, and the check is next"
    );
    // An item it comes to depend on has not merged: it waits where it stands, at the check, begins no round of it in
    // any room, and comes to no turn.
    let held = Decision::Wait {
        at: "check".into(),
        on: held_on(),
    };
    w.outside_sets("dependencies", "no");
    assert_eq!(w.decide(), held);
    assert_eq!(decide_in(&w.flow, &w.records, NOT_OURS), held);
    assert_eq!(stands(&w), (false, false));
    w.outside_sets("dependencies", "yes");
    assert_eq!(stands(&w), (false, true));
    // Its check's round under way holds the turn whatever the items say, and ends as it would; the run at rest after it
    // has no next round while the work waits for the items, and the work holds the turn no more.
    let check = w.start("check");
    w.outside_sets("dependencies", "no");
    assert_eq!(
        stands(&w),
        (true, false),
        "the round under way holds the turn until it ends"
    );
    w.returns("check");
    assert_eq!(
        w.decide(),
        Decision::Wait {
            at: "check".into(),
            on: held_on()
        }
    );
    assert_eq!(
        stands(&w),
        (false, false),
        "a run at rest that waits for the items keeps no turn"
    );
    assert!(matches!(hold_in(&w.flow, &w.records, check, Room::FREE), Hold::Nothing));
    // The items merge: the run goes on. Its walk stands where a hold would go on and comes to the check, but the work
    // let go of the turn while it waited, and wants it again until its next round begins.
    w.outside_sets("dependencies", "yes");
    assert_eq!(w.decide(), go_on("check", check));
    assert_eq!(stands(&w), (true, true));
}
