//! What each line's agents may run reaches the command line of every round, and its system prompt says it.

use service::Act;

use crate::{Call, Fake, Real, ended};

/// The values the call was given after each time it was given the flag, up to the next flag.
fn after<'a>(call: &'a Call, flag: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut taking = false;
    for arg in &call.args {
        if arg.starts_with("--") {
            taking = arg == flag;
        } else if taking {
            out.push(arg.as_str());
        }
    }
    out
}

/// Whether the call's system prompt holds the text, which is on one line of it.
fn told(call: &Call, text: &str) -> bool {
    call.args.iter().any(|arg| arg.contains(text))
}

#[tokio::test]
async fn a_line_that_writes_and_one_that_only_reads_are_each_given_what_they_may_run() {
    let mut real = Real::trial(Fake::default());
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    let (plan, review) = (&calls[0], &calls[2]);
    assert!(plan.stdin.contains("cell `plan`") && review.stdin.contains("cell `review`"));

    // The engineer's line writes: it may update a lock file, and is refused the heavy commands and pushing.
    let refused = after(plan, "--disallowedTools");
    for rule in ["Bash(rustc *)", "Bash(npx *)", "Bash(vitest *)", "Bash(git push *)"] {
        assert!(refused.contains(&rule), "{rule}: {refused:?}");
    }
    assert!(
        !refused.contains(&"Edit") && !refused.contains(&"Bash(git commit *)"),
        "{refused:?}"
    );
    assert!(after(plan, "--allowedTools").contains(&"Bash(cargo update *)"));
    assert!(told(plan, "Make your changes there and commit them there."));
    assert!(told(plan, "`cargo update`"));
    assert!(!told(plan, "{your_line}"), "the line's words are put in");

    // The reviewer's line only reads: it is refused what edits a file or changes the repository as well.
    let refused = after(review, "--disallowedTools");
    for rule in [
        "Edit",
        "Write",
        "Bash(git commit *)",
        "Bash(git checkout *)",
        "Bash(cargo *)",
        "Bash(git push *)",
    ] {
        assert!(refused.contains(&rule), "{rule}: {refused:?}");
    }
    assert!(after(review, "--allowedTools").is_empty());
    assert!(told(review, "You only read it"));
    assert!(!told(review, "Make your changes there"));
    // Every line is told how it builds and tests.
    assert!(told(plan, "To build and test, ask for `check`.") && told(review, "To build and test, ask for `check`."));
}
