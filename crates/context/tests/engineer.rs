//! What a cell of the repository's engineer flow is sent, its work taken through the engine's own loop: a cell's first
//! entry in the session of its line names each datum the session holds already, with its version; and the owner's
//! words reach the cell the engine chooses, in that cell's own message.

mod support;

use std::collections::BTreeSet;
use std::path::Path;

use context::message::{Called, Datum, Run, Version};
use context::{Situation, Stands, Values};
use decide::{BlockId, CellId, Outsider, Seq};
use engine::spoken::{self, NoOne, To};
use engine::{Came, Effect, Event, Fill, Handed};
use flow::agents::Agents;
use flow::system::Settings;
use flow::{CellSpec, Compiled, Has, Kind};
use support::{Sim, UNRULY, read_as_markdown};
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
        acts: names(&["start", "check", "merge", "deliver", "run_tests"]),
        tools: names(&["query"]),
        system: flow::system::reserved(&beside).into_iter().collect(),
        details: names(&["files"]),
    };
    flow::compile(&source, &has).expect("the engineer's flow compiles")
}

/// The files beside the repository's flows.
fn beside() -> Agents {
    flow::agents::read(&root()).expect("the files beside the flows are read")
}

#[test]
fn the_implementation_entered_first_after_the_plan_names_each_datum_its_session_holds_with_its_version() {
    let beside = flow::agents::read(&root()).expect("the files beside the flows are read");
    let mut sim = Sim::of(engineer(), beside);
    sim.wrote("dependencies", "", Some("merged"), Outsider::Outside);
    sim.writes("background", "Greet the user by name.");
    // The start, the reading, the plan with no deep research and no edit of the tree, and the plan thought through
    // again, each as the engine hands it out, until the implementation is entered.
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

/// A work of the engineer's flow, carried out with the plan written by the research and nothing better found, until
/// the round of this cell is given, which is not carried out: gives the work and that round.
fn until(id: &str) -> (Sim, support::Given) {
    carried_until(id, false)
}

/// A work of the engineer's flow, carried out with the plan written by the deep research, nothing better found and the
/// plan's review content, until the round of this cell is given, which is not carried out: gives the work and that
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

/// The content of the latest version of `discussion`.
fn last_entry(sim: &Sim) -> &str {
    let at = *sim.versions("discussion").last().expect("the record has an entry");
    sim.content("discussion", at)
}

/// The cell each of the owner's entries of `discussion` is kept said to, in order.
fn said_to(sim: &Sim) -> Vec<&str> {
    (sim.work.records.written.iter())
        .filter(|w| w.block.as_str() == "discussion")
        .map(|w| w.said_to.as_ref().map_or("", |c| c.as_str()))
        .collect()
}

/// Each datum `discussion` of the message, up to the next heading of the message, as the cell is given it.
fn record_given(text: &str) -> Vec<&str> {
    let mut given = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("## `discussion`") {
        let from = &rest[at..];
        let end = from[1..].find("\n#").map_or(from.len(), |n| n + 1);
        given.push(&from[..end]);
        rest = &from[end..];
    }
    given
}

/// Each version of `q_and_a` these fill into a session, in order.
fn record_filled(fills: &[Fill]) -> Vec<decide::Seq> {
    (fills.iter())
        .filter_map(|f| match f {
            Fill::Whole { block, version } | Fill::Change { block, version, .. } if block.as_str() == "q_and_a" => {
                Some(*version)
            }
            _ => None,
        })
        .collect()
}

/// Whether the text names any cell of the flow by its id.
fn names_a_cell(flow: &Compiled, text: &str) -> bool {
    (flow.cells.iter()).any(|c| text.contains(c.id.as_str()))
}

// Words to a line whose cell that ran last does not apply now are refused, saying which: R1, once it wrote that deep
// research is needed. Well under a second alone; no other test holds this refusal.
#[test]
fn words_to_a_line_whose_cell_no_longer_applies_are_refused() {
    let (sim, _) = deeply_until("D1");
    assert_eq!(
        receiver(&sim, To::Line("engineer".to_string())),
        Err(NoOne::Skipped(CellId::from("R1")))
    );
}

// Words to a line the flow does not have are refused, saying so. Well under a second alone; no other test holds this
// refusal.
#[test]
fn words_to_a_line_the_flow_does_not_have_are_refused() {
    let (sim, _) = deeply_until("D1");
    assert_eq!(
        receiver(&sim, To::Line("nobody".to_string())),
        Err(NoOne::NoSuchLine("nobody".to_string()))
    );
}

#[test]
fn words_about_the_question_r2_waits_for_reach_r2_and_its_message_holds_them_under_discussion() {
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
    // Words about the question go to the cell that asked it, and words to its line to it as well, which holds the
    // work; a line no cell of which ran, and that the work stands at none of, takes none.
    assert_eq!(receiver(&sim, To::Question("naming".into())), Ok(CellId::from("R2")));
    assert_eq!(receiver(&sim, To::Line("engineer".into())), Ok(CellId::from("R2")));
    let deep = To::Line("deep_researcher".into());
    assert_eq!(receiver(&sim, deep), Err(NoOne::NotReached("deep_researcher".into())));
    assert_eq!(
        receiver(&sim, To::Question("none".into())),
        Err(NoOne::NoSuchQuestion("none".into()))
    );

    sim.take(Event::Said {
        to: To::Question("naming".into()),
        words: "Why not the name the item gives?".to_string(),
    });
    let round = sim
        .round()
        .expect("R2 is entered for the owner's words while its question waits");
    assert_eq!(round.cell.as_str(), "R2");
    let entered = sim.enter(&round, sim.begins(&round));
    // The entry is kept said to R2, and its words, as R2 is given them, name no cell.
    assert_eq!(said_to(&sim), ["R2"]);
    let entry = "The owner:\n\nWhy not the name the item gives?";
    assert!(entered.text.contains("## `discussion`, version "), "{}", entered.text);
    assert!(
        entered.text.contains(entry),
        "the owner's words are not in R2's message:\n{}",
        entered.text
    );
    let given = record_given(&entered.text);
    assert!(
        !given.is_empty() && !given.iter().any(|g| names_a_cell(&sim.flow, g)),
        "{given:?}"
    );
    assert!(entered.text.contains("- `on_spoken`"), "{}", entered.text);

    // R2 answers in `discussion`, held to nothing its cell must write while its question waits; the answer is its
    // entry of the record.
    sim.tries(&round, 1, entered.fills);
    let came = Came {
        dealt_with: Some(vec![BlockId::from("r2_spoken_to")]),
        writes: vec![Handed {
            block: BlockId::from("discussion"),
            content: "The item gives none: the first name it is.".to_string(),
            matters: true,
            value: None,
        }],
        ..Came::default()
    };
    sim.returns(&round, came, 0);
    assert_eq!(
        last_entry(&sim),
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
    assert!(entered.text.contains("- `on_spoken`"), "{}", entered.text);
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
                block: BlockId::from("discussion"),
                content: "Kept short.".to_string(),
                matters: true,
                value: None,
            },
        ],
        ..Came::default()
    };
    sim.returns(&round, came, 0);
    assert_eq!(last_entry(&sim), "An agent:\n\nKept short.");
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
    let entries = sim.versions("discussion").len();
    let said = Event::Said {
        to: To::Line("engineer".into()),
        words: "Too late.".to_string(),
    };
    let refused = engine::handle(&sim.flow, &mut sim.work, engine::Time(1_000), said);
    assert_eq!(refused, Err(engine::Refused::NoOneSpokenTo(NoOne::Ended)));
    assert_eq!(sim.versions("discussion").len(), entries, "nothing is written");
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
    // The reviews ask nothing, and the owner's words run none of their cells: words to their lines are refused, the
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
    // Each entry is kept said to its cell; the implementation is given the one said to the deep research, which names
    // no cell.
    assert_eq!(said_to(&sim), ["I1", "D2"]);
    let given = record_given(&entered.text);
    assert!(
        !given.is_empty() && !given.iter().any(|g| names_a_cell(&sim.flow, g)),
        "{given:?}"
    );
    assert!(entered.text.contains("- `on_spoken`"), "{}", entered.text);
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
    let (questions, discussion) = (words(&sim, "q_and_a"), words(&sim, "discussion"));
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

// Each question a cell asks and each answer of the owner's is an entry of the record of questions and answers, a
// version of it the engine writes, which runs no cell; the answer runs the asking cell and no other; and every session
// of a cell that takes the record holds it whole, so that it reads the work's questions and an answer comes with its
// question: the asking cell's holds the entry of its question as what its return handed in, and is given the answer
// alone; another is given both.
#[test]
fn each_question_and_each_answer_is_an_entry_of_the_record_held_whole_and_the_answer_runs_the_asking_cell_alone() {
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
    let entries = sim.versions("q_and_a");
    assert_eq!(entries.len(), 1, "the question is an entry");
    let question = sim.content("q_and_a", entries[0]).to_string();
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
        "the work waits for the answer at R2, the record running no cell"
    );

    sim.take(Event::Answer {
        question: "naming".to_string(),
        version,
        option: 2,
        why: Some("It is formal.".to_string()),
    });
    let entries = sim.versions("q_and_a");
    assert_eq!(entries.len(), 2, "the answer is an entry");
    let answer = sim.content("q_and_a", entries[1]).to_string();
    let said = format!(
        "`naming`, version {}, answered: option 2, The full name: Longer. Why: It is formal.",
        version.0
    );
    assert_eq!(answer, said);
    // The answer runs the cell that asked, by its trigger's own block, and no other cell.
    let answered: Vec<&str> = (sim.work.records.written.iter())
        .map(|w| w.block.as_str())
        .filter(|b| b.ends_with("_answered"))
        .collect();
    assert_eq!(answered, ["r2_answered"]);
    let round = sim.round().expect("R2 is entered for the answer");
    assert_eq!(round.cell.as_str(), "R2");
    let given_with_its_question = |text: &str| {
        let (q, a) = (text.find(question.as_str()), text.find(answer.as_str()));
        q.is_some() && a.is_some() && q < a
    };
    // R2's session holds the question its own return asked, as the entry the engine kept it as, which the message says
    // after that return: R2 is given the answer alone.
    let entered = sim.enter(&round, sim.begins(&round));
    let kept = format!("`q_and_a`, version {}", entries[0].0);
    let first = entered.text.lines().next().unwrap_or_default();
    assert!(
        first.starts_with("What your last return handed in is kept as: ") && first.contains(&kept),
        "{}",
        entered.text
    );
    assert!(!entered.text.contains(question.as_str()), "{}", entered.text);
    assert!(entered.text.contains(answer.as_str()), "{}", entered.text);
    assert_eq!(record_filled(&entered.fills), [entries[1]], "{}", entered.text);
    sim.tries(&round, 1, entered.fills);
    sim.hands_in(&round, &[]);
    // The code review, of a line of its own, takes the record too: it reads the question, and the answer with it. The
    // implementation, after R2 on its line, holds both.
    let round = loop {
        let round = sim.round().expect("a round is given");
        if round.cell.as_str() == "T1" {
            break round;
        }
        assert_eq!(round.cell.as_str(), "I1", "only the implementation comes between");
        let entered = sim.enter(&round, sim.begins(&round));
        assert!(record_filled(&entered.fills).is_empty(), "{}", entered.text);
        tried(&mut sim, &round);
        sim.hands_in(&round, &[("code", "abc", None)]);
    };
    let entered = sim.enter(&round, sim.begins(&round));
    assert!(given_with_its_question(&entered.text), "{}", entered.text);
}

// An answer to the owner is an entry added to the record of what is said, which replaces none before it: a cell that
// answers while the owner speaks to a cell after it, an entry its round was never told of, has its answer taken and not
// sent back.
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
    // While it runs, the owner speaks to the implementation, after it: its round goes on, never told of the entry.
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
        dealt_with: Some(vec![BlockId::from("d2_spoken_to")]),
        writes: vec![Handed {
            block: BlockId::from("discussion"),
            content: "Shorter it is.".to_string(),
            matters: true,
            value: None,
        }],
        ..Came::default()
    };
    sim.returns(&rethought, came, 0);
    let finished = (sim.work.records.finished.iter()).any(|f| f.run == rethought.run);
    assert!(finished, "the answer is taken, not sent back");
    assert_eq!(last_entry(&sim), "An agent:\n\nShorter it is.");
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

// An answer's entry ends the option's sentence once: what follows from the option, ending as a sentence does in Chinese
// or in English, is not given the words' full stop after it as well, as the answer of W2 was (its entry read "边界。.").
// Under a second alone, most of it taking the work to the research; no other test holds the answer's words filled.
#[test]
fn an_answer_whose_option_ends_its_sentence_is_written_with_one_full_stop() {
    for (follows, ends) in [
        ("较长，也更正式。", "较长，也更正式。 Why: 正式。"),
        ("Longer?", "Longer? Why: 正式。"),
    ] {
        let (mut sim, round) = until("R2");
        tried(&mut sim, &round);
        let mut asked = naming();
        if let Question::Asked { options, .. } = &mut asked {
            options[1].follows = follows.to_string();
        }
        let came = Came {
            said: Said {
                questions: vec![asked],
                ..Said::default()
            },
            ..Came::default()
        };
        sim.returns(&round, came, 0);
        let version = sim.work.records.questions.last().expect("the question is kept").at;
        sim.take(Event::Answer {
            question: "naming".to_string(),
            version,
            option: 2,
            why: Some("正式。".to_string()),
        });
        let entries = sim.versions("q_and_a");
        let answer = sim.content("q_and_a", entries[1]).to_string();
        let said = format!(
            "`naming`, version {}, answered: option 2, The full name: {ends}",
            version.0
        );
        assert_eq!(answer, said);
    }
}

/// The message the cell is sent where each datum it is given holds this text and the session stands with each as
/// `stands` says, and the round is given the result of a call, which holds it too: of a datum given all of it, two
/// versions; of one given what changed, the change. And how many times it holds the text.
fn sent_holding(flow: &Compiled, words: &Agents, cell: &CellSpec, holds: &str, stands: Stands) -> (String, usize) {
    let mut times = 1;
    let mut data = std::collections::BTreeMap::new();
    for given in &cell.given {
        let value = (flow.blocks.get(&given.block))
            .and_then(|b| b.values.as_ref())
            .and_then(|values| values.first().cloned());
        let version = |number| Version {
            number: Seq(number),
            value: value.clone(),
            content: holds.to_string(),
        };
        times += if given.kind == flow::InputKind::All { 2 } else { 1 };
        let datum = Datum {
            latest: version(9),
            last: Some(Seq(5)),
            diff: Some(holds.to_string()),
            versions: vec![version(5), version(9)],
        };
        data.insert(given.block.clone(), datum);
    }
    let situation = Situation {
        triggers: cell.triggers.iter().map(|t| t.name.clone()).collect(),
        data: (cell.given.iter())
            .map(|g| (g.block.as_str().to_string(), stands))
            .collect(),
        results: vec!["tool".to_string()],
        ..Situation::default()
    };
    let run = Run {
        work: "1".to_string(),
        item: "1".to_string(),
        item_title: "Greet".to_string(),
        data,
        results: vec![context::message::Result {
            call: Called::Tool {
                name: "query".to_string(),
                input: "{}".to_string(),
            },
            result: Some(holds.to_string()),
        }],
        ..Run::default()
    };
    let sent = context::message(flow, words, cell, &situation, Values::Run(&run));
    (context::message::text(&sent), times)
}

// Whatever a datum or a call's result holds, the message of every agent cell of the engineer's flow keeps its own
// headings, and gives what it holds as it is written, whole, each version of a datum given all of it, and a change
// since a version alike: set apart, so that nothing in it stands among the message's own headings, as the heading of a
// decision `tree_edits` held stood among the review's in W2. Well under a second alone, every cell and form at once.
#[test]
fn whatever_its_data_hold_every_cells_message_keeps_its_own_headings_and_gives_them_as_written() {
    let (flow, words) = (engineer(), beside());
    for cell in flow.cells.iter().filter(|c| c.kind == Kind::Agent) {
        for stands in [Stands::Lacks, Stands::HoldsLast] {
            let (plain, _) = sent_holding(&flow, &words, cell, "Plain words.", stands);
            let (own, _) = read_as_markdown(&plain);
            for holds in UNRULY {
                let (sent, times) = sent_holding(&flow, &words, cell, holds, stands);
                let (headings, blocks) = read_as_markdown(&sent);
                let at = format!("{}, {stands:?}, holding {holds:?}:\n{sent}", cell.id.as_str());
                assert_eq!(headings, own, "{at}");
                assert_eq!(blocks, vec![holds.to_string(); times], "{at}");
            }
        }
    }
}

// Whatever a file of the project holds, the system prompt of every line of the engineer's flow keeps its own headings,
// and gives the file as the project wrote it: set apart, so that nothing in it stands among the prompt's own headings,
// as the line's rules read, in a trial, as the end of the project's `design/README.md`. Well under a second alone.
#[test]
fn whatever_the_projects_files_hold_every_lines_system_prompt_keeps_its_own_headings_and_gives_them_as_written() {
    let (flow, words) = (engineer(), beside());
    let settings = Settings::of(&words.project, &words.installation, |name| {
        Some(format!("the value of {name}"))
    });
    let fill = |name: &str| match name {
        "your_line" => Some("YOUR LINE".to_string()),
        "store" => Some(String::new()),
        "named_checks" => Some("THE CHECKS".to_string()),
        _ => None,
    };
    let files = (words.session.parts.iter())
        .filter(|p| matches!(p, flow::agents::SessionPart::Project(_)))
        .count();
    assert!(files > 1, "the system prompt holds files of the project");
    for cell in flow.cells.iter().filter(|c| c.kind == Kind::Agent) {
        let prompt = |holds: &str| {
            let parts = flow::system::parts(&words, &flow, cell, |_| Ok(Some(holds.to_string())));
            flow::system::prompt(&parts.expect("the parts are made"), &settings, fill).expect("the prompt is made")
        };
        let (own, _) = read_as_markdown(&prompt("Plain words."));
        for holds in UNRULY {
            let sent = prompt(holds);
            let (headings, blocks) = read_as_markdown(&sent);
            let at = format!("{}, holding {holds:?}:\n{sent}", cell.id.as_str());
            assert_eq!(headings, own, "{at}");
            assert_eq!(blocks, vec![holds.trim_end().to_string(); files], "{at}");
        }
    }
}
