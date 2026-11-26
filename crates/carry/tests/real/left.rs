//! A program a process of the engine left running is stopped before its round is run again, so that a session has
//! one program at a time; and what the round's last try kept of its stream says what came of that try.

use std::os::unix::fs::MetadataExt;

use decide::run::Failure;
use decide::{Decision, Outsider};
use service::Act;
use work::Back;

use crate::{Fake, On, Real, ended, git_in};

#[tokio::test]
async fn a_round_left_running_by_an_engine_that_stopped_is_stopped_before_it_runs_again() {
    let fake = Fake {
        plan_sleeps: 30,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.wait(|real| real.calls().len() == 1).await;
    let plan = real.run_of(work, "plan");
    let pgid = real.data.join(format!("rounds/1/{}-1.pgid", plan.0));
    real.wait(|_| pgid.exists()).await;

    real.restart();
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    assert_eq!(calls.len(), 4, "the plan twice, the build and the review");
    assert!(
        !calls[1].overlapped,
        "the plan's first program was stopped before its second began"
    );
    assert!(calls.iter().all(|call| !call.overlapped));
    // The plan's first try never reached the model, so its second is told the same, whole: it is stored once, as one
    // file under two names.
    let base = real.data.join(format!("rounds/1/{}-1", plan.0));
    let told = |name: &str| base.with_extension(name);
    assert_eq!(
        std::fs::read_to_string(told("message.2.md")).unwrap(),
        std::fs::read_to_string(told("message.md")).unwrap()
    );
    let file = |name: &str| std::fs::metadata(told(name)).unwrap().ino();
    assert_eq!(file("message.2.md"), file("message.md"));
}

/// What the message of a round says in place of its task where its session holds the task already, as the files beside
/// the flows word it (`message.toml`, the part `task`, `held`).
const TASK_HELD: &str = "As given before in this session";

/// Once the engine has waited the first wait before a round that failed in passing is tried again, ten seconds, the
/// run's next round is given: what the run's first round was told, and what its second is.
async fn told_next(real: &mut Real, work: store::WorkNumber, run: decide::RunId) -> (String, String) {
    let calls = real.calls().len();
    real.until(|real| real.calls().len() > calls).await;
    let told = |round: usize| {
        let at = real
            .data
            .join(format!("rounds/{}/{}-{round}.message.md", work.0, run.0));
        std::fs::read_to_string(&at).unwrap_or_else(|e| panic!("{}: {e}", at.display()))
    };
    (told(1), told(2))
}

// A round whose program said it took its message in, and called the model, before its engine ended holds that message
// in its session: given again, the round reads what came of that try from its kept stream, a crash, and is not told
// it again; the run's next round is told only what its session lacks, its task as held, never the whole message. The
// first engine takes what the try's start says, its fill among it, as an engine does before its program can read its
// message. About eleven seconds alone, ten of them the engine's wait before a round that failed in passing is tried
// again, which the harness's clock cannot shorten; no other test holds a round its engine did not see end, nor the
// round after it.
#[tokio::test]
async fn a_round_whose_program_took_its_message_in_before_its_engine_ended_fails_in_passing_and_is_not_told_it_again() {
    let fake = Fake {
        plan_sleeps_once_begun: 30,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| !real.work(work).filled.is_empty()).await;
    real.wait(|real| real.calls().len() == 1).await;
    let plan = real.run_of(work, "plan");
    let base = real.data.join(format!("rounds/1/{}-1", plan.0));
    let stream = base.with_extension("stream.jsonl");
    real.wait(|_| std::fs::read_to_string(&stream).is_ok_and(|s| s.contains(r#""type":"assistant""#)))
        .await;

    real.restart();
    real.until(|real| real.failures == 1 || real.calls().len() == 2).await;
    assert_eq!(
        real.calls().len(),
        1,
        "the round's program was not given its message again"
    );
    assert!(
        !base.with_extension("message.2.md").exists(),
        "no second try was told it"
    );
    let kept = real.work(work);
    match kept.back.get(&(plan, 1)) {
        Some(Back::Failed {
            failure: Failure::Passing,
            ..
        }) => {}
        other => panic!("the round did not fail in passing: {other:?}"),
    }
    let measures = work::measured::of_round(&kept.measured, plan, 1).expect("the try is measured");
    assert!(measures.received, "its program took its message in");
    assert_eq!(
        (measures.calls, measures.first_context),
        (1, Some(1_100)),
        "its call, as the start of it kept with its stream says"
    );
    // No earlier report of the work prices its one call's model: what it cost is not known, and the work waits for the
    // owner, who lets it on.
    let spent = kept.spent[&(plan, 1)];
    assert_eq!((spent.usd_micros, spent.unpriced), (0, true));
    assert!(decide::paused(&kept.records), "the work waits for the owner");
    real.act(work, Act::Resume);
    let (first, next) = told_next(&mut real, work, plan).await;
    assert!(!first.contains(TASK_HELD), "{first}");
    assert!(
        next.contains(TASK_HELD),
        "the next round was told the whole message again:\n{next}"
    );
    // The round after it costs what its own result says, $0.01: the program's own count of the session began again with
    // the program.
    real.until(|real| real.work(work).spent.contains_key(&(plan, 2))).await;
    let kept = real.work(work);
    assert_eq!(kept.spent[&(plan, 2)].usd_micros, 10_000);
    assert!(!kept.spent[&(plan, 2)].unpriced, "all of it is known");
}

// A round whose program took its message in before its engine ended, and printed no return, fails as one whose program
// ended with none, though a new program could not run now, as for a level the settings no longer have: it takes what
// that try took, and its measures are told, so that the session is known to hold its message. Under two seconds alone;
// no other test holds a left try with no return whose item's level is gone.
#[tokio::test]
async fn a_left_try_whose_round_now_fails_for_its_item_s_level_is_counted() {
    let fake = Fake {
        plan_sleeps_once_begun: 30,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    real.level = "2".to_string();
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| !real.work(work).filled.is_empty()).await;
    real.wait(|real| real.calls().len() == 1).await;
    let plan = real.run_of(work, "plan");
    let stream = real.data.join(format!("rounds/1/{}-1.stream.jsonl", plan.0));
    real.wait(|_| std::fs::read_to_string(&stream).is_ok_and(|s| s.contains(r#""type":"assistant""#)))
        .await;

    real.settings.items.highest_level = Some("1".to_string());
    real.restart();
    real.until(|real| real.failures == 1).await;
    assert_eq!(real.calls().len(), 1, "the round was not run again");
    let kept = real.work(work);
    let measures = work::measured::of_round(&kept.measured, plan, 1).expect("the try is measured");
    assert!(measures.received, "its program took its message in");
    // No earlier report of the work prices its one call's model: what the try took is not known, and said so.
    let spent = kept
        .spent
        .get(&(plan, 1))
        .map(|spent| (spent.usd_micros, spent.unpriced));
    assert_eq!(spent, Some((0, true)), "what the try took");
}

// A return the program printed before its engine ended is the round's even where a new program could not run now, as
// for a level the settings no longer have: the left try runs no program, so nothing a program needs to run is asked of
// it, and only the cell after it fails for the level. Under two seconds alone; the test before holds a left try with no
// return, and no other test one with a return whose round could not run a program now.
#[tokio::test]
async fn a_return_printed_before_its_engine_ended_is_taken_though_its_item_s_level_is_gone() {
    let fake = Fake {
        plan_sleeps_once_returned: 30,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    real.level = "2".to_string();
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.wait(|real| real.calls().len() == 1).await;
    let plan = real.run_of(work, "plan");
    let stream = real.data.join(format!("rounds/1/{}-1.stream.jsonl", plan.0));
    real.wait(|_| std::fs::read_to_string(&stream).is_ok_and(|s| s.contains(r#""type":"result""#)))
        .await;

    real.settings.items.highest_level = Some("1".to_string());
    real.restart();
    real.until(|real| real.failures == 1).await;
    assert_eq!(real.calls().len(), 1, "no program ran again");
    let kept = real.work(work);
    let Some(Back::Returned { handed, .. }) = kept.back.get(&(plan, 1)) else {
        panic!("the plan's round handed nothing in: {:?}", kept.back);
    };
    let handed: Vec<(&str, &str)> = (handed.iter())
        .map(|h| (h.block.0.as_str(), h.content.as_str()))
        .collect();
    assert_eq!(handed, [("plan", "Greet by name.")]);
}

// A round whose program took its message in before its engine ended, and whose engine ended before it recorded what
// the try filled into the session, has that told from what the try kept beside its message: the session is known to
// hold the message, and the run's next round is told only what it lacks. About eleven seconds alone, ten of them the
// engine's wait before a round that failed in passing is tried again; the test that holds such a round with its fill
// recorded cannot tell a fill kept from one recorded.
#[tokio::test]
async fn a_left_try_whose_fill_its_engine_did_not_record_has_it_told_from_what_the_try_kept() {
    let fake = Fake {
        plan_sleeps_once_begun: 30,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    // The first engine takes nothing more: what the try's start tells it is never recorded.
    real.wait(|real| real.calls().len() == 1).await;
    let plan = real.run_of(work, "plan");
    let base = real.data.join(format!("rounds/1/{}-1", plan.0));
    let stream = base.with_extension("stream.jsonl");
    real.wait(|_| std::fs::read_to_string(&stream).is_ok_and(|s| s.contains(r#""type":"assistant""#)))
        .await;
    assert!(real.work(work).filled.is_empty(), "the first engine recorded no fill");

    real.restart();
    real.until(|real| real.failures == 1).await;
    let kept = real.work(work);
    let fills = (kept.filled.iter()).filter(|f| f.run == plan && f.round == 1).count();
    assert_eq!(fills, 1, "the try's fill is told once");
    // What the left try's call cost is not known, no earlier report of the work pricing its model: the work waits for
    // the owner, who lets it on.
    assert!(decide::paused(&kept.records), "the work waits for the owner");
    real.act(work, Act::Resume);
    let (first, next) = told_next(&mut real, work, plan).await;
    assert!(!first.contains(TASK_HELD), "{first}");
    assert!(
        next.contains(TASK_HELD),
        "the next round was told the whole message again:\n{next}"
    );
}

// The owner's words the hook gave a round's program before its engine ended are the round's: given again, the round
// says so from what the hook kept, and the words are not new to the cell. Under two seconds alone; no other test holds
// words given to a round its engine did not see end.
#[tokio::test]
async fn words_the_hook_gave_a_try_its_engine_did_not_see_end_are_put_into_its_round() {
    let fake = Fake {
        build_hears_once_begun: 30,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| matches!(real.decision(work), Decision::Busy { cell, .. } if cell.as_str() == "build"))
        .await;
    real.wait(|real| real.calls().iter().any(|call| call.of("Build"))).await;
    let build = real.run_of(work, "build");
    let base = real.data.join(format!("rounds/{}/{}-1", work.0, build.0));
    let stream = base.with_extension("stream.jsonl");
    real.wait(|_| std::fs::read_to_string(&stream).is_ok_and(|s| s.contains(r#""type":"assistant""#)))
        .await;
    let to = engine::spoken::To::Cell("build".into());
    real.act(
        work,
        Act::Say {
            to,
            words: "Greet them warmly.".to_string(),
        },
    );
    let given = base.with_extension("given.1.json");
    real.wait(|_| given.exists()).await;

    real.restart();
    let put = |real: &Real| {
        (real.work(work).records.put_in.iter()).any(|p| p.run == build && p.block.as_str() == "build_words")
    };
    real.until(|real| put(real) || real.failures == 1).await;
    assert!(put(&real), "the words the hook gave are not put into the round");
}

// The owner's words the hook gave a round's program before its engine ended are the round's when the round is stopped
// after, as when it is given again: the stop reads what the hook gave from the round's files, and the words are put into
// the round before it ends, its session known to hold them. Under two seconds alone; the test before holds such a round
// given again, and no other test one stopped.
#[tokio::test]
async fn words_the_hook_gave_a_try_whose_round_is_stopped_after_its_engine_ended_are_put_into_its_round() {
    let fake = Fake {
        build_hears_once_begun: 30,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    // The first process of the engine ends before it stops anything: its stops are dropped.
    real.deafen();
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| matches!(real.decision(work), Decision::Busy { cell, .. } if cell.as_str() == "build"))
        .await;
    real.wait(|real| real.calls().iter().any(|call| call.of("Build"))).await;
    let build = real.run_of(work, "build");
    let base = real.data.join(format!("rounds/{}/{}-1", work.0, build.0));
    let stream = base.with_extension("stream.jsonl");
    real.wait(|_| std::fs::read_to_string(&stream).is_ok_and(|s| s.contains(r#""type":"assistant""#)))
        .await;
    let to = engine::spoken::To::Cell("build".into());
    real.act(
        work,
        Act::Say {
            to,
            words: "Greet them warmly.".to_string(),
        },
    );
    let given = base.with_extension("given.1.json");
    real.wait(|_| given.exists()).await;
    real.act(work, Act::Cancel { why: None });

    // Started again, the stop finds nothing held, and ends the round with what its hook gave.
    real.restart();
    real.until(|real| real.ended_rounds(work, build)).await;
    let put = (real.work(work).records.put_in.iter()).any(|p| p.run == build && p.block.as_str() == "build_words");
    assert!(put, "the words the hook gave are not put into the stopped round");
}

// Pinned as it stands: a return the program wrote, which the command line took, before its engine ended and before the
// program printed its result, is not taken, the stream holding no result. The round fails in passing, and the run's
// next round is told only what its session lacks, its task as held; nothing tells the agent that its return was not
// taken. About eleven seconds alone, ten of them the engine's wait before a round that failed in passing is tried
// again; no other test holds a return written with no result.
#[tokio::test]
async fn a_return_written_before_its_engine_ended_with_no_result_is_not_taken_and_the_next_round_told_what_it_lacks() {
    let fake = Fake {
        plan_sleeps_once_it_wrote_its_return: 30,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| !real.work(work).filled.is_empty()).await;
    real.wait(|real| real.calls().len() == 1).await;
    let plan = real.run_of(work, "plan");
    let stream = real.data.join(format!("rounds/1/{}-1.stream.jsonl", plan.0));
    real.wait(|_| std::fs::read_to_string(&stream).is_ok_and(|s| s.contains(r#""type":"user""#)))
        .await;

    real.restart();
    real.until(|real| real.failures == 1).await;
    let kept = real.work(work);
    match kept.back.get(&(plan, 1)) {
        Some(Back::Failed {
            failure: Failure::Passing,
            ..
        }) => {}
        other => panic!("the round did not fail in passing: {other:?}"),
    }
    // What the cut-off round's calls cost is not known, no earlier report of the work pricing their model: the work
    // waits for the owner, who lets it on.
    assert!(
        kept.spent[&(plan, 1)].unpriced,
        "what the cut-off round cost is not known"
    );
    assert!(decide::paused(&kept.records), "the work waits for the owner");
    real.act(work, Act::Resume);
    let (first, next) = told_next(&mut real, work, plan).await;
    assert!(!first.contains(TASK_HELD), "{first}");
    assert!(
        next.contains(TASK_HELD),
        "the next round was told the whole message again:\n{next}"
    );
    // The round after it costs what its own result says, $0.01: the program's own count of the session began again with
    // the program.
    real.until(|real| real.work(work).spent.contains_key(&(plan, 2))).await;
    let kept = real.work(work);
    assert_eq!(kept.spent[&(plan, 2)].usd_micros, 10_000);
    assert!(!kept.spent[&(plan, 2)].unpriced, "all of it is known");
}

// A return the program printed before its engine ended, kept with the round's stream, is the round's: given again, the
// round takes it and runs nothing again. Under two seconds alone; no other test holds a return its engine did not see.
#[tokio::test]
async fn a_return_printed_before_its_engine_ended_is_taken_and_its_round_not_run_again() {
    let fake = Fake {
        plan_sleeps_once_returned: 30,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.wait(|real| real.calls().len() == 1).await;
    let plan = real.run_of(work, "plan");
    let base = real.data.join(format!("rounds/1/{}-1", plan.0));
    let stream = base.with_extension("stream.jsonl");
    real.wait(|_| std::fs::read_to_string(&stream).is_ok_and(|s| s.contains(r#""type":"result""#)))
        .await;

    real.restart();
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.calls().len(), 3, "the plan once, the build and the review");
    assert!(
        !base.with_extension("message.2.md").exists(),
        "no second try was told it"
    );
    let kept = real.work(work);
    let Some(Back::Returned { handed, .. }) = kept.back.get(&(plan, 1)) else {
        panic!("the plan's round handed nothing in: {:?}", kept.back);
    };
    let handed: Vec<(&str, &str)> = (handed.iter())
        .map(|h| (h.block.0.as_str(), h.content.as_str()))
        .collect();
    assert_eq!(handed, [("plan", "Greet by name.")]);
}

// What a try its engine did not see end committed is the round's: the round of the build, which takes main in, given
// again, reads nothing of the branch as it begins, which would write the try's commit as the outside's. Under two
// seconds alone; no other test holds the branch at a round given so.
#[tokio::test]
async fn what_a_try_its_engine_did_not_see_end_committed_is_not_read_as_the_outsides() {
    let fake = Fake {
        build_sleeps_once_committed: 30,
        ..Fake::default()
    };
    let mut real = Real::open(On::Acts, fake, None, "true");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| matches!(real.decision(work), Decision::Busy { cell, .. } if cell.as_str() == "build"))
        .await;
    real.wait(|real| real.calls().iter().any(|call| call.of("Build"))).await;
    let build = real.run_of(work, "build");
    let stream = real.data.join(format!("rounds/{}/{}-1.stream.jsonl", work.0, build.0));
    real.wait(|_| std::fs::read_to_string(&stream).is_ok_and(|s| s.contains(r#""type":"assistant""#)))
        .await;
    let place = real.place(work);
    real.wait(|_| git_in(&place.dir, &["log", "-1", "--format=%s"]) == "Build the design")
        .await;
    let outside = |real: &Real| {
        (real.work(work).records.written.iter())
            .filter(|w| w.block.as_str() == "tree" && w.by == Outsider::Outside)
            .count()
    };
    let before = outside(&real);

    real.restart();
    let calls = real.calls().len();
    real.until(|real| real.failures == 1 || real.calls().len() > calls)
        .await;
    assert_eq!(
        real.calls().len(),
        calls,
        "the build's program was not given its message again"
    );
    assert_eq!(outside(&real), before, "the try's commit was read as the outside's");
}

#[tokio::test]
async fn a_program_left_running_is_stopped_when_its_round_now_fails_for_its_item_s_level() {
    let fake = Fake {
        plan_sleeps: 30,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    real.level = "2".to_string();
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.wait(|real| real.calls().len() == 1).await;
    let plan = real.run_of(work, "plan");
    let pgid = real.data.join(format!("rounds/1/{}-1.pgid", plan.0));
    real.wait(|_| pgid.exists()).await;

    // Started again on settings that hold items below its level, the round fails for the level: the program the
    // first process left running is stopped all the same, and no other is begun.
    real.settings.items.highest_level = Some("1".to_string());
    real.restart();
    real.until(|real| real.failures == 1).await;
    let leader = real.call_process(1);
    real.wait(|_| !alive(leader)).await;
    assert_eq!(real.calls().len(), 1, "the round was not run again");
    assert!(real.paused(work));
}

#[tokio::test]
async fn a_round_stopped_after_its_engine_ended_has_its_left_program_stopped() {
    let fake = Fake {
        plan_sleeps: 30,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    // The first process of the engine ends before it stops anything: its stops are dropped.
    real.deafen();
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.wait(|real| real.calls().len() == 1).await;
    let plan = real.run_of(work, "plan");
    let pgid = real.data.join(format!("rounds/1/{}-1.pgid", plan.0));
    real.wait(|_| pgid.exists()).await;
    real.act(work, Act::Cancel { why: None });

    // Started again, the stop finds nothing held, and stops the program left running before it ends the round.
    real.restart();
    real.until(|real| real.ended_rounds(work, plan)).await;
    assert_eq!(real.calls().len(), 1, "the round was not run again");
    let leader = real.call_process(1);
    real.wait(|_| !alive(leader)).await;
}

/// Whether the process is alive.
fn alive(process: u32) -> bool {
    let status = std::process::Command::new("kill")
        .args(["-0", &process.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .expect("kill runs");
    status.success()
}
