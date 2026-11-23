//! A work goes through its flow to its merge, its agents run by the command line in the work's worktree.

use service::Act;
use work::Spent;

use crate::{Fake, NAMED, On, Real, ended, git_in};

#[tokio::test]
async fn a_work_goes_to_its_merge_with_the_command_line() {
    let mut real = Real::trial(Fake::default());
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.finished(work), ["plan", "build", "review", "merge"]);

    let calls = real.calls();
    assert_eq!(calls.len(), 3, "one call for each agent's round");
    assert!(calls[0].stdin.starts_with("# Plan\n"), "{}", calls[0].stdin);
    let worktree = real.place(work).dir;
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
            .join(format!("rounds/1/{}-1.stream.jsonl", plan_run.0))
            .is_file()
    );
    assert!(
        !real.data.join(format!("rounds/1/{}-1.pgid", plan_run.0)).exists(),
        "no program runs"
    );

    // A session's first round costs what it reports; a round that goes on in it, what the session cost more.
    let spent: Vec<Spent> = kept.spent.values().copied().collect();
    let plan_spent = Spent {
        usd_micros: 10_000,
        input_tokens: 15,
        cached_tokens: 100,
        output_tokens: 20,
        ..Default::default()
    };
    assert_eq!(spent[0], plan_spent);
    assert_eq!(spent[1].usd_micros, 0);
    assert_eq!(spent[2].usd_micros, 10_000);

    // The build's commit is merged into main, by a merge named by the work's number: the flow has no block that
    // names it.
    let log = git_in(&real.repo, &["log", "main", "--format=%s"]);
    assert!(log.lines().any(|line| line == "Build the greeting"), "{log}");
    assert_eq!(log.lines().next(), Some("Work 1"), "{log}");

    // Every commit of the work ends with the lines that name its item and its work: the build's, made by its git, from
    // the hook the engine hands each round's, and the merge's, from the engine's words.
    let ends = "\n\nItem: 1\nWork: W1";
    let said = |subject: &str| {
        git_in(
            &real.repo,
            &["log", "-1", "--format=%B", &format!("--grep=^{subject}$"), "main"],
        )
    };
    assert_eq!(said("Build the greeting"), format!("Build the greeting{ends}"));
    assert_eq!(said("Work 1"), format!("Work 1{ends}"));
    let found = git_in(
        &real.repo,
        &[
            "log",
            "main",
            "--grep=^Item: 1$",
            "--grep=^Work: W1$",
            "--all-match",
            "--format=%s",
        ],
    );
    for subject in ["Work 1", "Build the greeting"] {
        assert!(found.lines().any(|line| line == subject), "{found}");
    }
}

#[tokio::test]
async fn a_work_merges_by_the_name_its_build_wrote_and_its_items_title_reaches_no_commit() {
    let mut real = Real::open(On::Acts, Fake::default(), None, "true");
    // The item's title is in the owner's language, the repository in its own.
    real.title = "向用户问好".to_string();
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.finished(work), ["start", "build", "check", "merge"]);
    assert_eq!(git_in(&real.repo, &["log", "-1", "--format=%s", "main"]), NAMED);
    let every = git_in(&real.repo, &["log", "--all", "--format=%B"]);
    assert!(!every.contains(&real.title), "{every}");
}

#[tokio::test]
async fn a_work_is_started_checked_and_merged_at_the_commit_the_check_passed() {
    let mut real = Real::open(On::Acts, Fake::default(), None, "true");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.finished(work), ["start", "build", "check", "merge"]);
    let place = real.place(work);
    assert!(place.branch.starts_with("work-1-"), "{}", place.branch);
    let head = git_in(&real.repo, &["rev-parse", &place.reference()]);

    // The check says the commit it checked first, and the merge merged that commit.
    let kept = real.work(work);
    let check = real.run_of(work, "check");
    let finished = kept
        .records
        .finished
        .iter()
        .find(|f| f.run == check)
        .expect("the check finished");
    let passed = &kept.contents[&("passed".into(), finished.at)];
    assert_eq!(
        passed.lines().next(),
        Some(format!("Checked commit {head}").as_str()),
        "{passed}"
    );
    let parents = git_in(&real.repo, &["rev-list", "--parents", "-n", "1", "main"]);
    assert_eq!(parents.split(' ').nth(2), Some(head.as_str()));
}
