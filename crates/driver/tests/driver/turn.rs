//! The turn of a project to check and merge, as the driver gives it to the works: of the works of a project due at a
//! cell that takes the turn only one begins, the one whose walk came to want it first (the lower number where at the
//! same moment), and it keeps the turn through the cells that take it, the waits the engine makes for its run among
//! them, until its walk stands elsewhere or its own pause lets it go; a work that let go takes the turn back only once
//! it is given to it again; the others wait, a paused one passed over and keeping its place; and the turn is the same
//! after a restart, being worked out from the works' records alone.

use decide::Holder;
use decide::run::Failure;
use flow::{Compiled, Has, Source, compile};

use super::*;

/// A check of what main says, which takes the turn, and the merge, which takes it too and is the last cell.
fn turning() -> Compiled {
    turning_of(false, "Check main.")
}

/// The same, with a writer before the check, which takes no turn, where `written` says so, and the check told to do
/// this task.
fn turning_of(written: bool, task: &str) -> Compiled {
    let flow = format!(
        r#"
        name = "turning"
        order = [{order}]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.main]
        [blocks.main]
        outside = "main"
        {text}
        [blocks.verdict]
        [blocks.merged]
        shown = true
    "#,
        order = if written {
            r#""write", "check", "land""#
        } else {
            r#""check", "land""#
        },
        text = if written { "[blocks.text]" } else { "" }
    );
    let write = r#"
        title = "Write"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Write the text."
        watches = ["main"]
        outputs = ["text"]

        [kinds]
        main = "last_and_latest"
    "#;
    let watched = if written { "text" } else { "main" };
    let check = format!(
        r#"
        title = "Check"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "{task}"
        takes_turn = true
        watches = ["{watched}"]
        outputs = ["verdict"]

        [kinds]
        {watched} = "last_and_latest"
    "#
    );
    let land = r#"
        title = "Land"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Land the work."
        takes_turn = true
        watches = ["verdict"]
        outputs = ["merged"]

        [kinds]
        verdict = "last_and_latest"
    "#;
    let mut cells = vec![("check", check.as_str()), ("land", land)];
    if written {
        cells.insert(0, ("write", write));
    }
    let source = Source {
        flow,
        cells: (cells.iter())
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
        ..Default::default()
    };
    let has = Has {
        sources: ["dependencies".to_string(), "main".to_string()].into(),
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    compile(&source, &has).expect("the flow compiles")
}

/// The work waits at its check for the turn.
fn waits_for_the_turn(at: &str) -> Decision {
    Decision::Wait {
        at: at.into(),
        on: vec![Waited::Turn],
    }
}

/// A driver with these works let on, each of the flow, with main said to have moved, which makes each due to check at
/// once. Gives the driver, the works and what moving main gave.
fn due(works: usize) -> (H, Vec<WorkNumber>, Vec<Todo>) {
    let mut h = H::with_slots(vec![turning()], 8);
    let numbers: Vec<WorkNumber> = (0..works)
        .map(|n| h.add_let_on(new_item(&format!("work {n}"), &[], "turning")).1)
        .collect();
    let todos = h.outside("main", "Main is at 1.");
    (h, numbers, todos)
}

/// The rounds among what is to be done, each by its work, its cell and its number: the engine's release of a work that
/// merged is no round.
fn begun(todos: &[Todo]) -> Vec<(WorkNumber, String, usize)> {
    (todos.iter())
        .filter_map(|todo| match &todo.effect {
            Effect::Round { cell, number, .. } => Some((todo.work, cell.to_string(), *number)),
            _ => None,
        })
        .collect()
}

/// The work's round under way hands in this block, which its cell wrote.
fn finishes(h: &mut H, work: WorkNumber, block: &str) -> Vec<(WorkNumber, String, usize)> {
    begun(&h.hands_in(work, &[(block, None)]))
}

/// The one call the engine made among what is to be done.
fn call_made(todos: &[Todo]) -> CallId {
    let calls: Vec<CallId> = (todos.iter())
        .filter_map(|todo| match &todo.effect {
            Effect::Call { call, .. } => Some(*call),
            _ => None,
        })
        .collect();
    assert_eq!(calls.len(), 1, "one call is made: {todos:?}");
    calls[0]
}

/// The work's own pause, by the owner.
fn pauses(h: &mut H, work: WorkNumber) {
    h.on(
        work,
        Event::Paused {
            by: Pauser::Owner,
            why: None,
            waits_for: Vec::new(),
        },
    );
}

#[test]
fn of_two_works_due_to_check_at_once_the_lower_number_begins_and_the_other_waits_for_the_turn() {
    let (mut h, works, todos) = due(2);
    let (a, b) = (works[0], works[1]);
    assert_eq!(begun(&todos), [(a, "check".to_string(), 1)]);
    assert_eq!(h.d().held(a), None);
    assert_eq!(h.d().held(b), Some(Holder::Turn));
    assert_eq!(h.decision(b), waits_for_the_turn("check"));
    // The first keeps the turn through its merge: the check that passed is followed by the merge in the same step, the
    // second still waiting.
    assert_eq!(finishes(&mut h, a, "verdict"), [(a, "land".to_string(), 1)]);
    assert_eq!(h.d().held(b), Some(Holder::Turn));
    assert_eq!(h.decision(b), waits_for_the_turn("check"));
    // The second begins in the step in which the first has merged.
    assert_eq!(finishes(&mut h, a, "merged"), [(b, "check".to_string(), 1)]);
    assert_eq!(h.d().held(b), None);
}

// What the page says of a work that waits for the turn is what the driver gives it: whose turn it is, which is none for
// the holder itself, and since when the work has wanted it, which is when its walk came to the check and goes on
// through the holder's merge. Fails where the time is the holder's, or the turn is named to the work that holds it.
#[test]
fn the_driver_says_whose_turn_a_waiting_work_waits_for_and_since_when_it_has_wanted_it() {
    let (mut h, works, _) = due(3);
    let (a, b, c) = (works[0], works[1], works[2]);
    let wanted = Time(h.clock);
    let at_once = h.d().at_once();
    assert_eq!(h.d().turn_waited_at(a, &at_once), None);
    assert_eq!(h.d().turn_waited_at(b, &at_once), Some((a, wanted)));
    assert_eq!(h.d().turn_waited_at(c, &at_once), Some((a, wanted)));
    // The turn passes to the next, and the one after waits for it from the same time.
    finishes(&mut h, a, "verdict");
    finishes(&mut h, a, "merged");
    let at_once = h.d().at_once();
    assert_eq!(h.d().turn_waited_at(a, &at_once), None);
    assert_eq!(h.d().turn_waited_at(b, &at_once), None);
    assert_eq!(h.d().turn_waited_at(c, &at_once), Some((b, wanted)));
}

// A check that fails in passing is tried again after a wait, which the engine makes as a call of the check's run: the
// work keeps the turn through that wait, so that the other work does not begin its check, and the check tried again is
// in the same hold. Fails where the wait lets go of the turn: the other work begins its check, and the first takes the
// turn back as its wait ends, both checking at once.
#[test]
fn a_check_that_failed_in_passing_keeps_the_turn_while_it_waits_to_be_tried_again() {
    let (mut h, works, _) = due(2);
    let (a, b) = (works[0], works[1]);
    let wanted = Time(h.clock);
    let todos = super::outage::fails(&mut h, a, Failure::Passing, None);
    let wait = call_made(&todos);
    assert_eq!(begun(&todos), [], "the other work does not begin its check");
    assert_eq!(h.d().held(b), Some(Holder::Turn));
    assert_eq!(begun(&h.answer(a, wait)), [(a, "check".to_string(), 2)]);
    assert_eq!(h.d().held(b), Some(Holder::Turn));
    let at_once = h.d().at_once();
    assert_eq!(h.d().turn_waited_at(b, &at_once), Some((a, wanted)));
    finishes(&mut h, a, "verdict");
    assert_eq!(finishes(&mut h, a, "merged"), [(b, "check".to_string(), 1)]);
}

// A work that let go of the turn holds it again only from its next round of a cell that takes it, which begins once the
// turn is given to it: its walk coming back to such a cell does not take the turn back. Here the check's run waits for
// the owner's word at its limit of returns, which lets go of the turn, and the other work begins its check; the word
// comes while that check is under way. Fails where the first takes the turn back as the word comes: two works hold the
// turn, the driver gives it to the first as the one whose round began first, which then begins no round, nothing
// looking at a work that holds the turn; and once the other's round ends, the driver says of it neither whose turn it
// waits for nor since when, which the page cannot show.
#[test]
fn a_work_that_let_go_of_the_turn_waits_for_it_once_its_walk_comes_back_to_the_check() {
    let (mut h, works, _) = due(2);
    let (a, b) = (works[0], works[1]);
    let word = h.at_limit(a);
    assert_eq!(
        h.round(b).1,
        "check",
        "the other work began its check once the first let go"
    );
    assert_eq!(begun(&h.answer(a, word)), [], "the first waits for the turn");
    let came_back = Time(h.clock);
    assert_eq!(h.d().held(a), Some(Holder::Turn));
    let at_once = h.d().at_once();
    assert_eq!(at_once.turn_of(Some(PROJECT)), Some(b));
    assert_eq!(h.d().turn_waited_at(a, &at_once), Some((b, came_back)));
    // The other keeps the turn through its merge, and the first checks once the other has merged.
    assert_eq!(finishes(&mut h, b, "verdict"), [(b, "land".to_string(), 1)]);
    let after: Vec<(WorkNumber, String)> = (finishes(&mut h, b, "merged").into_iter())
        .map(|(work, cell, _)| (work, cell))
        .collect();
    assert_eq!(after, [(a, "check".to_string())]);
}

#[test]
fn three_works_take_the_turn_one_after_another_in_the_order_of_their_numbers_where_they_began_to_want_it_together() {
    let (mut h, works, todos) = due(3);
    let (a, b, c) = (works[0], works[1], works[2]);
    assert_eq!(begun(&todos), [(a, "check".to_string(), 1)]);
    finishes(&mut h, a, "verdict");
    assert_eq!(finishes(&mut h, a, "merged"), [(b, "check".to_string(), 1)]);
    assert_eq!(h.d().held(c), Some(Holder::Turn));
    finishes(&mut h, b, "verdict");
    assert_eq!(finishes(&mut h, b, "merged"), [(c, "check".to_string(), 1)]);
}

#[test]
fn a_work_of_another_project_is_not_held_by_the_turn_of_this_one() {
    let mut h = H::with_slots(vec![turning()], 8);
    let (_, a) = h.add_let_on(new_item("a", &[], "turning"));
    let (_, b) = h.add_let_on(driver::NewItem {
        project: "two".to_string(),
        ..new_item("b", &[], "turning")
    });
    let todos = h.outside("main", "Main is at 1.");
    assert_eq!(begun(&todos), [(a, "check".to_string(), 1)]);
    let todos = h.outside_of("two", "main", "Main is at 1.");
    assert_eq!(begun(&todos), [(b, "check".to_string(), 1)]);
    assert_eq!(h.d().held(b), None);
}

#[test]
fn the_pause_of_everything_keeps_the_work_that_holds_the_turn_its_turn() {
    let (mut h, works, _) = due(2);
    let (a, b) = (works[0], works[1]);
    h.pause_everything(true);
    // What the round under way hands in is taken, and the merge does not begin while the pause holds.
    assert_eq!(finishes(&mut h, a, "verdict"), []);
    // Lifted, the holder goes on to its merge and the other work does not begin its check.
    assert_eq!(begun(&h.pause_everything(false)), [(a, "land".to_string(), 1)]);
    assert_eq!(h.d().held(b), Some(Holder::Turn));
}

#[test]
fn a_paused_work_that_waits_for_the_turn_is_passed_over_and_keeps_its_place() {
    let (mut h, works, _) = due(3);
    let (a, b, c) = (works[0], works[1], works[2]);
    pauses(&mut h, b);
    finishes(&mut h, a, "verdict");
    // The turn goes to the work after the paused one.
    assert_eq!(finishes(&mut h, a, "merged"), [(c, "check".to_string(), 1)]);
    // Let on again, the paused one waits for the work that has the turn, as one that came to want it before.
    assert_eq!(begun(&h.let_on(b)), []);
    assert_eq!(h.d().held(b), Some(Holder::Turn));
    finishes(&mut h, c, "verdict");
    assert_eq!(finishes(&mut h, c, "merged"), [(b, "check".to_string(), 1)]);
}

#[test]
fn a_work_paused_before_its_check_ended_lets_the_next_begin_and_waits_behind_those_that_wanted_it_before_its_pause() {
    let (mut h, works, _) = due(3);
    let (a, b, c) = (works[0], works[1], works[2]);
    // Its round under way goes on, and what it hands in is taken; its own pause then lets go of the turn.
    pauses(&mut h, a);
    assert_eq!(finishes(&mut h, a, "verdict"), [(b, "check".to_string(), 1)]);
    // Let on, it waits behind the work that began, and the one that wanted the turn before its pause comes before it.
    assert_eq!(begun(&h.let_on(a)), []);
    assert_eq!(h.d().held(a), Some(Holder::Turn));
    finishes(&mut h, b, "verdict");
    assert_eq!(finishes(&mut h, b, "merged"), [(c, "check".to_string(), 1)]);
    finishes(&mut h, c, "verdict");
    assert_eq!(finishes(&mut h, c, "merged"), [(a, "land".to_string(), 1)]);
}

#[test]
fn a_driver_opened_on_the_store_again_gives_the_same_work_the_turn() {
    let (mut h, works, _) = due(3);
    let (a, b, c) = (works[0], works[1], works[2]);
    let todos = h.restart();
    // Nothing of the works that wait is begun, and the holder's round under way is given again.
    assert!(
        (todos.iter()).all(|todo| todo.work == a),
        "only the holder's round is given: {todos:?}"
    );
    assert_eq!(h.d().held(a), None);
    assert_eq!(h.d().held(b), Some(Holder::Turn));
    assert_eq!(h.d().held(c), Some(Holder::Turn));
    assert_eq!(h.decision(b), waits_for_the_turn("check"));
    finishes(&mut h, a, "verdict");
    assert_eq!(finishes(&mut h, a, "merged"), [(b, "check".to_string(), 1)]);
    // And again between the two that are left.
    h.restart();
    assert_eq!(h.d().held(c), Some(Holder::Turn));
    assert_eq!(h.d().held(b), None);
}

// The stretches a work held the turn are kept as it ends, so that a work that waited behind it still draws that wait
// where the flow the holder ended on is no longer installed and its turn can no longer be worked out. Here the second
// work waited, after its writer ran, for the first to merge; another version of the flow is installed after.
#[test]
fn a_work_that_waited_for_a_work_that_ended_still_draws_the_wait_once_another_version_is_installed() {
    let mut h = H::with_slots(vec![turning_of(true, "Check main.")], 8);
    let (_, a) = h.add_let_on(new_item("a", &[], "turning"));
    let (_, b) = h.add_let_on(new_item("b", &[], "turning"));
    let todos = h.outside("main", "Main is at 1.");
    assert_eq!(
        begun(&todos),
        [(a, "write".to_string(), 1), (b, "write".to_string(), 1)]
    );
    assert_eq!(finishes(&mut h, a, "text"), [(a, "check".to_string(), 1)]);
    let a_began = Time(h.clock);
    assert_eq!(finishes(&mut h, b, "text"), []);
    assert_eq!(h.d().held(b), Some(Holder::Turn));
    let b_waited = Time(h.clock);
    finishes(&mut h, a, "verdict");
    assert_eq!(finishes(&mut h, a, "merged"), [(b, "check".to_string(), 1)]);
    let a_ended = Time(h.clock);
    let held = work::TurnSpan {
        from: a_began,
        until: Some(a_ended),
    };
    let kept = h.d().work(a).expect("the work is there").work.kept_time.clone();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].turn, Some(vec![held]));
    // The bar of the work that waited: from the end of its writer's round to the end of the other's merge.
    let waited = |h: &H| -> Vec<(Time, Time)> {
        let driver = h.d();
        let kept = driver.work(b).expect("the work is there");
        let turns: Vec<work::TurnSpan> = (driver.turn_spans().into_iter())
            .filter(|(_, work, _)| *work != b)
            .flat_map(|(_, _, spans)| spans)
            .collect();
        let (cannot_run, down) = driver.shared_waits(&driver.at_once());
        let stretches = driver::time::stretches(
            &kept,
            driver.everything_spans(),
            &driver.stop_spans(),
            &cannot_run,
            &down,
            turns,
            Time(h.clock),
        );
        (stretches.iter())
            .filter(|s| s.went == work::Went::Turn)
            .map(|s| (s.from, s.until))
            .collect()
    };
    assert_eq!(waited(&h), [(b_waited, a_ended)]);
    // Another version is installed: the work that held the turn stays on the one it ended on, which is no longer
    // installed, and what it kept still says that it held the turn.
    h.install(vec![turning_of(true, "Check main again.")]);
    assert!(!h.d().work(a).expect("the work is there").on.installed());
    assert!(h.d().work(b).expect("the work is there").on.installed());
    assert_eq!(
        h.d().work(a).expect("the work is there").work.kept_time[0].turn,
        Some(vec![held])
    );
    assert_eq!(waited(&h), [(b_waited, a_ended)]);
}
