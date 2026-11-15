//! A work goes through its flow to its merge, its agents run by the command line in the work's worktree.

use service::Act;
use work::Spent;

use crate::{Call, Fake, NAMED, On, Real, ended, git_in};

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

// A build that asks for the tests once, through the real carriers: its return is read with the entry it wrote, the cell
// of the act that runs the tests runs them on the local repository and hands the result in as the build's own block,
// and the build, entered again with it, is given it as the result of what it asked for and goes on to the merge. A few
// seconds alone, a repository on disk and the fake command line; no other test runs an entry through the carriers.
#[tokio::test]
async fn a_build_that_asks_for_tests_once_is_given_their_result_and_the_work_merges() {
    let mut real = Real::open(
        On::Tested,
        Fake {
            asks_tests: true,
            ..Fake::default()
        },
        None,
        "echo the tests passed",
    );
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(
        real.finished(work),
        ["start", "build", "tests", "build", "check", "merge"]
    );
    let kept = real.work(work);
    let entry = (kept.contents.iter())
        .find(|((block, _), _)| block.as_str() == "run_tests")
        .map(|(_, content)| content.clone())
        .expect("the entry is kept");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&entry).ok(),
        Some(serde_json::json!({"tests": [], "checks": []}))
    );
    let result = (kept.contents.iter())
        .find(|((block, _), _)| block.as_str() == "build_tested")
        .map(|(_, content)| content.clone())
        .expect("the result is handed in");
    assert!(result.contains("echo the tests passed"), "{result}");
    let builds: Vec<Call> = real.calls().into_iter().filter(|c| c.of("Build")).collect();
    assert_eq!(builds.len(), 2, "the build is entered again for the result");
    assert!(
        builds[1].stdin.contains("The tests you asked for"),
        "the result is given"
    );
}

// A build that asks for the tests on a commit of its own and then on its head, through the real carriers: each result
// reaches it as the result of what it asked for, saying first its entry and the commit it ran on; and the review,
// which asked for none, is given both, each under the heading of the version of the build's block it came in, cut as a
// result is given. A few seconds alone, a repository on disk and the fake command line; no other test gives one cell's
// results of tests to another through the carriers.
#[tokio::test]
async fn a_build_that_asks_for_tests_on_two_of_its_commits_is_given_each_result_and_its_review_both_cut() {
    let mut real = Real::open(
        On::Reviewed,
        Fake {
            asks_named_tests: true,
            ..Fake::default()
        },
        None,
        "head -c 30000 /dev/zero | tr '\\0' x",
    );
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let finished = real.finished(work);
    let first = ["start", "build", "tests", "build", "tests", "build"];
    assert_eq!(finished[..first.len()], first, "{finished:?}");
    assert!(finished.iter().any(|cell| cell == "review"), "{finished:?}");
    let kept = real.work(work);
    let versions = |name: &str| -> Vec<(u64, String)> {
        (kept.contents.iter())
            .filter(|((block, _), _)| block.as_str() == name)
            .map(|((_, at), content)| (at.0, content.clone()))
            .collect()
    };
    let (entries, results) = (versions("run_tests"), versions("build_tested"));
    assert_eq!((entries.len(), results.len()), (2, 2), "{entries:?}");
    // The first entry names the first of the build's two commits, which is not its head, and the second its head.
    let commit = |entry: &str| {
        let entry: serde_json::Value = serde_json::from_str(entry).expect("an entry");
        entry["commit"].as_str().expect("a commit named").to_string()
    };
    let named = [commit(&entries[0].1), commit(&entries[1].1)];
    assert_ne!(named[0], named[1]);
    let bound = crate::beside().acts.run_tests.given.bound;
    let builds: Vec<Call> = real.calls().into_iter().filter(|c| c.of("Build")).collect();
    assert_eq!(builds.len(), 3, "the build is entered again for each result");
    for (i, (_, result)) in results.iter().enumerate() {
        let opening = format!(
            "For `run_tests`, version {}:\nOn the commit {}:\n",
            entries[i].0, named[i]
        );
        assert!(result.starts_with(&opening), "{opening}");
        assert!(result.len() > bound, "the result is longer than a round is given");
        let given = &builds[i + 1].stdin;
        assert!(
            given.contains("### The tests you asked for") && given.contains(&opening),
            "{opening}"
        );
    }
    let reviews: Vec<Call> = real.calls().into_iter().filter(|c| c.of("Review")).collect();
    let review = &reviews.first().expect("the review ran").stdin;
    assert!(
        !review.contains("The tests you asked for"),
        "given as no result of its own"
    );
    for (at, result) in &results {
        // Cut as a result is given: its first bytes up to the bound, then the words that say so.
        let given = format!("## `build_tested`, version {at}\n\n```\n{}\n\n", &result[..bound]);
        assert!(review.contains(&given), "version {at} is not given cut");
    }
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
