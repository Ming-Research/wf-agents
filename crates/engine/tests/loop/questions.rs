//! The questions a cell puts to the owner and the owner's answers (design document `data-and-context`, 32 to 36): a
//! question is data of its own, by its name, with versions, which a return asks, asks anew or withdraws; the owner's
//! answer is kept, and runs the asking cell again where it takes another option than the one the work goes by; the
//! engine says in the block of the source `asked` whether every question that stands is answered, which the merge
//! requires.

use decide::{Questioned, Waited};
use engine::{Question, Way};

use super::*;

/// A research that what people ask for and the owner's answers to it run, writing the plan; an implementation of the
/// plan; and a merge that requires every question to be answered.
fn asking() -> Compiled {
    let flow = r#"
        name = "asking"
        order = ["research", "implement", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.main]
        [blocks.request]
        people = true
        [blocks.answered]
        outside = "answer"
        [blocks.asked]
        values = ["waiting", "answered"]
        default = "answered"
        outside = "asked"
        shown = true
        [blocks.asked.writes]
        answered = "Every question has the owner's answer."
        waiting = "Waits for the owner's answer to {questions}."
        between = " and "
        [blocks.plan]
        [blocks.code]
        [blocks.merged]
        shown = true
    "#;
    let research = r#"
        title = "Research"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Write the plan the request asks for."
        inputs = ["request", "answered"]
        outputs = ["plan"]
    "#;
    let implement = r#"
        title = "Implement"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Build the plan."
        inputs = ["plan"]
        outputs = ["code"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        requires = { asked = "answered" }
        inputs = ["code"]
        outputs = ["merged"]
    "#;
    let source = Source {
        flow: flow.to_string(),
        cells: [("research", research), ("implement", implement), ("merge", merge)]
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
        ..Default::default()
    };
    compile(&source, &has(&["answer", "asked"])).expect("the definition compiles")
}

/// A question asked, which the cell waits for or not, with two options, recommending the first.
fn asked(name: &str, waits: bool) -> Question {
    Question::Asked {
        name: name.to_string(),
        waits,
        recommends: Some(1),
        ask: format!("Which way for {name}?"),
        options: vec![
            Way {
                does: "the short way".to_string(),
                follows: "less to read".to_string(),
            },
            Way {
                does: "the long way".to_string(),
                follows: "more to read".to_string(),
            },
        ],
        about: None,
    }
}

/// A return that hands in the plan, dealing with everything that ran it, and asks these questions beside.
fn plans(questions: Vec<Question>) -> Came {
    let mut came = hands_in(&[("plan", None)]);
    came.said.questions = questions;
    came
}

/// A work on the asking flow whose research's first round is under way, for what people asked for.
fn researching() -> (W, RunId) {
    let mut w = W {
        flow: asking(),
        work: Work::default(),
        clock: START,
    };
    let (effects, _) = w.person_writes("request");
    let (run, cell, ..) = round(&effects);
    assert_eq!(cell, "research");
    (w, run)
}

/// The research's first run asks these questions as it hands in the plan; gives what is to be done.
fn researched(questions: Vec<Question>) -> (W, Vec<Effect>) {
    let (mut w, run) = researching();
    let effects = w.returns(run, plans(questions));
    (w, effects)
}

impl W {
    /// The latest version of the question.
    fn version(&self, name: &str) -> Seq {
        (self.work.records.questions.iter())
            .filter(|q| q.name == name)
            .map(|q| q.at)
            .max()
            .expect("the question was asked")
    }

    /// The owner answers the version of the question, taking the option.
    fn answers(&mut self, name: &str, version: Seq, option: u64) -> Vec<Effect> {
        self.on(Event::Answer {
            question: name.to_string(),
            version,
            option,
            why: Some(format!("Option {option}.")),
        })
    }

    /// How many versions of the block the outside wrote.
    fn written(&self, block: &str) -> usize {
        (self.work.records.written.iter())
            .filter(|w| w.block.as_str() == block && w.by == Outsider::Outside)
            .count()
    }

    /// The value and the words of the latest version of `asked`; none where it was never written.
    fn asked(&self) -> Option<(String, String)> {
        let latest = (self.work.records.written.iter()).rfind(|w| w.block.as_str() == "asked")?;
        let content = self.work.contents[&(BlockId::from("asked"), latest.at)].clone();
        Some((latest.value.clone().expect("a choice"), content))
    }
}

#[test]
fn a_question_a_return_asks_is_a_version_of_its_own_of_the_asking_cell() {
    let (w, _) = researched(vec![asked("which", false)]);
    let run = w.work.records.finished[0].run;
    let finish = w.last_finished();
    let q = &w.work.records.questions[0];
    assert_eq!(
        *q,
        Questioned {
            name: "which".to_string(),
            cell: "research".into(),
            run,
            engine: false,
            waits: false,
            withdrawn: false,
            at: Seq(finish.0 + 1),
        }
    );
    assert_eq!(
        w.work.question(q),
        Some(&asked("which", false)),
        "its content is what came back"
    );
}

#[test]
fn a_question_that_waits_holds_the_work_at_its_cell_and_the_merge_waits_with_it() {
    let (w, effects) = researched(vec![asked("which", true)]);
    assert_eq!(effects, [], "nothing begins");
    assert_eq!(
        w.decision(),
        decide::Decision::Wait {
            at: "research".into(),
            on: vec![Waited::Answer {
                question: "which".to_string()
            }],
        }
    );
    assert_eq!(
        w.asked(),
        Some((
            "waiting".to_string(),
            "Waits for the owner's answer to which.".to_string()
        ))
    );
}

#[test]
fn a_question_that_does_not_wait_lets_the_work_go_on_by_its_recommendation_but_holds_the_merge() {
    let (mut w, effects) = researched(vec![asked("which", false)]);
    let (run, cell, ..) = round(&effects);
    assert_eq!(cell, "implement");
    let effects = w.returns(run, hands_in(&[("code", None)]));
    assert_eq!(effects, [], "the merge does not begin");
    let decide::Decision::Wait { at, .. } = w.decision() else {
        panic!("the work does not wait: {:?}", w.decision())
    };
    assert_eq!(at.as_str(), "merge");
    // The answer that takes the recommended option runs nothing, and the merge begins.
    let version = w.version("which");
    let effects = w.answers("which", version, 1);
    assert_eq!(w.written("answered"), 0, "the research does not run again");
    let (_, cell, ..) = round(&effects);
    assert_eq!(cell, "merge");
    assert_eq!(
        w.asked(),
        Some((
            "answered".to_string(),
            "Every question has the owner's answer.".to_string()
        ))
    );
}

#[test]
fn any_answer_to_a_question_that_waits_runs_the_asking_cell_again() {
    let (mut w, _) = researched(vec![asked("which", true)]);
    let version = w.version("which");
    let effects = w.answers("which", version, 1);
    assert_eq!(w.written("answered"), 1);
    let (run, cell, ..) = round(&effects);
    assert_eq!(cell, "research");
    let (because, _) = start_of(&w, run);
    assert_eq!(because, vec![changed("answered", None, false)]);
    let answer = w.work.records.answers.last().expect("the answer is kept");
    assert_eq!((answer.version, answer.option), (version, 1));
    assert_eq!(answer.why.as_deref(), Some("Option 1."));
    assert_eq!(w.asked().map(|(value, _)| value), Some("answered".to_string()));
}

#[test]
fn an_answer_taking_another_option_than_the_one_the_work_goes_by_stops_a_later_round_and_runs_the_cell() {
    let (mut w, effects) = researched(vec![asked("which", false)]);
    let (implementing, ..) = round(&effects);
    let version = w.version("which");
    let effects = w.answers("which", version, 2);
    assert_eq!(effects, [Effect::Stop { run: implementing }]);
    let effects = w.returns(implementing, Came::hands_in(Vec::new()));
    let (_, cell, ..) = round(&effects);
    assert_eq!(cell, "research");
    // Answered again with the option the work now goes by, it runs nothing more.
    let before = w.written("answered");
    w.answers("which", version, 2);
    assert_eq!(w.written("answered"), before);
}

#[test]
fn an_answer_to_an_earlier_version_of_a_question_answers_nothing() {
    let (mut w, run) = researching();
    w.returns(run, plans(vec![asked("which", false)]));
    let first = w.version("which");
    // The question is asked anew by the research, run again for a new request.
    let (implementing, ..) = (w.work.records.started.iter())
        .map(|s| (s.run, s.cell.clone()))
        .rfind(|(_, cell)| cell.as_str() == "implement")
        .expect("the implementation runs");
    w.on(wrote("request", Outsider::Person, true, None));
    w.returns(implementing, Came::hands_in(Vec::new()));
    let research = (w.work.records.started.iter())
        .rfind(|s| s.cell.as_str() == "research")
        .expect("the research runs again")
        .run;
    w.returns(research, plans(vec![asked("which", true)]));
    let second = w.version("which");
    assert!(second > first);
    let asked_before = w.asked();
    let effects = w.answers("which", first, 2);
    assert_eq!(effects, []);
    assert_eq!(w.written("answered"), 0, "the cell does not run");
    assert_eq!(w.asked(), asked_before, "the question still waits");
    assert!(matches!(w.decision(), decide::Decision::Wait { at, .. } if at.as_str() == "research"));
    assert_eq!(w.work.records.answers.len(), 1, "the answer is kept");
}

#[test]
fn a_question_written_twice_in_one_return_is_sent_back() {
    let (mut w, _) = researched(vec![asked("which", true)]);
    let version = w.version("which");
    w.answers("which", version, 1);
    let research = (w.work.records.started.iter())
        .rfind(|s| s.cell.as_str() == "research")
        .expect("the research runs again")
        .run;
    let mut came = Came::works(vec!["answered".into()], Vec::new());
    came.said.questions = vec![
        asked("other", false),
        Question::Withdrawn {
            name: "other".to_string(),
            why: "Settled.".to_string(),
        },
    ];
    let effects = w.returns(research, came);
    let (_, _, _, given) = round(&effects);
    assert_eq!(
        given,
        Given::SentBack(vec![Unmet::QuestionTwice {
            name: "other".to_string()
        }])
    );
}

#[test]
fn a_cell_may_withdraw_only_its_own_question_that_stands() {
    let (mut w, effects) = researched(vec![asked("which", false)]);
    let (implementing, ..) = round(&effects);
    let mut came = hands_in(&[("code", None)]);
    came.said.questions = vec![Question::Withdrawn {
        name: "which".to_string(),
        why: "Not mine.".to_string(),
    }];
    let effects = w.returns(implementing, came);
    let (_, _, _, given) = round(&effects);
    assert_eq!(
        given,
        Given::SentBack(vec![Unmet::QuestionTaken {
            name: "which".to_string()
        }])
    );
}

#[test]
fn a_question_the_cell_withdraws_lets_the_merge_begin() {
    let (mut w, effects) = researched(vec![asked("which", false)]);
    let (implementing, ..) = round(&effects);
    w.on(wrote("request", Outsider::Person, true, None));
    w.returns(implementing, hands_in(&[("code", None)]));
    let research = (w.work.records.started.iter())
        .rfind(|s| s.cell.as_str() == "research")
        .expect("the research runs again")
        .run;
    let mut came = Came::works(vec!["request".into()], Vec::new());
    came.said.questions = vec![Question::Withdrawn {
        name: "which".to_string(),
        why: "The request settles it.".to_string(),
    }];
    let effects = w.returns(research, came);
    let (_, cell, ..) = round(&effects);
    assert_eq!(cell, "merge");
    assert_eq!(w.asked().map(|(value, _)| value), Some("answered".to_string()));
}

#[test]
fn what_the_engine_asks_with_a_round_is_kept_as_the_round_ends_whatever_becomes_of_the_return() {
    let (mut w, run) = researching();
    let mut came = hands_in(&[("code", None)]);
    came.engine = vec![asked("protected_files", true)];
    let effects = w.returns(run, came);
    // The return is sent back; the engine's question stands, of the research, and is no word of the agent's.
    let (_, _, _, given) = round(&effects);
    assert!(matches!(given, Given::SentBack(_)), "{given:?}");
    let q = w.work.records.questions.last().expect("the question is kept");
    assert_eq!(
        (q.cell.as_str(), q.run, q.engine, q.waits),
        ("research", run, true, true)
    );
    assert_eq!(w.work.question(q), Some(&asked("protected_files", true)));
    assert_eq!(w.asked().map(|(value, _)| value), Some("waiting".to_string()));
    // Handed in rightly, the run finishes, and the work waits at the research for the owner's answer.
    w.returns(run, plans(Vec::new()));
    assert!(matches!(w.decision(), decide::Decision::Wait { at, .. } if at.as_str() == "research"));
}

#[test]
fn an_answer_to_the_engine_s_question_runs_the_last_cell_that_takes_an_answer_whichever_round_it_was_asked_with() {
    let (mut w, effects) = researched(Vec::new());
    let (implementing, cell, ..) = round(&effects);
    assert_eq!(cell, "implement");
    let mut came = hands_in(&[("code", None)]);
    let mut question = asked("protected_files", true);
    if let Question::Asked { recommends, .. } = &mut question {
        *recommends = None;
    }
    came.engine = vec![question];
    w.returns(implementing, came);
    let q = w.work.records.questions.last().expect("the question is kept");
    assert_eq!((q.cell.as_str(), q.engine), ("implement", true));
    let version = w.version("protected_files");
    let before = w.written("answered");
    let effects = w.answers("protected_files", version, 2);
    // The implementation has no trigger of an answer: the research, the last cell that has one, runs for it.
    assert_eq!(w.written("answered"), before + 1, "the research's answer is written");
    let (_, cell, ..) = round(&effects);
    assert_eq!(cell, "research");
}

#[test]
fn an_answer_to_no_question_to_no_version_or_with_no_such_option_is_refused() {
    let (mut w, _) = researched(vec![asked("which", false)]);
    let version = w.version("which");
    let answer = |question: &str, version: Seq, option: u64| Event::Answer {
        question: question.to_string(),
        version,
        option,
        why: None,
    };
    assert_eq!(
        w.refused(answer("none", version, 1)),
        Refused::NoSuchQuestion("none".to_string())
    );
    let elsewhere = Seq(version.0 + 50);
    assert_eq!(
        w.refused(answer("which", elsewhere, 1)),
        Refused::NoSuchVersion {
            question: "which".to_string(),
            version: elsewhere
        }
    );
    for option in [0, 3] {
        assert_eq!(
            w.refused(answer("which", version, option)),
            Refused::NotAnOption {
                question: "which".to_string(),
                option
            }
        );
    }
}

#[test]
fn what_stands_for_the_questions_and_the_answers_is_the_engine_s_alone_to_write() {
    let (mut w, _) = researched(vec![asked("which", true)]);
    for (block, value) in [("asked", Some("answered")), ("answered", None)] {
        assert_eq!(
            w.refused(wrote(block, Outsider::Outside, true, value)),
            Refused::NotTheirs {
                block: block.into(),
                by: Outsider::Outside
            }
        );
    }
}

/// A flow laid out one thing to a file: a cell `ask` that what people ask for and the owner's answers to it run, its
/// agent reading the record `answers` for an answer, which the engine writes in the words of its file; and the merge.
fn laid_out() -> Compiled {
    let flow = "name = \"laid\"\norder = [\"ask\", \"merge\"]\ndata = [\"request\", \"answers\"]\n\
                [limits]\nruns = 3\nreturns = 2\ncalls = 3\n";
    let ask = "title = \"Ask\"\nkind = \"agent\"\neffort = \"high\"\nline = \"main\"\ntask = \"Plan, and ask.\"\n\
               triggered_by = [\"on_request\", \"on_answer\"]\ncontext = [\"request\", \"answers\"]\n\
               outputs = [\"plan\"]\n[kinds]\nrequest = \"latest\"\nanswers = \"all\"\n";
    let merge = "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\ntriggered_by = [\"on_plan\"]\n";
    let files = [
        ("lines/main.toml", ""),
        ("blocks/plan.toml", "shown = true\n"),
        ("../data/request.toml", "from = \"owner\"\n"),
        (
            "../data/answers.toml",
            "from = \"owner\"\nshown = true\n[writes]\ntext = \"{question}, {version}: {option}, {does}.{why}\"\n\
             why = \" Why: {why}\"\n",
        ),
        (
            "../triggers/of_a_datum/changed.toml",
            "name = \"on_{datum}\"\nread = [\"{datum}\"]\n",
        ),
        (
            "../triggers/of_a_cell/on_answer.toml",
            "block = \"{cell}_answered\"\nfrom = \"answer\"\nread = [\"answers\"]\n",
        ),
    ];
    let source = Source {
        laid_out: true,
        flow: flow.to_string(),
        cells: [("ask", ask), ("merge", merge)]
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
        files: (files.iter())
            .map(|(path, text)| (path.to_string(), text.to_string()))
            .collect(),
        ..Default::default()
    };
    compile(&source, &has(&["answer"])).unwrap_or_else(|faults| panic!("the flow does not compile: {faults:#?}"))
}

#[test]
fn an_answer_to_the_latest_version_is_the_owners_entry_in_what_the_asking_cell_reads_for_it() {
    let mut w = W {
        flow: laid_out(),
        work: Work::default(),
        clock: START,
    };
    let (effects, _) = w.person_writes("request");
    let (run, cell, ..) = round(&effects);
    assert_eq!(cell, "ask");
    let mut came = Came::works(vec!["request".into()], vec![handed("plan", None)]);
    came.said.questions = vec![asked("which", false)];
    w.returns(run, came);
    let version = w.version("which");
    let entries = |w: &W| -> Vec<String> {
        (w.work.records.written.iter())
            .filter(|e| e.block.as_str() == "answers" && e.by == Outsider::Person)
            .map(|e| w.work.contents[&(BlockId::from("answers"), e.at)].clone())
            .collect()
    };
    // The option the work goes by: the entry is written, and the cell is not run.
    w.answers("which", version, 1);
    assert_eq!(
        entries(&w),
        [format!("which, {}: 1, the short way. Why: Option 1.", version.0)]
    );
    assert_eq!(w.written("ask_answered"), 0);
    // Another option: the entry, and the cell's block of an answer, which stops the merge's round under way.
    let effects = w.answers("which", version, 2);
    assert_eq!(entries(&w).len(), 2);
    assert_eq!(w.written("ask_answered"), 1);
    let [Effect::Stop { run: merging }] = effects.as_slice() else {
        panic!("the merge's round is not stopped: {effects:?}");
    };
    w.returns(*merging, Came::hands_in(Vec::new()));
    // Asked anew, an answer to the earlier version is kept in the owner's record, and is no entry of what the cell
    // reads.
    let again = (w.work.records.started.iter())
        .rfind(|s| s.cell.as_str() == "ask")
        .expect("the cell runs again")
        .run;
    let mut asked_anew = Came::works(vec!["ask_answered".into()], Vec::new());
    asked_anew.said.questions = vec![asked("which", false)];
    w.returns(again, asked_anew);
    assert!(w.version("which") > version);
    w.answers("which", version, 2);
    assert_eq!(entries(&w).len(), 2);
    assert_eq!(w.work.records.answers.len(), 3);
}
