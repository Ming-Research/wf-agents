//! What a cell of the repository's engineer flow is sent, its work taken through the engine's own loop: a cell's first
//! entry in the session of its line names each datum the session holds already, with its version; and the owner's
//! words reach the cell the engine chooses, in that cell's own message.

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
            "main",
            "session",
            "start",
            "tree",
            "words",
        ]),
        acts: names(&["start", "check", "merge", "deliver"]),
        tools: names(&["query", "run_tests"]),
        system: flow::system::reserved(&beside).into_iter().collect(),
        fields: names(&["dealt_with", "meaning_unchanged", "observations", "questions", "reply"]),
        details: names(&["files"]),
    };
    flow::compile(&source, &has).expect("the engineer's flow compiles")
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
            "R2" => &[("better_found", "", Some("no"))],
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
            "R2" | "D2" => vec![("better_found", "", Some("no"))],
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

/// Whether the text names any cell of the flow by its id.
fn names_a_cell(flow: &Compiled, text: &str) -> bool {
    (flow.cells.iter()).any(|c| text.contains(c.id.as_str()))
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
        writes: vec![Handed {
            block: BlockId::from("better_found"),
            content: String::new(),
            matters: true,
            value: Some("no".to_string()),
        }],
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

    // R2 answers in `reply`, held to nothing its cell must write while its question waits; the answer is its entry of
    // the record.
    sim.tries(&round, 1, entered.fills);
    let came = Came {
        dealt_with: Some(vec![BlockId::from("r2_spoken_to")]),
        said: Said {
            reply: Some("The item gives none: the first name it is.".to_string()),
            ..Said::default()
        },
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
        writes: vec![Handed {
            block: BlockId::from("code"),
            content: "abc".to_string(),
            matters: true,
            value: None,
        }],
        said: Said {
            reply: Some("Kept short.".to_string()),
            ..Said::default()
        },
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
    sim.hands_in(&rethought, &[("better_found", "", Some("no"))]);
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
