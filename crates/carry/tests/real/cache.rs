//! A round that reads less from the prompt cache than its session's previous round left there pauses its work with
//! the reason, what it handed in taken; the numbers and the judgement of each round are kept beside its files.

use carry::cache::{Judged, Kept, Unjudged};
use decide::Decision;
use service::Act;
use store::WorkNumber;

use crate::{Fake, On, Real, ended};

/// What is kept of the cache of the round of this number of the work's latest run of the cell.
fn kept(real: &Real, work: WorkNumber, cell: &str, number: usize) -> Kept {
    let run = real.run_of(work, cell);
    let at = real
        .data
        .join(format!("rounds/{}/{}-{number}.cache.json", work.0, run.0));
    let text = std::fs::read(&at).unwrap_or_else(|e| panic!("{}: {e}", at.display()));
    serde_json::from_slice(&text).expect("what is kept reads")
}

/// Why the work is paused.
fn why(real: &Real, work: WorkNumber) -> String {
    let kept = real.work(work);
    let pause = decide::pause(&kept.records).expect("the work is paused");
    pause.why.clone().expect("a reason")
}

/// The fake whose calls that go on in their session read nothing from the cache.
fn missing() -> Fake {
    Fake {
        cache_miss: true,
        ..Fake::default()
    }
}

#[tokio::test]
async fn a_round_that_reads_the_cache_as_expected_goes_on() {
    let mut real = Real::trial(Fake::default());
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.holds, 0);
    let begins = Judged::NotJudged { why: Unjudged::Begins };
    assert_eq!(kept(&real, work, "plan", 1).judged, begins);
    assert_eq!(
        kept(&real, work, "review", 1).judged,
        begins,
        "the review begins its own session"
    );
    let build = kept(&real, work, "build", 1);
    assert_eq!(
        build.judged,
        Judged::Hit {
            expected: 1100,
            read: 1100
        }
    );
    assert_eq!((build.first.read, build.first.written), (1100, 50));
    assert_eq!((build.last.read, build.last.written), (1000, 100));

    // The stream is kept whole, and its result line alone where it was kept before.
    let plan = real.run_of(work, "plan");
    let base = real.data.join(format!("rounds/1/{}-1", plan.0));
    let stream = std::fs::read_to_string(base.with_extension("result.jsonl")).unwrap();
    assert_eq!(stream.lines().count(), 5, "{stream}");
    let result = std::fs::read_to_string(base.with_extension("result.json")).unwrap();
    assert_eq!(result.trim_end(), stream.lines().last().unwrap());
    assert!(result.starts_with(r#"{"type":"result""#), "{result}");
}

#[tokio::test]
async fn a_round_that_reads_nothing_from_the_cache_pauses_its_work_with_the_reason() {
    let mut real = Real::trial(missing());
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.finished(work) == ["plan", "build"] && real.decision(work) == Decision::Paused)
        .await;
    assert_eq!(real.holds, 1);
    assert_eq!(real.calls().len(), 2, "no round began after the miss");
    let build = real.run_of(work, "build");
    let why = why(&real, work);
    let read = format!(
        "round 1 of run {} read 0 tokens from the cache where 1100 were expected",
        build.0
    );
    assert!(why.starts_with(&read), "{why}");
    assert!(
        why.ends_with("what is sent to this session changed, or the cache was lost"),
        "{why}"
    );
    assert_eq!(
        kept(&real, work, "build", 1).judged,
        Judged::Miss {
            expected: 1100,
            read: 0
        }
    );

    // Let on, the review begins a session of its own, and the work goes to its merge.
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.holds, 1);
}

#[tokio::test]
async fn with_the_check_off_a_round_that_misses_the_cache_goes_on() {
    let mut real = Real::open_with(On::Trial, missing(), None, "true", |settings| {
        settings.agents.cache_check = false;
    });
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.holds, 0);
    assert!(
        matches!(kept(&real, work, "build", 1).judged, Judged::Miss { .. }),
        "the judgement is kept all the same"
    );
}

#[tokio::test]
async fn a_round_that_misses_the_cache_over_the_budget_says_both() {
    // Each call says the session has cost four dollars so far; the budget is three.
    let fake = Fake {
        cost: "4.0",
        ..missing()
    };
    let mut real = Real::open(On::Trial, fake, Some(3.0), "true");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.finished(work) == ["plan"] && real.decision(work) == Decision::Paused)
        .await;
    assert!(!why(&real, work).contains("cache"), "the plan begins its session");
    real.act(work, Act::Resume);
    real.until(|real| real.finished(work) == ["plan", "build"] && real.decision(work) == Decision::Paused)
        .await;
    assert_eq!(real.holds, 2);
    let why = why(&real, work);
    assert!(
        why.contains("read 0 tokens from the cache where 1100 were expected"),
        "{why}"
    );
    assert!(
        why.ends_with("; and work 1 has spent $4.00, over its budget of $3.00"),
        "{why}"
    );
}
