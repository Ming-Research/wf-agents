//! What a cell of the repository's engineer flow is sent, its work taken through the engine's own loop: a cell's first
//! entry in the session of its line names each datum the session holds already, with its version; and the owner's
//! words reach the cell the engine chooses, as a version of that cell's own words.

mod support;

use std::collections::BTreeSet;
use std::path::Path;

use context::Stands;
use decide::{BlockId, CellId, Outsider};
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
            "guide",
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
            "D3" => &[("plan_ok", "", Some("yes"))],
            "D4" => &[("tree_ok", "", Some("yes"))],
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
            "D3" => vec![("plan_ok", "", Some("yes"))],
            "D4" => vec![("tree_ok", "", Some("yes"))],
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
            "D3" => &[("plan_ok", "", Some("yes"))],
            "D4" => &[("tree_ok", "", Some("yes"))],
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
