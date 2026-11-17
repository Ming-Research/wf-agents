//! What a cell of the repository's engineer flow is sent, its work taken through the engine's own loop: a cell's first
//! entry in the session of its line names each datum the session holds already, with its version; and the owner's
//! words reach the cell the engine chooses, as a version of that cell's own words.

mod support;

use std::collections::BTreeSet;
use std::path::Path;

use context::Stands;
use decide::{BlockId, CellId, Decision, Outsider, Pauser, Seq, Waited};
use engine::spoken::{self, NoOne, To};
use engine::{Came, Effect, Event, Handed};
use flow::{Compiled, Has};
use support::Sim;
use work::{Question, Said, Way};

/// The directory that holds the repository's flows.
fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow")
}

/// The engineer's flow, compiled against what the engine has for it, as `wf-agents` compiles it.
fn engineer() -> Compiled {
    let beside = flow::agents::read(&root()).expect("the files beside the flows are read");
    let source = flow::read(&root().join("engineer")).expect("the engineer's flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<BTreeSet<_>>();
    let has = Has {
        sources: names(&[
            "answer",
            "asked",
            "dependencies",
            "main",
            "session",
            "start",
            "head",
            "tree",
            "words",
        ]),
        acts: names(&["start", "check", "merge", "deliver"]),
        tools: names(&["query", "run_tests"]),
        system: flow::system::reserved(&beside).into_iter().collect(),
    };
    flow::compile(&source, &has).expect("the engineer's flow compiles")
}

#[test]
fn the_implementation_entered_first_after_the_plan_names_each_datum_its_session_holds_with_its_version() {
    let beside = flow::agents::read(&root()).expect("the files beside the flows are read");
    let mut sim = Sim::of(engineer(), beside);
    sim.wrote("dependencies", "", Some("merged"), Outsider::Outside);
    sim.writes("background", "Greet the user by name.");
    // The start, the reading, the plan with no deep research and no edit of the tree, the plan thought through again,
    // and both reviews of the plan finding nothing to change, each as the engine hands it out, until the implementation
    // is entered.
    let round = loop {
        let round = sim.round().expect("a round is given");
        let id = sim.cell(&round).id.as_str().to_string();
        let blocks: &[(&str, &str, Option<&str>)] = match id.as_str() {
            "S1" => &[("place", "the branch and its worktree", None)],
            "R0" => &[],
            "R1" => &[
                ("deep_needed", "", Some("no")),
                ("plan_owner", "Greet by name.", None),
                ("plan_agent", "Add the name to the greeting.", None),
                ("acceptance", "The greeting names the user.", None),
            ],
            "R2" => &[],
            "P1" => &[("plan_ok", "", Some("yes"))],
            "P2" => &[("tree_ok", "", Some("yes"))],
            "I1" => break round,
            other => panic!("{other} is entered before the implementation"),
        };
        if sim.cell(&round).line.is_some() {
            let entered = sim.enter(&round, sim.begins(&round));
            sim.tries(&round, 1, entered.fills);
        }
        sim.hands_in(&round, blocks);
    };
    let entered = sim.enter(&round, sim.begins(&round));
    // Each datum the implementation is given that has a version is named with that version: the session holds them
    // from the cells of its line before it, which the implementation never worked from.
    let cell = sim.cell(&round);
    for given in &cell.given {
        let block = given.block.as_str();
        let Some(&latest) = sim.versions(block).last() else {
            continue;
        };
        let named = format!("## `{block}`, version {}", latest.0);
        assert!(entered.text.contains(&named), "{named} is not in:\n{}", entered.text);
    }
    for block in ["background", "plan_owner", "plan_agent", "acceptance"] {
        assert_eq!(entered.situation.data[block], Stands::HoldsNew, "{block}");
    }
}

// The project's guide is in the system prompt of each session, and is no datum a cell is given (design document
// `data-and-context`, 24): the first message of the first agent's cell of a work names it nowhere, neither among what
// it is given nor among what ran it.
#[test]
fn the_first_message_of_an_agent_s_cell_names_no_guide() {
    let beside = flow::agents::read(&root()).expect("the files beside the flows are read");
    let mut sim = Sim::of(engineer(), beside);
    sim.wrote("dependencies", "", Some("merged"), Outsider::Outside);
    sim.writes("background", "Greet the user by name.");
    let start = sim.round().expect("the start is given");
    sim.hands_in(&start, &[("place", "the branch and its worktree", None)]);
    let round = sim.round().expect("the reading is given");
    assert_eq!(sim.cell(&round).id.as_str(), "R0");
    let entered = sim.enter(&round, sim.begins(&round));
    assert!(entered.text.contains("`background`"), "{}", entered.text);
    assert!(!entered.text.contains("guide"), "{}", entered.text);
    assert!(
        !entered.situation.data.contains_key("guide"),
        "{:?}",
        entered.situation.data
    );
}

/// A work of the engineer's flow, carried out with the plan written by the research, nothing better found and the
/// plan's reviews content, until the round of this cell is given, which is not carried out: gives the work and that
/// round.
fn until(id: &str) -> (Sim, support::Given) {
    carried_until(id, false)
}

/// A work of the engineer's flow, carried out with the plan written by the deep research, nothing better found and the
/// plan's reviews content, until the round of this cell is given, which is not carried out: gives the work and that
/// round.
fn deeply_until(id: &str) -> (Sim, support::Given) {
    carried_until(id, true)
}

/// A work of the engineer's flow, carried out until the round of this cell is given, which is not carried out, the plan
/// written by the deep research where `deep` holds and by the research otherwise: gives the work and that round.
fn carried_until(id: &str, deep: bool) -> (Sim, support::Given) {
    let beside = flow::agents::read(&root()).expect("the files beside the flows are read");
    let mut sim = Sim::of(engineer(), beside);
    sim.wrote("dependencies", "", Some("merged"), Outsider::Outside);
    sim.writes("background", "Greet the user by name.");
    let plan = [
        ("plan_owner", "Greet by name.", None),
        ("plan_agent", "Add the name to the greeting.", None),
        ("acceptance", "The greeting names the user.", None),
    ];
    loop {
        let round = sim.round().expect("a round is given");
        let cell = sim.cell(&round).id.as_str().to_string();
        if cell == id {
            return (sim, round);
        }
        let blocks: Vec<(&str, &str, Option<&str>)> = match cell.as_str() {
            "S1" => vec![("place", "the branch and its worktree", None)],
            "R0" => vec![],
            "R1" if deep => vec![
                ("deep_needed", "", Some("yes")),
                ("found", "Where the name comes from is open.", None),
            ],
            "R1" => [("deep_needed", "", Some("no"))].into_iter().chain(plan).collect(),
            "D1" => plan.to_vec(),
            "R2" | "D2" => vec![],
            "P1" => vec![("plan_ok", "", Some("yes"))],
            "P2" => vec![("tree_ok", "", Some("yes"))],
            "I1" => vec![("code", "abc", None)],
            other => panic!("{other} is entered before {id}"),
        };
        tried(&mut sim, &round);
        sim.hands_in(&round, &blocks);
    }
}

/// A try of the round of an agent's cell fills what its message fills, as the carrier says it.
fn tried(sim: &mut Sim, round: &support::Given) {
    if sim.cell(round).line.is_some() {
        let entered = sim.enter(round, sim.begins(round));
        sim.tries(round, 1, entered.fills);
    }
}

/// Where the owner's words would go now.
fn receiver(sim: &Sim, to: To) -> Result<CellId, NoOne> {
    spoken::receiver(&sim.flow, &sim.work, &to)
}

/// The content of the latest version of the block.
fn last_entry<'s>(sim: &'s Sim, block: &str) -> &'s str {
    let at = *sim.versions(block).last().expect("the block has a version");
    sim.content(block, at)
}

/// Each version of the block in the message, up to the next heading of the message, as the cell is given it.
fn record_given<'t>(text: &'t str, block: &str) -> Vec<&'t str> {
    let heading = format!("## `{block}`");
    let mut given = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(&heading) {
        let from = &rest[at..];
        let end = from[1..].find("\n#").map_or(from.len(), |n| n + 1);
        given.push(&from[..end]);
        rest = &from[end..];
    }
    given
}

/// Whether the text names any cell of the flow by its id.
fn names_a_cell(flow: &Compiled, text: &str) -> bool {
    (flow.cells.iter()).any(|c| text.contains(c.id.as_str()))
}

#[test]
fn words_to_r2_while_its_question_waits_reach_it_and_its_message_holds_them_under_its_words() {
    let (mut sim, round) = until("R2");
    tried(&mut sim, &round);
    let asked = Question::Asked {
        name: "naming".to_string(),
        waits: true,
        recommends: Some(1),
        ask: "Which name does the greeting use?".to_string(),
        options: vec![
            Way {
                does: "The first name".to_string(),
                follows: "Shorter".to_string(),
            },
            Way {
                does: "The full name".to_string(),
                follows: "Longer".to_string(),
            },
        ],
        about: None,
        reasons: Default::default(),
    };
    let came = Came {
        said: Said {
            questions: vec![asked],
            ..Said::default()
        },
        ..Came::default()
    };
    sim.returns(&round, came, 0);
    assert_eq!(sim.round(), None, "the work waits for the answer at R2");
    // Words to the cell that asked go to it, and words to its line to it as well, which holds the work; a line no cell
    // of which ran, and that the work stands at none of, takes none; nor does a cell the flow does not have.
    assert_eq!(receiver(&sim, To::Cell("R2".into())), Ok(CellId::from("R2")));
    assert_eq!(receiver(&sim, To::Line("engineer".into())), Ok(CellId::from("R2")));
    let deep = To::Line("deep_researcher".into());
    assert_eq!(receiver(&sim, deep), Err(NoOne::NotReached("deep_researcher".into())));
    assert_eq!(
        receiver(&sim, To::Cell("none".into())),
        Err(NoOne::NoSuchCell("none".into()))
    );

    sim.take(Event::Said {
        to: To::Cell("R2".into()),
        words: "Why not the name the item gives?".to_string(),
    });
    let round = sim
        .round()
        .expect("R2 is entered for the owner's words while its question waits");
    assert_eq!(round.cell.as_str(), "R2");
    let entered = sim.enter(&round, sim.begins(&round));
    // The words are a version of R2's own, and, as R2 is given them, name no cell.
    assert_eq!(sim.versions("r2_words").len(), 1);
    let entry = "The owner:\n\nWhy not the name the item gives?";
    assert!(entered.text.contains("## `r2_words`, version "), "{}", entered.text);
    assert!(
        entered.text.contains(entry),
        "the owner's words are not in R2's message:\n{}",
        entered.text
    );
    let given = record_given(&entered.text, "r2_words");
    assert!(
        !given.is_empty() && !given.iter().any(|g| names_a_cell(&sim.flow, g)),
        "{given:?}"
    );
    assert!(entered.text.contains("- `r2_words`"), "{}", entered.text);

    // R2 answers in its words, held to nothing its cell must write while its question waits; the answer is their next
    // version.
    sim.tries(&round, 1, entered.fills);
    let came = Came {
        dealt_with: Some(vec![BlockId::from("r2_words")]),
        writes: vec![Handed {
            block: BlockId::from("r2_words"),
            content: "The item gives none: the first name it is.".to_string(),
            matters: true,
            value: None,
        }],
        ..Came::default()
    };
    sim.returns(&round, came, 0);
    assert_eq!(
        last_entry(&sim, "r2_words"),
        "An agent:\n\nThe item gives none: the first name it is."
    );
    assert_eq!(sim.round(), None, "the work waits for the answer at R2 again");
}

#[test]
fn words_to_a_line_go_to_its_cell_under_way_and_stay_new_to_it_until_its_round_was_told_them() {
    let (mut sim, round) = until("I1");
    tried(&mut sim, &round);
    assert_eq!(receiver(&sim, To::Line("engineer".into())), Ok(CellId::from("I1")));
    let effects = sim.take(Event::Said {
        to: To::Line("engineer".into()),
        words: "Keep the greeting short.".to_string(),
    });
    let told = (effects.iter()).find_map(|e| match e {
        Effect::Tell { run, telling } => Some((*run, telling.entry.clone())),
        _ => None,
    });
    assert_eq!(
        told,
        Some((round.run, "The owner:\n\nKeep the greeting short.".to_string()))
    );
    // The round ends without having been told them: its return deals with nothing of them, and I1 is entered again,
    // given them.
    sim.hands_in(&round, &[("code", "abc", None)]);
    let again = sim.round().expect("a round is given");
    assert_eq!(again.cell.as_str(), "I1");
    let entered = sim.enter(&again, sim.begins(&again));
    assert!(entered.text.contains("Keep the greeting short."), "{}", entered.text);
    assert!(entered.text.contains("- `i1_words`"), "{}", entered.text);
}

#[test]
fn words_a_round_under_way_was_told_are_consumed_by_its_return() {
    let (mut sim, round) = until("I1");
    tried(&mut sim, &round);
    let effects = sim.take(Event::Said {
        to: To::Line("engineer".into()),
        words: "Keep the greeting short.".to_string(),
    });
    let versions = (effects.into_iter())
        .find_map(|e| match e {
            Effect::Tell { telling, .. } => Some(telling.versions),
            _ => None,
        })
        .expect("the round under way is told");
    sim.take(Event::SaidToRound {
        run: round.run,
        versions,
    });
    // It deals with everything that ran it, the words put into it among them.
    let came = Came {
        writes: vec![
            Handed {
                block: BlockId::from("code"),
                content: "abc".to_string(),
                matters: true,
                value: None,
            },
            Handed {
                block: BlockId::from("i1_words"),
                content: "Kept short.".to_string(),
                matters: true,
                value: None,
            },
        ],
        ..Came::default()
    };
    sim.returns(&round, came, 0);
    assert_eq!(last_entry(&sim, "i1_words"), "An agent:\n\nKept short.");
    let next = sim.round().expect("a round is given");
    assert_ne!(
        next.cell.as_str(),
        "I1",
        "I1 is not entered again for words it dealt with"
    );
}

#[test]
fn words_to_a_work_that_has_ended_are_refused_and_go_to_no_cell() {
    let (mut sim, _) = until("I1");
    sim.take(Event::Cancelled { why: None });
    assert_eq!(receiver(&sim, To::Line("engineer".into())), Err(NoOne::Ended));
    let entries = sim.versions("i1_words").len();
    let said = Event::Said {
        to: To::Line("engineer".into()),
        words: "Too late.".to_string(),
    };
    let refused = engine::handle(&sim.flow, &mut sim.work, engine::Time(1_000), said);
    assert_eq!(refused, Err(engine::Refused::NoOneSpokenTo(NoOne::Ended)));
    assert_eq!(sim.versions("i1_words").len(), entries, "nothing is written");
}

#[test]
fn a_reopened_session_s_reading_of_the_tree_leaves_the_line_where_its_cell_that_takes_words_stands() {
    let (mut sim, round) = until("I1");
    tried(&mut sim, &round);
    // The engineer's session is reopened: R0, which the owner's words do not run, reads the tree again before the
    // review.
    sim.anew("engineer", engine::Why::Owner);
    let new = "A new session of the line `engineer`: the owner asked for one.";
    sim.wrote("engineer_session", new, None, Outsider::Outside);
    sim.hands_in(&round, &[("code", "abc", None)]);
    let reading = sim.round().expect("a round is given");
    assert_eq!(reading.cell.as_str(), "R0");
    assert_eq!(receiver(&sim, To::Line("engineer".into())), Ok(CellId::from("I1")));
}

#[test]
fn words_the_hook_gave_a_round_that_was_then_stopped_are_held_by_its_session() {
    let (mut sim, implement) = deeply_until("I1");
    tried(&mut sim, &implement);
    // The reviews ask nothing, and none of their cells has words of its own: words to their lines are refused, the
    // plan's review having run.
    for review in ["plan_reviewer", "code_reviewer"] {
        let to = To::Line(review.into());
        assert_eq!(receiver(&sim, to), Err(NoOne::NoneOfTheLine(review.into())));
    }
    let effects = sim.take(Event::Said {
        to: To::Line("engineer".into()),
        words: "Look at the names too.".to_string(),
    });
    let versions = (effects.into_iter())
        .find_map(|e| match e {
            Effect::Tell { telling, .. } => Some(telling.versions),
            _ => None,
        })
        .expect("the implementation under way is told");
    // Words to the deep research, before the implementation, stop the implementation's round.
    assert_eq!(
        receiver(&sim, To::Line("deep_researcher".into())),
        Ok(CellId::from("D2"))
    );
    let effects = sim.take(Event::Said {
        to: To::Line("deep_researcher".into()),
        words: "Shorter, please.".to_string(),
    });
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Stop { run } if *run == implement.run))
    );
    // The hook gave the implementation the owner's words before its round ended.
    sim.take(Event::SaidToRound {
        run: implement.run,
        versions,
    });
    sim.returns(&implement, Came::default(), 0);
    let rethought = sim.round().expect("a round is given");
    assert_eq!(rethought.cell.as_str(), "D2");
    tried(&mut sim, &rethought);
    sim.hands_in(&rethought, &[]);
    // The implementation runs again for the words it never dealt with; its session holds them, and is given what it
    // lacks.
    let again = sim.round().expect("a round is given");
    assert_eq!(again.cell.as_str(), "I1");
    let entered = sim.enter(&again, sim.begins(&again));
    assert!(
        entered.text.contains("The owner:\n\nShorter, please."),
        "{}",
        entered.text
    );
    assert!(
        !entered.text.contains("Look at the names too."),
        "given again:\n{}",
        entered.text
    );
    // Each cell's words hold what was said to it; the implementation is given the deep research's too, which names no
    // cell.
    assert_eq!((sim.versions("i1_words").len(), sim.versions("d2_words").len()), (1, 1));
    let given = record_given(&entered.text, "d2_words");
    assert!(
        !given.is_empty() && !given.iter().any(|g| names_a_cell(&sim.flow, g)),
        "{given:?}"
    );
    assert!(entered.text.contains("- `i1_words`"), "{}", entered.text);
}

// A cell is told how to write a block, in the words of the block's own file, after the list of what it returns, only
// where the block is among its outputs: the research, which may ask the owner and answer what the owner says to it, is
// told how to do both; the code review, which does neither, is told neither.
#[test]
fn a_cell_is_told_how_to_write_a_block_only_where_it_writes_it() {
    let words = |sim: &Sim, block: &str| {
        let declared = &sim.flow.blocks[&BlockId::from(block)];
        declared.words.clone().expect("the block's file says how it is written")
    };
    let (sim, research) = until("R1");
    let (questions, discussion) = (words(&sim, "r1_questions"), words(&sim, "r1_words"));
    let entered = sim.enter(&research, sim.begins(&research));
    for told in [&questions, &discussion] {
        assert!(
            entered.text.contains(told.trim()),
            "{told} is not in:\n{}",
            entered.text
        );
    }
    let (sim, review) = until("T1");
    let entered = sim.enter(&review, sim.begins(&review));
    for told in [&questions, &discussion] {
        assert!(!entered.text.contains(told.trim()), "{told} is in:\n{}", entered.text);
    }
}

/// The question R2 asks and waits for in these tests: which name the greeting uses, recommending the first.
fn naming() -> Question {
    Question::Asked {
        name: "naming".to_string(),
        waits: true,
        recommends: Some(1),
        ask: "Which name does the greeting use?".to_string(),
        options: vec![
            Way {
                does: "The first name".to_string(),
                follows: "Shorter".to_string(),
            },
            Way {
                does: "The full name".to_string(),
                follows: "Longer".to_string(),
            },
        ],
        about: None,
        reasons: Default::default(),
    }
}

// Each question a cell asks and each answer of the owner's is a version of the cell's own questions, which the engine
// writes; the question runs no cell, and the answer runs the cells that watch them; and every agent cell is given every
// cell's questions whole, so that it reads the work's questions and an answer comes with its question.
#[test]
fn each_question_and_each_answer_is_a_version_of_the_asking_cell_s_questions_given_whole_to_every_agent() {
    let (mut sim, round) = until("R2");
    tried(&mut sim, &round);
    let came = Came {
        said: Said {
            questions: vec![naming()],
            ..Said::default()
        },
        ..Came::default()
    };
    sim.returns(&round, came, 0);
    let version = sim.work.records.questions.last().expect("the question is kept").at;
    let entries = sim.versions("r2_questions");
    assert_eq!(entries.len(), 1, "the question is a version");
    let question = sim.content("r2_questions", entries[0]).to_string();
    let head = format!(
        "`naming`, version {}, asked: Which name does the greeting use?",
        version.0
    );
    assert!(question.starts_with(&head), "{question}");
    for part in [
        "option 1, The first name: Shorter",
        "option 2, The full name: Longer",
        "Recommended: option 1.",
        "The work waits for the answer.",
    ] {
        assert!(question.contains(part), "{part} is not in {question}");
    }
    assert_eq!(
        sim.round(),
        None,
        "the work waits for the answer at R2, the question running no cell"
    );

    sim.take(Event::Answer {
        question: "naming".to_string(),
        version,
        option: 2,
    });
    let entries = sim.versions("r2_questions");
    assert_eq!(entries.len(), 2, "the answer is a version");
    let answer = sim.content("r2_questions", entries[1]).to_string();
    let said = format!(
        "`naming`, version {}, answered: option 2, The full name: Longer.",
        version.0
    );
    assert_eq!(answer, said);
    // The answer is the one version of any cell's questions that changes their meaning, which runs the cell that asked.
    let answered: Vec<&str> = (sim.work.records.written.iter())
        .filter(|w| w.matters && w.block.as_str().ends_with("_questions"))
        .map(|w| w.block.as_str())
        .collect();
    assert_eq!(answered, ["r2_questions"]);
    let round = sim.round().expect("R2 is entered for the answer");
    assert_eq!(round.cell.as_str(), "R2");
    let given_with_its_question = |text: &str| {
        let (q, a) = (text.find(question.as_str()), text.find(answer.as_str()));
        q.is_some() && a.is_some() && q < a
    };
    let entered = sim.enter(&round, sim.begins(&round));
    assert!(given_with_its_question(&entered.text), "{}", entered.text);
    sim.tries(&round, 1, entered.fills);
    sim.hands_in(&round, &[]);
    // The code review, of a line of its own, is given every cell's questions too: it reads the question, and the answer
    // with it.
    let round = loop {
        let round = sim.round().expect("a round is given");
        let blocks: &[(&str, &str, Option<&str>)] = match round.cell.as_str() {
            "T1" => break round,
            "P1" => &[("plan_ok", "", Some("yes"))],
            "P2" => &[("tree_ok", "", Some("yes"))],
            "I1" => &[("code", "abc", None)],
            other => panic!("{other} comes between: only the plan's reviews and the implementation do"),
        };
        tried(&mut sim, &round);
        sim.hands_in(&round, blocks);
    };
    let entered = sim.enter(&round, sim.begins(&round));
    assert!(given_with_its_question(&entered.text), "{}", entered.text);
}

// An answer to the owner is a version of the cell's own words, which replaces none before it: a cell that answers while
// the owner speaks to a cell after it has its answer taken and not sent back.
#[test]
fn an_answer_added_while_another_cell_was_spoken_to_in_its_round_is_taken() {
    let (mut sim, implement) = deeply_until("I1");
    tried(&mut sim, &implement);
    // Words to the deep research stop the implementation, and the deep research is entered for them.
    sim.take(Event::Said {
        to: To::Line("deep_researcher".into()),
        words: "Shorter, please.".to_string(),
    });
    sim.returns(&implement, Came::default(), 0);
    let rethought = sim.round().expect("a round is given");
    assert_eq!(rethought.cell.as_str(), "D2");
    tried(&mut sim, &rethought);
    // While it runs, the owner speaks to the implementation, after it: its round goes on, never told of the words.
    assert_eq!(receiver(&sim, To::Line("engineer".into())), Ok(CellId::from("I1")));
    let effects = sim.take(Event::Said {
        to: To::Line("engineer".into()),
        words: "And the names.".to_string(),
    });
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::Stop { run } if *run == rethought.run))
    );
    let came = Came {
        dealt_with: Some(vec![BlockId::from("d2_words")]),
        writes: vec![Handed {
            block: BlockId::from("d2_words"),
            content: "Shorter it is.".to_string(),
            matters: true,
            value: None,
        }],
        ..Came::default()
    };
    sim.returns(&rethought, came, 0);
    let finished = (sim.work.records.finished.iter()).any(|f| f.run == rethought.run);
    assert!(finished, "the answer is taken, not sent back");
    assert_eq!(last_entry(&sim, "d2_words"), "An agent:\n\nShorter it is.");
}

// What the owner reads is written in the owner's language: a cell whose list of what it returns has a block the owner
// reads is told so once, with the list; a cell whose list has none, as the reading of the tree, which writes no block, is
// not told it.
#[test]
fn a_cell_that_returns_what_the_owner_reads_is_told_its_language_once_with_the_list() {
    let (sim, reading) = until("R0");
    let told = sim.words.returns.outputs.owner_reads.trim().to_string();
    let entered = sim.enter(&reading, sim.begins(&reading));
    assert!(!entered.text.contains(&told), "{}", entered.text);
    let (sim, research) = until("R1");
    let entered = sim.enter(&research, sim.begins(&research));
    assert_eq!(entered.text.matches(&told).count(), 1, "{}", entered.text);
    let list = entered.text.find("## What this cell returns").expect("the list");
    assert!(entered.text[list..].contains(&told), "{}", entered.text);
}

/// A block as a return hands it in, mattering, with its value where it is a choice.
fn handed(block: &str, content: &str, value: Option<&str>) -> Handed {
    Handed {
        block: BlockId::from(block),
        content: content.to_string(),
        matters: true,
        value: value.map(str::to_string),
    }
}

/// The round given next, of this cell, carried out: it fills what its message fills, and hands in these blocks.
fn next(sim: &mut Sim, cell: &str, blocks: &[(&str, &str, Option<&str>)]) {
    let round = sim.round().expect("a round is given");
    assert_eq!(
        round.cell.as_str(),
        cell,
        "the walk runs {} where {cell} was expected",
        round.cell
    );
    tried(sim, &round);
    sim.hands_in(&round, blocks);
}

/// The owner speaks to the plan's second look, which changes the plan as the words ask and answers them; while the
/// work is paused, the words run it all the same.
fn the_plan_changed_by_the_owner_s_words(sim: &mut Sim, words: &str, plan: &str) {
    sim.take(Event::Said {
        to: To::Cell("R2".into()),
        words: words.to_string(),
    });
    next(
        sim,
        "R2",
        &[("r2_words", "Changed as you say.", None), ("plan_agent", plan, None)],
    );
}

// The owner's ruling in the design review's place (design document `data-and-context`, 39). The review fails the plan
// with points A and B, and the owner writes its verdict at yes before the research has taken them; the plan changes
// since, by the owner's words to the plan's second look, and the review fails again with A' and C. The points the review
// wrote before the ruling are new to no cell from then on, in the walk, in the points a round is told and in what the
// context manager tells a round is new to it alike (5, 11): the research, which writes the plan and answers in the
// dialogue, owes a word for A' and C alone, and is told the dialogue is new to it since the points the owner ruled on.
// The research answers, the review fails a third time with D, the owner rules again, and the review fails a fourth time
// with E on a plan changed since: the second ruling marks the points before it, D among them, and neither E nor the
// research's answer, and the research is told the dialogue is new since D.
//
// Every check is made, and each that fails is said, in one run. A round told the dialogue is new since the version its
// cell last consumed or wrote, none at first and its own answer later, would be told the points the owner ruled on, A
// and B and then D, as new to it, which 39 says they are to no cell.
#[test]
fn points_the_owner_ruled_on_are_new_to_no_cell_in_the_walk_in_the_points_told_and_in_the_context_alike() {
    let (mut sim, review) = until("P1");
    let dialogue = BlockId::from("plan_review");
    let mut failures: Vec<String> = Vec::new();
    // The owner pauses the work while the review judges, so that its points run nothing before the owner rules.
    tried(&mut sim, &review);
    sim.take(Event::Paused {
        by: Pauser::Owner,
        why: None,
    });
    let points = [
        ("plan_ok", "", Some("no")),
        ("plan_review", "A: the name. B: the hello.", None),
    ];
    sim.hands_in(&review, &points);
    let a_and_b = Seq(sim.version("plan_review"));
    sim.wrote("plan_ok", "", Some("yes"), Outsider::Person);
    let new_to = |sim: &Sim, cell: &str| decide::new_to(&sim.flow.flow, &sim.work.records, &CellId::from(cell));
    if !new_to(&sim, "R1").is_empty() {
        failures.push(format!(
            "the walk: points the owner ruled on run the research: {:?}",
            new_to(&sim, "R1")
        ));
    }
    the_plan_changed_by_the_owner_s_words(&mut sim, "Greet by the first name.", "Add the first name.");
    sim.take(Event::Resumed);
    next(
        &mut sim,
        "P1",
        &[
            ("plan_ok", "", Some("no")),
            ("plan_review", "A': the first name. C: warmly.", None),
        ],
    );
    let a_and_c = Seq(sim.version("plan_review"));
    // What the walk, the round and the context manager each take as new to the research, which runs next.
    let check = |sim: &Sim, failures: &mut Vec<String>, ruled: &[Seq], unruled: &[Seq], since: Seq| {
        let round = sim.round().expect("a round is given");
        assert_eq!(round.cell.as_str(), "R1", "the research takes the review's points");
        let versions = decide::ruled_versions(&sim.flow.flow, &sim.work.records);
        for v in &versions[&dialogue] {
            if ruled.contains(&v.at) != v.ruled && (ruled.contains(&v.at) || unruled.contains(&v.at)) {
                failures.push(format!("the ruling: version {} is marked ruled {}", v.at.0, v.ruled));
            }
        }
        if new_to(sim, "R1") != [dialogue.clone()] {
            failures.push(format!("the walk: new to the research: {:?}", new_to(sim, "R1")));
        }
        let told = decide::told(&sim.flow.flow, &sim.work.records, round.run).expect("the run started");
        if told.points != [dialogue.clone()] {
            failures.push(format!("the round: told points in {:?}", told.points));
        }
        let entered = sim.enter(&round, sim.begins(&round));
        let new = entered.run.new.get("plan_review").map(|n| (n.last, n.latest));
        let latest = Seq(sim.version("plan_review"));
        if new != Some((Some(since), latest)) {
            failures.push(format!(
                "the context: the research is told plan_review is new as {new:?} (last, latest), where the points \
                 before version {} are new to no cell: expected {:?}",
                since.0,
                (Some(since), latest)
            ));
        }
        round
    };
    let round = check(&sim, &mut failures, &[a_and_b], &[a_and_c], a_and_b);
    // The research answers A' and C.
    let entered = sim.enter(&round, sim.begins(&round));
    sim.tries(&round, 1, entered.fills);
    sim.hands_in(
        &round,
        &[
            ("deep_needed", "", Some("no")),
            ("plan_review", "A': done. C: done.", None),
        ],
    );
    let answer = Seq(sim.version("plan_review"));
    // The review fails again with D, and the owner rules again before the research takes it.
    let review = sim.round().expect("a round is given");
    assert_eq!(review.cell.as_str(), "P1", "the review takes the answer");
    tried(&mut sim, &review);
    sim.take(Event::Paused {
        by: Pauser::Owner,
        why: None,
    });
    sim.hands_in(
        &review,
        &[("plan_ok", "", Some("no")), ("plan_review", "D: the goodbye.", None)],
    );
    let d = Seq(sim.version("plan_review"));
    sim.wrote("plan_ok", "", Some("yes"), Outsider::Person);
    for cell in ["R1", "P1"] {
        if !new_to(&sim, cell).is_empty() {
            failures.push(format!(
                "the walk, after the second ruling: new to {cell}: {:?}",
                new_to(&sim, cell)
            ));
        }
    }
    the_plan_changed_by_the_owner_s_words(&mut sim, "Say goodbye too.", "Add the first name and a goodbye.");
    sim.take(Event::Resumed);
    next(
        &mut sim,
        "P1",
        &[
            ("plan_ok", "", Some("no")),
            ("plan_review", "E: the time of day.", None),
        ],
    );
    let e = Seq(sim.version("plan_review"));
    check(&sim, &mut failures, &[a_and_b, a_and_c, d], &[answer, e], d);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// A question the work waits for holds the work at its cell whether or not the cell applies now (design document
// `data-and-context`, 32): the research, in one return, hands the plan to the deep research, `deep_needed` yes, which
// skips the research from then on, and asks a question the work waits for. Before the answer no cell after the research
// runs, whatever is new to it; the owner's words to the engineer's line, which stands at the research, are refused,
// saying the research does not apply now, and go to no other cell (35). The answer, though it takes the option the
// research recommends, changes the research's questions in meaning, since the work waited for it (31), and runs the
// deep research, which took charge and watches them (28), told that they ran it; the research does not run.
#[test]
fn a_question_that_waits_at_the_research_it_skipped_holds_the_deep_research_until_the_answer_runs_it() {
    let (mut sim, research) = until("R1");
    tried(&mut sim, &research);
    let mut came = Came::hands_in(vec![
        handed("deep_needed", "", Some("yes")),
        handed("found", "Where the name comes from is open.", None),
    ]);
    came.said.questions = vec![naming()];
    sim.returns(&research, came, 0);
    let waits_at_the_research = |sim: &Sim| {
        let decision = decide::decide(&sim.flow.flow, &sim.work.records);
        let held = matches!(&decision, Decision::Wait { at, on }
            if at.as_str() == "R1" && *on == [Waited::Answer { question: "naming".to_string() }]);
        (held, decision)
    };
    assert_eq!(sim.round(), None, "the work waits for the answer");
    let (held, decision) = waits_at_the_research(&sim);
    assert!(held, "{decision:?}");
    assert!(!decide::applies(&sim.flow.flow, &sim.work.records, &CellId::from("R1")));

    // What is new to the cells after the research runs none of them: the owner rewrites the background, which runs the
    // reading of the tree before the research, and the deep research after it; main moves, which runs the check.
    sim.writes("background", "Greet the user by name, warmly.");
    next(&mut sim, "R0", &[]);
    sim.wrote("main", "The main branch main is at m2.", None, Outsider::Outside);
    assert_eq!(sim.round(), None, "a cell after the research runs before the answer");
    let (held, decision) = waits_at_the_research(&sim);
    assert!(held, "{decision:?}");
    for cell in ["D1", "M1"] {
        let new = decide::new_to(&sim.flow.flow, &sim.work.records, &CellId::from(cell));
        assert!(
            !new.is_empty(),
            "nothing is new to {cell}: the hold is not what keeps it from running"
        );
    }

    // The owner's words to the engineer's line would reach the research, which does not apply now: refused, saying so,
    // and written nowhere.
    let skipped = Err(NoOne::Skipped(CellId::from("R1")));
    assert_eq!(receiver(&sim, To::Line("engineer".into())), skipped);
    let words: usize = (sim.flow.cells.iter())
        .filter_map(|c| spoken::words_of(&sim.flow, &c.id))
        .map(|b| sim.versions(b.as_str()).len())
        .sum();
    let said = Event::Said {
        to: To::Line("engineer".into()),
        words: "Which name does the item give?".to_string(),
    };
    let refused = engine::handle(&sim.flow, &mut sim.work, engine::Time(1_000), said);
    assert_eq!(
        refused,
        Err(engine::Refused::NoOneSpokenTo(NoOne::Skipped(CellId::from("R1"))))
    );
    let after: usize = (sim.flow.cells.iter())
        .filter_map(|c| spoken::words_of(&sim.flow, &c.id))
        .map(|b| sim.versions(b.as_str()).len())
        .sum();
    assert_eq!(after, words, "the refused words are written to no cell");

    // The owner answers by the option the research recommends.
    let version = sim.work.records.questions.last().expect("the question is kept").at;
    sim.take(Event::Answer {
        question: "naming".to_string(),
        version,
        option: 1,
    });
    let versions = decide::versions(&sim.work.records);
    let answer = versions[&BlockId::from("r1_questions")]
        .last()
        .expect("the answer is a version");
    assert_eq!(
        answer.meaning,
        Some(answer.at),
        "an answer to a question the work waited for changes nothing"
    );
    let deep = sim.round().expect("the answer runs a cell");
    assert_eq!(
        deep.cell.as_str(),
        "D1",
        "the deep research, in charge, takes the answer"
    );
    let told = decide::told(&sim.flow.flow, &sim.work.records, deep.run).expect("the run started");
    assert!(
        told.ran_by.contains(&BlockId::from("r1_questions")),
        "{:?}",
        told.ran_by
    );
    let entered = sim.enter(&deep, sim.begins(&deep));
    assert!(
        entered.situation.new.contains(&"r1_questions".to_string()),
        "{:?}",
        entered.situation.new
    );
    sim.tries(&deep, 1, entered.fills);
    sim.hands_in(
        &deep,
        &[
            ("plan_owner", "Greet by the first name.", None),
            ("plan_agent", "Add the first name to the greeting.", None),
            ("acceptance", "The greeting names the user.", None),
        ],
    );
    let after = sim.round().expect("a round is given");
    assert_ne!(after.cell.as_str(), "R1", "the research, skipped, runs for the answer");
}
