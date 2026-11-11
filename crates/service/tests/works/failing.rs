//! A round that fails, and one that ends with its work to wait for the owner: the engine pauses the work with the
//! reason, which its records keep; a restart while it is paused hands nothing out; and once the owner lets it on, the
//! round is given again and the work goes on to its end.

use std::sync::Arc;

use decide::{Decision, End, Pauser};
use engine::{Came, Event};
use service::{Act, Carriers, Message, Round, RoundReply, Rounds, Script, Scripted, Service, TokioClock};
use store::Store;
use work::{Back, Call, Handed, Spent, Time, Work};

use crate::{START, Trial, act_on, add_to, new_item, trial_dir, trial_flow};

const WHY: &str = "The agent's program could not start.";

/// The trial's script, the plan's second round, the one given the owner's answer, failing.
fn failing() -> Script {
    let text = std::fs::read_to_string(trial_dir().join("script.toml")).expect("the script is read");
    let text = format!("{text}\n[[flows.trial.rounds]]\ncell = \"plan\"\nround = 2\nfails = \"{WHY}\"\n");
    let script = Script::read(&text).expect("the script reads");
    assert_eq!(script.check(&[trial_flow()]), Vec::<String>::new());
    script
}

/// The trial work with the failing script, let on, the owner answering the plan's question: the plan's second round
/// is under way.
async fn answered() -> (Trial, store::WorkNumber) {
    let mut trial = Trial::with(failing());
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial.step().await.unwrap();
    trial.answer(work, "Yes, by name.");
    (trial, work)
}

#[tokio::test(start_paused = true)]
async fn a_round_that_fails_pauses_its_work_with_the_reason_and_hands_out_nothing_more() {
    let (mut trial, work) = answered().await;
    let event = trial.step().await.unwrap();
    assert!(matches!(&event, Event::Failed { why, .. } if why == WHY), "{event:?}");
    let kept = trial.work(work);
    let plan = kept.records.started[0].run;
    assert_eq!(
        kept.back.get(&(plan, 2)),
        Some(&Back::Failed {
            why: WHY.to_string(),
            failure: decide::run::Failure::Other,
            resets: None,
        })
    );
    let pause = decide::pause(&kept.records).expect("the work is paused");
    assert_eq!((pause.by, pause.why.as_deref()), (Pauser::Engine, Some(WHY)));
    // The plan's run is at rest, its next round held by the pause: nothing more comes.
    assert_eq!(
        trial.decision(work),
        Decision::GoOn {
            cell: "plan".into(),
            run: plan
        }
    );
    assert!(trial.step().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn a_restart_while_the_work_is_paused_hands_out_nothing_and_the_owners_resume_runs_it_again_to_its_end() {
    let (mut trial, work) = answered().await;
    trial.step().await.unwrap();
    let before = trial.work(work);
    trial.restart();
    assert_eq!(trial.service.driver().pending(), vec![]);
    assert!(
        trial.step().await.is_none(),
        "something was handed out after the restart"
    );
    assert_eq!(trial.work(work).records, before.records);

    trial.act(work, Act::Resume).unwrap();
    let kept = trial.work(work);
    let plan = kept.records.started[0].run;
    // The round after the one that failed is given what that one was: the owner's answer.
    assert_eq!(kept.given.get(&(plan, 3)), kept.given.get(&(plan, 2)));
    assert_eq!(trial.drive(work).await, Decision::Ended(End::Merged));
    assert_eq!(
        trial.finished(work),
        ["plan", "build", "review", "build", "review", "merge"]
    );
}

/// Everything kept of the work but when each thing happened, which a stop puts off, and so the time until which a
/// wait of the engine's is.
fn kept_but_times(work: &Work) -> Work {
    let calls = (work.calls.iter())
        .map(|(call, what)| {
            let what = match what {
                Call::Wait { failed, .. } => Call::Wait {
                    until: Time(0),
                    failed: *failed,
                },
                other => other.clone(),
            };
            (*call, what)
        })
        .collect();
    Work {
        moments: Vec::new(),
        calls,
        ..work.clone()
    }
}

/// The trial work with the failing script taken to its end, the owner letting it on whenever the engine paused it,
/// with the process stopped and started again after each of these steps.
async fn history(stops: &[usize]) -> (Work, usize) {
    history_of(failing(), stops).await
}

/// The trial work with this script taken to its end, as [`history`] takes it.
async fn history_of(script: Script, stops: &[usize]) -> (Work, usize) {
    let mut trial = Trial::with(script);
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    loop {
        if stops.contains(&trial.steps) {
            trial.restart();
        }
        if matches!(trial.decision(work), Decision::Ended(_)) {
            break;
        }
        let kept = trial.work(work);
        if decide::pause(&kept.records).is_some_and(|p| p.by == Pauser::Engine) {
            trial.act(work, Act::Resume).unwrap();
        } else if trial.owners_call(work).is_some() {
            trial.answer(work, "Yes, by name.");
        } else {
            assert!(trial.step().await.is_some(), "the work comes to its end");
        }
    }
    assert_eq!(trial.decision(work), Decision::Ended(End::Merged));
    (kept_but_times(&trial.work(work)), trial.steps)
}

#[tokio::test(start_paused = true)]
async fn a_stop_at_any_step_of_a_work_whose_round_fails_changes_nothing() {
    let (without, steps) = history(&[]).await;
    assert!(
        (without.back.values()).any(|b| matches!(b, Back::Failed { .. })),
        "no round failed"
    );
    for stop in 0..=steps {
        let (with, _) = history(&[stop]).await;
        assert_eq!(with, without, "stopped after step {stop}");
    }
}

/// The trial's script, the plan's second round, the one given the owner's answer, failing for a cause of this kind;
/// where `resets`, the usage limit resets that many milliseconds after the start.
pub(crate) fn failing_as(failure: &str, resets: Option<u64>) -> Script {
    let text = std::fs::read_to_string(trial_dir().join("script.toml")).expect("the script is read");
    let resets = resets.map_or(String::new(), |ms| format!("resets = {}\n", START.0 + ms));
    let text = format!(
        "{text}\n[[flows.trial.rounds]]\ncell = \"plan\"\nround = 2\nfails = \"{WHY}\"\nfailure = \"{failure}\"\n{resets}"
    );
    let script = Script::read(&text).expect("the script reads");
    assert_eq!(script.check(&[trial_flow()]), Vec::<String>::new());
    script
}

#[tokio::test(start_paused = true)]
async fn a_round_that_fails_in_passing_is_given_again_once_its_wait_is_over_and_the_work_is_never_paused() {
    let mut trial = Trial::with(failing_as("passing", None));
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial.step().await.unwrap();
    trial.answer(work, "Yes, by name.");
    let event = trial.step().await.unwrap();
    assert!(matches!(&event, Event::Failed { .. }), "{event:?}");
    let kept = trial.work(work);
    assert!(!decide::paused(&kept.records), "a failure in passing pauses nothing");
    let plan = kept.records.started[0].run;
    let wait = kept.calls.values().find_map(|c| match c {
        Call::Wait { until, failed } => Some((*until, *failed)),
        _ => None,
    });
    let failed_at = kept.moments.last().unwrap().at;
    assert_eq!(wait, Some((Time(failed_at.0 + decide::run::FIRST_WAIT), 1)));
    assert_eq!(trial.drive(work).await, Decision::Ended(End::Merged));
    let kept = trial.work(work);
    assert_eq!(kept.given.get(&(plan, 3)), kept.given.get(&(plan, 2)));
    let given_again = (kept.records.rounds.iter())
        .filter(|r| r.run == plan)
        .nth(2)
        .unwrap()
        .at;
    let again_at = kept.time_of(given_again).unwrap();
    assert!(
        again_at.0 >= failed_at.0 + decide::run::FIRST_WAIT,
        "given again before its wait was over"
    );
    assert!(kept.records.pauses.iter().all(|p| p.by == Pauser::Owner));
}

#[tokio::test(start_paused = true)]
async fn a_stop_at_any_step_of_a_work_whose_round_fails_in_passing_changes_nothing() {
    let (without, steps) = history_of(failing_as("passing", None), &[]).await;
    assert!(
        (without.calls.values()).any(|c| matches!(c, Call::Wait { .. })),
        "no wait was made"
    );
    for stop in 0..=steps {
        let (with, _) = history_of(failing_as("passing", None), &[stop]).await;
        assert_eq!(with, without, "stopped after step {stop}");
    }
}

#[tokio::test(start_paused = true)]
async fn at_the_usage_limit_no_round_begins_until_it_resets_and_then_the_work_goes_on_unpaused() {
    let hour = 3_600_000;
    let mut trial = Trial::with(failing_as("usage_limit", Some(hour)));
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial.step().await.unwrap();
    trial.answer(work, "Yes, by name.");
    trial.step().await.unwrap();
    let outage = trial.service.driver().outage().expect("the wait stands");
    assert_eq!(outage.resets, Some(Time(START.0 + hour)));
    assert!(!decide::paused(&trial.work(work).records));
    assert_eq!(trial.drive(work).await, Decision::Ended(End::Merged));
    let kept = trial.work(work);
    let plan = kept.records.started[0].run;
    let given_again = (kept.records.rounds.iter())
        .filter(|r| r.run == plan)
        .nth(2)
        .unwrap()
        .at;
    assert!(
        kept.time_of(given_again).unwrap().0 >= START.0 + hour,
        "a round began before the limit reset"
    );
    assert_eq!(trial.service.driver().outage(), None);
}

/// Agents that end the plan's first round handing in the plan, its work to wait for the owner: the work spent more
/// than it may. Every other round they leave to the script.
struct OverBudget {
    scripted: Scripted,
}

const OVER: &str = "The work spent more than it may.";

impl Rounds for OverBudget {
    fn give(&self, round: Round, reply: RoundReply) {
        if round.cell.id.as_str() == "plan" && round.number == 1 {
            let plan = Handed {
                block: "plan".into(),
                content: "Greet the user by name.".to_string(),
                matters: true,
                value: None,
            };
            reply.ended_and_paused(Came::HandsIn(vec![plan]), Spent::default(), OVER.to_string());
        } else {
            self.scripted.give(round, reply);
        }
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        self.scripted.stop(round, reply);
    }
}

#[tokio::test(start_paused = true)]
async fn a_round_that_ends_with_its_work_to_wait_is_taken_and_no_round_begins_after_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let scripted = Scripted::new(Script::default());
    let carriers = Carriers {
        agents: Arc::new(OverBudget {
            scripted: scripted.clone(),
        }),
        acts: Arc::new(scripted.clone()),
        tools: Arc::new(scripted.clone()),
        network: Arc::new(scripted),
    };
    let clock = Arc::new(TokioClock::new(START));
    let (mut service, mut inbox) = Service::open(Box::new(store), vec![trial_flow()], carriers, clock).unwrap();
    let (_, work) = add_to(&mut service, new_item("Greet the user", &[]));
    assert_eq!(act_on(&mut service, work, Act::Resume), (Ok(()), Ok(())));
    let decision = |service: &Service| service.driver().view(work).unwrap().decision;
    let records = |service: &Service| service.driver().work(work).unwrap().work.records.clone();

    // The pause comes first, while the round is still under way.
    let first = inbox.recv().await.unwrap();
    assert!(matches!(
        &first,
        Message::Happened {
            event: Event::Paused { by: Pauser::Engine, .. },
            ..
        }
    ));
    service.take(first).unwrap();
    assert!(matches!(decision(&service), Decision::Busy { cell, .. } if cell.as_str() == "plan"));
    assert!(decide::paused(&records(&service)));

    // Then the end, which is taken: the plan finishes, and the build does not begin.
    service.take(inbox.recv().await.unwrap()).unwrap();
    let kept = records(&service);
    assert_eq!(kept.finished.len(), 1);
    assert_eq!(decision(&service), Decision::Paused);
    let pause = decide::pause(&kept).expect("the work is paused");
    assert_eq!((pause.by, pause.why.as_deref()), (Pauser::Engine, Some(OVER)));
    assert_eq!(service.driver().pending(), vec![]);
}
