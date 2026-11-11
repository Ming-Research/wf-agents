//! A work from its adding to its merge.

use decide::{CallId, Cause, Decision, End, Waited, Who};
use engine::Event;
use service::{Act, Script};
use work::Given;

use crate::Trial;

#[tokio::test(start_paused = true)]
async fn a_work_goes_through_its_cells_round_the_objection_to_the_merge() {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    assert_eq!(trial.decision(work), Decision::Paused, "a work is added paused");

    trial.act(work, Act::Resume).unwrap();
    assert!(matches!(trial.decision(work), Decision::Busy { cell, .. } if cell.as_str() == "plan"));

    // The plan asks the owner, and waits for the answer.
    trial.step().await.unwrap();
    let Decision::Wait { at, on } = trial.decision(work) else {
        panic!("the plan waits for the owner");
    };
    assert_eq!(at.as_str(), "plan");
    assert_eq!(
        on,
        [Waited::Call {
            call: CallId(2),
            on: Some(Who::Owner)
        }]
    );
    trial.answer(work, "Yes, by name.");
    let kept = trial.work(work);
    assert_eq!(kept.results[&CallId(2)], "Yes, by name.");

    // The build asks for the CI, which the tool answers.
    trial
        .until(
            work,
            |d| matches!(d, Decision::Wait { at, .. } if at.as_str() == "build"),
        )
        .await;
    let Decision::Wait { on, .. } = trial.decision(work) else {
        unreachable!()
    };
    assert_eq!(
        on,
        [Waited::Call {
            call: CallId(4),
            on: None
        }]
    );
    trial.step().await.unwrap();
    assert_eq!(trial.work(work).results[&CallId(4)], "Every check passed.");

    let end = trial.until(work, |d| matches!(d, Decision::Ended(_))).await;
    assert_eq!(end, Decision::Ended(End::Merged));
    assert_eq!(
        trial.finished(work),
        ["plan", "build", "review", "build", "review", "merge"]
    );

    // The second build ran for what the review wrote, and was given its task anew.
    let kept = trial.work(work);
    let second = kept
        .records
        .started
        .iter()
        .filter(|s| s.cell.as_str() == "build")
        .nth(1)
        .unwrap();
    assert_eq!(
        second.because,
        [Cause::Changed {
            block: "opinions".into(),
            was: None,
            behind: true
        }]
    );
    assert_eq!(kept.given[&(second.run, 1)], Given::Task);
    // What it held is let go; nothing more is to be done, and nothing more comes.
    assert_eq!(trial.step().await, Some(Event::Released));
    assert!(trial.step().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn each_round_takes_the_time_the_script_gives_it() {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial.drive(work).await;
    let kept = trial.work(work);
    let plan = kept.records.started[0].run;
    let began = kept
        .time_of(kept.records.rounds.iter().find(|r| r.run == plan).unwrap().at)
        .unwrap();
    let ended = kept
        .time_of(kept.records.returned.iter().find(|r| r.run == plan).unwrap().at)
        .unwrap();
    assert_eq!(ended.0 - began.0, 1500);
    assert_eq!(kept.spent[&(plan, 1)].usd_micros, 21000);
    // The engine's own act takes no cost.
    let merge = kept.records.started.last().unwrap().run;
    assert_eq!(kept.spent[&(merge, 1)], work::Spent::default());
}

#[tokio::test(start_paused = true)]
async fn a_work_whose_script_says_nothing_comes_to_its_merge() {
    let mut trial = Trial::with(Script::default());
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    assert_eq!(trial.drive(work).await, Decision::Ended(End::Merged));
    assert_eq!(trial.finished(work), ["plan", "build", "review", "merge"]);
}

#[tokio::test(start_paused = true)]
async fn a_work_whose_script_says_nothing_comes_to_its_merge_with_the_owners_opinion_midway() {
    let mut trial = Trial::with(Script::default());
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial
        .until(
            work,
            |d| matches!(d, Decision::Busy { cell, .. } if cell.as_str() == "review"),
        )
        .await;
    let opinion = Act::Write {
        block: "opinions".into(),
        content: "Keep it short.".to_string(),
        value: None,
    };
    trial.act(work, opinion).unwrap();
    assert_eq!(trial.drive(work).await, Decision::Ended(End::Merged));
    // The review, which started from the opinions, is abandoned as its round ends; the build runs again for the
    // owner's opinion, and the review for what it built.
    assert_eq!(trial.finished(work), ["plan", "build", "build", "review", "merge"]);
}
