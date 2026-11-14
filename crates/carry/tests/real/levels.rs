//! A round runs on the model of its item's level, raised where its line says so, at its cell's effort, and what each
//! try ran on is in the work's records; a round whose item's level the settings no longer allow fails, saying so.

use decide::Pauser;
use service::Act;
use store::WorkNumber;
use work::RanOn;

use crate::{Call, Fake, Real, ended};

/// The model and the effort a call of the command line was given.
fn ran_with(call: &Call) -> (&str, Option<&str>) {
    let after = |flag: &str| {
        let at = call.args.iter().position(|a| a == flag)?;
        call.args.get(at + 1).map(String::as_str)
    };
    (after("--model").expect("a model is named"), after("--effort"))
}

/// What the try of this number of this round of the cell's last run ran on, as the work's records say.
fn kept(real: &Real, work: WorkNumber, cell: &str, round: usize, try_: usize) -> RanOn {
    kept_of(real, work, real.run_of(work, cell), round, try_)
}

/// The runs of the cell, in the order they started.
fn runs_of(real: &Real, work: WorkNumber, cell: &str) -> Vec<decide::RunId> {
    (real.work(work).records.started.iter())
        .filter(|s| s.cell.as_str() == cell)
        .map(|s| s.run)
        .collect()
}

/// What the try of this number of this round of the run ran on, as the work's records say.
fn kept_of(real: &Real, work: WorkNumber, run: decide::RunId, round: usize, try_: usize) -> RanOn {
    let kept = real.work(work);
    let tries: Vec<&RanOn> = (kept.ran.iter())
        .filter(|r| r.run == run && r.round == round)
        .map(|r| &r.on)
        .collect();
    tries.get(try_ - 1).copied().cloned().expect("it is recorded")
}

fn on(level: &str, model: &str) -> RanOn {
    RanOn {
        level: level.to_string(),
        model: model.to_string(),
        effort: Some("high".to_string()),
        account: None,
    }
}

#[tokio::test]
async fn each_round_runs_on_the_model_of_its_item_s_level_at_its_cell_s_effort_and_keeps_what_it_ran_on() {
    let mut real = Real::trial(Fake::default());
    real.level = "2".to_string();
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    assert_eq!(calls.len(), 3);
    for call in &calls {
        assert_eq!(
            ran_with(call),
            ("claude-sonnet-5-5", Some("high")),
            "level 2, at the cells' effort"
        );
    }
    assert_eq!(kept(&real, work, "plan", 1, 1), on("2", "claude-sonnet-5-5"));
}

#[tokio::test]
async fn a_line_that_runs_above_its_item_s_level_runs_on_the_model_of_the_level_above() {
    let mut real = Real::trial(Fake::default());
    // The trial's review is on a line that runs one level above its item's.
    for cell in real
        .flow
        .cells
        .iter_mut()
        .filter(|c| c.line.as_deref() == Some("reviewer"))
    {
        cell.levels_above = 1;
    }
    real.restart();
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    assert_eq!(
        calls.iter().map(|c| ran_with(c).0).collect::<Vec<_>>(),
        ["claude-haiku-5-5", "claude-haiku-5-5", "claude-sonnet-5-5"],
        "the plan and the build at level 1, the review at 2"
    );
    assert_eq!(kept(&real, work, "review", 1, 1), on("2", "claude-sonnet-5-5"));
}

#[tokio::test]
async fn a_round_tried_again_keeps_what_each_try_ran_on() {
    let fake = Fake {
        lose_session: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    // The build, entered again for the check's result, was refused its session and tried again: each try kept what it
    // ran on.
    assert_eq!(kept(&real, work, "build", 1, 1), on("1", "claude-haiku-5-5"));
    assert_eq!(kept(&real, work, "build", 1, 2), on("1", "claude-haiku-5-5"));
}

#[tokio::test]
async fn a_try_keeps_what_it_ran_on_after_the_settings_give_its_level_another_model() {
    let fake = Fake {
        holds_first: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.holds(work)).await;
    // The settings change while the build waits for its query: level 1 runs on another model from now on.
    let level = real.settings.agents.levels.get_mut("1").expect("level 1");
    level.model = "claude-opus-5-5".to_string();
    real.restart();
    assert!(real.holds(work), "the query is given again, and held");
    real.answer(work, "By their first name.");
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(
        kept(&real, work, "plan", 1, 1),
        on("1", "claude-haiku-5-5"),
        "as it ran"
    );
    let builds = runs_of(&real, work, "build");
    assert_eq!(
        kept_of(&real, work, builds[0], 1, 1),
        on("1", "claude-haiku-5-5"),
        "as it ran"
    );
    assert_eq!(
        kept_of(&real, work, builds[1], 1, 1),
        on("1", "claude-opus-5-5"),
        "on the settings it ran on"
    );
}

/// The work's pause, which must be the engine's, with its reason.
fn paused_for(real: &Real, work: WorkNumber) -> String {
    let kept = real.work(work);
    let pause = decide::pause(&kept.records).expect("the work is paused");
    assert_eq!(pause.by, Pauser::Engine);
    pause.why.clone().expect("a reason")
}

#[tokio::test]
async fn a_round_of_an_item_whose_level_the_settings_no_longer_have_fails_and_pauses_the_work() {
    let mut real = Real::trial(Fake::default());
    real.level = "2".to_string();
    let work = real.add().await;
    real.settings.agents.levels.remove("2");
    real.restart();
    real.act(work, Act::Resume);
    real.until(|real| real.failures == 1).await;
    let why = paused_for(&real, work);
    assert!(
        why.contains("the item's level: 2 is no level: the levels are 1"),
        "{why}"
    );
    assert!(real.calls().is_empty(), "no program was run");
}

#[tokio::test]
async fn a_round_of_an_item_above_the_highest_level_now_fails_and_pauses_the_work() {
    let mut real = Real::trial(Fake::default());
    real.level = "2".to_string();
    let work = real.add().await;
    real.settings.items.highest_level = Some("1".to_string());
    real.restart();
    real.act(work, Act::Resume);
    real.until(|real| real.failures == 1).await;
    let why = paused_for(&real, work);
    assert!(
        why.contains("the item's level: 2 is above 1, the highest level an item may be at (items.highest_level)"),
        "{why}"
    );
    assert!(real.calls().is_empty(), "no program was run");
}
