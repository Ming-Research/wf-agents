//! A line that changes only the project's tests: its system prompt says so with the paths of the tests, a round whose
//! run changed a file that is none of them is sent back naming it, and once the run's change is all tests the work
//! goes on.

use decide::run::Unmet;
use service::Act;
use work::Given;

use crate::{Fake, On, Real, ended};

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
    let beside = Unmet::NotATest {
        path: "built.txt".to_string(),
    };
    assert_eq!(kept.given.get(&(build, 2)), Some(&Given::SentBack(vec![beside])));
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
