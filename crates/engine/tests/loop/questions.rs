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
        reasons: Default::default(),
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
/// agent reading for an answer the record `q_and_a` of the questions and the answers, each entry of which the engine
/// writes in the words of its file; and the merge.
fn laid_out() -> Compiled {
    let flow = "name = \"laid\"\norder = [\"ask\", \"merge\"]\ndata = [\"request\", \"q_and_a\"]\n\
                [limits]\nruns = 3\nreturns = 2\ncalls = 3\n";
    let ask = "title = \"Ask\"\nkind = \"agent\"\neffort = \"high\"\nline = \"main\"\ntask = \"Plan, and ask.\"\n\
               triggered_by = [\"on_request\", \"on_answer\"]\ncontext = [\"request\", \"q_and_a\"]\n\
               outputs = [\"plan\"]\n[kinds]\nrequest = \"latest\"\nq_and_a = \"all\"\n";
    let merge = "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\ntriggered_by = [\"on_plan\"]\n";
    let files = [
        ("lines/main.toml", ""),
        ("blocks/plan.toml", "shown = true\n"),
        ("../data/request.toml", "from = \"owner\"\n"),
        (
            "../data/q_and_a.toml",
            "from = \"answer\"\nshown = true\n[writes]\ntext = \"{question}, {version}: {option}, {does}.{why}\"\n\
             why = \" Why: {why}\"\nasked = \"{question}, {version}: {ask}{options}{recommends}{waits}\"\n\
             asked_anew = \"{question}, {version} for {was}: {ask}{options}{recommends}{waits}\"\n\
             option = \" [{option}. {does}]\"\nrecommends = \" Recommends {option}.\"\nwaits = \" Waits.\"\n\
             goes_on = \" Goes on.\"\nwithdrawn = \"{question}, {version}: {was} withdrawn.{why}\"\n",
        ),
        (
            "../triggers/of_a_datum/changed.toml",
            "name = \"on_{datum}\"\nread = [\"{datum}\"]\n",
        ),
        (
            "../triggers/of_a_cell/on_answer.toml",
            "block = \"{cell}_answered\"\nfrom = \"answer\"\nread = [\"q_and_a\"]\n",
        ),
        (
            "../agents/waits.toml",
            include_str!("../../../../flow/agents/waits.toml"),
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

// Each version of a question and each answer to a question's latest version is an entry of the record of the questions
// and the answers the asking cell reads for its trigger of an answer, a version the engine writes; an answer to an
// earlier version is none, and the answer runs the asking cell alone, by its trigger's block.
#[test]
fn each_question_and_each_answer_to_its_latest_version_is_an_entry_of_the_record_the_asking_cell_reads() {
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
            .filter(|e| e.block.as_str() == "q_and_a" && e.by == Outsider::Outside)
            .map(|e| w.work.contents[&(BlockId::from("q_and_a"), e.at)].clone())
            .collect()
    };
    // The question is the record's first entry, with its options, the one it recommends and whether it waits.
    let question = format!(
        "which, {}: Which way for which? [1. the short way] [2. the long way] Recommends 1. Goes on.",
        version.0
    );
    assert_eq!(entries(&w), vec![question.clone()]);
    // The option the work goes by: the entry is written, and the cell is not run.
    w.answers("which", version, 1);
    assert_eq!(
        entries(&w),
        [
            question,
            format!("which, {}: 1, the short way. Why: Option 1.", version.0)
        ]
    );
    assert_eq!(w.written("ask_answered"), 0);
    // Another option: the entry, and the cell's block of an answer, which stops the merge's round under way.
    let effects = w.answers("which", version, 2);
    assert_eq!(entries(&w).len(), 3);
    assert_eq!(w.written("ask_answered"), 1);
    let [Effect::Stop { run: merging }] = effects.as_slice() else {
        panic!("the merge's round is not stopped: {effects:?}");
    };
    w.returns(*merging, Came::hands_in(Vec::new()));
    // Asked anew, the new version is an entry that says which it replaces; an answer to the earlier version is kept
    // in the owner's record, and is no entry of what the cell reads.
    let again = (w.work.records.started.iter())
        .rfind(|s| s.cell.as_str() == "ask")
        .expect("the cell runs again")
        .run;
    let mut asked_anew = Came::works(vec!["ask_answered".into()], Vec::new());
    asked_anew.said.questions = vec![asked("which", false)];
    w.returns(again, asked_anew);
    let anew = w.version("which");
    assert!(anew > version);
    assert_eq!(entries(&w).len(), 4);
    let replaces = format!("which, {} for {}: Which way for which?", anew.0, version.0);
    assert!(entries(&w)[3].starts_with(&replaces), "{:?}", entries(&w));
    w.answers("which", version, 2);
    assert_eq!(entries(&w).len(), 4);
    assert_eq!(w.work.records.answers.len(), 3);
}

/// The research and the implementation of the asking flow, the implementation run as well by what the check finds and
/// by the owner's answers to its own questions; the check, an act of the engine's that the work's head, main and the
/// questions' standing run, that alone says whether the work passed, and that is skipped where a person turns
/// `checking` off; and a merge that waits for its yes.
fn asking_then_checked() -> Compiled {
    let flow = r#"
        name = "asking_then_checked"
        order = ["research", "implement", "check", "merge"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.main]
        [blocks.request]
        people = true
        [blocks.answered]
        outside = "answer"
        [blocks.implement_answered]
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
        [blocks.head]
        outside = "head"
        [blocks.main]
        outside = "main"
        [blocks.checking]
        people = true
        values = ["on", "off"]
        default = "on"
        [blocks.plan]
        [blocks.code]
        shown = true
        [blocks.passed]
        values = ["yes", "no"]
        [blocks.failures]
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
        task = "Build the plan, and fix what the check finds."
        inputs = ["plan", "failures", "implement_answered"]
        outputs = ["code"]
    "#;
    let check = r#"
        title = "Check"
        kind = "engine"
        act = "check"
        when = { checking = "on" }
        inputs = ["head", "main", "asked"]
        outputs = ["passed", "failures"]
        describes = ["code"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        requires = { passed = "yes" }
        inputs = ["passed"]
        outputs = ["merged"]
    "#;
    let source = Source {
        flow: flow.to_string(),
        cells: [
            ("research", research),
            ("implement", implement),
            ("check", check),
            ("merge", merge),
        ]
        .iter()
        .map(|(id, text)| (id.to_string(), text.to_string()))
        .collect(),
        ..Default::default()
    };
    let has = Has {
        sources: ["answer", "asked", "head", "main"].map(str::to_string).into(),
        acts: ["check", "merge"].map(str::to_string).into(),
        ..Default::default()
    };
    compile(&source, &has).expect("the definition compiles")
}

// A question the owner has answered gets no answer to come, which is what would run the check again: a return that
// deals with what the check found, committing nothing and asking nothing, is sent back though such a question stands
// (decision an-act-is-answered-by-what-runs-it), where taken it would leave the work stalled at the merge.
#[test]
fn a_question_answered_does_not_answer_what_the_check_found() {
    let mut w = W {
        flow: asking_then_checked(),
        work: Work::default(),
        clock: START,
    };
    let (effects, _) = w.person_writes("request");
    let (research, ..) = round(&effects);
    let effects = w.returns(research, plans(vec![asked("which", false)]));
    let (implementing, cell, ..) = round(&effects);
    assert_eq!(cell, "implement");
    let version = w.version("which");
    w.answers("which", version, 1);
    // The implementation commits, which moves the work's head, and the check finds the change red.
    let mut built = hands_in(&[("code", None)]);
    built.read = vec![handed("head", None)];
    let effects = w.returns(implementing, built);
    let (checking, cell, ..) = round(&effects);
    assert_eq!(cell, "check");
    let effects = w.returns(checking, hands_in(&[("passed", Some("no")), ("failures", None)]));
    let (fixing, cell, number, _) = round(&effects);
    assert_eq!((cell.as_str(), number), ("implement", 1));
    // It deals with the failures and leaves the work's head where it was.
    let effects = w.returns(fixing, Came::works(vec!["failures".into()], Vec::new()));
    let unanswered = vec![Unmet::Unanswered {
        block: "failures".into(),
    }];
    assert_eq!(
        effects,
        [round_effect(fixing, "implement", 2, Given::SentBack(unanswered))]
    );
}

// The owner's answer to the implementation's own question, asked of what the check found, is new to the check, which
// it runs again: the return for the answer that commits nothing, as the owner had it, is taken, and the check runs.
#[test]
fn an_answer_the_check_has_not_run_on_answers_what_the_check_found() {
    let mut w = W {
        flow: asking_then_checked(),
        work: Work::default(),
        clock: START,
    };
    let (effects, _) = w.person_writes("request");
    let (research, ..) = round(&effects);
    let effects = w.returns(research, plans(Vec::new()));
    let (implementing, ..) = round(&effects);
    let mut built = hands_in(&[("code", None)]);
    built.read = vec![handed("head", None)];
    let effects = w.returns(implementing, built);
    let (checking, ..) = round(&effects);
    let effects = w.returns(checking, hands_in(&[("passed", Some("no")), ("failures", None)]));
    let (fixing, ..) = round(&effects);
    // It judges the failure not this change's to fix and asks the owner, leaving the failures for after the answer.
    let mut asks_the_owner = Came::works(Vec::new(), Vec::new());
    asks_the_owner.said.questions = vec![asked("not_this_change", true)];
    assert_eq!(w.returns(fixing, asks_the_owner), []);
    let version = w.version("not_this_change");
    let effects = w.answers("not_this_change", version, 1);
    let (answered, cell, ..) = round(&effects);
    assert_eq!(cell, "implement");
    let dealt_with = vec!["failures".into(), "implement_answered".into()];
    let effects = w.returns(answered, Came::works(dealt_with, Vec::new()));
    let (_, cell, number, _) = round(&effects);
    assert_eq!((cell.as_str(), number), ("check", 1));
}

/// A work on the flow whose check runs after an implementation.
fn checking() -> W {
    W {
        flow: asking_then_checked(),
        work: Work::default(),
        clock: START,
    }
}

/// A return of the implementation that hands in the code and commits, the work's head read as this commit.
fn building(commit: u64) -> Came {
    let mut built = hands_in(&[("code", None)]);
    built.read = vec![Handed {
        block: "head".into(),
        content: format!("commit {commit}"),
        matters: true,
        value: None,
    }];
    built
}

/// What the check hands in that finds the change red.
fn red() -> Came {
    hands_in(&[("passed", Some("no")), ("failures", None)])
}

/// A return of the implementation that deals with what the check found and commits nothing.
fn nothing() -> Came {
    Came::works(vec!["failures".into()], Vec::new())
}

/// The round that sends back the return of the implementation's run, as it answers what the check found with nothing
/// that runs the check again.
fn sent_back_unanswered(run: RunId) -> Vec<Effect> {
    let unanswered = vec![Unmet::Unanswered {
        block: "failures".into(),
    }];
    vec![round_effect(run, "implement", 2, Given::SentBack(unanswered))]
}

/// The work begun, its research handing in the plan, its implementation committing and asking these, and its check
/// finding the change red: the run of the implementation that what the check found begins, its round under way.
fn red_check(w: &mut W, asks: Vec<Question>) -> RunId {
    let (effects, _) = w.person_writes("request");
    let (research, ..) = round(&effects);
    let effects = w.returns(research, plans(Vec::new()));
    let (implementing, ..) = round(&effects);
    let mut built = building(1);
    built.said.questions = asks;
    let effects = w.returns(implementing, built);
    let (check, cell, ..) = round(&effects);
    assert_eq!(cell, "check");
    let effects = w.returns(check, red());
    let (fixing, cell, number, _) = round(&effects);
    assert_eq!((cell.as_str(), number), ("implement", 1));
    fixing
}

// The latest version of a question asked anew stands with no answer, though the owner answered the one before it: the
// answer to come runs the check again.
#[test]
fn a_question_asked_anew_and_not_yet_answered_answers_what_the_check_found() {
    let mut w = checking();
    let (effects, _) = w.person_writes("request");
    let (research, ..) = round(&effects);
    let effects = w.returns(research, plans(vec![asked("which", false)]));
    let (implementing, ..) = round(&effects);
    let first = w.version("which");
    w.answers("which", first, 1);
    // A new request runs the research again, which stops the implementation's round and asks the question anew.
    w.on(wrote("request", Outsider::Person, true, None));
    w.returns(implementing, Came::hands_in(Vec::new()));
    let research = (w.work.records.started.iter())
        .rfind(|s| s.cell.as_str() == "research")
        .expect("the research runs again")
        .run;
    let effects = w.returns(research, plans(vec![asked("which", false)]));
    let anew = w.version("which");
    assert!(anew > first);
    let (implementing, ..) = round(&effects);
    let effects = w.returns(implementing, building(1));
    let (check, cell, ..) = round(&effects);
    assert_eq!(cell, "check");
    let effects = w.returns(check, red());
    let (fixing, ..) = round(&effects);
    assert_eq!(w.returns(fixing, nothing()), []);
    let effects = w.answers("which", anew, 1);
    let (_, cell, ..) = round(&effects);
    assert_eq!(cell, "check");
}

// A return that withdraws one question of its cell's leaves what the check found answered by another that stands with
// no answer.
#[test]
fn a_question_withdrawn_beside_one_with_an_answer_to_come_leaves_what_the_check_found_answered() {
    let mut w = checking();
    let fixing = red_check(&mut w, vec![asked("one", false), asked("two", false)]);
    let mut withdraws = nothing();
    withdraws.said.questions = vec![Question::Withdrawn {
        name: "one".to_string(),
        why: "Settled.".to_string(),
    }];
    assert_eq!(w.returns(fixing, withdraws), []);
    let two = w.version("two");
    let effects = w.answers("two", two, 1);
    let (_, cell, ..) = round(&effects);
    assert_eq!(cell, "check");
}

// A second answer to the version the owner answered before the check ran leaves what stands for the questions as it
// was, which the check ran on: a return that deals with what the check found and commits nothing is sent back.
#[test]
fn a_second_answer_to_the_same_version_does_not_answer_what_the_check_found() {
    let mut w = checking();
    let (effects, _) = w.person_writes("request");
    let (research, ..) = round(&effects);
    let effects = w.returns(research, plans(Vec::new()));
    let (implementing, ..) = round(&effects);
    // The implementation asks the owner first, and waits for the answer.
    let mut asks = Came::works(vec!["plan".into()], Vec::new());
    asks.said.questions = vec![asked("which", true)];
    assert_eq!(w.returns(implementing, asks), []);
    let version = w.version("which");
    let effects = w.answers("which", version, 1);
    let (answered, cell, ..) = round(&effects);
    assert_eq!(cell, "implement");
    let effects = w.returns(answered, building(1));
    let (check, cell, ..) = round(&effects);
    assert_eq!(cell, "check");
    let effects = w.returns(check, red());
    let (fixing, ..) = round(&effects);
    let before = w.asked();
    w.answers("which", version, 2);
    assert_eq!(w.asked(), before, "what stands for the questions is as it was");
    assert_eq!(w.returns(fixing, nothing()), sent_back_unanswered(fixing));
}

// The engine's own question, asked as the round ends, has an answer to come, which runs the check again.
#[test]
fn the_engine_s_question_asked_as_the_round_ends_answers_what_the_check_found() {
    let mut w = checking();
    let fixing = red_check(&mut w, Vec::new());
    let mut asking = nothing();
    asking.engine = vec![asked("protected_files", true)];
    assert_eq!(w.returns(fixing, asking), []);
    assert!(
        matches!(w.decision(), decide::Decision::Wait { at, .. } if at.as_str() == "implement"),
        "{:?}",
        w.decision()
    );
}

// Main moved since the check ran is new to it, and runs it again whatever the return leaves.
#[test]
fn main_moved_since_the_check_ran_answers_what_the_check_found() {
    let mut w = checking();
    let fixing = red_check(&mut w, Vec::new());
    w.on(wrote("main", Outsider::Outside, true, None));
    let effects = w.returns(fixing, nothing());
    let (_, cell, ..) = round(&effects);
    assert_eq!(cell, "check");
}

// A head moved while the check judged the one before, by a commit no round made, is new to the check, and runs it
// again whatever the return leaves.
#[test]
fn a_head_moved_while_the_check_judged_answers_what_the_check_found() {
    let mut w = checking();
    let (effects, _) = w.person_writes("request");
    let (research, ..) = round(&effects);
    let effects = w.returns(research, plans(Vec::new()));
    let (implementing, ..) = round(&effects);
    let effects = w.returns(implementing, building(1));
    let (check, cell, ..) = round(&effects);
    assert_eq!(cell, "check");
    w.on(Event::Wrote {
        block: "head".into(),
        by: Outsider::Outside,
        content: "commit 2".to_string(),
        matters: true,
        value: None,
    });
    let effects = w.returns(check, red());
    let (fixing, cell, ..) = round(&effects);
    assert_eq!(cell, "implement");
    let effects = w.returns(fixing, nothing());
    let (_, cell, ..) = round(&effects);
    assert_eq!(cell, "check");
}

// What is new to an act its condition skips runs it on nothing: it answers no return, which is sent back.
#[test]
fn what_is_new_to_a_skipped_check_does_not_answer_what_it_found() {
    let mut w = checking();
    let fixing = red_check(&mut w, Vec::new());
    w.on(wrote("checking", Outsider::Person, true, Some("off")));
    w.on(wrote("main", Outsider::Outside, true, None));
    let new = decide::new_to(&w.flow.flow, &w.work.records, &"check".into());
    assert_eq!(new, Vec::<BlockId>::new());
    assert_eq!(w.returns(fixing, nothing()), sent_back_unanswered(fixing));
}
