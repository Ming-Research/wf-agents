//! The datum of the calls of `run_tests`: the engine writes a version of it for each call as the call ends, with what
//! the call asked for and the result it was given, and writes a call once however often the engine starts again.

use decide::BlockId;
use engine::Ask;
use flow::{Compiled, Has, Source, compile};

use super::*;

/// A flow of a cell that asks for calls and reads the datum of them, and the merge after it.
fn runs() -> Compiled {
    let flow = r#"
        name = "runs"
        order = ["write", "merge"]
        [limits]
        runs = 3
        returns = 8
        calls = 8
        [lines.main]
        [blocks.text]
        [blocks.merged]
        shown = true
        [blocks.start]
        outside = "start"
        writes = { text = "The owner started the work." }
        [blocks.calls]
        outside = "test_runs"
        writes = { call = "Call {n}, asked as {request}. It gave:\n{result}" }
    "#;
    let write = r#"
        title = "Write"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Write the text, running the tests."
        watches = ["start"]
        reads = ["calls"]
        outputs = ["text"]

        [kinds]
        start = "last_and_latest"
        calls = "all"
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        watches = ["text"]
        outputs = ["merged"]
    "#;
    let source = Source {
        flow: flow.to_string(),
        cells: [
            ("write".to_string(), write.to_string()),
            ("merge".to_string(), merge.to_string()),
        ]
        .into(),
        files: [(
            flow::waits_file(),
            include_str!("../../../../flow/agents/waits.toml").to_string(),
        )]
        .into(),
        ..Default::default()
    };
    let has = Has {
        sources: [driver::START.to_string(), driver::timed::TEST_RUNS.to_string()].into(),
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    compile(&source, &has).expect("the flow compiles")
}

/// The versions of the datum, as their content.
fn versions(h: &H, work: WorkNumber) -> Vec<String> {
    let kept = h.d().work(work).expect("the work").work;
    let block = BlockId::from("calls");
    (kept.records.written.iter())
        .filter(|w| w.block == block)
        .map(|w| kept.contents[&(block.clone(), w.at)].clone())
        .collect()
}

/// The run of the work asks for this tool with this input, and the call has this result.
fn calls(h: &mut H, work: WorkNumber, tool: &str, input: &str, result: &str) {
    let ask = Ask::Tool {
        name: tool.to_string(),
        input: input.to_string(),
    };
    h.returns(work, Came::asks(vec![ask]));
    let call = h
        .d()
        .work(work)
        .expect("the work")
        .work
        .records
        .called
        .last()
        .expect("the call")
        .call;
    h.on(
        work,
        Event::Done {
            call,
            result: result.to_string(),
        },
    );
}

// A call of the tool that runs the tests is written as it ends, whole and as it was asked, and each call after it is
// another version: the first stays as it was. A call of another tool is no version.
#[test]
fn each_call_of_the_tests_is_a_version_with_what_it_asked_for_and_gave_and_no_other_tool_is() {
    let mut h = H::with(vec![runs()]);
    let (_, work) = h.add_let_on(new_item("Greet", &[], "runs"));
    assert_eq!(
        versions(&h, work),
        Vec::<String>::new(),
        "nothing is said before a call ends"
    );
    calls(
        &mut h,
        work,
        driver::timed::RUN_TESTS,
        r#"{"tests":["a"]}"#,
        "a passed.\n\n",
    );
    let first = r#"Call 1, asked as {"tests":["a"]}. It gave:
a passed."#;
    assert_eq!(
        versions(&h, work),
        [first],
        "the result's end of whitespace is left off"
    );
    calls(&mut h, work, "query", r#"{"sql":"select 1"}"#, "1");
    assert_eq!(versions(&h, work), [first], "a call of another tool is no version");
    calls(&mut h, work, driver::timed::RUN_TESTS, "{}", "Nothing was run: {n}");
    let second = "Call 2, asked as {}. It gave:\nNothing was run: {n}";
    assert_eq!(
        versions(&h, work),
        [first.to_string(), second.to_string()],
        "a call of the tests is a version of its own, counted among the calls of the tests alone, and what stands in a \
         result is not filled again"
    );
}

// The engine starting again writes no version a call already has: the next call is the next version, and not the
// calls before it again.
#[test]
fn a_call_is_written_once_however_often_the_engine_starts_again() {
    let mut h = H::with(vec![runs()]);
    let (_, work) = h.add_let_on(new_item("Greet", &[], "runs"));
    calls(&mut h, work, driver::timed::RUN_TESTS, "{}", "one");
    h.restart();
    calls(&mut h, work, driver::timed::RUN_TESTS, "{}", "two");
    h.restart();
    calls(&mut h, work, driver::timed::RUN_TESTS, "{}", "three");
    assert_eq!(
        versions(&h, work),
        [
            "Call 1, asked as {}. It gave:\none",
            "Call 2, asked as {}. It gave:\ntwo",
            "Call 3, asked as {}. It gave:\nthree",
        ]
    );
}
