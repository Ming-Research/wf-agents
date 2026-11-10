//! A work goes through the trial flow to its merge, its agents run by the command line in the work's worktree.

use service::Act;
use work::Spent;

use crate::{Fake, Real, ended, git_in};

#[tokio::test]
async fn a_work_goes_to_its_merge_with_the_command_line() {
    let fake = Fake {
        cost: "0.01",
        fail_first: false,
    };
    let mut real = Real::open(fake, None);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.finished(work), ["plan", "build", "review", "merge"]);

    let calls = real.calls();
    assert_eq!(calls.len(), 3, "one call for each agent's round");
    assert!(
        calls[0].stdin.starts_with("# Plan (cell `plan`), round 1\n"),
        "{}",
        calls[0].stdin
    );
    let worktree = real.data.join("worktrees/1");
    assert!(
        calls.iter().all(|call| call.dir == worktree),
        "each round runs in the work's worktree"
    );
    // The plan begins the engineer's session, and the build goes on in it; the review begins its own.
    let (plan, build, review) = (calls[0].session(), calls[1].session(), calls[2].session());
    assert_eq!(plan.0, "--session-id");
    assert_eq!(build, ("--resume", plan.1));
    assert_eq!(review.0, "--session-id");
    assert_ne!(review.1, plan.1);

    // What each round was told and returned is kept for reading.
    let kept = real.work(work);
    let plan_run = kept.records.started[0].run;
    let told = std::fs::read_to_string(real.data.join(format!("rounds/1/{}-1.message.md", plan_run.0))).unwrap();
    assert_eq!(told, calls[0].stdin);
    assert!(
        real.data
            .join(format!("rounds/1/{}-1.result.json", plan_run.0))
            .is_file()
    );

    // A session's first round costs what it reports; a round that goes on in it, what the session cost more.
    let spent: Vec<Spent> = kept.spent.values().copied().collect();
    let plan_spent = Spent {
        usd_micros: 10_000,
        input_tokens: 15,
        cached_tokens: 100,
        output_tokens: 20,
    };
    assert_eq!(spent[0], plan_spent);
    assert_eq!(spent[1].usd_micros, 0);
    assert_eq!(spent[2].usd_micros, 10_000);

    // The build's commit is merged into main.
    let log = git_in(&real.repo, &["log", "main", "--format=%s"]);
    assert!(log.lines().any(|line| line == "Build the greeting"), "{log}");
    assert!(
        log.lines().next().is_some_and(|line| line.starts_with("Merge work-1")),
        "{log}"
    );
}
