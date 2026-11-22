//! The form of return the agents' program is given: the one the flow and what is written beside it make for the line
//! of the cell entered (`flow::form`), made as the session begins and kept with it, as the system prompt is, so that
//! every call of a session, which is a line's, is given the same bytes; a line whose session began with another form
//! than a new session would begin with now begins a new session at its next round.

use service::Act;

use crate::{Call, Fake, Real, ended};

/// The system prompt the call was given.
fn system(call: &Call) -> Vec<String> {
    let at = |arg: &str| call.args.iter().position(|a| a == arg);
    let from = at("--append-system-prompt").expect("a system prompt") + 1;
    let to = at("--json-schema").expect("a schema");
    call.args[from..to].to_vec()
}

// Each call is given the form of its cell's line, and every call of one line the same: the plan and the build share a
// line, whose form holds the build's tool and its questions; the review, of a line of its own, is given another.
#[tokio::test]
async fn every_call_of_a_line_is_given_the_form_of_that_line_as_the_files_beside_it_make_it() {
    let mut real = Real::trial(Fake::default());
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    assert_eq!(calls.len(), 3, "the plan, the build and the review");
    let (engineer, reviewer) = (real.form("engineer"), real.form("reviewer"));
    assert!(
        engineer.contains("\"query\""),
        "the build may ask for `query`: {engineer}"
    );
    assert!(
        engineer.contains("\"build_questions\""),
        "the build may ask the owner: {engineer}"
    );
    assert!(
        !reviewer.contains("_questions\""),
        "the review may not ask the owner: {reviewer}"
    );
    assert_ne!(engineer, reviewer);
    assert_eq!(real.form_given(1), engineer, "the plan's call");
    assert_eq!(real.form_given(2), engineer, "the build's call");
    assert_eq!(real.form_given(3), reviewer, "the review's call");
}

// A session is given on every call the form and the system prompt it began with, and a line whose session began with
// another form or system prompt than a new session would begin with now begins a new session at its next round
// (decision `a-changed-system-prompt-or-form-begins-a-new-session`): the engine started again with a flow whose plan
// may ask for one more tool, which changes both, and the build, entered again for the owner's answer, begins a new
// session under another id, given the new form and system prompt, and goes on in it with the same bytes. Seconds, with
// the fake program, like the tests beside it; the one test that sees the program's command line after a renewal.
#[tokio::test]
async fn a_line_whose_session_began_on_a_flow_since_changed_begins_a_new_one_with_the_new_form_and_prompt() {
    let mut real = Real::trial(Fake {
        question_first: true,
        ..Fake::default()
    });
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.waited_question(work).is_some()).await;
    let before = real.form("engineer");
    let plan = (real.flow.cells.iter_mut())
        .find(|c| c.id.as_str() == "plan")
        .expect("the plan");
    plan.tools.push("run_tests".to_string());
    real.restart();
    let after = real.form("engineer");
    assert_ne!(after, before, "the change reaches a form made anew");
    real.answer(work);
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    let line: Vec<(usize, &Call)> = (calls.iter().enumerate())
        .filter(|(_, call)| call.of("Plan") || call.of("Build"))
        .map(|(i, call)| (i + 1, call))
        .collect();
    // The plan's call and the build's that asked the owner, before the engine started again, are in the session the
    // plan began; every later call of the line is in the new one.
    let first = line[0].1.session().1;
    let (old, new): (Vec<(usize, &Call)>, Vec<(usize, &Call)>) =
        line.iter().copied().partition(|(_, call)| call.session().1 == first);
    assert_eq!(old.len(), 2, "the plan, and the build that asked the owner");
    assert!(
        new.len() > 1,
        "the build is entered again for the answer, and for its query's result"
    );
    assert_eq!(new[0].1.session().0, "--session-id", "the build begins the new session");
    for &(n, call) in &new[1..] {
        assert_eq!(
            call.session(),
            ("--resume", new[0].1.session().1),
            "call {n} goes on in it"
        );
    }
    for (session, form) in [(&old, &before), (&new, &after)] {
        for &(n, call) in session {
            assert_eq!(&real.form_given(n), form, "call {n}");
            assert_eq!(system(call), system(session[0].1), "call {n}");
        }
    }
    // The words of a tool begin with its name, as the list of the tools gives them.
    let told = "`run_tests`: ";
    assert!(crate::beside().tools["run_tests"].words.trim_start().starts_with(told));
    let tells = |call: &Call| system(call).iter().any(|l| l.contains(told));
    assert!(!tells(old[0].1), "the session the plan began is not told of the tool");
    assert!(tells(new[0].1), "the new session is told of it");
    let kept = real.work(work);
    let renewed: Vec<&work::Why> = (kept.sessions.iter())
        .filter(|s| s.line == "engineer")
        .map(|s| &s.why)
        .collect();
    assert!(matches!(renewed.as_slice(), [work::Why::Changed { .. }]), "{renewed:?}");
}
