//! The form of return the agents' program is given: the one the flow and what is written beside it make for the line
//! of the cell entered (`flow::form`), made as the session begins and kept with it, as the system prompt is, so that
//! every call of a session, which is a line's, is given the same bytes.

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
        engineer.contains("\"q_and_a\""),
        "the build may ask the owner: {engineer}"
    );
    assert!(
        !reviewer.contains("\"q_and_a\""),
        "the review may not ask the owner: {reviewer}"
    );
    assert_ne!(engineer, reviewer);
    assert_eq!(real.form_given(1), engineer, "the plan's call");
    assert_eq!(real.form_given(2), engineer, "the build's call");
    assert_eq!(real.form_given(3), reviewer, "the review's call");
}

// A session is given on every call the form and the system prompt it began with, though the flow changed in the
// meantime: the engine started again with a flow whose build may ask for no tool, which would change both, gives the
// build's later calls, in the session the plan began, what the plan's call was given.
#[tokio::test]
async fn every_call_of_a_session_is_given_the_form_and_the_system_prompt_it_began_with_after_the_flow_changed() {
    let mut real = Real::trial(Fake {
        question_first: true,
        ..Fake::default()
    });
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.waited_question(work).is_some()).await;
    let before = real.form("engineer");
    let build = (real.flow.cells.iter_mut())
        .find(|c| c.id.as_str() == "build")
        .expect("the build");
    build.tools.clear();
    real.restart();
    let after = real.form("engineer");
    assert_ne!(after, before, "the change reaches a form made anew");
    real.answer(work, "By their first name.");
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    let session: Vec<(usize, &Call)> = (calls.iter().enumerate())
        .filter(|(_, call)| call.of("Plan") || call.of("Build"))
        .map(|(i, call)| (i + 1, call))
        .collect();
    assert!(session.len() > 2, "the build is entered again after the answer");
    for &(n, call) in &session[1..] {
        assert!(
            call.args.iter().any(|a| a == "--resume"),
            "call {n} goes on in the session"
        );
        assert_eq!(real.form_given(n), before, "call {n}");
        assert_eq!(system(call), system(session[0].1), "call {n}");
    }
    // The words of a tool begin with its name, as the list of the tools gives them.
    let told = "`query`: ";
    assert!(crate::beside().tools["query"].words.trim_start().starts_with(told));
    assert!(
        (session.iter()).all(|(_, call)| system(call).iter().any(|l| l.contains(told))),
        "every call of the session is told of the tool the flow named as the session began"
    );
}
