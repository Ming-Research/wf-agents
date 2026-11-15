//! The judge of a play, on records built by hand, of the trial flow and of a flow whose check may wait for main: each
//! thing that happened given in turn to the engine's own keeping of the records (`engine::handle`), as the loop would
//! give it. For each kind of step a path names, a path the records hold is taken, and one wrong at that step left
//! there, the judge saying what the records hold there.

use decide::run::Failure;
use decide::{CallId, CellId, Decision, Outsider, Pauser, Room, Stalling, Unrun, Waited};
use engine::{Came, Effect, Event};
use flow::Compiled;
use service::judge::Added;
use service::script::items::Step;
use service::{Judged, Script, Shown};
use work::{Ask, Call, Handed, Question, Spent, Time, Way, Work};

use crate::{START, scoped, trial_flow};

/// A work whose records are built by hand.
struct Built {
    flow: Compiled,
    work: Work,
    now: Time,
    /// How many blocks people wrote for its item as it was added.
    writes: usize,
}

impl Built {
    /// The work of an item of the trial flow as the driver adds it: paused by the owner, its block of dependencies
    /// written with this value, and its request; then let on.
    fn added(dependencies: &str) -> Built {
        let mut built = Built::held(dependencies, true);
        built.take(Event::Resumed);
        built
    }

    /// The work of an item added as [`Built::added`] adds it, with its request where `request`, and never let on.
    fn held(dependencies: &str, request: bool) -> Built {
        let writes: &[(&str, Option<&str>)] = if request { &[("request", None)] } else { &[] };
        Built::on(trial_flow(), Some(dependencies), writes)
    }

    /// The work of an item of this flow as the driver adds it, never let on: paused by the owner, its block of
    /// dependencies written with this value where the flow has one, and these blocks people write for it, each with its
    /// value where it is a choice, each a thing that happened to it, and then looked at.
    fn on(flow: Compiled, dependencies: Option<&str>, writes: &[(&str, Option<&str>)]) -> Built {
        let mut built = Built {
            flow,
            work: Work::default(),
            now: START,
            writes: writes.len(),
        };
        let taking = Room {
            paused: true,
            slot: false,
        };
        let mut first = vec![Event::Paused {
            by: Pauser::Owner,
            why: None,
        }];
        if let Some(dependencies) = dependencies {
            first.push(Event::Wrote {
                block: "dependencies".into(),
                by: Outsider::Outside,
                content: format!("The items it depends on: {dependencies}."),
                matters: true,
                value: Some(dependencies.to_string()),
            });
        }
        for (block, value) in writes {
            first.push(Event::Wrote {
                block: (*block).into(),
                by: Outsider::Person,
                content: format!("The {block} as a person wrote it."),
                matters: true,
                value: value.map(str::to_string),
            });
        }
        for event in first {
            built.take_in(event, taking);
        }
        built.take(Event::Look);
        built
    }

    /// The engine takes what happened, a second after what it took before.
    fn take(&mut self, event: Event) -> Vec<Effect> {
        self.take_in(event, Room::FREE)
    }

    /// The engine takes what happened in the room the engine leaves the work.
    fn take_in(&mut self, event: Event, room: Room) -> Vec<Effect> {
        self.now = Time(self.now.0 + 1000);
        engine::handle_in(&self.flow, &mut self.work, self.now, event, room).expect("the engine takes it")
    }

    /// The run whose round is under way.
    fn under_way(&self) -> decide::RunId {
        let r = &self.work.records;
        (r.started.iter())
            .map(|s| s.run)
            .find(|run| {
                let began = r.rounds.iter().filter(|x| x.run == *run).count();
                began > r.returned.iter().filter(|x| x.run == *run).count()
            })
            .expect("a round is under way")
    }

    /// The round under way returns this.
    fn returns(&mut self, came: Came) -> Vec<Effect> {
        let run = self.under_way();
        self.take(Event::Returned {
            run,
            came,
            spent: Spent::default(),
        })
    }

    /// The round under way hands in these blocks, each with its value where it is a choice.
    fn hands_in(&mut self, blocks: &[(&str, Option<&str>)]) {
        let handed = (blocks.iter())
            .map(|(block, value)| Handed {
                block: (*block).into(),
                content: format!("The {block}."),
                matters: true,
                value: value.map(str::to_string),
            })
            .collect();
        self.returns(Came::hands_in(handed));
    }

    /// The round under way asks for this, and the call it makes.
    fn asks(&mut self, ask: Ask) -> CallId {
        let effects = self.returns(Came::asks(vec![ask]));
        (effects.iter())
            .find_map(|effect| match effect {
                Effect::Call { call, .. } => Some(*call),
                _ => None,
            })
            .expect("the call is made")
    }

    /// The call has its result.
    fn done(&mut self, call: CallId) {
        self.take(Event::Done {
            call,
            result: "Done.".to_string(),
        });
    }

    /// The round under way fails, of this kind, and what is to be done for it.
    fn fails(&mut self, failure: Failure) -> Vec<Effect> {
        let run = self.under_way();
        self.take(Event::Failed {
            run,
            why: "It failed.".to_string(),
            failure,
            resets: None,
            spent: Spent::default(),
        })
    }

    /// The work from its add to the merge, the plan asking the owner first and the build a tool, the review content.
    fn merged() -> Built {
        let mut built = Built::merging();
        built.hands_in(&[("merged", None)]);
        built
    }

    /// The work from its add to the merge's round under way.
    fn merging() -> Built {
        let mut built = Built::to_the_review();
        built.hands_in(&[("verdict", Some("yes"))]);
        built
    }

    /// The work from its add to the review's round under way.
    fn to_the_review() -> Built {
        let mut built = Built::to_the_build();
        let call = built.asks(Ask::Tool {
            name: "query".to_string(),
            input: "select number from items".to_string(),
        });
        built.done(call);
        built.hands_in(&[("code", None)]);
        built
    }

    /// The work from its add to the build's first round under way, the owner having answered the plan's question.
    fn to_the_build() -> Built {
        let mut built = Built::added("merged");
        built.asks_the_owner();
        built.answers("named");
        built.hands_in(&[("plan", None)]);
        built
    }

    /// The round under way asks the owner a question named `named`, which it waits for, and hands in nothing.
    fn asks_the_owner(&mut self) {
        let way = |does: &str| Way {
            does: does.to_string(),
            follows: format!("The greeting {does}."),
        };
        let question = Question::Asked {
            name: "named".to_string(),
            waits: true,
            recommends: Some(1),
            ask: "Should the greeting name the user?".to_string(),
            options: vec![way("names the user"), way("names nobody")],
            about: None,
            reasons: Default::default(),
        };
        let mut came = Came::hands_in(Vec::new());
        came.said.questions = vec![question];
        self.returns(came);
    }

    /// The owner answers the latest version of the question of this name with its first option.
    fn answers(&mut self, question: &str) {
        let version = (self.work.records.questions.iter())
            .filter(|q| q.name == question)
            .map(|q| q.at)
            .max()
            .expect("the question was asked");
        self.take(Event::Answer {
            question: question.to_string(),
            version,
            option: 1,
            why: None,
        });
    }

    /// What the engine's view shows of the work now, were nothing outside its records to hold it back.
    fn shown(&self) -> Shown {
        Shown {
            decision: decide::decide(&self.flow.flow, &self.work.records),
            turn: false,
            shared: false,
            everything: None,
        }
    }

    /// How the records stand against the path of these steps, as a script writes them, the engine showing what it
    /// shows of the work now.
    fn judged(&self, steps: &str) -> Judged {
        self.judged_as(steps, false, &self.shown())
    }

    /// How the records stand against the path of these steps, its item held where `held`, the engine showing this.
    fn judged_as(&self, steps: &str, held: bool, shown: &Shown) -> Judged {
        let added = Added {
            writes: self.writes,
            held,
        };
        service::judge::judge(&self.flow, &self.work, added, &path(steps), shown)
    }
}

/// The steps of a path, as a script writes them.
fn path(steps: &str) -> Vec<Step> {
    let text = format!("[[items]]\ntitle = \"Greet\"\nflow = \"trial\"\npath = [{steps}]\n");
    let script = Script::read(&text).expect("the path reads");
    (script.items[0].path.iter())
        .map(|step| step.step().expect("the step says one thing"))
        .collect()
}

/// What the path of a work merged on the trial flow says up to its build's first round.
const TO_THE_BUILD: &str = r#"{ runs = "plan" }, { waits = "plan", for = "answer", question = "named" }, { runs = "plan" }, { runs = "build" }"#;

fn left(step: usize, said: &str, records: &str) -> Judged {
    Judged::Left {
        step,
        said: Some(said.to_string()),
        records: records.to_string(),
    }
}

#[test]
fn a_path_the_records_hold_is_taken_and_one_of_another_cell_leaves_where_it_differs() {
    let built = Built::merged();
    let steps = format!(
        "{TO_THE_BUILD}, {{ waits = \"build\", for = \"tool\" }}, {{ runs = \"build\" }}, {{ runs = \"review\" }}, \
         {{ runs = \"merge\" }}, {{ ends = \"merged\" }}"
    );
    assert_eq!(built.judged(&steps), Judged::Took);
    let other = steps.replacen(
        r#"{ runs = "build" }, { runs = "review" }"#,
        r#"{ runs = "review" }"#,
        1,
    );
    assert_eq!(
        built.judged(&other),
        left(6, r#"{ runs = "review" }"#, r#"{ runs = "build" }"#)
    );
    let cancelled = steps.replace(r#"{ ends = "merged" }"#, r#"{ ends = "cancelled" }"#);
    assert_eq!(
        built.judged(&cancelled),
        left(9, r#"{ ends = "cancelled" }"#, r#"{ ends = "merged" }"#)
    );
}

#[test]
fn a_wait_for_a_tool_is_no_wait_for_an_answer() {
    let built = Built::to_the_review();
    let steps = format!("{TO_THE_BUILD}, {{ waits = \"build\", for = \"answer\" }}");
    assert_eq!(
        built.judged(&steps),
        left(
            5,
            r#"{ waits = "build", for = "answer" }"#,
            r#"{ waits = "build", for = "tool" }"#
        )
    );
}

#[test]
fn a_round_under_way_is_an_end_the_records_hold() {
    let built = Built::to_the_build();
    let under_way = format!("{TO_THE_BUILD}, {{ under_way = \"build\", run = 1, round = 1 }}");
    assert_eq!(built.judged(&under_way), Judged::Took);
    let other = format!("{TO_THE_BUILD}, {{ under_way = \"review\" }}");
    assert_eq!(
        built.judged(&other),
        Judged::Short {
            step: 5,
            said: r#"{ under_way = "review" }"#.to_string(),
            shown: Some(r#"{ under_way = "build", run = 1, round = 1 }"#.to_string()),
        }
    );
}

#[test]
fn a_return_sent_back_is_a_step_of_its_cell() {
    let mut built = Built::to_the_review();
    // The review writes a block that is not its own: what it returned is sent back, and its next round is given why.
    built.hands_in(&[("plan", None)]);
    built.hands_in(&[("verdict", Some("yes"))]);
    let tail = r#"{ waits = "build", for = "tool" }, { runs = "build" }, { runs = "review" }"#;
    let merge = r#"{ runs = "merge" }, { under_way = "merge" }"#;
    let sent_back = format!("{TO_THE_BUILD}, {tail}, {{ sent_back = \"review\" }}, {merge}");
    assert_eq!(built.judged(&sent_back), Judged::Took);
    let straight = format!("{TO_THE_BUILD}, {tail}, {merge}");
    assert_eq!(
        built.judged(&straight),
        left(8, r#"{ runs = "merge" }"#, r#"{ sent_back = "review" }"#)
    );
}

#[test]
fn a_round_that_fails_is_a_step_of_its_kind_and_the_engines_pause_another() {
    let mut built = Built::to_the_build();
    built.fails(Failure::Other);
    let failed = format!("{TO_THE_BUILD}, {{ fails = \"build\", failure = \"other\" }}, {{ paused = \"engine\" }}");
    assert_eq!(built.judged(&failed), Judged::Took);
    let passing = failed.replace("\"other\"", "\"passing\"");
    assert_eq!(
        built.judged(&passing),
        left(
            5,
            r#"{ fails = "build", failure = "passing" }"#,
            r#"{ fails = "build", failure = "other" }"#
        )
    );
    let by_owner = failed.replace(r#"{ paused = "engine" }"#, r#"{ paused = "owner" }"#);
    assert_eq!(
        built.judged(&by_owner),
        left(6, r#"{ paused = "owner" }"#, r#"{ paused = "engine" }"#)
    );
}

#[test]
fn a_round_that_fails_in_passing_waits_for_the_engines_retry_and_is_given_again() {
    let mut built = Built::to_the_build();
    let effects = built.fails(Failure::Passing);
    let call = (effects.iter())
        .find_map(|effect| match effect {
            Effect::Call {
                call,
                what: Call::Wait { .. },
                ..
            } => Some(*call),
            _ => None,
        })
        .expect("the engine waits before the round is tried again");
    let retry = format!(
        "{TO_THE_BUILD}, {{ fails = \"build\", failure = \"passing\" }}, {{ waits = \"build\", for = \"retry\" }}"
    );
    assert_eq!(built.judged(&retry), Judged::Took);
    let tool = retry.replace(r#"for = "retry""#, r#"for = "tool""#);
    assert_eq!(
        built.judged(&tool),
        left(
            6,
            r#"{ waits = "build", for = "tool" }"#,
            r#"{ waits = "build", for = "retry" }"#
        )
    );
    // A failure of no kind named is one of any kind.
    let any = retry.replace(r#", failure = "passing""#, "");
    assert_eq!(built.judged(&any), Judged::Took);
    built.done(call);
    let again = format!("{retry}, {{ under_way = \"build\", run = 1, round = 2 }}");
    assert_eq!(built.judged(&again), Judged::Took);
}

#[test]
fn an_answer_where_the_records_hold_the_owners_word_at_a_limit_leaves_the_path() {
    let mut built = Built::to_the_review();
    // The review writes a block not its own three times: the engine sent it back as often as its limit lets, and asks
    // the owner's word.
    for _ in 0..3 {
        built.hands_in(&[("plan", None)]);
    }
    let tail = r#"{ waits = "build", for = "tool" }, { runs = "build" }, { runs = "review" }, { sent_back = "review" }, { sent_back = "review" }"#;
    let limit = format!("{TO_THE_BUILD}, {tail}, {{ waits = \"review\", for = \"limit\" }}");
    assert_eq!(built.judged(&limit), Judged::Took);
    let answer = format!("{TO_THE_BUILD}, {tail}, {{ waits = \"review\", for = \"answer\" }}");
    assert_eq!(
        built.judged(&answer),
        left(
            10,
            r#"{ waits = "review", for = "answer" }"#,
            r#"{ waits = "review", for = "limit" }"#
        )
    );
    // The owner lets the review go on: the engine's call at its limit has its word, and what the run's last round
    // returned is sent back in its next.
    built.take(Event::LetGo {
        cell: CellId::from("review"),
    });
    let let_go = format!(
        "{limit}, {{ let_go = \"review\" }}, {{ sent_back = \"review\" }}, {{ under_way = \"review\", run = 1, round = 4 }}"
    );
    assert_eq!(built.judged(&let_go), Judged::Took);
}

#[test]
fn a_question_that_waits_is_a_wait_for_its_answer_while_it_stands() {
    let mut built = Built::added("merged");
    built.asks_the_owner();
    let asked = r#"{ runs = "plan" }, { waits = "plan", for = "answer", question = "named" }"#;
    assert_eq!(built.judged(asked), Judged::Took);
    let tool = r#"{ runs = "plan" }, { waits = "plan", for = "tool" }"#;
    assert_eq!(
        built.judged(tool),
        left(
            2,
            r#"{ waits = "plan", for = "tool" }"#,
            r#"{ waits = "plan", for = "answer", question = "named" }"#
        )
    );
    // Answered, the question holds the work no more, and the plan runs again for the answer.
    built.answers("named");
    assert_eq!(
        built.judged(asked),
        Judged::Left {
            step: 3,
            said: None,
            records: r#"{ runs = "plan" }"#.to_string(),
        }
    );
    assert_eq!(built.judged(&format!("{asked}, {{ runs = \"plan\" }}")), Judged::Took);
}

/// A research that runs while the owner has not said that the deep research is needed, which then alone writes the
/// plan, the research no longer applying; and the merge, which requires every question to be answered.
fn deepening() -> Compiled {
    let flow = r#"
        name = "deepening"
        order = ["research", "deep", "merge"]
        [limits]
        again = 2
        [lines.main]
        [blocks.request]
        people = true
        [blocks.answered]
        outside = "answer"
        [blocks.asked]
        values = ["waiting", "answered"]
        default = "answered"
        outside = "asked"
        [blocks.asked.writes]
        answered = "Every question has the owner's answer."
        waiting = "Waits for the owner's answer to {questions}."
        between = ", "
        [blocks.deep_needed]
        values = ["yes", "no"]
        default = "no"
        people = true
        [blocks.plan]
        [blocks.merged]
        shown = true
    "#;
    let agent = |when: &str, inputs: &str, outputs: &str| {
        format!(
            "title = \"A cell\"\nkind = \"agent\"\neffort = \"high\"\nline = \"main\"\ntask = \"Do the cell's part.\"\n\
             when = {{ deep_needed = \"{when}\" }}\ninputs = [{inputs}]\noutputs = [{outputs}]\n"
        )
    };
    let merge = "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nrequires = { asked = \"answered\" }\n\
                 inputs = [\"plan\"]\noutputs = [\"merged\"]\n";
    let cells = [
        ("research", agent("no", "\"request\", \"answered\"", "\"plan\"")),
        ("deep", agent("yes", "\"request\"", "\"plan\"")),
        ("merge", merge.to_string()),
    ];
    let source = flow::Source {
        flow: flow.to_string(),
        cells: cells.into_iter().map(|(id, text)| (id.to_string(), text)).collect(),
        ..Default::default()
    };
    let has = flow::Has {
        sources: ["answer".to_string(), "asked".to_string()].into(),
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the definition compiles")
}

// A question that waits, of a cell skipped once it asked it, holds nothing from then on (decision
// a-wait-of-a-skipped-cell-is-put-away): the research waits for the answer until the owner says that the deep research
// is needed, and the work then goes on, and waits at the merge for the owner's answer by the block that says how the
// questions stand, a key of its preconditions the owner's answer alone makes hold; a path that says the research still
// holds the work leaves where the deep research runs. A research's own return cannot skip it so while it still waits:
// that return is sent back (decision a-return-that-waits-keeps-its-cell).
#[test]
fn a_question_of_a_cell_skipped_once_it_asked_holds_nothing_and_the_merge_waits_for_the_answer() {
    let mut built = Built::on(deepening(), None, &[("request", None)]);
    built.take(Event::Resumed);
    built.asks_the_owner();
    built.take(Event::Wrote {
        block: "deep_needed".into(),
        by: Outsider::Person,
        content: "The deep research is needed.".to_string(),
        matters: true,
        value: Some("yes".to_string()),
    });
    built.hands_in(&[("plan", None)]);
    assert!(
        matches!(&built.shown().decision, Decision::Wait { at, on }
            if at.as_str() == "merge" && matches!(on[..], [Waited::Precondition { .. }])),
        "{:?}",
        built.shown().decision
    );
    let asked = r#"{ runs = "research" }, { waits = "research", for = "answer", question = "named" }"#;
    let waits = format!(r#"{asked}, {{ runs = "deep" }}, {{ waits = "merge", for = "precondition", key = "asked" }}"#);
    assert_eq!(built.judged(&waits), Judged::Took);
    // A path that says the research still holds the work there leaves it where the deep research runs.
    assert_eq!(
        built.judged(asked),
        Judged::Left {
            step: 3,
            said: None,
            records: r#"{ runs = "deep" }"#.to_string(),
        }
    );
    built.answers("named");
    assert_eq!(built.judged(&format!("{waits}, {{ runs = \"merge\" }}")), Judged::Took);
}

/// A writer of the text a request asks for; a check of the text, which reads main as well and alone writes whether it
/// passed; and a merge, the engine's own cell, that waits for the check to say yes.
fn checked() -> Compiled {
    let flow = r#"
        name = "checked"
        order = ["write", "check", "merge"]
        [limits]
        again = 2
        [lines.main]
        [blocks.request]
        people = true
        [blocks.main]
        outside = "main"
        [blocks.text]
        [blocks.checked]
        values = ["yes", "no"]
        [blocks.merged]
        shown = true
    "#;
    let write = r#"
        title = "Write"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Write the text the request asks for."
        inputs = ["request"]
        outputs = ["text"]
    "#;
    let check = r#"
        title = "Check"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Check the text."
        inputs = ["text", "main"]
        outputs = ["checked"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        requires = { checked = "yes" }
        inputs = ["checked"]
        outputs = ["merged"]
    "#;
    let source = flow::Source {
        flow: flow.to_string(),
        cells: [("write", write), ("check", check), ("merge", merge)]
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
        ..Default::default()
    };
    let has = flow::Has {
        sources: ["main".to_string()].into(),
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the definition compiles")
}

#[test]
fn a_check_whose_run_waits_for_main_has_the_work_wait_for_it_at_the_precondition_it_holds() {
    let mut built = Built::on(checked(), None, &[("request", None)]);
    built.take(Event::Resumed);
    built.hands_in(&[("text", None)]);
    // The check says no, and that what it wrote waits for main: the merge's precondition, which only the check writes,
    // waits for main, with the block the check wrote, and not for the owner.
    let no = Handed {
        block: "checked".into(),
        content: "It does not pass, and what fails also fails on main.".to_string(),
        matters: true,
        value: Some("no".to_string()),
    };
    built.returns(Came {
        waits_for: Some("main".into()),
        ..Came::hands_in(vec![no])
    });
    assert!(
        matches!(&built.shown().decision, Decision::Wait { at, on }
            if at.as_str() == "merge" && matches!(on[..], [Waited::Outside { .. }])),
        "{:?}",
        built.shown().decision
    );
    let outside = r#"{ runs = "write" }, { runs = "check" }, { waits = "merge", for = "outside", block = "checked" }"#;
    assert_eq!(built.judged(outside), Judged::Took);
    let precondition = r#"{ runs = "write" }, { runs = "check" }, { waits = "merge", for = "precondition" }"#;
    assert_eq!(
        built.judged(precondition),
        left(
            3,
            r#"{ waits = "merge", for = "precondition" }"#,
            r#"{ waits = "merge", for = "outside", block = "checked" }"#
        )
    );
    // Main moving runs the check again.
    built.take(Event::Wrote {
        block: "main".into(),
        by: Outsider::Outside,
        content: "The head of main.".to_string(),
        matters: true,
        value: None,
    });
    let again = format!("{outside}, {{ runs = \"check\" }}");
    assert_eq!(built.judged(&again), Judged::Took);
}

#[test]
fn a_cell_at_its_limit_of_a_sender_waits_where_the_work_rests_until_the_owner_lets_it_go() {
    let mut built = Built::added("merged");
    built.hands_in(&[("plan", None)]);
    built.hands_in(&[("code", None)]);
    // The review sends the build back three times: the build has run twice for what it wrote, its limit.
    for n in 0..3 {
        built.hands_in(&[("opinions", None), ("verdict", Some("no"))]);
        if n < 2 {
            built.hands_in(&[("code", None)]);
        }
    }
    let round = r#"{ runs = "build" }, { runs = "review" }"#;
    let to_the_limit = format!("{{ runs = \"plan\" }}, {round}, {round}, {round}");
    let limit = format!("{to_the_limit}, {{ waits = \"build\", for = \"limit\", by = \"review\" }}");
    assert_eq!(built.judged(&limit), Judged::Took);
    let precondition = format!("{to_the_limit}, {{ waits = \"build\", for = \"precondition\" }}");
    assert_eq!(
        built.judged(&precondition),
        left(
            8,
            r#"{ waits = "build", for = "precondition" }"#,
            r#"{ waits = "build", for = "limit", by = "review" }"#
        )
    );
    built.take(Event::LetGo {
        cell: CellId::from("build"),
    });
    let let_go = format!("{limit}, {{ let_go = \"build\" }}, {{ runs = \"build\" }}");
    assert_eq!(built.judged(&let_go), Judged::Took);
    let other = let_go.replace(r#"{ let_go = "build" }"#, r#"{ let_go = "review" }"#);
    assert_eq!(
        built.judged(&other),
        left(9, r#"{ let_go = "review" }"#, r#"{ let_go = "build" }"#)
    );
}

#[test]
fn a_limit_where_the_records_hold_a_precondition_leaves_the_path() {
    // An item whose dependency has not merged: the plan's precondition does not hold, and only the outside makes it.
    let built = Built::added("waiting");
    let precondition = r#"{ waits = "plan", for = "precondition", key = "ready" }"#;
    assert_eq!(built.judged(precondition), Judged::Took);
    let limit = r#"{ waits = "plan", for = "limit", by = "build" }"#;
    assert_eq!(built.judged(limit), left(1, limit, precondition));
}

#[test]
fn the_owners_pause_and_letting_on_are_steps_and_the_items_start_is_none() {
    let mut built = Built::to_the_build();
    built.take(Event::Paused {
        by: Pauser::Owner,
        why: None,
    });
    built.hands_in(&[("code", None)]);
    built.take(Event::Resumed);
    let paused = format!(
        "{TO_THE_BUILD}, {{ paused = \"owner\" }}, {{ let_on = true }}, {{ runs = \"review\" }}, \
         {{ under_way = \"review\" }}"
    );
    assert_eq!(built.judged(&paused), Judged::Took);
    let by_engine = paused.replace(r#"{ paused = "owner" }"#, r#"{ paused = "engine" }"#);
    assert_eq!(
        built.judged(&by_engine),
        left(5, r#"{ paused = "engine" }"#, r#"{ paused = "owner" }"#)
    );
    let never_let_on = paused.replace(r#"{ let_on = true }, "#, "");
    assert_eq!(
        built.judged(&never_let_on),
        left(6, r#"{ runs = "review" }"#, "{ let_on = true }")
    );
    // An item never let on has no step for the pause it was added with.
    let held = Built::held("merged", true);
    let shown = held.shown();
    assert_eq!(
        held.judged_as(r#"{ paused = "owner" }"#, true, &shown),
        Judged::Short {
            step: 1,
            said: r#"{ paused = "owner" }"#.to_string(),
            shown: None,
        }
    );
}

#[test]
fn the_pause_of_everything_is_an_end_by_who_set_it() {
    let built = Built::to_the_build();
    let shown = Shown {
        everything: Some(Pauser::Owner),
        ..built.shown()
    };
    let owner = format!("{TO_THE_BUILD}, {{ everything = \"owner\" }}");
    assert_eq!(built.judged_as(&owner, false, &shown), Judged::Took);
    let engine = format!("{TO_THE_BUILD}, {{ everything = \"engine\" }}");
    assert!(
        matches!(built.judged_as(&engine, false, &shown), Judged::Short { step: 5, shown: Some(s), .. } if s.contains(r#"{ everything = "owner" }"#))
    );
}

#[test]
fn a_cancel_ends_the_path_while_it_stands() {
    let mut built = Built::to_the_build();
    built.take(Event::Cancelled { why: None });
    let cancelled = format!("{TO_THE_BUILD}, {{ ends = \"cancelled\" }}");
    assert_eq!(built.judged(&cancelled), Judged::Took);
    let merged = format!("{TO_THE_BUILD}, {{ ends = \"merged\" }}");
    assert_eq!(
        built.judged(&merged),
        left(5, r#"{ ends = "merged" }"#, r#"{ ends = "cancelled" }"#)
    );
    // Reopened, the work is paused by the owner, as a work added is, and the cancel stands no more.
    built.take(Event::Reopened {
        why: "It is needed after all.".to_string(),
    });
    let reopened = format!("{cancelled}, {{ reopened = true }}, {{ paused = \"owner\" }}");
    assert_eq!(built.judged(&reopened), Judged::Took);
    assert!(matches!(
        built.judged(&cancelled),
        Judged::Left {
            step: 6,
            said: None,
            ..
        }
    ));
}

#[test]
fn a_stall_is_read_from_the_view_and_borne_out_by_the_records() {
    // A work with no request: nothing runs its cells, and only the review writes the verdict the merge waits for.
    let built = Built::held("merged", false);
    let shown = built.shown();
    assert!(
        matches!(&shown.decision, Decision::Stalled { at, .. } if at.as_str() == "merge"),
        "{:?}",
        shown.decision
    );
    assert_eq!(built.judged_as(r#"{ stalled = "merge" }"#, true, &shown), Judged::Took);
    assert!(matches!(
        built.judged_as(r#"{ stalled = "review" }"#, true, &shown),
        Judged::Short { step: 1, .. }
    ));
    // A view that showed a stall where the plan has a request to run for would not be borne out by the records.
    let requested = Built::held("merged", true);
    let stalled = Shown {
        decision: Decision::Stalled {
            at: CellId::from("merge"),
            on: Vec::new(),
            by: vec![Stalling {
                cell: CellId::from("review"),
                why: Unrun::NothingNew,
            }],
        },
        ..requested.shown()
    };
    assert!(matches!(
        requested.judged_as(r#"{ stalled = "merge" }"#, true, &stalled),
        Judged::Left { step: 1, records, .. } if records.contains("plan")
    ));
}

/// A writer of a text and of whether it is ready, a gate that requires it ready, and a merge of the text and of what
/// the gate noted, the engine's own cell.
fn gated() -> Compiled {
    let flow = r#"
        name = "gated"
        order = ["write", "gate", "merge"]
        [limits]
        again = 2
        [lines.main]
        [blocks.request]
        people = true
        [blocks.text]
        [blocks.ready]
        values = ["yes", "no"]
        [blocks.notes]
        [blocks.merged]
        shown = true
    "#;
    let write = r#"
        title = "Write"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Write the text the request asks for, and say whether it is ready."
        inputs = ["request"]
        outputs = ["text", "ready"]
    "#;
    let gate = r#"
        title = "Gate"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Note what the text lacks."
        requires = { ready = "yes" }
        inputs = ["text"]
        outputs = ["notes"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        inputs = ["text", "notes"]
        outputs = ["merged"]
    "#;
    let source = flow::Source {
        flow: flow.to_string(),
        cells: [("write", write), ("gate", gate), ("merge", merge)]
            .iter()
            .map(|(id, text)| (id.to_string(), text.to_string()))
            .collect(),
        ..Default::default()
    };
    let has = flow::Has {
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the definition compiles")
}

#[test]
fn a_stall_at_a_middle_cell_holds_what_comes_after_it() {
    // The writer says the text is not ready, and nothing runs it again: the gate holds the work, though the text is new
    // to the merge after it.
    let mut built = Built::on(gated(), None, &[("request", None)]);
    built.take(Event::Resumed);
    built.hands_in(&[("text", None), ("ready", Some("no"))]);
    let shown = built.shown();
    assert!(
        matches!(&shown.decision, Decision::Stalled { at, .. } if at.as_str() == "gate"),
        "{:?}",
        shown.decision
    );
    let stalled = r#"{ runs = "write" }, { stalled = "gate" }"#;
    assert_eq!(built.judged(stalled), Judged::Took);
    let at_the_merge = r#"{ runs = "write" }, { stalled = "merge" }"#;
    assert!(matches!(built.judged(at_the_merge), Judged::Short { step: 2, .. }));
}

#[test]
fn what_an_item_is_added_with_is_written_before_the_work_rests() {
    // Between its request and its scope the item is still being added: the plan never waited for the scope.
    let mut built = Built::on(scoped(), None, &[("request", None), ("scope", Some("narrow"))]);
    built.take(Event::Resumed);
    assert_eq!(
        built.judged(r#"{ runs = "plan" }, { under_way = "plan" }"#),
        Judged::Took
    );
}

#[test]
fn a_slot_the_turn_and_the_shared_wait_are_ends_the_view_shows() {
    let mut built = Built::held("merged", true);
    let no_slot = Room {
        paused: false,
        slot: false,
    };
    built.take_in(Event::Resumed, no_slot);
    let waiting = Shown {
        decision: decide::decide_in(&built.flow.flow, &built.work.records, no_slot),
        ..built.shown()
    };
    assert_eq!(
        waiting.decision,
        Decision::Wait {
            at: CellId::from("plan"),
            on: vec![Waited::Slot]
        }
    );
    let slot = r#"{ waits = "plan", for = "slot" }"#;
    assert_eq!(built.judged_as(slot, false, &waiting), Judged::Took);
    let shared = r#"{ waits = "plan", for = "shared" }"#;
    assert!(matches!(
        built.judged_as(shared, false, &waiting),
        Judged::Short { step: 1, shown: Some(s), .. } if s == slot
    ));
    let held = Shown {
        shared: true,
        ..built.shown()
    };
    assert_eq!(built.judged_as(shared, false, &held), Judged::Took);
    // The merge's round under way, waiting for the turn to check and merge that another work holds.
    let merging = Built::merging();
    let turn = Shown {
        turn: true,
        ..merging.shown()
    };
    let steps = format!(
        "{TO_THE_BUILD}, {{ waits = \"build\", for = \"tool\" }}, {{ runs = \"build\" }}, {{ runs = \"review\" }}, \
         {{ runs = \"merge\" }}, {{ waits = \"merge\", for = \"turn\" }}"
    );
    assert_eq!(merging.judged_as(&steps, false, &turn), Judged::Took);
    assert!(matches!(
        merging.judged_as(&steps, false, &merging.shown()),
        Judged::Short { step: 9, .. }
    ));
}

#[test]
fn a_path_that_stops_short_or_goes_on_past_its_end_is_not_taken() {
    let built = Built::to_the_build();
    let longer = format!("{TO_THE_BUILD}, {{ waits = \"build\", for = \"tool\" }}");
    assert_eq!(
        built.judged(&longer),
        Judged::Short {
            step: 5,
            said: r#"{ waits = "build", for = "tool" }"#.to_string(),
            shown: None,
        }
    );
    let shorter = r#"{ runs = "plan" }, { waits = "plan", for = "answer" }, { runs = "plan" }"#;
    assert_eq!(
        built.judged(shorter),
        Judged::Left {
            step: 4,
            said: None,
            records: r#"{ runs = "build" }"#.to_string(),
        }
    );
}
