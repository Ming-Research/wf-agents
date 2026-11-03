//! How a workflow run by hand that fails is told apart, over the real gh command.

use engine::hub::{Gh, GitHub};

// What gh says of a failed command begins with the command line, which holds the inputs the engineer gave. A dispatch
// that fails for a reason other than GitHub refusing the request (here the repository does not exist, or GitHub cannot
// be reached, or gh has no credentials) is not a refusal because an input of the engineer's reads like one: only what
// GitHub said counts. The reference is the contract of the failure kind `rejected`: GitHub reached, and the request
// itself refused.
#[test]
fn an_input_that_reads_like_a_refusal_does_not_make_a_failed_dispatch_a_refused_one() {
    let logs = std::env::temp_dir().join(format!("wf-agents-gh-dispatch-{}", std::process::id()));
    let gh = Gh::new(logs);
    for command in ["grep 'HTTP 422' build.log", "echo HTTP 422"] {
        let failure = gh
            .dispatch(
                "wf-agents-no-such-owner/no-such-repository",
                "measure.yml",
                "wf-agents-measure/1-1-0",
                &[("command", command)],
            )
            .expect_err("a repository that does not exist cannot be dispatched to");
        assert_ne!(failure.kind, "rejected", "{command}: {}", failure.what);
    }
}
