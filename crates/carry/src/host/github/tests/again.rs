//! A check or a call of `run_tests` given again after the engine started again, on the GitHub host: it finds the run
//! it asked for by the value the run's name holds, waits for it or reads it where it has ended, and asks for a run
//! again only where it finds none of its own, or only cancelled ones; for the full workflows and the fast one alike,
//! and where it was told to stop, the branch its commit was pushed to is kept until the result is read.

use std::future::Future;

use super::super::fast::Request;
use super::fast::{junit_green, measured, measuring, named, timings};
use super::*;
use crate::host::Named;

/// Waits until GitHub was asked for this many runs, or for as long as a test may.
async fn asked_for(hub: &Hub, runs: usize) {
    for _ in 0..2000 {
        if hub.dispatched().len() >= runs {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// Drives what asks for a run until GitHub was asked for one more, and drops it there, as the engine that stops
/// drops what it waits for: the run goes on on GitHub.
async fn cut_off<T>(hub: &Hub, asking: impl Future<Output = T>) {
    let before = hub.dispatched().len();
    tokio::select! {
        _ = asking => panic!("it waits for the run it asked for"),
        () = asked_for(hub, before + 1) => {}
    }
    assert_eq!(hub.dispatched().len(), before + 1, "a run was asked for");
}

impl Hub {
    /// What the call of `run_tests` finds of the work's tests and checks, these named.
    async fn tests_asked(&self, named: &Named, call: Asking) -> Result<Option<String>, String> {
        let place = self.place();
        let branch = branch(&place);
        self.host.run_tests(&branch, named, call, &mut Stop::never()).await
    }

    /// The verdict the round of the check finds on the commit.
    async fn judged(&self, commit: &str, round: Asking) -> Result<Option<Verdict>, String> {
        let place = self.place();
        let (branch, main, mut stop) = (branch(&place), self.main_on_github(), Stop::never());
        let repository = Mutex::new(());
        let verdict = self.host.verdict(&branch, commit, &main, round, &repository, &mut stop);
        verdict.await.map_err(String::from)
    }

    /// The value of each run GitHub was asked for, in order.
    fn calls(&self) -> Vec<String> {
        (self.dispatched().into_iter())
            .filter_map(|asked| match asked {
                Asked::Dispatch { inputs, .. } => inputs.into_iter().find(|(name, _)| name == "call"),
                _ => None,
            })
            .map(|(_, value)| value)
            .collect()
    }

    /// Finishes the runs under way once the request given again has had time to ask for another, which it must not.
    async fn finished_after_a_while(&self, asked: usize) {
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(self.dispatched().len(), asked, "{:?}", self.dispatched());
        self.github().finish_runs();
    }
}

// The engine stopped while the full workflows ran for the check before the merge: the check, given again, asks for no
// other run, waits for the one it asked for, and that run's is its verdict.
#[tokio::test]
async fn a_check_given_again_while_its_run_goes_on_waits_for_that_run() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.green(&head);
    hub.github().runs_go_on();
    let round = hub.round();
    cut_off(&hub, hub.judged(&head, round)).await;
    let (verdict, ()) = tokio::join!(hub.judged(&head, round), hub.finished_after_a_while(1));
    let verdict = verdict.unwrap().expect("nothing stopped it");
    assert_eq!(verdict.failed, None);
    assert_eq!(
        verdict.by.as_deref(),
        Some("the full workflows of the commit (https://github.example/actions/runs/1000)")
    );
    assert_eq!(hub.dispatched().len(), 1);
}

// The engine stopped while the fast workflow ran for a call of `run_tests`: the call, given again, waits for that run;
// had the run ended meanwhile, it would read it at once.
#[tokio::test]
async fn a_call_given_again_while_its_fast_run_goes_on_waits_for_that_run_and_one_ended_is_read() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github()
        .fast(&head, Vec::new(), Some(("test-results", junit_green())));
    hub.github().runs_go_on();
    let asked = named(&["tools::tests::a"], &[]);
    let call = hub.call();
    cut_off(&hub, hub.tests_asked(&asked, call)).await;
    // The run goes on, and the commit it checks out is still on the branch pushed for the call.
    assert_eq!(hub.trials().len(), 1, "the branch is kept while the run is not read");
    let (said, ()) = tokio::join!(hub.tests_asked(&asked, call), hub.finished_after_a_while(1));
    let said = said.unwrap().expect("nothing stopped it");
    let ran =
        format!("The fast workflow ran on the commit {head} (https://github.example/actions/runs/1000): 1 of the 1");
    assert!(said.starts_with(&ran), "{said}");
    assert!(hub.trials().is_empty(), "the branch is deleted once the result is read");
    // Given again once more, the run has ended: it is read, and no other is asked for.
    let again = hub.tests_asked(&asked, call).await.unwrap().expect("read");
    assert_eq!(again, said);
    assert_eq!(hub.dispatched().len(), 1);
    assert!(hub.trials().is_empty());
}

// The engine stopped while the measure workflow ran for a call of `run_tests`: the call, given again, waits for that run
// and asks for no other, with the same value, and the run ended meanwhile is read once more without another. About a
// tenth of a second alone, which is the wait the test gives the call to ask for a run it must not ask for: the one test
// that a measure run is found again as the fast one is.
#[tokio::test]
async fn a_call_given_again_while_its_measure_run_goes_on_waits_for_that_run_and_one_ended_is_read() {
    let hub = Hub::new().with(measuring);
    let head = hub.committed().await;
    hub.github().of_workflow(
        "measure.yml",
        &head,
        vec![passed("measure")],
        Some(("test-results", timings())),
    );
    hub.github().runs_go_on();
    let asked = measured(&["design-lint"]);
    let call = hub.call();
    cut_off(&hub, hub.tests_asked(&asked, call)).await;
    assert_eq!(hub.trials().len(), 1, "the branch is kept while the run is not read");
    let (said, ()) = tokio::join!(hub.tests_asked(&asked, call), hub.finished_after_a_while(1));
    let said = said.unwrap().expect("nothing stopped it");
    let ran = format!(
        "The measure workflow ran on the commit {head} (https://github.example/actions/runs/1000): 1 of the 1 \
         measurements ran."
    );
    assert!(said.starts_with(&ran), "{said}");
    assert!(said.contains("- design-lint [candidate]: passed in 0.40 s"), "{said}");
    assert!(hub.trials().is_empty(), "the branch is deleted once the result is read");
    let again = hub.tests_asked(&asked, call).await.unwrap().expect("read");
    assert_eq!(again, said);
    assert_eq!(hub.dispatched().len(), 1);
    assert_eq!(hub.calls().len(), 1);
}

// The engine stopped while the full workflows ran for the check before the merge, and the run ended before it started
// again: the check, given again, reads that run's result.
#[tokio::test]
async fn a_check_given_again_after_its_full_run_ended_reads_that_run() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.green(&head);
    hub.github().runs_go_on();
    let round = hub.round();
    cut_off(&hub, hub.judged(&head, round)).await;
    hub.github().finish_runs();
    let verdict = hub.judged(&head, round).await.unwrap().expect("read");
    assert_eq!(verdict.failed, None);
    assert_eq!(
        verdict.by.as_deref(),
        Some("the full workflows of the commit (https://github.example/actions/runs/1000)")
    );
    assert_eq!(hub.dispatched().len(), 1);
}

// A run cancelled while the engine was stopped judged nothing: the request, given again, asks for another, with the
// same value, and waits for that one, never reading the cancelled one.
#[tokio::test]
async fn a_request_given_again_whose_run_was_cancelled_asks_for_another_and_waits_for_it() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github()
        .fast(&head, Vec::new(), Some(("test-results", junit_green())));
    hub.github().runs_go_on();
    let asked = named(&["tools::tests::a"], &[]);
    let call = hub.call();
    cut_off(&hub, hub.tests_asked(&asked, call)).await;
    hub.github().cancel_runs();
    let finishing = async {
        asked_for(&hub, 2).await;
        hub.finished_after_a_while(2).await;
    };
    let (said, ()) = tokio::join!(hub.tests_asked(&asked, call), finishing);
    let said = said.unwrap().expect("nothing stopped it");
    assert!(
        said.starts_with(&format!(
            "The fast workflow ran on the commit {head} (https://github.example/actions/runs/1001): 1 of the 1"
        )),
        "{said}"
    );
    let calls = hub.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0], calls[1], "the request asks again under its own value");
}

// The engine stopped before the request asked for its run: given again, it finds none of its own, though runs of other
// requests on the same commit are there, and asks for one.
#[tokio::test]
async fn a_request_given_again_that_finds_no_run_of_its_own_asks_for_one() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.green(&head);
    hub.judged(&head, hub.round()).await.unwrap().expect("judged");
    hub.tests_asked(&named(&["tools::tests::a"], &[]), hub.call())
        .await
        .unwrap()
        .expect("run");
    let verdict = hub.judged(&head, hub.round()).await.unwrap().expect("judged");
    assert_eq!(
        verdict.by.as_deref(),
        Some("the full workflows of the commit (https://github.example/actions/runs/1002)")
    );
    let calls = hub.calls();
    assert_eq!(calls.len(), 3);
    assert!(
        calls[0] != calls[1] && calls[1] != calls[2] && calls[0] != calls[2],
        "{calls:?}"
    );
}

// The value a run's name holds is worked out whole from the request: the same for the same call or round, workflow and
// commit of the same work, whenever it is worked out, and another for any other, or for another installation.
#[test]
fn the_value_of_a_run_is_its_requests_own_and_the_same_whenever_it_is_worked_out() {
    let hub = Hub::new();
    let request = |work: u64, asking: Asking, commit: &'static str| Request {
        work: WorkNumber(work),
        commit,
        asking,
    };
    let (call, round) = (Asking::Call(CallId(3)), Asking::Round(RunId(3), 1));
    let value = hub.host.call(request(1, call, "c0ffee"), "ci.yml");
    assert_eq!(value, hub.host.call(request(1, call, "c0ffee"), "ci.yml"));
    assert!(
        value.len() == 32 && value.chars().all(|c| c.is_ascii_hexdigit()),
        "{value}"
    );
    for other in [
        hub.host.call(request(2, call, "c0ffee"), "ci.yml"),
        hub.host.call(request(1, Asking::Call(CallId(4)), "c0ffee"), "ci.yml"),
        hub.host.call(request(1, round, "c0ffee"), "ci.yml"),
        hub.host
            .call(request(1, Asking::Round(RunId(3), 2), "c0ffee"), "ci.yml"),
        hub.host.call(request(1, call, "beef"), "ci.yml"),
        hub.host.call(request(1, call, "c0ffee"), "fast.yml"),
    ] {
        assert_ne!(value, other);
    }
    // A request of tests and measurements asks for two runs, each with a value of its own.
    assert_ne!(
        hub.host.call(request(1, call, "c0ffee"), "fast.yml"),
        hub.host.call(request(1, call, "c0ffee"), "measure.yml")
    );
    let elsewhere = Hub::new().with(|config| config.installation = Uuid::from_u128(7));
    assert_ne!(value, elsewhere.host.call(request(1, call, "c0ffee"), "ci.yml"));
}
