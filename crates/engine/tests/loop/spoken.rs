//! What the owner says to a work and what its cells answer (design document `data-and-context`, 37 to 44): the owner
//! speaks to a line, and in one act the words are the owner's entry of the record of what is said and a version of the
//! trigger of the cell the line has reached, which runs it; a round of that cell under way is told them, and one of a
//! later cell is stopped; a cell's answer is its entry of the record as its run finishes, and the questions a run asks
//! are a version of the questions as they then stand.

use decide::{Questioned, Writer};
use engine::{Question, Way};

use super::*;

/// A laid-out flow of two lines: a research and a build, of the line `main`, each run by what came before it and by
/// the owner's words, the research by the owner's answers too; a review, of the line `other`, run by the build; and the merge. The record of what is said is
/// written by the owner and through the field `reply`, the questions through the field `questions`.
fn speaking() -> Compiled {
    let flow = "name = \"speaking\"\norder = [\"research\", \"build\", \"review\", \"merge\"]\n\
                data = [\"request\", \"discussion\", \"questions\"]\n\
                [limits]\nruns = 3\nreturns = 2\ncalls = 3\n";
    let agent = |task: &str, triggers: &str, context: &str, outputs: &str, line: &str| {
        let kinds: String = (context.split(", ").map(|b| b.trim_matches('"')))
            .map(|b| format!("{b} = \"all\"\n"))
            .collect();
        format!(
            "title = \"{task}\"\nkind = \"agent\"\neffort = \"high\"\nline = \"{line}\"\ntask = \"{task}.\"\n\
             triggered_by = [{triggers}]\ncontext = [{context}]\noutputs = [{outputs}]\n[kinds]\n{kinds}"
        )
    };
    let research = agent(
        "Plan",
        "\"on_request\", \"on_spoken\", \"on_answer\"",
        "\"request\", \"discussion\"",
        "\"plan\", \"discussion\", \"questions\"",
        "main",
    );
    let build = agent(
        "Build",
        "\"on_plan\", \"on_spoken\"",
        "\"plan\", \"discussion\", \"questions\"",
        "\"code\", \"discussion\"",
        "main",
    );
    let review = agent("Review", "\"on_code\"", "\"code\"", "\"verdict\"", "other");
    let merge = "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\ntriggered_by = [\"on_verdict\"]\n";
    let files = [
        ("lines/main.toml", ""),
        ("lines/other.toml", ""),
        ("blocks/plan.toml", ""),
        ("blocks/code.toml", ""),
        ("blocks/verdict.toml", "shown = true\n"),
        ("../data/request.toml", "from = \"owner\"\n"),
        (
            "../data/discussion.toml",
            "from = \"owner\"\nshown = true\nfield = \"reply\"\n",
        ),
        (
            "../data/questions.toml",
            "shown = true\nfield = \"questions\"\n[writes]\nquestion = \"{name} of {cell}: {ask}\\n{options}\"\n\
             waits = \"\"\noption = \"{number}. {does}{recommended}\"\nrecommended = \" (recommended)\"\n\
             between = \"\\n\\n\"\nnone = \"None stands.\"\n",
        ),
        (
            "../triggers/of_a_datum/changed.toml",
            "name = \"on_{datum}\"\nread = [\"{datum}\"]\n",
        ),
        (
            "../triggers/of_a_cell/on_spoken.toml",
            "block = \"{cell}_spoken_to\"\nfrom = \"words\"\nread = [\"discussion\"]\n\
             do = \"Answer the owner in `reply`.\"\n",
        ),
        (
            "../triggers/of_a_cell/on_answer.toml",
            "block = \"{cell}_answered\"\nfrom = \"answer\"\nread = [\"discussion\"]\n",
        ),
    ];
    let source = Source {
        laid_out: true,
        flow: flow.to_string(),
        cells: [
            ("research", research),
            ("build", build),
            ("review", review),
            ("merge", merge.to_string()),
        ]
        .into_iter()
        .map(|(id, text)| (id.to_string(), text))
        .collect(),
        files: (files.iter())
            .map(|(path, text)| (path.to_string(), text.to_string()))
            .collect(),
        ..Default::default()
    };
    let mut has = has(&["words", "answer"]);
    has.fields = ["reply".to_string(), "questions".to_string()].into();
    compile(&source, &has).unwrap_or_else(|faults| panic!("the flow does not compile: {faults:#?}"))
}

/// A work on the speaking flow.
fn speaking_work() -> W {
    W {
        flow: speaking(),
        work: Work::default(),
        clock: START,
    }
}

/// The owner says these words to the line.
fn says(line: &str, words: &str) -> Event {
    Event::Said {
        line: line.to_string(),
        words: words.to_string(),
    }
}

/// Who wrote a version: the owner, the engine, or a cell by its name.
fn by(w: &W, writer: &Writer) -> String {
    match writer {
        Writer::Person => "owner".to_string(),
        Writer::Outside => "engine".to_string(),
        Writer::Run(run) => start_cell(w, *run),
    }
}

/// The cell of the run.
fn start_cell(w: &W, run: RunId) -> String {
    let started = (w.work.records.started.iter()).find(|s| s.run == run);
    started.expect("the run started").cell.to_string()
}

/// The versions of the block, each with who wrote it and its content, in order.
fn versions(w: &W, block: &str) -> Vec<(String, String)> {
    let all = decide::versions(&w.work.records);
    (all.get(&BlockId::from(block)).into_iter().flatten())
        .map(|v| {
            (
                by(w, &v.by),
                w.work.contents.get(&(block.into(), v.at)).cloned().unwrap_or_default(),
            )
        })
        .collect()
}

/// A work whose research handed in its plan and whose build's first round is under way: the build's run.
fn building() -> (W, RunId) {
    let mut w = speaking_work();
    let (effects, _) = w.person_writes("request");
    let (research, cell, ..) = round(&effects);
    assert_eq!(cell, "research");
    let effects = w.returns(research, hands_in(&[("plan", None)]));
    let (build, cell, ..) = round(&effects);
    assert_eq!(cell, "build");
    (w, build)
}

#[test]
fn the_owners_words_are_one_act_an_entry_of_the_record_and_a_version_of_the_trigger_of_the_cell_the_line_reached() {
    let (mut w, build) = building();
    let effects = w.returns(build, hands_in(&[("code", None)]));
    let (review, cell, ..) = round(&effects);
    assert_eq!(cell, "review");
    let effects = w.returns(review, hands_in(&[("verdict", None)]));
    let (merge, cell, ..) = round(&effects);
    assert_eq!(cell, "merge");
    let before = w.work.records.written.len();
    // The merge, after the cell spoken to, is stopped, and the cell runs once its round has ended.
    let effects = w.on(says("main", "Keep it to one table."));
    assert_eq!(effects, [Effect::Stop { run: merge }]);
    // The build ran last of the line's cells the owner's words run: it is the one spoken to.
    let written: Vec<(&str, Outsider)> = (w.work.records.written[before..].iter())
        .map(|x| (x.block.as_str(), x.by))
        .collect();
    // The owner's words go on the record as they are.
    assert_eq!(
        written,
        [("discussion", Outsider::Person), ("build_spoken_to", Outsider::Outside)]
    );
    assert_eq!(
        versions(&w, "discussion").last().map(|(_, c)| c.as_str()),
        Some("Keep it to one table.")
    );
    assert_eq!(
        versions(&w, "discussion").last().map(|(by, _)| by.as_str()),
        Some("owner")
    );
    let effects = w.returns(merge, hands_in(&[]));
    let (run, cell, ..) = round(&effects);
    assert_eq!(cell, "build");
    let (because, _) = start_of(&w, run);
    assert_eq!(because, [changed("build_spoken_to", None, false)]);
}

#[test]
fn words_to_a_line_of_no_cell_the_owner_speaks_to_or_none_of_whose_cells_applies_are_refused() {
    let mut w = speaking_work();
    assert_eq!(
        w.refused(says("nowhere", "Hello.")),
        Refused::NoSuchLine("nowhere".to_string())
    );
    // The line of the review has no cell the owner's words run.
    assert_eq!(
        w.refused(says("other", "Hello.")),
        Refused::NoOneSpokenTo("other".to_string())
    );
    // What the owner's words run is written by their act alone, never from outside.
    for by in [Outsider::Outside, Outsider::Person] {
        assert_eq!(
            w.refused(wrote("build_spoken_to", by, true, None)),
            Refused::NotTheirs {
                block: "build_spoken_to".into(),
                by
            }
        );
    }
}

#[test]
fn the_owner_speaks_to_the_first_of_the_lines_cells_before_any_ran() {
    let mut w = speaking_work();
    let effects = w.on(says("main", "Start with the schema."));
    let (_, cell, ..) = round(&effects);
    assert_eq!(cell, "research");
    assert_eq!(versions(&w, "research_spoken_to").len(), 1);
}

#[test]
fn a_round_of_the_cell_spoken_to_under_way_is_told_the_words_and_the_trigger_is_put_into_it() {
    let (mut w, build) = building();
    let effects = w.on(says("main", "Name it greet."));
    let version = w.last_written();
    assert_eq!(
        effects,
        [Effect::Tell {
            run: build,
            trigger: "on_spoken".to_string(),
            words: "Name it greet.".to_string(),
            does: Some("Answer the owner in `reply`.".to_string()),
        }]
    );
    let put = w.work.records.put_in.last().expect("the trigger is put in");
    assert_eq!(
        (put.run, put.block.as_str(), put.version),
        (build, "build_spoken_to", version)
    );
    // A return that deals with the words and answers them finishes the run, the answer the build's entry of the record.
    let mut came = Came::works(
        vec!["plan".into(), "build_spoken_to".into()],
        vec![handed("code", None)],
    );
    came.said.reply = Some("Named it greet.".to_string());
    let effects = w.returns(build, came);
    let (_, cell, ..) = round(&effects);
    assert_eq!(cell, "review");
    let record = versions(&w, "discussion");
    assert_eq!(
        record,
        [
            ("owner".to_string(), "Name it greet.".to_string()),
            ("build".to_string(), "Named it greet.".to_string())
        ]
    );
}

#[test]
fn the_owner_speaking_to_a_cell_before_a_round_under_way_stops_that_round_and_runs_the_cell_spoken_to() {
    let (mut w, build) = building();
    let effects = w.returns(build, hands_in(&[("code", None)]));
    let (review, cell, ..) = round(&effects);
    assert_eq!(cell, "review");
    let effects = w.on(says("main", "Use the other table."));
    assert_eq!(effects, [Effect::Stop { run: review }]);
}

#[test]
fn a_cells_answer_is_its_entry_of_the_record_and_its_questions_are_a_version_of_the_questions_as_they_stand() {
    let mut w = speaking_work();
    let (effects, _) = w.person_writes("request");
    let (research, ..) = round(&effects);
    let mut came = hands_in(&[("plan", None)]);
    came.said.questions = vec![Question::Asked {
        name: "which".to_string(),
        waits: false,
        recommends: Some(1),
        ask: "Which table?".to_string(),
        options: vec![
            Way {
                does: "the old one".to_string(),
                follows: "nothing to move".to_string(),
            },
            Way {
                does: "a new one".to_string(),
                follows: "a migration".to_string(),
            },
        ],
        about: None,
    }];
    came.said.reply = Some("Nobody spoke to me.".to_string());
    w.returns(research, came);
    assert_eq!(
        versions(&w, "questions"),
        [(
            "research".to_string(),
            "which of research: Which table?\n1. the old one (recommended)\n2. a new one".to_string()
        )]
    );
    // An answer given with nothing said to the cell is kept all the same.
    assert_eq!(
        versions(&w, "discussion"),
        [("research".to_string(), "Nobody spoke to me.".to_string())]
    );
    let asked: &Questioned = w.work.records.questions.last().expect("the question is kept");
    assert_eq!(asked.cell.as_str(), "research");
}
