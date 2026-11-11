//! A round runs on the model of its item's level, and what it ran on is kept with what it was told.

use service::Act;

use crate::{Fake, Real, ended};

#[tokio::test]
async fn each_round_runs_on_the_model_of_its_item_s_level_and_keeps_what_it_ran_on() {
    let mut real = Real::trial(Fake::default());
    real.level = "2".to_string();
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    assert!(!calls.is_empty());
    for call in &calls {
        let at = call.args.iter().position(|a| a == "--model").expect("a model is named");
        assert_eq!(call.args[at + 1], "claude-sonnet-5-5", "the model of level 2");
        assert!(
            !call.args.contains(&"--effort".to_string()),
            "the trial's cells set no effort"
        );
    }
    let run = real.work(work).records.started[0].run;
    let kept = real.data.join(format!("rounds/{}/{}-1.ran.json", work.0, run.0));
    let kept: serde_json::Value = serde_json::from_slice(&std::fs::read(kept).expect("it is kept")).unwrap();
    assert_eq!(
        kept,
        serde_json::json!({ "level": "2", "model": "claude-sonnet-5-5", "effort": null })
    );
}
