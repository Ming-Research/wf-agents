//! What runs a cell, by its data (design document `data-and-context`, 5 to 12, 17): the data it watches, a version of
//! which new to it runs it, and those it only reads, each given to it in its kind; what every agent cell of the flow
//! watches and reads, said once in `flow.toml`; the data each cell or each line has its own of, beside the flows; the
//! cell the engine asks in (34); and what the item's card shows. A cell written so is compiled to what a cell written
//! before is: each datum it watches is a trigger of the datum's name, which its agent reads. A flow may hold cells of
//! this form beside cells of the forms before it, each file in one.

use decide::{BlockId, CellId};
use flow::FaultKind::*;
use flow::{CellSpec, Compiled, FLOW_FILE, Has, InputKind, Source, Trigger};

use crate::support::{at, compiled, edit, faults, faults_with, has, laid_out, set};

/// What the planning cell of the twin laid out one thing to a file takes, in the form of triggers.
const PLAN_TAKES: &str = "triggered_by = [\"on_request\", \"on_opinions\"]\ncontext = [\"request\", \"opinions\"]";

/// What the answering cell of the twin takes, in the form of triggers.
const ANSWER_TAKES: &str = "triggered_by = [\"on_comments\", \"on_plan\"]\ncontext = [\"comments\", \"plan\"]";

fn cell<'a>(compiled: &'a Compiled, id: &str) -> &'a CellSpec {
    (compiled.cells.iter()).find(|c| c.id.as_str() == id).expect("the cell")
}

fn trigger(name: &str, read: &[&str], does: Option<&str>) -> Trigger {
    Trigger {
        name: name.to_string(),
        what: None,
        block: name.into(),
        group: Vec::new(),
        read: read.iter().map(|&b| b.into()).collect(),
        detail: None,
        does: does.map(str::to_string),
    }
}

/// What the cell is given, each block with its kind, in order.
fn given(spec: &CellSpec) -> Vec<(String, InputKind)> {
    spec.given.iter().map(|g| (g.block.to_string(), g.kind)).collect()
}

/// What runs the cell, in order.
fn runs_on(spec: &CellSpec) -> Vec<&str> {
    spec.runs_on.iter().map(|r| r.block.as_str()).collect()
}

/// The twin laid out one thing to a file, its planning cell taking this in place of its triggers and context.
fn plan_takes(takes: &str) -> Source {
    edit(laid_out(), "cells/plan.toml", PLAN_TAKES, takes)
}

/// What the engine has for the twin, and besides the sources of the owner's answers and words and of a line's session.
fn has_all() -> Has {
    let mut has = has();
    for source in ["answer", "words", "session"] {
        has.sources.insert(source.to_string());
    }
    has
}

/// The definition compiled against [`has_all`].
fn compiled_all(source: &Source) -> Compiled {
    flow::compile(source, &has_all()).unwrap_or_else(|faults| panic!("the definition should compile: {faults:#?}"))
}

/// The twin with the data each cell has its own of, the questions it asks the owner and the owner's words to it, and
/// its answering cell watching its own of both, which it writes.
fn answer_asks() -> Source {
    let source = set(
        laid_out(),
        "../data/of_a_cell/questions.toml",
        "name = \"{cell}_questions\"\nfrom = \"answer\"\nshown = true\nwhat = \"What {cell} asks the owner.\"\n",
    );
    let source = set(
        source,
        "../data/of_a_cell/words.toml",
        "name = \"{cell}_words\"\nfrom = \"words\"\nshown = true\n",
    );
    let takes = "watches = [\"comments\", \"plan\", \"answer_questions\", \"answer_words\"]";
    let source = edit(source, "cells/answer.toml", ANSWER_TAKES, takes);
    let source = edit(
        source,
        "cells/answer.toml",
        "outputs = [\"reply\"]",
        "outputs = [\"reply\", \"answer_questions\", \"answer_words\"]",
    );
    edit(
        source,
        "cells/answer.toml",
        "plan = \"last_and_latest\"",
        "plan = \"last_and_latest\"\nanswer_questions = \"all\"\nanswer_words = \"all\"",
    )
}

#[test]
fn a_cell_runs_on_what_it_watches_and_is_given_what_it_reads_and_watches_each_in_its_kind() {
    let compiled = compiled(&plan_takes("watches = [\"request\"]\nreads = [\"opinions\"]"));
    let plan = cell(&compiled, "plan");
    assert_eq!(plan.triggers, [trigger("request", &["request"], None)]);
    assert_eq!(runs_on(plan), ["request"]);
    let walked = &compiled.flow.cells()[0];
    assert_eq!(walked.runs_on, [BlockId::from("request")]);
    assert_eq!(
        given(plan),
        [
            ("opinions".to_string(), InputKind::LastAndLatest),
            ("request".to_string(), InputKind::LastAndLatest)
        ]
    );
    // What it reads, it reads: the opinions have a reader.
    let opinions = &compiled.blocks[&BlockId::from("opinions")];
    assert!(opinions.readers.cells.contains(&CellId::from("plan")));
}

#[test]
fn what_a_cell_does_when_a_datum_it_watches_is_new_is_said_by_the_datum_s_name() {
    let mut source = plan_takes("watches = [\"request\"]\nreads = [\"opinions\"]");
    let plan = source.cells.get_mut("plan").expect("the planning cell");
    plan.push_str("\n[do]\nrequest = \"Plan again from what changed.\"\n");
    let compiled = compiled(&source);
    let plan = cell(&compiled, "plan");
    assert_eq!(
        plan.triggers,
        [trigger("request", &["request"], Some("Plan again from what changed."))]
    );
    assert_eq!(plan.runs_on[0].why.as_deref(), Some("Plan again from what changed."));
}

#[test]
fn a_group_watched_runs_the_cell_on_each_of_its_blocks_given_each_in_its_kind() {
    let mut source = plan_takes("watches = [\"asked_for\"]");
    source
        .flow
        .push_str("\n[groups]\nasked_for = [\"request\", \"opinions\"]\n");
    let compiled = compiled(&source);
    let plan = cell(&compiled, "plan");
    let group = Trigger {
        name: "asked_for".to_string(),
        what: None,
        block: "asked_for".into(),
        group: vec!["request".into(), "opinions".into()],
        read: vec!["request".into(), "opinions".into()],
        detail: None,
        does: None,
    };
    assert_eq!(plan.triggers, [group]);
    assert_eq!(runs_on(plan), ["request", "opinions"]);
    assert_eq!(
        given(plan),
        [
            ("request".to_string(), InputKind::LastAndLatest),
            ("opinions".to_string(), InputKind::LastAndLatest)
        ]
    );
}

#[test]
fn a_cell_names_what_it_takes_in_one_form() {
    let source = plan_takes("watches = [\"request\", \"opinions\"]\ncontext = [\"opinions\"]");
    assert_eq!(faults(&source), [at("cells/plan.toml", "context", TwoForms)]);
}

#[test]
fn a_cell_watches_and_reads_only_declared_data_each_once() {
    let unknown = plan_takes("watches = [\"request\", \"nowhere\"]\nreads = [\"opinions\"]");
    let what = NoSuchBlock {
        block: "nowhere".to_string(),
    };
    assert_eq!(faults(&unknown), [at("cells/plan.toml", "watches[1]", what)]);
    let twice = plan_takes("watches = [\"request\", \"request\"]\nreads = [\"opinions\"]");
    let what = Twice {
        name: "request".to_string(),
    };
    assert_eq!(faults(&twice), [at("cells/plan.toml", "watches[1]", what)]);
    // A datum it watches it is given already: reading it as well names it twice.
    let both = plan_takes("watches = [\"request\"]\nreads = [\"opinions\", \"request\"]");
    let what = Twice {
        name: "request".to_string(),
    };
    assert_eq!(faults(&both), [at("cells/plan.toml", "reads[1]", what)]);
}

#[test]
fn an_agent_cell_that_watches_nothing_is_refused() {
    let source = plan_takes("reads = [\"request\", \"opinions\"]");
    assert_eq!(faults(&source), [at("cells/plan.toml", "watches", RunByNothing)]);
}

#[test]
fn each_datum_an_agent_cell_is_given_has_its_kind_and_each_kind_is_of_a_datum_it_is_given() {
    let lacking = plan_takes("watches = [\"request\", \"design\"]\nreads = [\"opinions\"]");
    let what = NoKind {
        block: "design".to_string(),
    };
    assert_eq!(faults(&lacking), [at("cells/plan.toml", "watches[1]", what)]);
    let source = plan_takes("watches = [\"request\"]\nreads = [\"opinions\"]");
    let extra = edit(
        source,
        "cells/plan.toml",
        "opinions = \"last_and_latest\"",
        "opinions = \"last_and_latest\"\ndesign = \"latest\"",
    );
    let what = NotGiven {
        block: "design".to_string(),
    };
    assert_eq!(faults(&extra), [at("cells/plan.toml", "kinds.design", what)]);
}

#[test]
fn what_a_cell_does_is_said_of_a_datum_it_watches() {
    let mut source = plan_takes("watches = [\"request\"]\nreads = [\"opinions\"]");
    let plan = source.cells.get_mut("plan").expect("the planning cell");
    plan.push_str("\n[do]\nopinions = \"Read the opinions.\"\n");
    let what = NotRunOn {
        block: "opinions".to_string(),
    };
    assert_eq!(faults(&source), [at("cells/plan.toml", "do.opinions", what)]);
}

#[test]
fn an_engine_cell_watches_and_is_given_nothing_nor_told_what_to_do() {
    let mut source = edit(
        laid_out(),
        "cells/check.toml",
        "triggered_by = [\"on_main\", \"on_design\"]",
        "watches = [\"design\"]\nreads = [\"request\"]",
    );
    let check = source.cells.get_mut("check").expect("the check");
    check.push_str("\n[do]\ndesign = \"Check it.\"\n");
    let given = EngineIsGiven {
        block: "request".to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at("cells/check.toml", "reads[0]", given),
            at("cells/check.toml", "do.design", EngineHasTask)
        ]
    );
}

#[test]
fn a_block_that_stands_only_for_a_trigger_is_not_watched() {
    let source = plan_takes("watches = [\"request\", \"main\"]\nreads = [\"opinions\"]");
    let what = TriggerOnly {
        block: "main".to_string(),
    };
    assert_eq!(faults(&source), [at("cells/plan.toml", "watches[1]", what)]);
}

#[test]
fn a_datum_watched_that_no_writer_can_write_where_the_cell_runs_is_refused() {
    // The check runs only where the judge chose no, the design is written only where it chose yes, and the judge, the
    // one writer of the choice, runs only where it has chosen no.
    let source = edit(
        laid_out(),
        "cells/check.toml",
        "triggered_by = [\"on_main\", \"on_design\"]",
        "when = { needs_design = \"no\" }\nwatches = [\"design\"]",
    );
    let source = edit(
        source,
        "cells/judge.toml",
        "triggered_by = [\"on_plan\"]",
        "when = { needs_design = \"no\" }\ntriggered_by = [\"on_plan\"]",
    );
    let what = TriggerNeverRuns {
        trigger: "design".to_string(),
        block: "design".to_string(),
        needs: "needs_design = \"no\"".to_string(),
        writers: vec![("design".to_string(), "needs_design = \"yes\"".to_string())],
    };
    assert_eq!(faults(&source), [at("cells/check.toml", "watches[0]", what)]);
}

#[test]
fn a_datum_a_cell_has_its_own_of_takes_its_kind_from_what_every_agent_cell_takes_where_its_file_gives_none() {
    let mut source = answer_asks();
    let answer = source.cells.get_mut("answer").expect("the answering cell");
    *answer = answer.replace("answer_questions = \"all\"\nanswer_words = \"all\"", "");
    source.flow.push_str(
        "\n[every_agent]\nreads = [\"{each}_questions\"]\n\n[every_agent.kinds]\n\"{each}_questions\" = \"all\"\n",
    );
    // Its words, of which every agent cell takes no kind, have none.
    let what = NoKind {
        block: "answer_words".to_string(),
    };
    assert_eq!(
        faults_with(&source, &has_all()),
        [at("cells/answer.toml", "watches[3]", what)]
    );
    source.flow = (source.flow)
        .replace(
            "reads = [\"{each}_questions\"]",
            "reads = [\"{each}_questions\", \"{each}_words\"]",
        )
        .replace(
            "\"{each}_questions\" = \"all\"\n",
            "\"{each}_questions\" = \"all\"\n\"{each}_words\" = \"latest\"\n",
        );
    let compiled = compiled_all(&source);
    let given = given(cell(&compiled, "answer"));
    assert!(
        given.contains(&("answer_questions".to_string(), InputKind::All)),
        "{given:?}"
    );
    assert!(
        given.contains(&("answer_words".to_string(), InputKind::Latest)),
        "{given:?}"
    );
}

#[test]
fn what_every_agent_cell_watches_and_reads_is_given_it_first_and_to_no_engine_cell() {
    let mut source = plan_takes("watches = [\"opinions\"]");
    let source = {
        let plan = source.cells.get_mut("plan").expect("the planning cell");
        *plan = plan.replace("request = \"last_and_latest\"\n", "");
        source
            .flow
            .push_str("\n[every_agent]\nreads = [\"request\"]\n\n[every_agent.kinds]\nrequest = \"latest\"\n");
        source
    };
    let compiled = compiled(&source);
    assert_eq!(
        given(cell(&compiled, "plan")),
        [
            ("request".to_string(), InputKind::Latest),
            ("opinions".to_string(), InputKind::LastAndLatest)
        ]
    );
    assert_eq!(runs_on(cell(&compiled, "plan")), ["opinions"]);
    // A cell of the forms before is the flow's as before; an engine cell is given nothing.
    assert!(
        cell(&compiled, "judge")
            .given
            .iter()
            .all(|g| g.block.as_str() != "request")
    );
    assert!(cell(&compiled, "check").given.is_empty());
}

#[test]
fn what_every_agent_cell_takes_is_the_engine_s_data_the_flow_names_each_in_its_kind() {
    let mut unknown = plan_takes("watches = [\"request\"]\nreads = [\"opinions\"]");
    unknown
        .flow
        .push_str("\n[every_agent]\nreads = [\"nowhere\"]\n\n[every_agent.kinds]\nnowhere = \"latest\"\n");
    let what = NoSuchData {
        name: "nowhere".to_string(),
    };
    assert_eq!(faults(&unknown), [at(FLOW_FILE, "every_agent.reads[0]", what)]);
    let mut lacking = plan_takes("watches = [\"opinions\"]");
    let plan = lacking.cells.get_mut("plan").expect("the planning cell");
    *plan = plan.replace("request = \"last_and_latest\"\n", "");
    lacking.flow.push_str("\n[every_agent]\nreads = [\"request\"]\n");
    let what = NoKind {
        block: "request".to_string(),
    };
    assert_eq!(faults(&lacking), [at(FLOW_FILE, "every_agent.reads[0]", what)]);
}

#[test]
fn a_datum_each_cell_has_its_own_of_is_made_for_the_cell_whose_id_its_name_holds() {
    let compiled = compiled_all(&answer_asks());
    let questions = &compiled.blocks[&BlockId::from("answer_questions")];
    assert_eq!(questions.file, "../data/of_a_cell/questions.toml");
    assert_eq!(questions.writers.outside.as_deref(), Some("answer"));
    assert_eq!(questions.what.as_deref(), Some("What answer asks the owner."));
    let answer = cell(&compiled, "answer");
    assert_eq!(
        runs_on(answer),
        ["comments", "plan", "answer_questions", "answer_words"]
    );
    // Every agent cell reads each cell's questions, those of each cell that has its own.
    let mut source = answer_asks();
    source.flow.push_str(
        "\n[every_agent]\nreads = [\"{each}_questions\"]\n\n[every_agent.kinds]\n\"{each}_questions\" = \"all\"\n",
    );
    let source = edit(
        source,
        "cells/plan.toml",
        PLAN_TAKES,
        "watches = [\"request\", \"opinions\"]",
    );
    let compiled = compiled_all(&source);
    assert_eq!(
        given(cell(&compiled, "plan"))[0],
        ("answer_questions".to_string(), InputKind::All)
    );
}

#[test]
fn a_cell_that_asks_the_owner_asks_in_its_own_questions_watches_them_and_is_spoken_to_in_its_own_words() {
    // Another cell's questions are not a cell's to write.
    let source = plan_takes("watches = [\"request\"]\nreads = [\"opinions\"]");
    let mut source = edit(
        source,
        "cells/plan.toml",
        "outputs = [\"plan\"]",
        "outputs = [\"plan\", \"answer_questions\"]",
    );
    for (path, text) in answer_asks().files.into_iter().filter(|(p, _)| p.contains("of_a_cell")) {
        source.files.insert(path, text);
    }
    let answer = answer_asks().cells["answer"].clone();
    source.cells.insert("answer".to_string(), answer);
    let what = NotItsOwn {
        block: "answer_questions".to_string(),
    };
    assert_eq!(
        faults_with(&source, &has_all()),
        [at("cells/plan.toml", "outputs[1]", what)]
    );
    // Its own questions not watched: the owner's answer would run no cell.
    let unwatched = edit(
        answer_asks(),
        "cells/answer.toml",
        "watches = [\"comments\", \"plan\", \"answer_questions\", \"answer_words\"]",
        "watches = [\"comments\", \"plan\", \"answer_words\"]\nreads = [\"answer_questions\"]",
    );
    let what = AsksWithNoAnswer {
        block: "answer_questions".to_string(),
    };
    assert_eq!(
        faults_with(&unwatched, &has_all()),
        [at("cells/answer.toml", "outputs[1]", what)]
    );
    // Asking with no words of its own: the owner's words about its question would reach no cell.
    let mute = edit(
        answer_asks(),
        "cells/answer.toml",
        "outputs = [\"reply\", \"answer_questions\", \"answer_words\"]",
        "outputs = [\"reply\", \"answer_questions\"]",
    );
    let mute = edit(
        mute,
        "cells/answer.toml",
        "watches = [\"comments\", \"plan\", \"answer_questions\", \"answer_words\"]",
        "watches = [\"comments\", \"plan\", \"answer_questions\"]",
    );
    let mute = edit(mute, "cells/answer.toml", "answer_words = \"all\"\n", "");
    let what = AsksAndCannotBeSpokenTo {
        block: "answer_questions".to_string(),
    };
    assert_eq!(
        faults_with(&mute, &has_all()),
        [at("cells/answer.toml", "outputs[1]", what)]
    );
    // Spoken to and asking nothing: there is no question the owner's words could be about.
    let silent = edit(
        answer_asks(),
        "cells/answer.toml",
        "outputs = [\"reply\", \"answer_questions\", \"answer_words\"]",
        "outputs = [\"reply\", \"answer_words\"]",
    );
    let silent = edit(
        silent,
        "cells/answer.toml",
        "watches = [\"comments\", \"plan\", \"answer_questions\", \"answer_words\"]",
        "watches = [\"comments\", \"plan\", \"answer_words\"]",
    );
    let silent = edit(silent, "cells/answer.toml", "answer_questions = \"all\"\n", "");
    let what = SpokenToAndAsksNothing {
        block: "answer_words".to_string(),
    };
    assert_eq!(
        faults_with(&silent, &has_all()),
        [at("cells/answer.toml", "outputs[1]", what)]
    );
}

#[test]
fn a_datum_each_line_has_its_own_of_is_watched_only_by_a_cell_of_that_line() {
    let session = "name = \"{line}_session\"\nfrom = \"session\"\n";
    let source = set(
        plan_takes("watches = [\"request\", \"engineer_session\"]\nreads = [\"opinions\"]"),
        "../data/of_a_line/session.toml",
        session,
    );
    let source = edit(
        source,
        "cells/plan.toml",
        "opinions = \"last_and_latest\"",
        "opinions = \"last_and_latest\"\nengineer_session = \"latest\"",
    );
    let compiled = compiled_all(&source);
    assert_eq!(runs_on(cell(&compiled, "plan")), ["request", "engineer_session"]);
    let block = &compiled.blocks[&BlockId::from("engineer_session")];
    assert_eq!(block.writers.outside.as_deref(), Some("session"));
    // The judge is on the line of the review.
    let source = edit(
        source,
        "cells/judge.toml",
        "triggered_by = [\"on_plan\"]\ncontext = [\"plan\"]",
        "watches = [\"plan\", \"engineer_session\"]",
    );
    let source = edit(
        source,
        "cells/judge.toml",
        "plan = \"last_and_latest\"",
        "plan = \"last_and_latest\"\nengineer_session = \"latest\"",
    );
    let what = LineDataOfAnother {
        block: "engineer_session".to_string(),
        line: "engineer".to_string(),
    };
    assert_eq!(
        faults_with(&source, &has_all()),
        [at("cells/judge.toml", "watches[1]", what)]
    );
}

#[test]
fn the_engine_asks_in_a_cell_that_can_change_the_work_and_be_spoken_to() {
    let source = edit(
        answer_asks(),
        FLOW_FILE,
        "data = [\"request\"]",
        "data = [\"request\"]\nengine_asks_in = \"answer\"",
    );
    assert_eq!(compiled_all(&source).engine_asks_in, Some(CellId::from("answer")));
    // The judge has no words of its own: it cannot be spoken to.
    let judge = edit(
        source.clone(),
        FLOW_FILE,
        "engine_asks_in = \"answer\"",
        "engine_asks_in = \"judge\"",
    );
    let what = CannotBeAskedIn {
        cell: "judge".to_string(),
    };
    assert_eq!(faults_with(&judge, &has_all()), [at(FLOW_FILE, "engine_asks_in", what)]);
    let nobody = edit(
        source,
        FLOW_FILE,
        "engine_asks_in = \"answer\"",
        "engine_asks_in = \"nobody\"",
    );
    let what = NoCellFile {
        cell: "nobody".to_string(),
    };
    assert_eq!(
        faults_with(&nobody, &has_all()),
        [at(FLOW_FILE, "engine_asks_in", what)]
    );
}

#[test]
fn the_item_s_card_shows_data_of_the_flow_in_the_order_it_names_them() {
    let source = edit(
        laid_out(),
        FLOW_FILE,
        "data = [\"request\"]",
        "data = [\"request\"]\ncard = [\"request\", \"plan\"]",
    );
    assert_eq!(
        compiled(&source).card,
        [BlockId::from("request"), BlockId::from("plan")]
    );
    let unknown = edit(
        laid_out(),
        FLOW_FILE,
        "data = [\"request\"]",
        "data = [\"request\"]\ncard = [\"request\", \"nowhere\"]",
    );
    let what = NoSuchBlock {
        block: "nowhere".to_string(),
    };
    assert_eq!(faults(&unknown), [at(FLOW_FILE, "card[1]", what)]);
}
