//! Which blocks of a cell would let a work go on that waits for what cells write and none of which will run, where its
//! run finished as a return would have it: the walk is read on the records with that finish added (decision
//! a-work-no-cell-can-move-is-stalled says what a stall is).

use decide::stuck_blocks;
use decide::{Test, TestKind, TestName};

use super::support::Says;
use super::*;

/// The research, which says whether a deeper look is needed and writes the plan; the implementation of the plan where
/// it is not; the check of the code, which alone writes whether it passed; and the merge, which waits for that. The
/// research's own cell is given, so that a test can say what it may write.
fn research_to_merge(research: Cell) -> World {
    research_to_merge_by(research, true)
}

/// The same, where the check's `checked` is written only by cells, or, `false`, by people too, as where a verdict may be
/// ruled on.
fn research_to_merge_by(research: Cell, by_cells: bool) -> World {
    let named = Test {
        kind: TestKind::All,
        reads: vec!["checked".into()],
        is: Some("yes".to_string()),
    };
    let cells = vec![
        research,
        when("deep", "no", cell("implement", &["plan"], &["code"])),
        cell("check", &["code"], &["checked"]),
        requiring("checked", cell("merge", &["checked"], &[])),
    ];
    let mut w = World::with_tests(cells, BTreeMap::from([(TestName::from("checked"), named)]));
    if by_cells {
        w.written_by_cells("checked");
    }
    w.person_writes("background");
    w
}

fn the_research() -> Cell {
    cell("research", &["background"], &["deep", "plan"])
}

fn deep_is_a_choice() -> BTreeMap<BlockId, Vec<String>> {
    BTreeMap::from([("deep".into(), vec!["yes".to_string(), "no".to_string()])])
}

/// The blocks that would let the work go on, where the research, its round ended, finishes dealing with the background
/// and writing these.
fn stuck_after(w: &mut World, writes: &[(&str, Says)]) -> Vec<BlockId> {
    stuck_after_pausing(w, writes, false)
}

/// The same, where `pausing` has the owner pause the work while the research's round is under way.
fn stuck_after_pausing(w: &mut World, writes: &[(&str, Says)], pausing: bool) -> Vec<BlockId> {
    let run = w.start("research");
    if pausing {
        w.pause();
    }
    w.returns("research");
    let writes: Vec<Write> = writes
        .iter()
        .map(|(block, says)| Write {
            block: (*block).into(),
            meaning_unchanged: false,
            empty: false,
            value: match says {
                Says::Value(v) => Some((*v).to_string()),
                _ => None,
            },
        })
        .collect();
    stuck_blocks(
        &w.flow,
        &w.records,
        run,
        &["background".into()],
        &writes,
        &deep_is_a_choice(),
    )
}

// The item's case: the research says no deeper look is needed and hands in no plan, so nothing runs the implementation,
// the check never writes what the merge waits for, and the work stalls. The plan is what would let it go on.
#[test]
fn a_research_that_says_no_deeper_look_with_no_plan_would_leave_the_work_stuck_for_the_plan() {
    let mut w = research_to_merge(the_research());
    assert_eq!(stuck_after(&mut w, &[("deep", Value("no"))]), ["plan".into()]);
}

#[test]
fn a_research_that_hands_in_the_plan_beside_its_choice_leaves_the_work_able_to_go_on() {
    let mut w = research_to_merge(the_research());
    assert!(stuck_after(&mut w, &[("deep", Value("no")), ("plan", Matters)]).is_empty());
}

#[test]
fn a_plan_that_has_a_version_already_is_no_block_the_return_lacks() {
    let mut w = research_to_merge(the_research());
    w.run("research", &[("deep", Value("no")), ("plan", Matters)]);
    w.run("implement", &[("code", Matters)]);
    w.run("check", &[("checked", Value("no"))]);
    w.person_writes("background");
    // The research runs again for the background and changes nothing: the work would stall at the merge, but the plan
    // has a version, so there is no block of the research's that has none.
    assert!(stuck_after(&mut w, &[("deep", Value("no"))]).is_empty());
}

#[test]
fn a_stall_no_block_of_the_cell_would_end_is_the_flows_and_names_nothing() {
    let mut w = research_to_merge(cell("research", &["background"], &["deep"]));
    assert!(stuck_after(&mut w, &[("deep", Value("no"))]).is_empty());
}

#[test]
fn a_dialogue_block_of_the_cell_is_no_block_it_is_told_to_write() {
    let mut research = the_research();
    research.dialogues.insert("plan".into(), vec!["code".into()]);
    let mut w = research_to_merge(research);
    assert!(stuck_after(&mut w, &[("deep", Value("no"))]).is_empty());
}

// The work waits at the merge for what the check writes, which the owner may write too, as where a verdict may be ruled
// on: no call, question or limit is on its way, and the check, behind the research, will not run, so it is as stuck as
// where only cells write it, and was so in the item's case.
#[test]
fn a_precondition_the_owner_may_also_make_hold_is_one_no_cell_will_make_hold_either() {
    let mut w = research_to_merge_by(the_research(), false);
    assert_eq!(stuck_after(&mut w, &[("deep", Value("no"))]), ["plan".into()]);
    let mut w = research_to_merge_by(the_research(), false);
    assert!(stuck_after(&mut w, &[("deep", Value("no")), ("plan", Matters)]).is_empty());
}

// The pause of the work holds back what a cell would run and nothing the walk says before that (a wait, a stall), so a
// paused work is as stuck as one that is not, and one that a cell would go on with is not stuck for being paused.
#[test]
fn a_paused_work_is_stuck_where_it_would_be_if_it_were_not_and_not_where_a_cell_would_go_on() {
    let mut w = research_to_merge(the_research());
    assert_eq!(
        stuck_after_pausing(&mut w, &[("deep", Value("no"))], true),
        ["plan".into()]
    );
    let mut w = research_to_merge(the_research());
    assert!(stuck_after_pausing(&mut w, &[("deep", Value("no")), ("plan", Matters)], true).is_empty());
}

// A work that waits at its last cell for what only the owner writes is not waiting for what cells write: it is to be
// woken by the owner, and no return of the research is held for it.
#[test]
fn a_work_that_waits_only_for_the_owner_is_not_stuck() {
    let cells = vec![
        the_research(),
        when("deep", "no", cell("implement", &["plan"], &["code"])),
        cell("close", &["finish"], &[]),
    ];
    let mut w = World::new(cells);
    w.person_writes("background");
    assert!(stuck_after(&mut w, &[("deep", Value("no"))]).is_empty());
}

// A block the cell watches would run the cell itself again, not let the work go on.
#[test]
fn a_block_the_cell_itself_watches_is_no_block_it_is_told_to_write() {
    let mut w = research_to_merge(cell("research", &["background", "plan"], &["deep", "plan"]));
    assert!(stuck_after(&mut w, &[("deep", Value("no"))]).is_empty());
}
