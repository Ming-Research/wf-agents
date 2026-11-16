//! The project's guide changing midway on the engineer's flow, single works taken through `handle` step by step (design
//! document `data-and-context`, 22, 24). Every agent's cell watches the guide; a round the guide alone runs looks over
//! the cell's outputs for what the change touches and, where it touches nothing, hands in nothing new, which is new to
//! no cell, so that the change ends there (9). A version of the guide read as a round of a line that may change the work
//! ended is that round's, and so no change to its own cell (decision a-version-read-at-a-rounds-end-is-that-rounds).
//!
//! The engineer's histories write the guide once, as the start reads it, and so does the exploration: no history or
//! path changes it midway. The state the second and third tests change it in, a judge's fail standing with nothing new
//! to it, lies past the exploration's bound besides (its list of what it never reaches: "the work waited at M1 on the
//! test settled"). So the rule is pinned here, by example.

use decide::questions::questions;
use decide::run::Unmet;
use decide::{CellId, Decision, Outsider, Pauser, RunId};
use engine::{Came, Effect, Event, Given, Question, Spent, Way, pending};

use super::single::{W, wrote};

/// The guide written from outside with this text, as the work's branch has it.
fn guide(text: &str) -> Event {
    Event::Wrote {
        block: "guide".into(),
        by: Outsider::Outside,
        content: text.to_string(),
        matters: true,
        value: None,
    }
}

/// The first version of the guide, as the start's act reads it as it makes the work's place.
const FIRST: &str = "Rule 1.";

/// The guide as main changes it, on the branch the works merge into.
const CHANGED: &str = "Rule 1.\nRule 2.";

/// What a cell hands in: each block, with its value where it is a choice.
type Blocks = &'static [(&'static str, Option<&'static str>)];

/// Each agent's cell before the implementation that a change of the guide alone runs, in the order of the walk, with
/// what it hands in when the change touches nothing of what it writes: what it must hand in every time, as it stands.
const BEFORE_THE_IMPLEMENTATION: [(&str, Blocks); 5] = [
    ("R0", &[]),
    ("R1", &[("deep_needed", Some("no"))]),
    ("R2", &[]),
    ("D3", &[("plan_ok", Some("yes"))]),
    ("D4", &[("tree_ok", Some("yes"))]),
];

/// The implementation, which a change of the guide that touches nothing of what it writes runs, handing in nothing.
const THE_IMPLEMENTATION: [(&str, Blocks); 1] = [("I1", &[])];

/// The work from its start, whose act read the guide's first version, to the review's fail of the implementation's code,
/// committed at `c1`, with its points: the research writes the plan with no deeper look, nothing better is found, both
/// reviews of the plan are content, and the review asks for no testing.
fn failed_by_the_review() -> W {
    let mut w = W::started();
    w.on(guide(FIRST));
    w.hands_in("S1", &[("place", None)], Vec::new());
    w.hands_in("R0", &[], Vec::new());
    w.hands_in(
        "R1",
        &[
            ("deep_needed", Some("no")),
            ("plan_owner", None),
            ("plan_agent", None),
            ("acceptance", None),
        ],
        Vec::new(),
    );
    w.hands_in("R2", &[], Vec::new());
    w.hands_in("D3", &[("plan_ok", Some("yes"))], Vec::new());
    w.hands_in("D4", &[("tree_ok", Some("yes"))], Vec::new());
    w.commits("I1", &[("code", None), ("evidence", None)], &[("head", "c1")]);
    w.hands_in(
        "T1",
        &[
            ("review_ok", Some("no")),
            ("needs_testing", Some("no")),
            ("review", None),
        ],
        Vec::new(),
    );
    w
}

/// Each of these cells runs next, in this order, the guide alone running it, and hands in these blocks.
fn the_guide_alone_runs(w: &mut W, cells: &[(&str, Blocks)]) {
    for (cell, blocks) in cells {
        let (run, at) = w.round();
        assert_eq!(at, *cell, "the walk runs {at} where {cell} was expected");
        assert_eq!(w.told(run, cell), ["guide"], "{cell} is run by the guide alone");
        w.hands_in(cell, blocks, Vec::new());
    }
}

/// Why the engine sent the run's return back, where it gave the run's next round for that.
fn sent_back(w: &W, run: RunId) -> Option<Vec<Unmet>> {
    (pending(&w.flow, &w.work).into_iter()).find_map(|effect| match effect {
        Effect::Round {
            run: r,
            given: Given::SentBack(unmet),
            ..
        } if r == run => Some(unmet),
        _ => None,
    })
}

/// Whether the work waits at the check before the merge.
fn waits_at_the_check(d: &Decision) -> bool {
    matches!(d, Decision::Wait { at, .. } if at.as_str() == "M1")
}

// A version of the guide read as the implementation's round ended, its round having merged main in, is that round's:
// the implementation does not run again for it. Each other agent's cell before the review runs once, the guide alone
// running it, and hands in nothing new; the review takes the guide with the implementation's answer and its commit,
// and judges again as it would; the check runs on the head it saw, and nothing is new to any cell before it (design
// document `data-and-context`, 9, 22).
#[test]
fn a_guide_read_as_the_implementation_s_round_ended_runs_every_other_agent_cell_once_and_the_change_ends_there() {
    let mut w = failed_by_the_review();
    let (run, cell) = w.round();
    assert_eq!(cell, "I1", "the implementation takes the review's points");
    assert_eq!(w.told(run, "I1"), ["review"]);
    // The implementation answers each point, committing the fix at `c2`, and the main its round merged in changed the
    // guide: the round's end reads both on the branch.
    w.commits("I1", &[("review", None)], &[("head", "c2"), ("guide", CHANGED)]);
    the_guide_alone_runs(&mut w, &BEFORE_THE_IMPLEMENTATION);
    let (run, cell) = w.round();
    assert_eq!(
        cell,
        "T1",
        "the implementation does not run for the guide its own round read: {:?}",
        w.decision()
    );
    let mut told = w.told(run, "T1");
    told.sort();
    assert_eq!(told, ["change", "guide", "review"]);
    w.hands_in(
        "T1",
        &[("review_ok", Some("yes")), ("needs_testing", Some("no"))],
        Vec::new(),
    );
    assert_eq!(w.round().1, "M1", "the check runs on the head the review saw");
    for cell in ["R0", "R1", "R2", "D3", "D4", "I1", "T1"] {
        let new = decide::new_to(&w.flow.flow, &w.work.records, &CellId::from(cell));
        assert!(new.is_empty(), "{new:?} is new to {cell}");
    }
}

/// The implementation's question that puts the review's points to the owner, recommending its own side.
fn dispute() -> Question {
    let way = |does: &str| Way {
        does: does.to_string(),
        follows: format!("what follows from {does}"),
    };
    Question::Asked {
        name: "dispute".to_string(),
        waits: true,
        recommends: Some(1),
        ask: "Whether the change stands as it is, against the review's points.".to_string(),
        options: vec![way("It stands as it is"), way("Change it as the review says")],
        about: None,
        reasons: Default::default(),
    }
}

// The review's fail stands with nothing new to it: the implementation put the dispute over its points to the owner, a
// question the work waits for, dealing with the points (design document `data-and-context`, 41), and, run by the
// owner's answer, found nothing to change and handed in nothing (43); the work waits at the check for the verdict.
// Then the guide changes, touching nothing the review judges. Every agent's cell runs once for it; the review, run by
// the guide alone, owes no word in its dialogue, which no answer and no change reopened (41: the judge owes it "woken
// by an answer or a change"), and hands in its verdict as it stands, every time, with no point. Expected by 22 and 9:
// the return is taken, it is new to no cell, the change ends there, and the work waits at the check again.
//
// 42 sends back a judge's fail that brings no point as a return that plainly leaves the word owed on the judge, which
// owes it only woken by an answer or a change (41), as a maker owes its word only woken by a point. Were this return
// sent back for bringing no point, the only returns the review could have taken would be a pass, saying the work passes
// though nothing it judges changed, or points that change meaning, which wake the implementation: a change that does
// not end where 22 says it does.
#[test]
fn a_review_whose_fail_stands_and_that_the_guide_alone_runs_hands_in_its_fail_unchanged_and_the_change_ends_there() {
    let mut w = failed_by_the_review();
    let (run, cell) = w.round();
    assert_eq!(cell, "I1");
    assert_eq!(w.told(run, "I1"), ["review"]);
    let mut came = Came {
        dealt_with: Some(vec!["review".into()]),
        ..Came::default()
    };
    came.said.questions = vec![dispute()];
    w.on(Event::Returned {
        run,
        came,
        spent: Spent::default(),
    });
    assert!(
        matches!(w.decision(), Decision::Wait { ref at, .. } if at.as_str() == "I1"),
        "the work waits for the answer at the implementation: {:?}",
        w.decision()
    );
    let version = questions(&w.work.records)["dispute"].latest.at;
    w.on(Event::Answer {
        question: "dispute".into(),
        version,
        option: 1,
    });
    let (run, cell) = w.round();
    assert_eq!(cell, "I1", "the answer runs the implementation");
    assert_eq!(w.told(run, "I1"), ["i1_questions"]);
    w.returns("I1", &["i1_questions"], &[]);
    assert!(waits_at_the_check(&w.decision()), "{:?}", w.decision());

    w.on(guide(CHANGED));
    the_guide_alone_runs(&mut w, &BEFORE_THE_IMPLEMENTATION);
    the_guide_alone_runs(&mut w, &THE_IMPLEMENTATION);
    let (run, cell) = w.round();
    assert_eq!(cell, "T1");
    assert_eq!(w.told(run, "T1"), ["guide"], "the review is run by the guide alone");
    w.hands_in(
        "T1",
        &[("review_ok", Some("no")), ("needs_testing", Some("no"))],
        Vec::new(),
    );
    assert_eq!(
        sent_back(&w, run),
        None,
        "the review, run by the guide alone with its fail standing, was sent back for handing in nothing new"
    );
    assert!(
        waits_at_the_check(&w.decision()),
        "the change does not end at the review: {:?}",
        w.decision()
    );
}

// The owner ruled the review's points in the review's place, at no: the points the review wrote before are new to no
// cell from then on, and the verdict stands until something the review watches is new to it; the review then judges
// again, and raises nothing the owner ruled on (design document `data-and-context`, 39). The guide alone runs it, and
// touches nothing it judges: it hands in its fail as it stands, with no point, since the only points it has are those
// the owner ruled on; expected by 22, 39 and 41, the return is taken, the change ending there.
//
// Sent back for bringing no point, as the test before says it may not be, the review would be left only a pass, against
// the owner's ruling and its own judgment, or points it may not raise (39).
#[test]
fn a_review_the_owner_ruled_at_no_that_the_guide_alone_runs_hands_in_its_fail_unchanged_and_the_change_ends_there() {
    let mut w = W::started();
    w.on(guide(FIRST));
    w.hands_in("S1", &[("place", None)], Vec::new());
    w.hands_in("R0", &[], Vec::new());
    w.hands_in(
        "R1",
        &[
            ("deep_needed", Some("no")),
            ("plan_owner", None),
            ("plan_agent", None),
            ("acceptance", None),
        ],
        Vec::new(),
    );
    w.hands_in("R2", &[], Vec::new());
    w.hands_in("D3", &[("plan_ok", Some("yes"))], Vec::new());
    w.hands_in("D4", &[("tree_ok", Some("yes"))], Vec::new());
    w.commits("I1", &[("code", None), ("evidence", None)], &[("head", "c1")]);
    // The owner pauses the work while the review judges, so that its points run nothing before the owner rules.
    assert_eq!(w.round().1, "T1");
    w.on(Event::Paused {
        by: Pauser::Owner,
        why: None,
    });
    w.hands_in(
        "T1",
        &[
            ("review_ok", Some("no")),
            ("needs_testing", Some("no")),
            ("review", None),
        ],
        Vec::new(),
    );
    w.on(wrote("review_ok", Outsider::Person, Some("no")));
    w.on(Event::Resumed);
    let new = decide::new_to(&w.flow.flow, &w.work.records, &CellId::from("I1"));
    assert!(
        new.is_empty(),
        "a point the owner ruled on runs the implementation: {new:?}"
    );
    assert!(waits_at_the_check(&w.decision()), "{:?}", w.decision());

    w.on(guide(CHANGED));
    the_guide_alone_runs(&mut w, &BEFORE_THE_IMPLEMENTATION);
    the_guide_alone_runs(&mut w, &THE_IMPLEMENTATION);
    let (run, cell) = w.round();
    assert_eq!(cell, "T1");
    assert_eq!(w.told(run, "T1"), ["guide"], "the review is run by the guide alone");
    w.hands_in(
        "T1",
        &[("review_ok", Some("no")), ("needs_testing", Some("no"))],
        Vec::new(),
    );
    assert_eq!(
        sent_back(&w, run),
        None,
        "the review the owner ruled at no, run by the guide alone, was sent back for handing in nothing new"
    );
    assert!(
        waits_at_the_check(&w.decision()),
        "the change does not end at the review: {:?}",
        w.decision()
    );
}
