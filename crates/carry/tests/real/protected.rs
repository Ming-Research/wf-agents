//! A change of a file the project protects, made by a round of the build: its return is sent back where it says not
//! why, and the engine asks the owner once it does, with the reason above the change; the owner's yes lets the work go
//! on, asking nothing again of the change as it is.

use decide::run::Unmet;
use service::Act;
use work::{Given, Question};

use crate::{Fake, Real, ended};

#[tokio::test]
async fn a_build_that_changes_the_guide_is_sent_back_until_it_says_why_and_the_owner_is_asked_with_the_reason() {
    let fake = Fake {
        build_changes_guide: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.waited_question(work).is_some()).await;
    let kept = real.work(work);
    let build = real.run_of(work, "build");
    // The build's first return left the guide changed and said not why: it was sent back for it, naming the file.
    let unexplained = Unmet::Unexplained {
        path: "AGENTS.md".to_string(),
    };
    assert_eq!(kept.given.get(&(build, 2)), Some(&Given::SentBack(vec![unexplained])));
    let calls = real.calls();
    let builds: Vec<_> = calls.iter().filter(|call| call.of("Build")).collect();
    let sent_back = "- `AGENTS.md` is a file the project protects, which the work's branch changes, and nothing says why: \
                     give in `protected_changes` why the change is needed, or take it back.";
    assert!(builds[1].stdin.contains(sent_back), "{}", builds[1].stdin);
    // The engine asked nothing of the change before the build said why, and then the question, with the reason.
    let (name, _) = real.waited_question(work).expect("the work waits for the owner");
    assert_eq!(name, "protected_files");
    let asked: Vec<_> = (kept.records.questions.iter()).filter(|q| q.name == name).collect();
    assert_eq!(asked.len(), 1, "asked once");
    assert_eq!((asked[0].run, asked[0].engine), (build, true));
    let Some(Question::Asked { ask, reasons, .. }) = kept.question(asked[0]) else {
        panic!("the question is not asked: {:?}", kept.question(asked[0]));
    };
    let said = "`AGENTS.md`, changed. Why, in the work's words: The build needs the line.\n\n```diff\n";
    let (why, line) = (
        ask.find(said).expect(ask),
        ask.find("+A line the build added.").expect(ask),
    );
    assert!(why < line, "the reason stands above the change: {ask}");
    assert_eq!(
        reasons.get("AGENTS.md").map(String::as_str),
        Some("The build needs the line.")
    );
    // The owner lets it merge: the build runs for the answer, the change as it was asks nothing again, and the work
    // ends.
    real.answer(work);
    real.until(|real| ended(&real.decision(work))).await;
    let kept = real.work(work);
    let asked = (kept.records.questions.iter())
        .filter(|q| q.name == "protected_files")
        .count();
    assert_eq!(asked, 1, "nothing is asked again");
}
