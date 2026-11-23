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
    assert!(plan.of("Plan") && review.of("Review"));

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
    // Every line's system prompt holds the project's files its declaration names, each headed by its path, the engine's
    // words, and how a cell returns, with each field's words and the owner's language beside its name.
    for call in [plan, review] {
        for (path, text) in crate::PROJECT_FILES {
            let first = text.lines().next().unwrap_or_default();
            assert!(told(call, &format!("# `{path}`")) && told(call, first), "{path}");
        }
        assert!(told(call, "# How you work here"));
        assert!(told(call, "## How you return") && told(call, "`dealt_with`"));
        assert!(told(call, "`owner.language` (Chinese)"));
    }
}

/// The lines of the system prompt the call was given.
fn system(call: &Call) -> &[String] {
    let from = call
        .args
        .iter()
        .position(|a| a == "--append-system-prompt")
        .expect("a system prompt")
        + 1;
    let to = call.args.iter().position(|a| a == "--json-schema").expect("a schema");
    &call.args[from..to]
}

// A session keeps the system prompt it began with, and a change of the project's guide in the worktree after its first
// round, as the work's own commit or main taken in makes, reaches the line's next round as a new session (decisions
// the-projects-files-as-the-work-has-them and `a-changed-system-prompt-or-form-begins-a-new-session`): the plan, whose
// round changed the guide, was given the guide as its session began with it, and the build begins a new session, given
// the guide as the worktree has it.
#[tokio::test]
async fn a_change_of_the_guide_in_the_worktree_reaches_the_lines_next_round_as_a_new_session() {
    let mut real = Real::trial(Fake {
        plan_edits_guide: true,
        ..Fake::default()
    });
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    let (plan, build) = (&calls[0], &calls[1]);
    assert!(plan.of("Plan") && build.of("Build"));
    assert_eq!(build.session().0, "--session-id", "the build begins a new session");
    assert_ne!(build.session().1, plan.session().1);
    let guide = std::fs::read_to_string(build.dir.join(crate::PROJECT_FILES[0].0)).expect("the guide");
    assert!(guide.contains("A line the work added."), "the plan changed the guide");
    let added = |call: &Call| system(call).iter().any(|l| l.contains("A line the work added."));
    assert!(!added(plan), "the plan's session began before the change");
    assert!(added(build), "the build's begins with the guide the worktree has");
    let kept = real.work(work);
    let renewed: Vec<&work::Why> = (kept.sessions.iter())
        .filter(|s| s.line == "engineer")
        .map(|s| &s.why)
        .collect();
    assert!(matches!(renewed.as_slice(), [work::Why::Changed { .. }]), "{renewed:?}");
}
