//! A line that changes only the project's tests: its system prompt says so with the paths of the tests, a round whose
//! run changed a file that is none of them is sent back naming it, whether its return does the cell's work or asks for
//! calls, and once the run's change is all tests the work goes on.

use decide::RunId;
use decide::run::Unmet;
use service::Act;
use work::Given;

use crate::{Fake, On, Real, ended};

/// The file the fake's build commits beside the project's tests, as the send-back names it.
fn beside() -> Unmet {
    Unmet::NotATest {
        path: "built.txt".to_string(),
    }
}

#[tokio::test]
async fn a_build_on_a_line_of_tests_that_commits_beside_them_is_sent_back_until_its_run_changes_only_tests() {
    let fake = Fake {
        build_beside_tests: true,
        ..Fake::default()
    };
    let mut real = Real::open(On::Testing, fake, None, "true");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let kept = real.work(work);
    let build = real.run_of(work, "build");
    // The build's first return left its file beside the tests: it was sent back for it, naming the file.
    assert_eq!(kept.given.get(&(build, 2)), Some(&Given::SentBack(vec![beside()])));
    assert_eq!(
        kept.given.get(&(build, 3)),
        None,
        "the run's second round changed only tests"
    );
    let calls = real.calls();
    let builds: Vec<_> = calls.iter().filter(|call| call.of("Build")).collect();
    assert_eq!(builds.len(), 2);
    let sent_back = "- `built.txt` is none of the project's tests, the only files your line changes: take back what this \
                     run changed in it.";
    assert!(builds[1].stdin.contains(sent_back), "{}", builds[1].stdin);
    // The line's agents are told the paths of the tests in their system prompt, and no lock command.
    let told = |text: &str| builds[0].args.iter().any(|arg| arg.contains(text));
    assert!(told(
        "Change only the project's tests, the files `:(glob)crates/**/tests/**`, "
    ));
    assert!(!told("Update a lock file with"));
}

// A return that asks for calls ends its run once taken, and the run entered for the results begins on the head it left:
// it is held to the tests as well, so that what it committed beside them is sent back before the call is made.
#[tokio::test]
async fn a_build_that_commits_beside_the_tests_and_then_asks_for_a_call_is_sent_back_before_the_call() {
    let fake = Fake {
        build_beside_tests: true,
        build_then_asks: true,
        ..Fake::default()
    };
    let mut real = Real::open(On::Testing, fake, None, "true");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let kept = real.work(work);
    let runs: Vec<RunId> = (kept.records.started.iter())
        .filter(|started| started.cell.as_str() == "build")
        .map(|started| started.run)
        .collect();
    let [asked, results] = runs[..] else {
        panic!("the build ran twice: {runs:?}");
    };
    // The run that asked was sent back for the file, and its next round, which took it back, had its call made.
    assert_eq!(kept.given.get(&(asked, 2)), Some(&Given::SentBack(vec![beside()])));
    assert_eq!(kept.given.get(&(asked, 3)), None, "the round changed only tests");
    assert!(
        matches!(kept.given.get(&(results, 1)), Some(Given::Results(_))),
        "{:?}",
        kept.given.get(&(results, 1))
    );
    assert_eq!(
        kept.given.get(&(results, 2)),
        None,
        "the run entered for the results changed nothing"
    );
}
