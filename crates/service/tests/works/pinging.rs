//! A keep-warm ping runs beside the rest of its work: the letting go of what the work holds alone waits for it to end,
//! its program running in the work's worktree, and a round of its own line, which the driver holds back.

use decide::{Decision, End};
use engine::Event;
use service::{Act, Script};

use crate::{Trial, trial_dir};

#[tokio::test(start_paused = true)]
async fn a_work_cancelled_while_its_session_is_pinged_lets_go_of_what_it_holds_once_the_ping_has_ended() {
    let mut trial = Trial::warm();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    // The plan asks the owner a question it waits for: the line's session waits, and is pinged before the cache lets
    // it go.
    trial.step().await.unwrap();
    while trial.pinged().is_empty() {
        trial.step().await.expect("the ping is due within the hour");
    }
    assert_eq!(trial.pinged(), [work].into());
    // Cancelled while the ping runs in its worktree, the work lets go of what it holds only once the ping has ended.
    trial.act(work, Act::Cancel { why: None }).unwrap();
    assert_eq!(trial.decision(work), Decision::Ended(End::Cancelled));
    assert!(trial.step().await.is_none(), "nothing is let go beside the ping");
    trial.ping_ends();
    assert!(
        matches!(trial.step().await, Some(Event::Pinged { failed: None, .. })),
        "the ping's end"
    );
    assert_eq!(
        trial.step().await,
        Some(Event::Released),
        "what the work held is let go"
    );
    assert!(trial.step().await.is_none());
}

/// The trial's script, the review's first round running longer than the cache keeps a session and asking for a query,
/// whose result enters the review again: the engineer's line, which the review's opinions run, is pinged while the
/// review's round runs.
fn a_long_first_round_of_the_review() -> Script {
    let text = std::fs::read_to_string(trial_dir().join("scripts/trial.toml")).expect("the script is read");
    let first = "[[flows.trial.rounds]]\ncell = \"review\"\nrun = 1\n";
    let asks = r#"took = { ms = 3_400_000 }
asks = [{ tool = "query", input = '{"sql": "select number from items"}' }]"#;
    let text = text.replacen(first, &format!("{first}{asks}\n\n{first}round = 2\n"), 1);
    Script::read(&text).expect("the script reads")
}

// The engineer's session is pinged on time while the review's round runs; the review's call and its next round, of
// another line, are handed out at once beside the ping; and the build, which the review's opinions run, waits for the
// ping to end. Under a second alone, on the paused clock, a store of its own; no other test holds what the loop hands
// out beside a ping.
#[tokio::test(start_paused = true)]
async fn a_ping_runs_beside_the_rounds_of_another_line_and_one_of_its_own_line_waits_for_it() {
    let mut trial = Trial::warm_with(a_long_first_round_of_the_review());
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    let asked = |d: &Decision| matches!(d, Decision::Wait { at, .. } if at.as_str() == "plan");
    trial.until(work, asked).await;
    trial.answer(work, "By name, as the user likes.");
    let reviewing = |d: &Decision| matches!(d, Decision::Busy { cell, .. } if cell.as_str() == "review");
    trial.until(work, reviewing).await;
    while trial.pinged().is_empty() {
        trial.step().await.expect("the ping is due within the hour");
    }
    assert!(
        reviewing(&trial.decision(work)),
        "the engineer was pinged beside the review's round"
    );
    // The review's round asks, the call has its result, and the review's next round ends, all beside the ping.
    let review = |trial: &Trial| {
        let kept = trial.work(work);
        let run = (kept.records.started.iter())
            .find(|s| s.cell.as_str() == "review")
            .map(|s| s.run);
        (kept.records.returned.iter()).filter(|r| Some(r.run) == run).count()
    };
    while review(&trial) < 2 {
        let event = trial.step().await.expect("the review goes on beside the ping");
        assert!(!matches!(event, Event::Pinged { .. }), "the ping ended by itself");
    }
    assert_eq!(trial.pinged(), [work].into(), "the ping is still under way");
    // The review objected: the build, of the line pinged, does not begin beside the ping.
    let builds = (trial.work(work).records.started.iter())
        .filter(|s| s.cell.as_str() == "build")
        .count();
    assert_eq!(builds, 2, "the build that asked and the one its result began, no more");
    assert!(
        matches!(trial.decision(work), Decision::Run { cell, .. } if cell.as_str() == "build"),
        "{:?}",
        trial.decision(work)
    );
    trial.ping_ends();
    assert!(matches!(trial.step().await, Some(Event::Pinged { failed: None, .. })));
    assert!(
        matches!(trial.decision(work), Decision::Busy { cell, .. } if cell.as_str() == "build"),
        "the build begins once the ping has ended"
    );
}
