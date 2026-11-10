//! A round that fails pauses its work, and is given again, the same round, once the owner lets the work on; a session
//! the command line no longer has is begun again, and its round told everything a run's first round is.

use service::Act;

use crate::{Fake, Real, ended};

#[tokio::test]
async fn a_failed_round_pauses_the_work_and_runs_again_on_resume() {
    let fake = Fake {
        fail_first: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.failures == 1).await;
    assert!(real.paused(work), "the work is paused");
    let kept = real.work(work);
    assert_eq!(kept.records.rounds.len(), 1, "the plan's round is under way");
    assert!(kept.records.returned.is_empty(), "and has not ended");
    assert!(kept.spent.is_empty());

    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.finished(work), ["plan", "build", "review", "merge"]);
    let calls = real.calls();
    assert_eq!(calls.len(), 4, "the plan's round was run twice");
    assert_eq!(calls[0].stdin, calls[1].stdin, "the same round, told the same");
    let kept = real.work(work);
    let plan_run = kept.records.started[0].run;
    let plan_rounds = kept.records.rounds.iter().filter(|r| r.run == plan_run).count();
    assert_eq!(plan_rounds, 1, "the round given again is the same round");
    // Each try is kept.
    let base = real.data.join(format!("rounds/1/{}-1", plan_run.0));
    assert!(base.with_extension("message.2.md").is_file());
    assert!(base.with_extension("result.2.json").is_file());
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
