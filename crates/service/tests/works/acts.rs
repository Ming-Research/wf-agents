//! What the owner does that the loop refuses.

use decide::{CallId, Decision, Outsider};
use driver::{Failed, FirstWrite, NewItem};
use engine::Refused;
use service::{Act, GO_ON, Message, Refusal};
use store::WorkNumber;
use tokio::sync::oneshot;

use crate::Trial;

#[tokio::test(start_paused = true)]
async fn an_answer_to_a_tools_call_is_refused() {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial.to_the_owners_call(work).await;
    trial.let_go_on(work);
    trial
        .until(
            work,
            |d| matches!(d, Decision::Wait { at, .. } if at.as_str() == "build"),
        )
        .await;
    let answer = Act::Result {
        call: CallId(4),
        result: "It passed.".to_string(),
    };
    assert_eq!(trial.act(work, answer), Err(Refusal::NotAPersonsCall(CallId(4))));
    // The tool's own result is taken.
    trial.step().await.unwrap();
    assert_eq!(trial.work(work).results[&CallId(4)], "Every check passed.");
}

#[tokio::test(start_paused = true)]
async fn a_write_of_a_block_people_do_not_write_is_refused() {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    let write = Act::Write {
        block: "plan".into(),
        content: "A plan of my own.".to_string(),
        value: None,
    };
    let refused = Refused::NotTheirs {
        block: "plan".into(),
        by: Outsider::Person,
    };
    assert_eq!(trial.act(work, write), Err(Refusal::Failed(Failed::Refused(refused))));
    assert_eq!(
        trial.act(
            WorkNumber(9),
            Act::Pause {
                why: None,
                waits_for: Vec::new()
            }
        ),
        Err(Refusal::Failed(Failed::NoSuchWork(WorkNumber(9))))
    );
}

#[tokio::test(start_paused = true)]
async fn an_item_on_a_flow_there_is_none_of_is_refused() {
    let mut trial = Trial::open();
    let item = NewItem {
        title: "Greet the user".to_string(),
        level: "1".to_string(),
        project: "trial".to_string(),
        depends_on: Vec::new(),
        tags: Vec::new(),
        continues: None,
        flow: "other".to_string(),
        writes: vec![FirstWrite {
            block: "request".into(),
            content: "Greet the user, please.".to_string(),
            value: None,
        }],
        why: None,
        waits_for: Vec::new(),
    };
    let (reply, mut answer) = oneshot::channel();
    trial.take(Message::Add { item, reply });
    assert_eq!(
        answer.try_recv().unwrap(),
        Err(Refusal::Failed(Failed::NoSuchFlow("other".to_string())))
    );
}

/// The work of a trial whose plan reached its limit of returns, and the owner's call at that limit.
async fn at_its_limit() -> (Trial, store::WorkNumber, CallId) {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    let call = trial.to_the_owners_call(work).await;
    assert!(matches!(
        trial.work(work).calls.get(&call),
        Some(work::Call::AtLimit(_))
    ));
    (trial, work, call)
}

#[tokio::test(start_paused = true)]
async fn a_run_at_its_limit_is_let_go_on_with_the_engines_own_result() {
    let (mut trial, work, call) = at_its_limit().await;
    trial.act(work, Act::GoOn { call }).unwrap();
    assert_eq!(trial.work(work).results[&call], GO_ON);
    // Let go on, its next round begins.
    assert!(matches!(trial.decision(work), Decision::Busy { .. }));
}

#[tokio::test(start_paused = true)]
async fn words_given_as_the_result_of_a_run_at_its_limit_are_refused_and_it_stays_at_its_limit() {
    let (mut trial, work, call) = at_its_limit().await;
    let words = Act::Result {
        call,
        result: "go on, and please commit as me".to_string(),
    };
    assert_eq!(trial.act(work, words), Err(Refusal::AtItsLimit(call)));
    assert!(!trial.work(work).results.contains_key(&call));
    assert!(matches!(trial.decision(work), Decision::Wait { .. }));
}

#[tokio::test(start_paused = true)]
async fn only_a_run_at_its_limit_is_let_go_on_by_its_call() {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial.to_the_owners_call(work).await;
    trial.let_go_on(work);
    trial
        .until(
            work,
            |d| matches!(d, Decision::Wait { at, .. } if at.as_str() == "build"),
        )
        .await;
    // The build's call of the CI is a call too, but the tool's, and it is no run at a limit.
    let ci = CallId(4);
    assert_eq!(trial.act(work, Act::GoOn { call: ci }), Err(Refusal::NotAtItsLimit(ci)));
    assert!(!trial.work(work).results.contains_key(&ci));
    // A work there is none of is refused as every act on it is.
    assert_eq!(
        trial.act(WorkNumber(9), Act::GoOn { call: ci }),
        Err(Refusal::Failed(Failed::NoSuchWork(WorkNumber(9))))
    );
}

/// A call has its result once, and only a call at its limit is let go on: a second let-go of the same call, and one of a
/// call there is none of, add no result and no `done` to the records.
#[tokio::test(start_paused = true)]
async fn a_run_let_go_on_is_not_let_go_on_a_second_time_and_a_call_there_is_none_of_is_refused() {
    let (mut trial, work, call) = at_its_limit().await;
    trial.act(work, Act::GoOn { call }).unwrap();
    let done = trial.work(work).records.done.len();
    assert_eq!(
        trial.act(work, Act::GoOn { call }),
        Err(Refusal::Failed(Failed::Refused(Refused::AlreadyDone(call))))
    );
    let nowhere = CallId(9999);
    assert_eq!(
        trial.act(work, Act::GoOn { call: nowhere }),
        Err(Refusal::NotAtItsLimit(nowhere))
    );
    assert_eq!(trial.work(work).records.done.len(), done);
    assert_eq!(trial.work(work).results[&call], GO_ON);
}
