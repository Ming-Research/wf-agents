//! The datum of the tests' times: the engine writes it as a call of `run_tests` ends, from what the work's runs of the
//! CI said of how long their tests took, and writes no new version where what it says is the same.

use decide::BlockId;
use engine::Ask;
use flow::{Compiled, Has, Source, compile};
use work::timed::{Reading, Test};

use super::*;

/// A flow of one cell that asks for the tests and reads the datum of their times.
fn timed() -> Compiled {
    let flow = r#"
        name = "timed"
        order = ["write"]
        [limits]
        runs = 3
        returns = 2
        calls = 3
        [lines.main]
        [blocks.text]
        [blocks.start]
        outside = "start"
        writes = { text = "The owner started the work." }
        [blocks.times]
        outside = "test_times"
        writes = { no_times = "No time is given.", none_read = "None read.", basis = "Basis {checks}.", not_compared = "Not compared: {checks} of {needed}.", covers = "Covers only what ran.", new_title = "New:", all_title = "All:", slowed_title = "Slowed:", test_new = "- {name} {millis} ms, slower than {share} %", test_plain = "- {name} ({unit}): {millis} ms in {readings}.", test_slowed = "- {name} slowed", more = "More {left}.", sum = "Sum {millis}.", sum_alone = "Alone {millis}.", merges = "Merges {merges}." }
    "#;
    let write = r#"
        title = "Write"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Write the text, running the tests."
        watches = ["start"]
        reads = ["times"]
        outputs = ["text"]

        [kinds]
        start = "last_and_latest"
        times = "latest"
    "#;
    let source = Source {
        flow: flow.to_string(),
        cells: [("write".to_string(), write.to_string())].into(),
        files: [(
            flow::waits_file(),
            include_str!("../../../../flow/agents/waits.toml").to_string(),
        )]
        .into(),
        ..Default::default()
    };
    let has = Has {
        sources: [driver::START.to_string(), driver::timed::TEST_TIMES.to_string()].into(),
        ..Default::default()
    };
    compile(&source, &has).expect("the flow compiles")
}

/// What a run of the fast workflow said of one test that passed in 120 ms.
fn reading(asking: &str) -> Reading {
    Reading {
        asking: asking.to_string(),
        source: work::Source::Fast,
        commit: "c0ffee".to_string(),
        jobs: Vec::new(),
        tests: vec![Test {
            unit: "carry".to_string(),
            name: "a".to_string(),
            passed: true,
            millis: 120,
        }],
    }
}

/// The versions of the datum, as their content.
fn versions(h: &H, work: WorkNumber) -> Vec<String> {
    let kept = h.d().work(work).expect("the work").work;
    let block = BlockId::from("times");
    (kept.records.written.iter())
        .filter(|w| w.block == block)
        .map(|w| kept.contents[&(block.clone(), w.at)].clone())
        .collect()
}

/// The run of the work asks for the tests, and has the result.
fn runs_tests(h: &mut H, work: WorkNumber) {
    let ask = Ask::Tool {
        name: driver::timed::RUN_TESTS.to_string(),
        input: "{}".to_string(),
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
    h.answer(work, call);
}

// A reading the host gave is told to the cells as the call ends: in the words the flow's file gives, with the tests the
// work's runs reported. The same run read again says nothing new, and a run that read another time of the same test
// does.
#[test]
fn the_datum_is_written_as_a_call_of_the_tests_ends_and_again_only_where_it_says_something_new() {
    let mut h = H::with(vec![timed()]);
    let (_, work) = h.add_let_on(new_item("Greet", &[], "timed"));
    assert_eq!(
        versions(&h, work),
        Vec::<String>::new(),
        "nothing is said before a run reports"
    );
    h.on(
        work,
        Event::Timed {
            reading: reading("call/1"),
        },
    );
    runs_tests(&mut h, work);
    let first = "Basis 0.\nNot compared: 0 of 0.\nCovers only what ran.\nAll:\n- a (carry): 120 ms in 1.\nAlone 120.";
    assert_eq!(versions(&h, work), [first]);
    // The same run read again, as after the engine started again, is kept once and says nothing new.
    h.on(
        work,
        Event::Timed {
            reading: reading("call/1"),
        },
    );
    runs_tests(&mut h, work);
    assert_eq!(versions(&h, work), [first]);
    // Another run of the same test is another reading of it.
    h.on(
        work,
        Event::Timed {
            reading: reading("call/2"),
        },
    );
    runs_tests(&mut h, work);
    let second = first.replace("120 ms in 1.", "120 ms in 2.");
    assert_eq!(versions(&h, work), [first.to_string(), second]);
}

// A work whose runs reported nothing and whose host gives no time of any test is told apart from one whose runs
// reported none yet.
#[test]
fn a_host_that_gives_no_time_is_said_so_and_a_call_that_read_none_says_none_read() {
    let mut h = H::with(vec![timed()]);
    let (_, work) = h.add_let_on(new_item("Greet", &[], "timed"));
    runs_tests(&mut h, work);
    assert_eq!(versions(&h, work), ["None read."]);
    let none = Reading {
        asking: "call/2".to_string(),
        source: work::Source::NoTimes,
        commit: String::new(),
        jobs: Vec::new(),
        tests: Vec::new(),
    };
    h.on(work, Event::Timed { reading: none });
    runs_tests(&mut h, work);
    assert_eq!(versions(&h, work), ["None read.", "No time is given."]);
}
