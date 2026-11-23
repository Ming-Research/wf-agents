//! What each line's agents may run reaches the command line of every round, and its system prompt says it.

use std::collections::BTreeMap;

use service::Act;

use crate::{Call, DECLARED_VARIABLE, Fake, Real, ended};

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

    // Every round is given what the flows' words say agents may do, turned into the program's own flags, with the
    // engine's own temporary directory written without a trailing slash, and the arguments that isolate the program
    // from the owner's tools and settings: the settings hold no argument. Each is what the installation's arguments gave
    // before they were the engine's. Every round is refused the agents' program the installation names, which here is
    // the fake program's path and no other name. Reference: the installation's former arguments.
    let temp = std::env::temp_dir();
    let temp = temp.to_string_lossy();
    let temp = temp.trim_end_matches('/');
    let program = format!("Bash({} *)", real.settings.agents.command);
    for call in &calls {
        assert_eq!(
            after(call, "--tools"),
            ["Bash,Read,Grep,Glob,Edit,Write,WebSearch,WebFetch,Task"],
            "{:?}",
            call.args
        );
        assert_eq!(
            after(call, "--allowedTools"),
            [
                "Bash".to_string(),
                "Grep".to_string(),
                "Glob".to_string(),
                "Edit(./**)".to_string(),
                format!("Edit(/{temp}/**)"),
                "WebSearch".to_string(),
                "WebFetch".to_string(),
            ]
        );
        assert_eq!(after(call, "--add-dir"), [temp]);
        assert_eq!(after(call, "--setting-sources"), ["user"]);
        assert_eq!(call.args.iter().filter(|a| *a == "--setting-sources").count(), 1);
        for flag in ["--strict-mcp-config", "--disable-slash-commands"] {
            assert!(call.args.iter().any(|a| a == flag), "{flag}");
        }
        let refused = after(call, "--disallowedTools");
        assert!(refused.contains(&program.as_str()), "{program}: {refused:?}");
        assert!(
            !refused.contains(&"Bash(claude *)"),
            "no program is assumed: {refused:?}"
        );
    }

    // The engineer's line writes: it is refused the heavy commands and pushing.
    let refused = after(plan, "--disallowedTools");
    for rule in ["Bash(rustc *)", "Bash(npx *)", "Bash(vitest *)", "Bash(git push *)"] {
        assert!(refused.contains(&rule), "{rule}: {refused:?}");
    }
    assert!(
        !refused.contains(&"Edit") && !refused.contains(&"Bash(git commit *)"),
        "{refused:?}"
    );
    assert!(told(plan, "Make your changes there and commit them there."));
    assert!(
        !told(plan, "To update a lock file"),
        "no lock command is given any line"
    );
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
    // What the words let agents do is the same for every line; a line that only reads is refused more.
    assert_eq!(after(review, "--allowedTools"), after(plan, "--allowedTools"));
    assert_eq!(after(review, "--tools"), after(plan, "--tools"));
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

// Every round's command line is given the variables the project's declaration names (`[environment]` of the declaration
// on its main), with the values it gives them: the test repository's declaration gives RUSTUP_AUTO_INSTALL a value of
// its own, which no value the engine writes could be. A few rounds of the stand-in, with git: about a second; the one
// test that the declared variables, and not the engine's, reach the command line a round runs.
#[tokio::test]
async fn every_round_is_given_the_variables_the_projects_declaration_names_with_the_values_it_gives() {
    let mut real = Real::trial(Fake::default());
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    assert!(!calls.is_empty());
    for n in 1..=calls.len() {
        let given =
            std::fs::read_to_string(real.calls_dir().join(format!("{n}.rustup"))).expect("the stand-in kept it");
        assert_eq!(given, DECLARED_VARIABLE, "call {n}");
    }
}

// A declared value that holds a NUL is refused as the engine starts: a NUL ends a string of a process's environment, and
// a program is not started with one (here shown with the operating system's own: `Command` fails to start `true` given
// it), so every round of the project would fail to start the agents' program, with the declaration read and accepted. A
// value with any other character is none. Milliseconds alone, one process started; the one test of what a declared
// value may hold.
#[test]
fn a_declared_value_that_holds_a_nul_is_refused_as_one_no_program_can_be_started_with() {
    let started = std::process::Command::new("true").env("SOME_VARIABLE", "a\0b").status();
    assert!(
        started.is_err(),
        "a program is started with a NUL in a variable: {started:?}"
    );
    let faults = |value: &str| {
        let declared = BTreeMap::from([("SOME_VARIABLE".to_string(), value.to_string())]);
        carry::cli::environment_faults(&declared, &[])
    };
    assert_eq!(faults("a b\nc"), Vec::<String>::new());
    assert_eq!(faults(""), Vec::<String>::new());
    let said = faults("a\0b");
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("SOME_VARIABLE"), "{said:?}");
}
