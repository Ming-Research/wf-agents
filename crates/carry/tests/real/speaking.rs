//! What the owner says to a round under way reaches it through the real carriers: the carrier keeps the round as it is
//! given, puts the words where the round's hook gives them, and as the round's program ends says which of them the hook
//! gave, which the engine puts into the round.

use engine::spoken::To;
use service::Act;

use crate::{Fake, On, Real, ended};

// The owner speaks to the build's line while its round works: the round's hook gives it the words, in the words of
// `said` in the message's file; the round answers them in the record of what is said, saying it dealt with them; the
// carrier says the round was given them, and the engine puts their versions into the round, so that the build is not
// run again for them. Under a second alone (a quarter of one in CI), a repository on disk and the fake command line,
// which runs the hook its settings give after a use of a tool, as the program does; no other test hands a round the
// owner's words through the carriers.
#[tokio::test]
async fn words_said_to_a_round_under_way_reach_it_by_its_hook_and_are_taken_with_its_return() {
    let mut real = Real::open(
        On::Spoken,
        Fake {
            hears_words: true,
            ..Fake::default()
        },
        None,
        "true",
    );
    let work = real.add().await;
    real.act(work, Act::Resume);
    // The build's round is under way, and works until the words wait for its hook.
    real.wait(|real| real.calls_dir().join("listening").exists()).await;
    let said = Act::Say {
        to: To::Line("engineer".to_string()),
        words: "Keep it to one table.".to_string(),
    };
    real.act(work, said);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.finished(work), ["build", "merge"], "the build ran once");
    let kept = real.work(work);
    let entry = |at: decide::Seq| kept.contents[&("discussion".into(), at)].clone();
    let written = (kept.records.written.iter()).find(|w| w.block.as_str() == "discussion");
    let owners = written.expect("the owner's entry");
    assert_eq!(owners.said_to.as_ref().map(|c| c.as_str()), Some("build"));
    // What the hook gave the round: the owner's entry, in the words of `said`.
    let heard = std::fs::read_to_string(real.calls_dir().join("1.heard")).expect("the hook gave the words");
    let heard: serde_json::Value = serde_json::from_str(&heard).expect("what the hook prints");
    let given = heard["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("what it gives");
    let opening = format!(
        "## `discussion`, version {}, said while this round runs\n\n```\n{}\n```\n\nIt is new to this round as \
         `on_spoken`",
        owners.at.0,
        entry(owners.at)
    );
    assert!(given.starts_with(&opening), "{given}");
    // Both versions the words wrote are put into the round, which the carrier said the hook gave it.
    let build = real.run_of(work, "build");
    let put: Vec<&str> = (kept.records.put_in.iter())
        .filter(|p| p.run == build)
        .map(|p| p.block.as_str())
        .collect();
    assert!(
        put.contains(&"discussion") && put.contains(&"build_spoken_to"),
        "{put:?}"
    );
    // The build's answer is the record's next entry, its run's.
    let finished = (kept.records.finished.iter())
        .find(|f| f.run == build)
        .expect("the build finished");
    let answer = (finished.writes.iter()).any(|w| w.block.as_str() == "discussion");
    assert!(answer, "the build answered in the record");
    assert_eq!(entry(finished.at), "An agent:\n\nHeard the owner.");
}
