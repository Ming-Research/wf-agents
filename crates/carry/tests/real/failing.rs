//! A round that fails pauses its work with the reason, its end kept in the records; once the owner lets the work on,
//! the run's next round is given what the failed one was, and begins the session the failed one could not. A session
//! the command line no longer has is begun again, and its round told everything a run's first round is.

use decide::Pauser;
use service::Act;
use work::{Back, Given};

use crate::{Fake, Real, ended};

#[tokio::test]
async fn a_failed_round_pauses_the_work_with_the_reason_and_the_next_round_runs_on_resume() {
    let fake = Fake {
        fail_first: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.failures == 1).await;
    let kept = real.work(work);
    let pause = decide::pause(&kept.records).expect("the work is paused");
    assert_eq!(pause.by, Pauser::Engine);
    let why = pause.why.clone().expect("a reason");
    assert!(why.contains("The fake fails its first call."), "{why}");
    let plan_run = kept.records.started[0].run;
    assert_eq!(kept.records.rounds.len(), 1);
    assert_eq!(kept.records.returned.len(), 1, "the plan's round has ended");
    assert_eq!(kept.back.get(&(plan_run, 1)), Some(&Back::Failed { why }));

    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.finished(work), ["plan", "build", "review", "merge"]);
    let calls = real.calls();
    assert_eq!(calls.len(), 4, "the plan's program was run twice");
    // The round after the failed one is given the same, and begins the session, as the failed one was to.
    let kept = real.work(work);
    assert_eq!(kept.given.get(&(plan_run, 2)), Some(&Given::Task));
    assert_eq!(kept.given.get(&(plan_run, 1)), Some(&Given::Task));
    assert_eq!(calls[0].session(), calls[1].session());
    assert_eq!(calls[1].session().0, "--session-id");
    assert!(calls[1].stdin.contains("## Your task"), "{}", calls[1].stdin);
    // Each round is kept.
    for n in [1, 2] {
        let base = real.data.join(format!("rounds/1/{}-{n}", plan_run.0));
        assert!(base.with_extension("message.md").is_file(), "{}", base.display());
        assert!(base.with_extension("result.json").is_file(), "{}", base.display());
    }
}

#[tokio::test]
async fn a_session_not_found_is_begun_again_with_the_task_and_the_list() {
    let fake = Fake {
        lose_session: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.failures, 0);
    let calls = real.calls();
    // The plan, the build's first round, its second refused, the second again, the review.
    assert_eq!(calls.len(), 5);
    let (refused, again) = (&calls[2], &calls[3]);
    assert_eq!(refused.session().0, "--resume");
    assert!(
        !refused.stdin.contains("## Your task"),
        "a round that goes on is told only its results"
    );
    assert_eq!(again.session(), ("--session-id", refused.session().1));
    for part in [
        "## Your task",
        "## What it reads",
        "## The results of what you asked for",
    ] {
        assert!(again.stdin.contains(part), "{part} is missing from:\n{}", again.stdin);
    }
    let build = real.run_of(work, "build");
    let base = real.data.join(format!("rounds/1/{}-2", build.0));
    assert_eq!(
        std::fs::read_to_string(base.with_extension("message.2.md")).unwrap(),
        again.stdin
    );
}

#[tokio::test]
async fn a_session_lost_on_a_later_round_is_begun_again_with_every_earlier_call_of_the_run() {
    let fake = Fake {
        lose_session: true,
        question_first: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.owners_call(work).is_some()).await;
    let call = real.owners_call(work).unwrap();
    let answer = "Greet them by their first name.".to_string();
    real.act(
        work,
        Act::Answer {
            call,
            result: answer.clone(),
        },
    );
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    // The plan, the build's question, its check, its third round refused and begun again, the review.
    assert_eq!(calls.len(), 6);
    let again = &calls[4];
    assert_eq!(again.session().0, "--session-id");
    assert!(
        again.stdin.starts_with("# Build (cell `build`), round 3\n"),
        "{}",
        again.stdin
    );
    for part in [
        "## Your task",
        "## Round 2 of this run: the results of what you asked for",
        "### Your question: Which name?",
        answer.as_str(),
        "## The results of what you asked for",
        "The check `true` passed.",
    ] {
        assert!(again.stdin.contains(part), "{part} is missing from:\n{}", again.stdin);
    }
}
