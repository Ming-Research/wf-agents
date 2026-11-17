//! The owner's words to a cell (design document `data-and-context`, 35): a cell that may ask has a datum of its own of
//! the owner's words, a dialogue with the owner, which it watches. The owner speaks to a cell, or to a line, whose words
//! reach the cell that line has reached; the words are the datum's next version, which the engine's source of the
//! owner's words writes and which runs the cell; the cell answers by writing the datum in its return.

use engine::spoken::{NoOne, To};

use super::questions::{asked, researching};
use super::*;

impl W {
    /// The owner says these words.
    fn says(&mut self, to: To, words: &str) -> Vec<Effect> {
        self.on(Event::Said {
            to,
            words: words.to_string(),
        })
    }
}

/// The versions of the block, each with who wrote it, whether it changes the meaning, and its content.
fn versions(w: &W, block: &str) -> Vec<(Outsider, bool, String)> {
    (w.work.records.written.iter())
        .filter(|v| v.block.as_str() == block)
        .map(|v| (v.by, v.matters, w.work.contents[&(BlockId::from(block), v.at)].clone()))
        .collect()
}

#[test]
fn words_said_to_a_cell_are_the_next_version_of_its_words_which_runs_it_and_stops_a_later_round() {
    let (mut w, run) = researching();
    let effects = w.returns(run, Came::hands_in(vec![handed("plan", None)]));
    let (implementing, cell, ..) = round(&effects);
    assert_eq!(cell, "implement");
    let effects = w.says(To::Cell("research".into()), "Keep it short.");
    assert_eq!(effects, [Effect::Stop { run: implementing }]);
    assert_eq!(
        versions(&w, "research_words"),
        [(Outsider::Outside, true, "The owner: Keep it short.".to_string())]
    );
    assert!(
        w.work.records.written.iter().all(|v| v.said_to.is_none()),
        "said to no cell but by the datum"
    );
    let effects = w.returns(implementing, Came::hands_in(Vec::new()));
    let (run, cell, ..) = round(&effects);
    assert_eq!(cell, "research");
    let (because, _) = start_of(&w, run);
    assert_eq!(because, vec![changed("research_words", None, false)]);
    // The research answers in its words, its answer in the words of their file.
    w.returns(
        run,
        Came::works(
            vec!["research_words".into()],
            vec![Handed {
                block: "research_words".into(),
                content: "Shortened.".to_string(),
                matters: true,
                value: None,
            }],
        ),
    );
    let finish = w.last_finished();
    assert_eq!(
        w.work.contents[&(BlockId::from("research_words"), finish)],
        "Shortened."
    );
}

#[test]
fn words_said_to_a_line_reach_the_cell_it_has_reached() {
    let (mut w, run) = researching();
    w.returns(run, Came::hands_in(vec![handed("plan", None)]));
    assert_eq!(
        engine::spoken::receiver(&w.flow, &w.work, &To::Line("main".into())),
        Ok("implement".into()),
        "the line stands at the implementation"
    );
    w.says(To::Line("main".into()), "Use one table.");
    assert_eq!(versions(&w, "implement_words").len(), 1);
    assert_eq!(versions(&w, "research_words"), []);
}

#[test]
fn words_said_to_a_round_under_way_are_told_to_it_as_a_version_of_its_cell_s_words() {
    let (mut w, run) = researching();
    let effects = w.says(To::Cell("research".into()), "Mind the cost.");
    let [Effect::Tell { run: told, telling }] = effects.as_slice() else {
        panic!("the round is not told: {effects:?}");
    };
    assert_eq!(*told, run);
    let at = w.last_written();
    assert_eq!((telling.block.as_str(), telling.number), ("research_words", at));
    assert_eq!(telling.entry, "The owner: Mind the cost.");
    assert_eq!(telling.versions, [(BlockId::from("research_words"), at)]);
}

#[test]
fn words_go_to_no_cell_that_has_none_of_its_own_nor_to_one_the_flow_does_not_have() {
    let (mut w, _) = researching();
    assert_eq!(
        w.refused(Event::Said {
            to: To::Cell("merge".into()),
            words: "Wait.".to_string()
        }),
        Refused::NoOneSpokenTo(NoOne::NotSpokenTo("merge".into()))
    );
    assert_eq!(
        w.refused(Event::Said {
            to: To::Cell("nowhere".into()),
            words: "Wait.".to_string()
        }),
        Refused::NoOneSpokenTo(NoOne::NoSuchCell("nowhere".into()))
    );
}

#[test]
fn words_to_a_cell_that_waits_for_its_question_enter_it_and_it_goes_on_waiting() {
    let (mut w, run) = researching();
    let mut came = Came::hands_in(vec![handed("plan", None)]);
    came.said.questions = vec![asked("which", true)];
    assert_eq!(w.returns(run, came), []);
    let effects = w.says(To::Cell("research".into()), "Take your time.");
    let (run, cell, ..) = round(&effects);
    assert_eq!(cell, "research");
    let (because, _) = start_of(&w, run);
    assert_eq!(because, vec![changed("research_words", None, false)]);
}
