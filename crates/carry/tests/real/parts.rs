//! A round a script of parts gives a part to play, run with the command line as any round is (design document
//! `testing`, 12): after all its cell gives it, it is told, where its session lacks them, the words that it is a test
//! agent, and its part; a round given no part is told neither, and the system prompt and the form are those of a round
//! given none.

use flow::agents::Part;
use service::Act;

use crate::{Call, Fake, Real, beside, ended};

/// The plan and the build of the first work each play a part; the review, and every round of another work, none.
const PARTS: &str = r#"
[[flows.trial.rounds]]
cell = "plan"
work = 1
part = "Plan the greeting in one line."

[[flows.trial.rounds]]
cell = "build"
work = 1
part = "Build the greeting the plan says."
"#;

/// The text of the part of the message, as the files beside the flows write it.
fn written(part: Part) -> String {
    let message = beside().message;
    let part = message
        .parts
        .iter()
        .find(|p| p.part == part)
        .expect("the part is written");
    part.text.clone().expect("it has a text")
}

/// The words that tell a round it is a test agent.
fn told() -> String {
    written(Part::Told)
}

/// The part a round plays, as it is told it.
fn playing(part: &str) -> String {
    written(Part::Played).replace("{part}", part)
}

/// What comes before the part a round plays, as it is told it.
fn heading() -> String {
    let written = written(Part::Played);
    let (heading, _) = written
        .split_once("{part}")
        .expect("the part has a place for what is played");
    heading.trim_end().to_string()
}

/// The system prompt the call was given.
fn system(call: &Call) -> Vec<String> {
    let at = |arg: &str| call.args.iter().position(|a| a == arg);
    let from = at("--append-system-prompt").expect("a system prompt") + 1;
    let to = at("--json-schema").expect("a schema");
    call.args[from..to].to_vec()
}

// The first work's plan is told, after what ran it, that it is a test agent and its part; its build, going on in the
// line's session, its own part and not the words again; its review, given no part, neither; and what each was told,
// as kept, holds what it was told. The second work, given no part, is told neither, and each of its calls is given the
// system prompt and the form the first work's call of the same cell was given, byte for byte. A few seconds alone; no
// other test holds a round to what a script of parts gives it.
#[tokio::test]
async fn a_round_is_told_once_a_session_that_it_is_a_test_agent_and_each_round_its_part() {
    let mut real = Real::playing(Fake::default(), PARTS);
    let first = real.add().await;
    real.act(first, Act::Resume);
    real.until(|real| ended(&real.decision(first))).await;
    let second = real.add().await;
    real.act(second, Act::Resume);
    real.until(|real| ended(&real.decision(second))).await;
    let calls = real.calls();
    assert_eq!(calls.len(), 6, "the plan, the build and the review of each work");
    let (plan, build, review) = (&calls[0], &calls[1], &calls[2]);
    assert!(plan.of("Plan") && build.of("Build") && review.of("Review"));

    let triggers = written(Part::Triggers);
    let ran = triggers.trim_end();
    let at = |call: &Call, text: &str| call.stdin.find(text);
    let plays = playing("Plan the greeting in one line.");
    let (why, words, part) = (at(plan, ran), at(plan, &told()), at(plan, &plays));
    assert!(
        why.is_some() && why < words && words < part,
        "after what ran it, the words and then its part: {}",
        plan.stdin
    );
    assert!(
        plan.stdin.trim_end().ends_with(plays.trim_end()),
        "nothing after its part: {}",
        plan.stdin
    );
    assert!(
        build.stdin.contains(&playing("Build the greeting the plan says.")),
        "{}",
        build.stdin
    );
    assert!(
        !build.stdin.contains(&told()),
        "its session holds the words: {}",
        build.stdin
    );
    for call in [review, &calls[3], &calls[4], &calls[5]] {
        assert!(!call.stdin.contains(&told()), "{}", call.stdin);
        assert!(!call.stdin.contains(&heading()), "{}", call.stdin);
    }

    for (with, without) in [(1, 4), (2, 5), (3, 6)] {
        assert_eq!(
            system(&calls[with - 1]),
            system(&calls[without - 1]),
            "the system prompt of call {with}"
        );
        assert_eq!(
            real.form_given(with),
            real.form_given(without),
            "the form of call {with}"
        );
    }

    let run = real.run_of(first, "plan");
    let kept = std::fs::read_to_string(real.data.join(format!("rounds/1/{}-1.message.md", run.0)))
        .expect("what the plan was told is kept");
    assert!(kept.contains(&told()) && kept.contains(&plays), "{kept}");
}

// A round given a part whose program the engine left running when it stopped is run again, and its second try is told
// what its first was, the words that it is a test agent and its part among it, the first having put nothing into the
// session. A few seconds alone, the plan's first program waiting to be stopped.
#[tokio::test]
async fn a_round_given_again_after_the_engine_stopped_is_told_its_part_the_same() {
    let fake = Fake {
        plan_sleeps: 30,
        ..Fake::default()
    };
    let mut real = Real::playing(fake, PARTS);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.wait(|real| real.calls().len() == 1).await;
    let plan = real.run_of(work, "plan");
    let pgid = real.data.join(format!("rounds/1/{}-1.pgid", plan.0));
    real.wait(|_| pgid.exists()).await;

    real.restart();
    real.until(|real| ended(&real.decision(work))).await;
    let base = real.data.join(format!("rounds/1/{}-1", plan.0));
    let told_in = |name: &str| std::fs::read_to_string(base.with_extension(name)).expect("it is kept");
    let again = told_in("message.2.md");
    assert_eq!(again, told_in("message.md"));
    assert!(again.contains(&told()), "{again}");
    assert!(again.contains(&playing("Plan the greeting in one line.")), "{again}");
}
