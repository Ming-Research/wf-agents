//! The next word in a dialogue (design document `data-and-context`, 41, 42): a maker woken by a point of the other side
//! answers it in the dialogue, changes what the other side watches or asks a question the work waits for; a judge's
//! fail brings the points that still stand. A return that plainly leaves the word owed is sent back.

use super::*;

/// The cell as the maker of the dialogue `opinions`, whose other side watches the plan: its round was run by a point in
/// it, and by the request.
fn maker() -> Spec {
    let mut s = spec();
    s.outputs.push("opinions".into());
    s.dialogues = BTreeMap::from([("opinions".into(), vec!["plan".into()])]);
    s.told.points = vec!["opinions".into()];
    s
}

/// The cell as a judge, whose verdict does not pass at no and is paired with its points in the dialogue `opinions`: its
/// round was woken to judge by an answer in it.
fn judge() -> Spec {
    let mut s = spec();
    s.outputs.push("opinions".into());
    s.told.woken_to_judge = vec!["opinions".into()];
    let verdict = Verdict {
        fails: "no".to_string(),
        dialogue: "opinions".into(),
    };
    s.verdicts = BTreeMap::from([("verdict".into(), verdict)]);
    s
}

/// A round whose return dealt with everything that ran it, wrote nothing, and committed what the carrier read as the
/// round ended.
fn committed(read: &[&str]) -> Round {
    Round {
        returned: Some(Return::Came {
            dealt_with: vec!["request".into(), "opinions".into()],
            writes: vec![],
            calls: 0,
            found: vec![],
            waits: false,
            questions: Vec::new(),
            read: read.iter().map(|&b| b.into()).collect(),
        }),
    }
}

#[test]
fn a_maker_that_names_its_dialogue_as_dealt_with_and_answers_nothing_is_sent_back_for_the_word_it_owes() {
    let unmet = vec![Unmet::WordOwed {
        name: "opinions".into(),
        watched: vec!["plan".into()],
    }];
    assert_eq!(step(vec![handed_in(vec![])], &maker()), send_back(unmet));
}

#[test]
fn a_maker_that_answers_changes_what_the_other_side_watches_or_asks_what_the_work_waits_for_owes_nothing() {
    let s = maker();
    // An answer in the dialogue.
    let answer = vec![prose("opinions")];
    assert_eq!(step(vec![handed_in(answer.clone())], &s), finish(answer));
    // A change of what the other side watches.
    let changed = vec![prose("plan")];
    assert_eq!(step(vec![handed_in(changed.clone())], &s), finish(changed));
    // What the round committed, read as it ended, which it hands in.
    assert_eq!(step(vec![committed(&["plan"])], &s), finish(vec![prose("plan")]));
    // A question the work waits for, the dispute put to the owner.
    let asking = came(&["request", "opinions"], vec![], vec![], true);
    assert_eq!(step(vec![asking], &s), finish(vec![]));
}

#[test]
fn a_maker_that_leaves_its_dialogue_out_of_what_it_dealt_with_or_was_run_by_no_point_in_it_owes_nothing_by_it() {
    let s = maker();
    let left_out = Step::Finish {
        dealt_with: vec!["request".into()],
        writes: vec![],
    };
    assert_eq!(step(vec![works(&["request"], vec![])], &s), left_out);
    // Run by the request alone, its dialogue holds no point new to it.
    let mut s = maker();
    s.told.ran_by = vec!["request".into()];
    s.told.points.clear();
    assert_eq!(step(vec![works(&["request"], vec![])], &s), left_out);
    // Run by the dialogue with no point new in it, as by another maker's answer or a point the judge has since passed
    // over, it owes nothing there (design document `data-and-context`, 43).
    let mut s = maker();
    s.told.points.clear();
    assert_eq!(step(vec![handed_in(vec![])], &s), finish(vec![]));
}

#[test]
fn a_judge_s_fail_that_brings_no_point_is_sent_back_and_one_with_its_points_or_a_pass_is_taken() {
    let s = judge();
    let unmet = vec![Unmet::NoPoint {
        block: "verdict".into(),
        value: "no".to_string(),
        name: "opinions".into(),
    }];
    assert_eq!(
        step(vec![handed_in(vec![valued("verdict", "no")])], &s),
        send_back(unmet)
    );
    let with_points = vec![valued("verdict", "no"), prose("opinions")];
    assert_eq!(step(vec![handed_in(with_points.clone())], &s), finish(with_points));
    let pass = vec![valued("verdict", "yes")];
    assert_eq!(step(vec![handed_in(pass.clone())], &s), finish(pass));
}

/// A block written as changing nothing in meaning.
fn unchanged(block: &str) -> Write {
    Write {
        meaning_unchanged: true,
        ..prose(block)
    }
}

// A version handed in as changing nothing is new to no cell (design document `data-and-context`, 9): it answers no
// point, changes nothing the other side watches, and brings no point.
#[test]
fn a_version_handed_in_as_changing_nothing_answers_nothing_and_brings_no_point() {
    let owed = vec![Unmet::WordOwed {
        name: "opinions".into(),
        watched: vec!["plan".into()],
    }];
    // The maker's plan, handed in unchanged, changes nothing the judge watches.
    let s = maker();
    assert_eq!(
        step(vec![handed_in(vec![unchanged("plan")])], &s),
        send_back(owed.clone())
    );
    // The maker's answer, handed in unchanged, answers no point.
    assert_eq!(step(vec![handed_in(vec![unchanged("opinions")])], &s), send_back(owed));
    // The judge's points, handed in unchanged beside its fail, bring none.
    let none = vec![Unmet::NoPoint {
        block: "verdict".into(),
        value: "no".to_string(),
        name: "opinions".into(),
    }];
    let fail = vec![valued("verdict", "no"), unchanged("opinions")];
    assert_eq!(step(vec![handed_in(fail)], &judge()), send_back(none));
}

// A choice handed in again with the value it has changes nothing, whether its writer marks it so or not, as the walk
// reads it (design document `data-and-context`, 9): it is no change of what the other side watches; another value is.
#[test]
fn a_choice_handed_in_again_with_its_value_changes_nothing_marked_or_not() {
    let mut s = maker();
    s.dialogues = BTreeMap::from([("opinions".into(), vec!["plan".into(), "verdict".into()])]);
    let owed = vec![Unmet::WordOwed {
        name: "opinions".into(),
        watched: vec!["plan".into(), "verdict".into()],
    }];
    assert_eq!(
        step(vec![handed_in(vec![valued("verdict", "yes")])], &s),
        send_back(owed)
    );
    let changed = vec![valued("verdict", "no")];
    assert_eq!(step(vec![handed_in(changed.clone())], &s), finish(changed));
}

// A judge woken by neither an answer nor a change of what it judges owes no word: its fail handed in as it stands
// changes nothing and is taken, the change ending there (22); a fail that is new, which with no point would leave the
// maker nothing to answer, brings its points, and is sent back without them (41, 42).
#[test]
fn a_judge_woken_by_neither_an_answer_nor_a_change_owes_points_only_for_a_fail_that_is_new() {
    let mut s = judge();
    s.told.woken_to_judge.clear();
    let failed = Standing {
        told: Some(Seq(5)),
        latest: Some(Seq(5)),
        value: Some("no".to_string()),
        added: false,
    };
    s.told.outputs.insert("verdict".into(), failed);
    let stands = vec![valued("verdict", "no")];
    assert_eq!(step(vec![handed_in(stands.clone())], &s), finish(stands));
    let mut s = judge();
    s.told.woken_to_judge.clear();
    let none = vec![Unmet::NoPoint {
        block: "verdict".into(),
        value: "no".to_string(),
        name: "opinions".into(),
    }];
    assert_eq!(
        step(vec![handed_in(vec![valued("verdict", "no")])], &s),
        send_back(none)
    );
}
