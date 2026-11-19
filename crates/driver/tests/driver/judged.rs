//! How each round and each call of a tool went against the lines of its history: worked out from the records whenever
//! it is read, by the history as it stood when the thing began, and stopping nothing.

use driver::history::Scope;
use driver::judged::Judged;
use engine::RanOn;
use work::Measures;

use super::*;

/// The policy where every agent's round runs on one model, and a line is worked out from two values at least.
fn policy() -> driver::Policy {
    driver::Policy {
        samples: 2,
        on: Box::new(|_, _| Some(on())),
        warm: None,
    }
}

/// What every agent's round runs on.
fn on() -> RanOn {
    RanOn {
        level: "1".to_string(),
        model: "opus".to_string(),
        effort: None,
        account: None,
    }
}

/// A driver with the plain flow, whose writer is an agent's cell and whose merge an act's, and the policy.
fn harness() -> H {
    H::with_policy(vec![plain()], SLOTS, Some(policy))
}

/// The writer's round of the work under way, which began as the last thing happened, ends `ms` after it began, at
/// least three seconds, with what came back: its one try ran on the model, its calls grew its context by 100 tokens
/// in two steps, and it cost `usd` millionths of a dollar.
fn ends(h: &mut H, work: WorkNumber, ms: u64, usd: u64, came: Came) -> Vec<Todo> {
    let began = h.clock;
    let (run, ..) = h.round(work);
    h.on(
        work,
        Event::Ran {
            run,
            attempt: 1,
            on: on(),
        },
    );
    let measures = Measures {
        calls: 3,
        added: 100,
        ..Measures::default()
    };
    h.on(
        work,
        Event::Measured {
            run,
            attempt: 1,
            measures,
        },
    );
    h.clock = began + ms - 1000;
    let spent = Spent {
        usd_micros: usd,
        ..Default::default()
    };
    h.on(work, Event::Returned { run, came, spent })
}

/// The merge's round of the work under way, which began as the last thing happened, hands in what it writes `ms`
/// after it began.
fn merges(h: &mut H, work: WorkNumber, ms: u64) {
    let began = h.clock;
    h.clock = began + ms - 1000;
    h.hands_in(work, &[("merged", None)]);
}

/// A work let on whose writer hands in its text `ms` after it began, having cost `usd`, and whose merge takes four
/// seconds: it merges.
fn through(h: &mut H, ms: u64, usd: u64) -> WorkNumber {
    let (_, work) = h.add_let_on(new_item("a", &[], "plain"));
    ends(h, work, ms, usd, hands_in(&[("text", None)]));
    merges(h, work, 4_000);
    assert_eq!(h.decision(work), Decision::Ended(End::Merged));
    work
}

/// The runs of the work's cell, in the order they started.
fn runs_of(h: &H, work: WorkNumber, cell: &str) -> Vec<RunId> {
    let kept = h.d().work(work).expect("the work");
    (kept.work.records.started.iter())
        .filter(|s| s.cell.as_str() == cell)
        .map(|s| s.run)
        .collect()
}

#[test]
fn a_round_is_judged_by_the_rounds_that_had_ended_as_it_began_and_going_past_its_lines_stops_nothing() {
    let mut h = harness();
    let first = through(&mut h, 10_000, 1_000);
    let second = through(&mut h, 20_000, 2_000);
    // The third writer takes far longer and costs far more than the two before it; its work goes on and merges all
    // the same.
    let third = through(&mut h, 100_000, 50_000);
    let now = Time(h.clock);
    let judged = h.d().judged(None, now);
    let write = runs_of(&h, third, "write")[0];
    let round = &judged.rounds[&(third, write, 1)];
    let cost = round.cost.expect("its cost is judged");
    assert_eq!(cost.value, 50_000.0);
    assert_eq!(
        (cost.line.value, cost.line.samples, cost.line.scope),
        (2.0 * 2_000.0 - 1_500.0, 2, Scope::Cell),
        "the two writers before it on its model"
    );
    assert!(cost.past());
    let time = round.time.expect("its time is judged");
    assert_eq!(time.value, 100_000.0);
    assert_eq!((time.line.value, time.line.samples), (25_000.0, 2));
    assert!(time.past());
    assert_eq!(round.lines, None, "a round that ended is given no lines it runs by");
    // Its merge took as long as the two before it: within the line they give, by the merges' rounds alone.
    let merge = runs_of(&h, third, "merge")[0];
    let merged = &judged.rounds[&(third, merge, 1)];
    assert_eq!(merged.cost, None, "an act costs nothing");
    let time = merged.time.expect("the merge's time is judged");
    assert_eq!(
        (time.value, time.line.value, time.line.samples, time.line.scope),
        (4_000.0, 4_000.0, 2, Scope::Cell)
    );
    assert!(!time.past());
    // The first two had too few rounds before them as they began, and still have, however the history grew since.
    for work in [first, second] {
        assert!(
            !judged.rounds.keys().any(|(w, ..)| *w == work),
            "work {} was judged by what came after it: {judged:?}",
            work.0
        );
    }
    // A work's own judgment is the same as everyone's.
    let own = h.d().judged(Some(third), now);
    assert_eq!(own.rounds[&(third, write, 1)], *round);
    assert!(own.rounds.keys().all(|(w, ..)| *w == third));
}

#[test]
fn a_round_under_way_is_judged_by_how_long_it_has_taken_so_far_and_given_the_lines_the_run_guard_got() {
    let mut h = harness();
    through(&mut h, 10_000, 1_000);
    through(&mut h, 20_000, 2_000);
    let (_, work) = h.add_let_on(new_item("a", &[], "plain"));
    let began = h.clock;
    let flow = plain();
    let cell = flow
        .cells
        .iter()
        .find(|c| c.id.as_str() == "write")
        .expect("the writer");
    let guarded = h.d().lines(work, cell);
    let (run, ..) = h.round(work);
    h.on(
        work,
        Event::Ran {
            run,
            attempt: 1,
            on: on(),
        },
    );
    let at = |ms: u64| h.d().judged(Some(work), Time(began + ms)).rounds[&(work, run, 1)].clone();
    let early = at(20_000);
    let time = early.time.expect("its time is judged");
    assert_eq!((time.value, time.line.value), (20_000.0, 25_000.0));
    assert!(!time.past());
    assert_eq!(early.cost, None, "what it costs is not known until it ends");
    let lines = early.lines.expect("the lines it runs by");
    assert_eq!(
        (lines.added, lines.rate, lines.jump, lines.ratio, lines.cost, lines.time),
        (
            guarded.added,
            guarded.rate,
            guarded.jump,
            guarded.ratio,
            guarded.cost,
            guarded.time
        ),
        "the lines the run guard was given as the round began"
    );
    assert_eq!(lines.time, Some(time.line));
    let late = at(30_000).time.expect("its time is judged");
    assert!(late.past(), "thirty seconds is past the line of twenty-five");
}

#[test]
fn a_call_of_a_tool_is_judged_by_the_calls_of_its_tool_that_had_their_results_as_it_was_made() {
    let mut h = harness();
    let mut calls = Vec::new();
    for ms in [5_000, 10_000, 60_000] {
        let (_, work) = h.add_let_on(new_item("a", &[], "plain"));
        let todos = ends(&mut h, work, 3_000, 10, Came::asks(vec![tool("test")]));
        let call = (todos.iter())
            .find_map(|todo| match &todo.effect {
                Effect::Call { call, .. } => Some(*call),
                _ => None,
            })
            .expect("the tool is called");
        h.clock += ms - 1000;
        h.answer(work, call);
        calls.push((work, call));
    }
    let judged: Judged = h.d().judged(None, Time(h.clock));
    assert!(
        !judged.calls.contains_key(&calls[0]) && !judged.calls.contains_key(&calls[1]),
        "too few calls of the tool had their results as the first two were made: {judged:?}"
    );
    let against = judged.calls[&calls[2]];
    assert_eq!(against.value, 60_000.0);
    assert_eq!(
        (against.line.value, against.line.samples, against.line.scope),
        (2.0 * 10_000.0 - 7_500.0, 2, Scope::Tool)
    );
    assert!(against.past());
}

#[test]
fn a_call_withdrawn_with_its_work_is_neither_judged_nor_part_of_its_tool_s_history() {
    let mut h = harness();
    // The call of a tool by a work let on, which has its result `ms` after it was made, or none.
    let call = |h: &mut H, ms: Option<u64>| {
        let (_, work) = h.add_let_on(new_item("a", &[], "plain"));
        let todos = ends(h, work, 3_000, 10, Came::asks(vec![tool("test")]));
        let call = (todos.iter())
            .find_map(|todo| match &todo.effect {
                Effect::Call { call, .. } => Some(*call),
                _ => None,
            })
            .expect("the tool is called");
        if let Some(ms) = ms {
            h.clock += ms - 1000;
            h.answer(work, call);
        }
        (work, call)
    };
    call(&mut h, Some(5_000));
    call(&mut h, Some(10_000));
    // The third work's item is cancelled while its call waits for its result: the call is withdrawn, and is not
    // judged however long the withdrawal takes, nor once it has its result.
    let (cancelled, withdrawn) = call(&mut h, None);
    h.on(cancelled, Event::Cancelled { why: None });
    assert!(
        (h.d().pending().iter()).any(|todo| matches!(todo.effect, Effect::Withdraw { call } if call == withdrawn)),
        "the call is withdrawn"
    );
    let judged = h.d().judged(None, Time(h.clock + 600_000));
    assert!(!judged.calls.contains_key(&(cancelled, withdrawn)), "{judged:?}");
    let result = "withdrawn".to_string();
    h.on(
        cancelled,
        Event::Done {
            call: withdrawn,
            result,
        },
    );
    // The call after it is judged by the two that had their results, the withdrawn one being none of them.
    let last = call(&mut h, Some(60_000));
    let judged = h.d().judged(None, Time(h.clock));
    assert!(!judged.calls.contains_key(&(cancelled, withdrawn)), "{judged:?}");
    let against = judged.calls[&last];
    assert_eq!(
        (against.line.value, against.line.samples),
        (2.0 * 10_000.0 - 7_500.0, 2)
    );
}

#[test]
fn nothing_is_judged_without_a_policy() {
    let mut h = H::with(vec![plain()]);
    for ms in [10_000, 20_000, 100_000] {
        let (_, work) = h.add_let_on(new_item("a", &[], "plain"));
        let began = h.clock;
        h.clock = began + ms - 1000;
        h.hands_in(work, &[("text", None)]);
        merges(&mut h, work, 4_000);
    }
    assert_eq!(h.d().judged(None, Time(h.clock)), Judged::default());
}
