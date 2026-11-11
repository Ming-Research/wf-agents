//! The blocks a cell must write: a hand-in that lacks one is sent back in the same run, an agent's and an engine act's
//! alike, and counts against the limit of returns like every other sent back.

use super::*;

/// The judged flow, its judge bound to give its verdict and its merge to write what it merged each time.
fn bound() -> W {
    let mut source = judged_source();
    for (cell, outputs, must_write) in [
        (
            "judge",
            r#"outputs = ["opinions", "verdict"]"#,
            r#"must_write = ["verdict"]"#,
        ),
        ("merge", r#"outputs = ["merged"]"#, r#"must_write = ["merged"]"#),
    ] {
        let text = source.cells.get_mut(cell).expect("the judged flow has the cell");
        assert!(text.contains(outputs), "{cell} has no {outputs:?}");
        *text = text.replacen(outputs, &format!("{outputs}\n{must_write}"), 1);
    }
    W {
        flow: compile(&source, &has(&["main"])).expect("the definition compiles"),
        work: Work::default(),
        clock: START,
    }
}

/// A work on the bound flow whose judge has its first round under way: the work and the judge's run.
fn judging() -> (W, RunId) {
    let mut w = bound();
    let (effects, _) = w.person_writes("request");
    let (write, ..) = round(&effects);
    let (judge, cell, ..) = round(&w.returns(write, hands_in(&[("text", None)])));
    assert_eq!(cell, "judge");
    (w, judge)
}

fn no_verdict() -> Vec<Unmet> {
    vec![Unmet::NotHandedIn {
        block: "verdict".into(),
    }]
}

/// Whether the run finished.
fn finished(w: &W, run: RunId) -> bool {
    w.work.records.finished.iter().any(|f| f.run == run)
}

#[test]
fn a_hand_in_that_lacks_a_block_its_cell_must_write_is_sent_back_in_the_same_run_naming_the_block() {
    let (mut w, judge) = judging();
    assert_eq!(
        w.returns(judge, hands_in(&[("opinions", None)])),
        vec![round_effect(judge, "judge", 2, Given::SentBack(no_verdict()))]
    );
    assert!(
        !finished(&w, judge),
        "a hand-in that lacks the verdict finished the run"
    );
    // A hand-in of nothing at all lacks it too.
    assert_eq!(
        w.returns(judge, hands_in(&[])),
        vec![round_effect(judge, "judge", 3, Given::SentBack(no_verdict()))]
    );
}

#[test]
fn a_hand_in_sent_back_for_what_it_lacks_is_taken_once_handed_in_again_with_it() {
    let (mut w, judge) = judging();
    w.returns(judge, hands_in(&[("opinions", None)]));
    // The judge gives its verdict and no opinion: the writer has nothing to run for, and the merge waits for the owner.
    assert_eq!(w.returns(judge, hands_in(&[("verdict", Some("no"))])), vec![]);
    let written: Vec<_> = (w.work.records.finished.iter())
        .find(|f| f.run == judge)
        .expect("the judge's run finished")
        .writes
        .iter()
        .map(|write| (write.block.clone(), write.value.clone()))
        .collect();
    assert_eq!(written, vec![("verdict".into(), Some("no".to_string()))]);
}

#[test]
fn hand_ins_sent_back_for_what_they_lack_reach_the_limit_of_returns_and_the_run_rests_for_the_owner() {
    let (mut w, judge) = judging();
    w.returns(judge, hands_in(&[("opinions", None)]));
    w.returns(judge, hands_in(&[]));
    let effects = w.returns(judge, hands_in(&[("opinions", None)]));
    let limit = Call::AtLimit(Limit::Returns { sent_back: 2 });
    match effects.as_slice() {
        [Effect::Call { run, what, .. }] if *run == judge && *what == limit => {}
        other => panic!("not the owner's call at the limit of returns: {other:?}"),
    }
    assert_eq!(w.work.records.called.last().expect("a call").on, Some(Who::Owner));
    assert!(!finished(&w, judge));
}

#[test]
fn a_return_that_asks_for_calls_is_no_hand_in_and_its_calls_are_made() {
    let (mut w, judge) = judging();
    let made = calls_of(&w.returns(judge, Came::Asks(vec![tool("lint")])));
    assert_eq!(
        made.into_iter().map(|(_, run, what)| (run, what)).collect::<Vec<_>>(),
        vec![(judge, Call::Asked(tool("lint")))]
    );
}

#[test]
fn an_engine_act_s_hand_in_is_held_to_what_its_cell_must_write() {
    let (mut w, judge) = judging();
    w.lets_merge();
    let (merge, cell, ..) = round(&w.returns(judge, hands_in(&[("verdict", Some("yes"))])));
    assert_eq!(cell, "merge");
    let unmet = vec![Unmet::NotHandedIn { block: "merged".into() }];
    assert_eq!(
        w.returns(merge, hands_in(&[])),
        vec![round_effect(merge, "merge", 2, Given::SentBack(unmet))]
    );
    assert!(!finished(&w, merge));
    assert_eq!(
        w.returns(merge, hands_in(&[("merged", None)])),
        vec![Effect::Release {
            end: decide::End::Merged
        }]
    );
    assert_eq!(w.decision(), decide::Decision::Ended(decide::End::Merged));
}
