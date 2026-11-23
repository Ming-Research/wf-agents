//! What the owner does that the loop refuses.

use decide::{CallId, Decision, Outsider};
use driver::{Failed, FirstWrite, NewItem};
use engine::Refused;
use service::{Act, GO_ON, Message, Refusal, Script};
use store::WorkNumber;
use tokio::sync::oneshot;

use crate::{Trial, trial_flow};

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
    let answer = Act::Result {
        call: CallId(5),
        result: "It passed.".to_string(),
    };
    assert_eq!(trial.act(work, answer), Err(Refusal::NotAPersonsCall(CallId(5))));
    // The tool's own result is taken.
    trial.step().await.unwrap();
    assert_eq!(trial.work(work).results[&CallId(5)], "Every check passed.");
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
        project: "trial".to_string(),
        depends_on: Vec::new(),
        tags: Vec::new(),
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

/// A script whose plan hands in nothing three times over: each return is sent back, and the third reaches the flow's
/// limit of two returns sent back, where the run's call at its limit waits for the owner.
fn plan_that_hands_in_nothing() -> Script {
    let rounds: String = (1..=3)
        .map(|round| format!("[[flows.trial.rounds]]\ncell = \"plan\"\nround = {round}\nhands_in = []\n\n"))
        .collect();
    let script = Script::read(&format!("[took]\nms = 20\n\n{rounds}")).expect("the script reads");
    assert_eq!(script.check(&[trial_flow()]), Vec::<String>::new());
    script
}

/// The work of a trial whose plan reached its limit, and the owner's call at that limit.
async fn at_its_limit() -> (Trial, store::WorkNumber, CallId) {
    let mut trial = Trial::with(plan_that_hands_in_nothing());
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    let decision = trial
        .until(work, |d| matches!(d, Decision::Wait { on, .. } if !on.is_empty()))
        .await;
    let Decision::Wait { on, .. } = decision else {
        unreachable!("the work waits")
    };
    let call = (on.iter())
        .find_map(|waited| match waited {
            decide::Waited::Call { call, .. } => Some(*call),
            _ => None,
        })
        .expect("the work waits for the owner's call at the limit");
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
    trial.step().await.unwrap();
    // The plan's question to the owner is a call too, but a person answers it.
    let question = trial.owners_call(work).expect("the work waits for the owner's answer");
    assert_eq!(
        trial.act(work, Act::GoOn { call: question }),
        Err(Refusal::NotAtItsLimit(question))
    );
    assert!(!trial.work(work).results.contains_key(&question));
}
