//! What the owner does that the loop refuses.

use decide::{CallId, Decision, Outsider};
use driver::{Failed, FirstWrite, NewItem};
use engine::Refused;
use service::{Act, Message, Refusal};
use store::WorkNumber;
use tokio::sync::oneshot;

use crate::Trial;

#[tokio::test(start_paused = true)]
async fn an_answer_to_a_tools_call_is_refused() {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial.step().await.unwrap();
    trial.answer(work, "Yes, by name.");
    trial
        .until(
            work,
            |d| matches!(d, Decision::Wait { at, .. } if at.as_str() == "build"),
        )
        .await;
    let answer = Act::Answer {
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
        trial.act(WorkNumber(9), Act::Pause),
        Err(Refusal::Failed(Failed::NoSuchWork(WorkNumber(9))))
    );
}

#[tokio::test(start_paused = true)]
async fn an_item_on_a_flow_there_is_none_of_is_refused() {
    let mut trial = Trial::open();
    let item = NewItem {
        title: "Greet the user".to_string(),
        level: "1".to_string(),
        depends_on: Vec::new(),
        flow: "other".to_string(),
        writes: vec![FirstWrite {
            block: "request".into(),
            content: "Greet the user, please.".to_string(),
            value: None,
        }],
    };
    let (reply, mut answer) = oneshot::channel();
    trial.take(Message::Add { item, reply });
    assert_eq!(
        answer.try_recv().unwrap(),
        Err(Refusal::Failed(Failed::NoSuchFlow("other".to_string())))
    );
}
