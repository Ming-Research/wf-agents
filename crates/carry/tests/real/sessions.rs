//! When a line goes on in its session and when it begins a new one, with the command line: the context a round ended
//! with, past the limit, has the line's next round begin a session of its own id; a compaction is counted and given
//! the round's whole message again; and the sessions are recorded with why they began.

use service::Act;
use work::Why;

use crate::{Call, Fake, On, Real, ended};

#[tokio::test]
async fn a_round_ending_past_the_context_limit_has_the_lines_next_round_begin_a_session_of_its_own() {
    let fake = Fake {
        context: Some(600_000),
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    assert_eq!(calls.len(), 3);
    let (plan, build) = (calls[0].session(), calls[1].session());
    assert_eq!(plan.0, "--session-id");
    assert_eq!(build.0, "--session-id", "the build begins a new session");
    assert_ne!(build.1, plan.1, "under an id of its own");
    let kept = real.work(work);
    let engineer: Vec<&Why> = (kept.sessions.iter())
        .filter(|s| s.line == "engineer")
        .map(|s| &s.why)
        .collect();
    let limit = Why::Limit {
        context: 600_000,
        limit: 500_000,
    };
    // The plan's round, then the build's, each ended past the limit.
    assert_eq!(engineer, [&limit, &limit]);
    assert!(
        (kept.spent.values().filter(|spent| spent.input_tokens > 0)).all(|spent| spent.context_tokens == Some(600_000)),
        "each agent's round's context is recorded"
    );
    // A new session costs what the command line says it cost, the earlier session's rounds being no part of it.
    let build_run = real.run_of(work, "build");
    assert_eq!(kept.spent[&(build_run, 1)].usd_micros, 10_000);
}

#[tokio::test]
async fn a_compaction_within_a_round_is_counted_and_given_the_rounds_whole_message_again() {
    let fake = Fake {
        compacts: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    assert!(calls[1].of("Build"), "{}", calls[1].stdin);
    // What the hook gave the build's session again is the whole message, as a session that begins is told it.
    let again = std::fs::read_to_string(real.calls_dir().join("2.again")).expect("the hook ran");
    assert!(
        again.starts_with("Your session was compacted during this round."),
        "{again}"
    );
    for part in ["\n# Build\n", "## Your task\n\nBuild", "## What this cell returns"] {
        assert!(again.contains(part), "{part} is missing from:\n{again}");
    }
    let kept = real.work(work);
    assert!(
        (kept.spent.values().filter(|spent| spent.input_tokens > 0)).all(|spent| spent.compactions == 1),
        "{:?}",
        kept.spent
    );
    // Each compaction is recorded as the session reopened, which the loop is told while the round is under way.
    let compactions: u32 = kept.spent.values().map(|spent| spent.compactions).sum();
    let reopened = (kept.sessions.iter()).filter(|s| s.why == work::Why::Compacted).count();
    assert!(compactions > 0);
    assert_eq!(reopened, compactions as usize, "{:?}", kept.sessions);
    let build = real.run_of(work, "build");
    let base = real.data.join(format!("rounds/1/{}-1", build.0));
    assert!(
        !base.with_extension("again.md").exists(),
        "what was given again is not kept beside the round"
    );
}

#[tokio::test]
async fn a_run_waiting_as_its_line_begins_a_new_session_is_told_its_task_and_its_rounds_again_in_the_session_it_goes_on_in()
 {
    let fake = Fake {
        question_first: true,
        plan_again_unchanged: true,
        ..Fake::default()
    };
    let mut real = Real::open(On::Loading, fake, None, "true");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.waited_question(work).is_some()).await;
    // The settings give level 1 another model while the build waits for the owner's answer: the engineer's line
    // begins a new session, and the plan, which loads it, runs again in it first.
    let level = real.settings.agents.levels.get_mut("1").expect("level 1");
    level.model = "claude-opus-5-5".to_string();
    real.restart();
    let planned = |real: &Real| real.finished(work).iter().filter(|cell| *cell == "plan").count();
    real.until(|real| planned(real) == 2).await;
    let waits = |real: &Real| {
        let decision = real.decision(work);
        let on_the_owner = |on: &[decide::Waited]| on.iter().any(|w| matches!(w, decide::Waited::Answer { .. }));
        matches!(&decision, decide::Decision::Wait { at, on } if at.as_str() == "build" && on_the_owner(on))
    };
    // The plan ran again in the new session, and the work is held at the build for the owner still.
    assert!(waits(&real), "{:?}", real.decision(work));
    real.answer(work, "By their first name.");
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    let builds: Vec<&Call> = (calls.iter()).filter(|c| c.of("Build")).collect();
    let first = builds[0];
    let again = (calls.iter())
        .filter(|c| c.of("Plan"))
        .nth(1)
        .expect("the plan ran again");
    let second = builds[1];
    assert_eq!(again.session().0, "--session-id", "the plan begins the new session");
    assert_ne!(again.session().1, first.session().1);
    // The build, entered again for the answer, goes on in the session the plan began, which never saw the build: it
    // is told its task and what it reads, as a round that begins a session is, and that the answer runs it. The answer
    // is in what it reads, in the words of the block the answer writes.
    assert_eq!(second.session(), ("--resume", again.session().1));
    for part in [
        "## Your task\n\nBuild",
        "## What this cell returns",
        "By their first name.",
    ] {
        assert!(second.stdin.contains(part), "{part} is missing from:\n{}", second.stdin);
    }
    // Entered again for the check, it goes on in the same session, which saw its last run: it is told its task by its
    // held line.
    let third = builds[2];
    assert_eq!(third.session(), ("--resume", again.session().1));
    assert!(
        third.stdin.contains("As given before in this session"),
        "{}",
        third.stdin
    );
}
