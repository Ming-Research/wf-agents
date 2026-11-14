//! A round that fails for a cause running it again cannot help pauses its work with the reason, its end kept in the
//! records with its kind; once the owner lets the work on, the run's next round is given what the failed one was, and
//! begins the session the failed one could not. A return the command line gave up on is rebuilt from its stream and
//! taken, and one that cannot be read is sent back, in the session. A session the command line no longer has is begun
//! again, and its round told everything a run's first round is.

use decide::Pauser;
use decide::run::{Failure, Unmet};
use service::Act;
use work::{Back, Given, Why};

use crate::{Fake, Real, ended, rows};

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
    assert!(
        why.contains("Prompt is too long: the fake fails its first call."),
        "{why}"
    );
    assert!(why.contains("the prompt is too long for the model"), "{why}");
    let plan_run = kept.records.started[0].run;
    assert_eq!(kept.records.rounds.len(), 1);
    assert_eq!(kept.records.returned.len(), 1, "the plan's round has ended");
    let Some(Back::Failed { why, failure, .. }) = kept.back.get(&(plan_run, 1)) else {
        panic!("the plan's round did not fail: {:?}", kept.back);
    };
    assert_eq!(*failure, Failure::PromptTooLong);
    assert!(
        why.contains("Prompt is too long: the fake fails its first call."),
        "{why}"
    );

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
        assert!(base.with_extension("stream.jsonl").is_file(), "{}", base.display());
    }
}

#[tokio::test]
async fn a_round_the_safety_check_flagged_is_given_again_at_once_in_a_new_session_of_its_line() {
    let fake = Fake {
        flag_first: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.failures, 1);
    let kept = real.work(work);
    let plan_run = kept.records.started[0].run;
    assert!(matches!(
        kept.back.get(&(plan_run, 1)),
        Some(Back::Failed {
            failure: Failure::Flagged,
            ..
        })
    ));
    assert!(
        decide::pause(&kept.records).is_none(),
        "a first flagged round pauses nothing"
    );
    let calls = real.calls();
    // The round flagged spent tokens in its session, and is given again all the same in a session begun anew, which
    // is told the task again.
    assert_eq!(calls[0].session().0, "--session-id");
    assert_eq!(calls[1].session().0, "--session-id");
    assert_ne!(calls[0].session().1, calls[1].session().1);
    assert!(calls[1].stdin.contains("## Your task"), "{}", calls[1].stdin);
    // The rounds of the line after it go on in the new session, charged only what they cost more.
    let later = (calls.iter().skip(2)).find(|c| c.of("Plan"));
    if let Some(later) = later {
        assert_eq!(later.session(), ("--resume", calls[1].session().1));
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
    // The plan, the build's first run, the run entered for the check's result refused and begun again, the review.
    assert_eq!(calls.len(), 5);
    let (refused, again) = (&calls[2], &calls[3]);
    assert_eq!(refused.session().0, "--resume");
    assert!(
        refused.stdin.contains("As given before in this session"),
        "a round that goes on is told its task by its held line: {}",
        refused.stdin
    );
    assert_eq!(again.session(), ("--session-id", refused.session().1));
    let lost = (real.work(work).sessions.iter())
        .filter(|s| s.line == "engineer" && s.why == Why::Lost)
        .count();
    assert_eq!(lost, 1, "the session begun again is recorded once, with why");
    for part in [
        "## Your task\n\nBuild",
        "## What this cell returns",
        "## The results of what you asked for",
    ] {
        assert!(again.stdin.contains(part), "{part} is missing from:\n{}", again.stdin);
    }
    let build = real.run_of(work, "build");
    let base = real.data.join(format!("rounds/1/{}-1", build.0));
    assert_eq!(
        std::fs::read_to_string(base.with_extension("message.2.md")).unwrap(),
        again.stdin
    );
}

#[tokio::test]
async fn a_session_lost_on_a_later_run_is_begun_again_with_the_task_and_the_runs_own_results() {
    let fake = Fake {
        lose_session: true,
        question_first: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    // The build's question waits for the owner: the work waits at the build until the owner answers.
    real.until(|real| real.waited_question(work).is_some()).await;
    let answer = "Greet them by their first name.";
    real.answer(work, answer);
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    // The plan, the build's question, its query of the check once the owner answered, the run entered for the check's
    // result refused and begun again, the review.
    assert_eq!(calls.len(), 6);
    let again = &calls[4];
    assert_eq!(again.session().0, "--session-id");
    assert!(again.stdin.starts_with("# Build\n"), "{}", again.stdin);
    let checked = rows(r#"{"sql":"select the check"}"#);
    for part in ["## Your task", "## The results of what you asked for", &checked] {
        assert!(again.stdin.contains(part), "{part} is missing from:\n{}", again.stdin);
    }
    // The session begun again is filled with what the build reads, the owner's answer among it, in the words of the
    // block the answer wrote.
    assert!(
        again.stdin.contains(answer),
        "the answer is missing from: {}",
        again.stdin
    );
}

/// The fake whose first call prints an error that cost eighty cents, every later call saying the session has cost
/// eighty-five.
fn erring() -> Fake {
    Fake {
        error_first: true,
        cost: "0.85",
        ..Fake::default()
    }
}

#[tokio::test]
async fn a_failed_round_whose_result_says_what_it_cost_is_charged_it_and_the_next_round_only_its_own() {
    let mut real = Real::trial(erring());
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.failures == 1).await;
    let kept = real.work(work);
    let plan_run = kept.records.started[0].run;
    let Some(Back::Failed { why, failure, .. }) = kept.back.get(&(plan_run, 1)) else {
        panic!("the plan's round did not fail: {:?}", kept.back);
    };
    assert!(why.contains(crate::GAVE_UP), "{why}");
    assert_eq!(*failure, Failure::Unread);
    assert!(decide::pause(&kept.records).is_none(), "the work is paused");
    let failed = kept.spent[&(plan_run, 1)];
    assert_eq!(
        (failed.usd_micros, failed.input_tokens, failed.output_tokens),
        (800_000, 1000, 300)
    );

    // What could not be read is sent back, in the session, without the owner.
    real.until(|real| ended(&real.decision(work))).await;
    let kept = real.work(work);
    assert!(
        matches!(kept.given.get(&(plan_run, 2)), Some(Given::SentBack(unmet)) if matches!(unmet[..], [Unmet::Unreadable { .. }])),
        "{:?}",
        kept.given.get(&(plan_run, 2))
    );
    // The next round goes on in the session the failed one began, and is charged only what it cost more.
    let calls = real.calls();
    assert_eq!(calls[1].session().0, "--resume");
    assert_eq!(kept.spent[&(plan_run, 2)].usd_micros, 50_000);
    let total: u64 = kept.spent.values().map(|s| s.usd_micros).sum();
    assert!(
        total >= 850_000,
        "the work's total, {total}, leaves out the failed round"
    );
}

#[tokio::test]
async fn a_return_the_command_line_gave_up_on_is_rebuilt_from_its_stream_and_taken() {
    let fake = Fake {
        refuses_first: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.failures, 0, "no round failed");
    assert_eq!(real.calls().len(), 3, "the plan's program was run once");
    let kept = real.work(work);
    let plan_run = kept.records.started[0].run;
    let Some(Back::Returned { handed, .. }) = kept.back.get(&(plan_run, 1)) else {
        panic!("the plan's round handed nothing in: {:?}", kept.back);
    };
    let handed: Vec<(&str, &str)> = (handed.iter())
        .map(|h| (h.block.0.as_str(), h.content.as_str()))
        .collect();
    assert_eq!(handed, [("plan", "Greet by name.")]);
    assert_eq!(kept.spent[&(plan_run, 1)].usd_micros, 400_000, "what the call cost");
    // What is kept of the stream holds the pieces the return was rebuilt from.
    let stream = real.data.join(format!("rounds/1/{}-1.stream.jsonl", plan_run.0));
    let stream = std::fs::read_to_string(stream).expect("the stream is kept");
    assert_eq!(stream.matches("input_json_delta").count(), 4, "{stream}");
}

#[tokio::test]
async fn a_failed_round_that_takes_the_work_over_its_budget_says_both() {
    let mut real = Real::open(crate::On::Trial, erring(), Some(0.5), "true");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.failures == 1).await;
    let kept = real.work(work);
    let why = decide::pause(&kept.records)
        .and_then(|p| p.why.clone())
        .expect("a reason");
    assert!(why.contains(crate::GAVE_UP), "{why}");
    assert!(why.contains("over its budget of $0.50"), "{why}");
}
