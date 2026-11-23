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

// What a try's program leaves running in its group as it ends is stopped, and counted with the try in the work's
// records before its round's end is: of the plan's first round and its first try. Under two seconds alone, most of it the
// leftover ending on TERM; no other test follows a try's count from its carrier into the records.
#[tokio::test]
async fn what_a_trys_program_leaves_running_is_stopped_and_counted_with_the_try_before_its_round_ends() {
    let fake = Fake {
        plan_leaves_running: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.wait(|real| real.calls().len() == 1).await;
    let plan = real.run_of(work, "plan");
    real.until(|real| real.ended_rounds(work, plan)).await;
    let kept = real.work(work);
    let of = work::Left::Try {
        run: plan,
        round: 1,
        attempt: 1,
    };
    let counted: Vec<(&work::Left, usize)> = (kept.left_behind.iter()).map(|l| (&l.of, l.processes)).collect();
    assert_eq!(counted, [(&of, 1)]);
    let returned = (kept.records.returned.iter())
        .find(|r| r.run == plan)
        .expect("the round ended");
    assert!(kept.left_behind[0].at < returned.at, "counted after its round's end");
    let left = std::fs::read_to_string(real.calls_dir().join("left_running")).expect("what the plan left");
    let running = std::process::Command::new("kill")
        .args(["-0", left.trim()])
        .stderr(std::process::Stdio::null())
        .status()
        .expect("kill runs")
        .success();
    let _ = std::process::Command::new("kill")
        .args(["-s", "KILL", left.trim()])
        .status();
    assert!(!running, "what the plan's program left runs on after its round ended");
}

/// Runs the plan's first round with its program leaving a process of this kind in its group, and gives how long the
/// round took to end once the program was called, what was counted of the try, and whether the process was still
/// running, not a zombie, once the round had ended. The process is killed before anything is asserted.
async fn leaves(leftover: crate::Leftover) -> (std::time::Duration, Vec<(work::Left, usize)>, bool, decide::RunId) {
    let fake = Fake {
        plan_leaves_running: true,
        plan_leftover: leftover,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.wait(|real| real.calls().len() == 1).await;
    let called = std::time::Instant::now();
    let plan = real.run_of(work, "plan");
    real.until(|real| real.ended_rounds(work, plan)).await;
    let took = called.elapsed();
    let counted = (real.work(work).left_behind.iter())
        .map(|l| (l.of.clone(), l.processes))
        .collect();
    let left = std::fs::read_to_string(real.calls_dir().join("left_running")).expect("what the plan left");
    let left = left.trim().to_string();
    let mut running = true;
    // A process killed is a zombie until the system reaps it, its parent having ended: that is no process running.
    for _ in 0..20 {
        let stat = std::process::Command::new("ps")
            .args(["-o", "stat=", "-p", &left])
            .output()
            .expect("ps runs");
        let stat = String::from_utf8_lossy(&stat.stdout).trim().to_string();
        running = !stat.is_empty() && !stat.starts_with('Z');
        if !running {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let _ = std::process::Command::new("kill").args(["-s", "KILL", &left]).status();
    (took, counted, running, plan)
}

// A process the program leaves in its group that ignores TERM is killed once the stop's wait of three seconds has
// passed, and counted, before the round ends: the reference is the owner's answer to `leftovers-are-stopped` (TERM, then
// KILL after 3 s) and acceptance condition 9. Fails where only TERM is sent, or where the round ends before the KILL.
// About four seconds alone, three of them the wait before the KILL, which no shorter test can show; the engineer's test
// holds only a process that ends on TERM.
#[tokio::test]
async fn what_a_trys_program_leaves_running_that_ignores_term_is_killed_and_counted_before_its_round_ends() {
    let (took, counted, running, plan) = leaves(crate::Leftover::IgnoresTerm).await;
    let of = work::Left::Try {
        run: plan,
        round: 1,
        attempt: 1,
    };
    assert!(
        !running,
        "what the plan's program left, ignoring TERM, runs on after its round ended"
    );
    assert_eq!(counted, [(of, 1)], "counted with the try");
    assert!(
        took >= std::time::Duration::from_secs(3),
        "ended in {took:?}, before the KILL was due"
    );
}

// A process the program leaves in its group that holds the program's standard output open, as `cmd &` in a script
// does, is stopped once the program has ended, and counted, and the round ends then: the reference is the owner's
// answer to `leftovers-are-stopped` ("程序一结束，就停掉组里留下的东西（TERM，3 秒后 KILL），再记这一轮结束"; the option
// of waiting for what is left was rejected) and acceptance condition 9. Fails where the carrier waits for the output to
// end before it looks at the program's end, which here waits for the leftover's twenty seconds. A few seconds alone
// where it holds; the engineer's test sends the leftover's output elsewhere, so no other test holds this.
#[tokio::test]
async fn what_a_trys_program_leaves_running_holding_its_output_is_stopped_once_the_program_ends() {
    let (took, counted, running, plan) = leaves(crate::Leftover::KeepsOutput).await;
    let of = work::Left::Try {
        run: plan,
        round: 1,
        attempt: 1,
    };
    assert!(
        took < std::time::Duration::from_secs(12),
        "the round ended {took:?} after its program was called, waiting for what the program left to end by itself"
    );
    assert!(!running, "what the plan's program left runs on after its round ended");
    assert_eq!(counted, [(of, 1)], "counted with the try");
}
