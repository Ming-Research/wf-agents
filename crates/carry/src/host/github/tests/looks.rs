//! What the waits for runs of a workflow cost GitHub's limit on requests: a look whose answer GitHub would give again
//! is answered as unchanged, which costs nothing, and the waits on one workflow share one look at it each poll, however
//! many wait.

use super::super::fast::{Request, Role};
use super::fast::{junit_green, named};
use super::*;

/// Waits until the runs were looked at this many times, or for as long as a test may.
async fn looked_at(hub: &Hub, looks: usize) {
    for _ in 0..5000 {
        if hub.github().looked().len() >= looks {
            return;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

/// How many of the looks at the runs GitHub counted against its limit.
fn counted(hub: &Hub) -> usize {
    hub.github().looked().iter().filter(|look| look.counted).count()
}

// A call of `run_tests` waits for its run while nothing of the runs changes: of its twenty and more looks, GitHub
// counts only the three that found the runs changed (the first, the one that finds the run asked for under way, and
// the one that finds it completed); every other is answered as unchanged, and costs nothing, however long the wait.
#[tokio::test]
async fn a_wait_that_sees_nothing_change_costs_nothing_against_githubs_limit() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github()
        .fast(&head, Vec::new(), Some(("test-results", junit_green())));
    hub.github().runs_go_on();
    let place = hub.place();
    let (branch, call, mut stop) = (branch(&place), hub.call(), Stop::never());
    let asked = named(&["tools::tests::a"], &[]);
    let finishing = async {
        looked_at(&hub, 20).await;
        hub.github().finish_runs();
    };
    let (said, ()) = tokio::join!(hub.host.run_tests(&branch, &asked, call, &mut stop), finishing);
    let said = said.unwrap().expect("nothing stopped it");
    let ran =
        format!("The fast workflow ran on the commit {head} (https://github.example/actions/runs/1000): 1 of the 1");
    assert!(said.starts_with(&ran), "{said}");
    let looks = hub.github().looked().len();
    assert!(looks > 20, "it waited: {looks} looks");
    assert_eq!(counted(&hub), 3, "of {looks} looks");
}

// Eight requests wait at once for runs of the fast workflow they asked for. Once each has asked for its run, they look
// at the runs together: forty looks take at least thirty polls, not the five they would take with each wait looking
// for itself. GitHub counts no more of all their looks than the first, one for each run asked for, which changes the
// runs, and one for the runs completed; each wait reads its own run.
#[tokio::test]
async fn the_waits_on_a_workflow_share_one_look_each_poll_however_many_wait() {
    const WAITS: usize = 8;
    let hub = Hub::new();
    let commit = hub.main_on_github();
    hub.github()
        .fast(&commit, Vec::new(), Some(("test-results", junit_green())));
    hub.github().runs_go_on();
    let ci = hub.host.config.ci.clone().expect("a declaration");
    let asked = named(&["tools::tests::a"], &[]);
    let (host, fast, asked) = (&hub.host, &ci.fast, &asked);
    let wait = |asking: Asking| {
        let commit = commit.as_str();
        async move {
            let request = Request {
                work: WORK,
                commit,
                asking,
            };
            let mut stop = Stop::never();
            host.run_role(Role::Fast(fast), request, asked, &mut stop).await
        }
    };
    let poll = hub.host.config.fast_poll;
    let watching = async {
        for _ in 0..5000 {
            if hub.dispatched().len() >= WAITS {
                break;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        assert_eq!(hub.dispatched().len(), WAITS, "each asked for its run");
        // Each wait has looked for its run before it asked for it: what looks from now on only waits.
        let (from, began) = (hub.github().looked().len(), std::time::Instant::now());
        looked_at(&hub, from + 40).await;
        let took = began.elapsed();
        hub.github().finish_runs();
        took
    };
    let (said, took) = tokio::join!(
        async {
            let (a, b, c, d, e, f, g, h) = tokio::join!(
                wait(hub.call()),
                wait(hub.call()),
                wait(hub.call()),
                wait(hub.call()),
                wait(hub.call()),
                wait(hub.call()),
                wait(hub.call()),
                wait(hub.call())
            );
            [a, b, c, d, e, f, g, h]
        },
        watching
    );
    assert!(
        took >= poll * 30,
        "forty looks took {took:?}, at a poll of {poll:?}: the waits each looked for themselves"
    );
    let mut runs: Vec<String> = Vec::new();
    for said in said {
        let said = said.unwrap().expect("nothing stopped it");
        assert!(said.contains("1 of the 1 tests passed"), "{said}");
        let url = (said.split_once("(https://")).and_then(|(_, rest)| rest.split_once(')'));
        runs.push(url.expect("the run's link").0.to_string());
    }
    runs.sort();
    runs.dedup();
    assert_eq!(runs.len(), WAITS, "each wait read its own run: {runs:?}");
    assert!(
        counted(&hub) <= 1 + WAITS + 1,
        "{} of {} looks were counted",
        counted(&hub),
        hub.github().looked().len()
    );
}
