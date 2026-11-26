//! What each thing that happens does to the records, and what comes back to be done.

use work::Forked;

use super::*;

#[test]
fn a_persons_write_is_kept_with_its_content_and_the_first_cell_is_given_its_task() {
    let mut w = W::new();
    let effects = w.on(wrote("request", Outsider::Person, true, None));
    let at = w.last_written();
    let written = decide::Written {
        block: "request".into(),
        by: Outsider::Person,
        matters: true,
        value: None,
        said_to: None,
        empty: false,
        at,
    };
    assert_eq!(w.work.records.written, vec![written]);
    let content = BTreeMap::from([(("request".into(), at), "request, as written by Person".to_string())]);
    assert_eq!(w.work.contents, content);
    let run = w.work.records.started[0].run;
    let start = started_with(
        vec![changed("request", None, false)],
        &[
            ("request", Some(at)),
            ("opinions", None),
            ("main", None),
            ("text", None),
        ],
    );
    assert_eq!(effects, vec![round_effect(run, "write", 1, Given::Task)]);
    assert_eq!(start_of(&w, run), start);
}

#[test]
fn a_write_from_outside_is_kept_as_the_outside_wrote_it() {
    let mut w = W::new();
    w.on(wrote("main", Outsider::Outside, false, None));
    let at = w.last_written();
    let written = decide::Written {
        block: "main".into(),
        by: Outsider::Outside,
        matters: false,
        value: None,
        said_to: None,
        empty: false,
        at,
    };
    assert_eq!(w.work.records.written, vec![written]);
    assert_eq!(
        w.work.contents.get(&("main".into(), at)),
        Some(&"main, as written by Outside".to_string())
    );
}

#[test]
fn a_return_that_meets_everything_finishes_the_run_keeps_each_block_under_the_finish_and_gives_the_next_cell() {
    let (mut w, write, _) = W::requested();
    let effects = w.returns(write, hands_in(&[("text", None)]));
    let at = w.last_finished();
    let finished = decide::Finished {
        run: write,
        at,
        writes: vec![decide::Write {
            block: "text".into(),
            meaning_unchanged: false,
            empty: false,
            value: None,
        }],
        // A return that only hands in says nothing of what it dealt with: it dealt with everything that ran its round,
        // which the engine records.
        dealt_with: Some(vec!["request".into()]),
        waits_for: None,
    };
    assert_eq!(w.work.records.finished, vec![finished]);
    assert_eq!(w.work.contents.get(&("text".into(), at)), Some(&"the text".to_string()));
    let judge = w.work.records.started[1].run;
    let start = started_with(
        vec![changed("text", None, false)],
        &[("text", Some(at)), ("opinions", None), ("verdict", None)],
    );
    assert_eq!(effects, vec![round_effect(judge, "judge", 1, Given::Task)]);
    assert_eq!(start_of(&w, judge), start);
}

#[test]
fn a_return_that_does_not_meet_what_is_required_is_sent_back_to_the_same_run_with_exactly_why() {
    let (mut w, write, _) = W::requested();
    let effects = w.returns(write, wanting());
    assert_eq!(
        effects,
        vec![round_effect(write, "write", 2, Given::SentBack(wanting_unmet()))]
    );
    assert_eq!(w.work.records.finished, vec![]);
    assert!(!w.work.contents.keys().any(|(b, _)| b.as_str() == "verdict"));
}

#[test]
fn a_return_that_deals_with_nothing_and_asks_for_nothing_is_sent_back() {
    let (mut w, write, _) = W::requested();
    let effects = w.returns(write, Came::asks(vec![]));
    assert_eq!(
        effects,
        vec![round_effect(
            write,
            "write",
            2,
            Given::SentBack(vec![Unmet::DealtWithNothing])
        )]
    );
}

#[test]
fn the_owner_pausing_resuming_and_cancelling_are_each_recorded() {
    let mut w = W::new();
    w.on(Event::Paused {
        by: Pauser::Owner,
        why: None,
        waits_for: Vec::new(),
    });
    w.on(Event::Resumed);
    w.on(Event::Cancelled { why: None });
    let r = &w.work.records;
    assert_eq!(r.pauses.iter().map(|p| p.paused).collect::<Vec<_>>(), vec![true, false]);
    assert!(r.cancelled().is_some());
}

#[test]
fn a_paused_work_is_given_nothing_and_when_let_on_is_given_its_next_round() {
    let mut w = W::new();
    assert_eq!(
        w.on(Event::Paused {
            by: Pauser::Owner,
            why: None,
            waits_for: Vec::new(),
        }),
        vec![]
    );
    let (effects, request) = w.person_writes("request");
    assert_eq!(effects, vec![]);
    let effects = w.on(Event::Resumed);
    let run = w.work.records.started[0].run;
    let start = started_with(
        vec![changed("request", None, false)],
        &[
            ("request", Some(request)),
            ("opinions", None),
            ("main", None),
            ("text", None),
        ],
    );
    assert_eq!(effects, vec![round_effect(run, "write", 1, Given::Task)]);
    assert_eq!(start_of(&w, run), start);
}

#[test]
fn a_write_while_a_round_is_under_way_gives_nothing_until_the_round_ends() {
    let (mut w, ..) = W::requested();
    assert_eq!(w.on(wrote("opinions", Outsider::Person, true, None)), vec![]);
}

#[test]
fn an_engine_cell_goes_through_the_same_mechanism() {
    let (mut w, write, _) = W::requested();
    w.lets_merge();
    let judge_effects = w.returns(write, hands_in(&[("text", None)]));
    let text = w.last_finished();
    let (judge, ..) = round(&judge_effects);
    let effects = w.returns(judge, hands_in(&[("verdict", Some("yes"))]));
    let verdict = w.last_finished();
    let merge = w.work.records.started[2].run;
    let start = started_with(
        vec![changed("text", None, false), changed("verdict", None, false)],
        &[("text", Some(text)), ("verdict", Some(verdict)), ("merged", None)],
    );
    assert_eq!(effects, vec![round_effect(merge, "merge", 1, Given::Task)]);
    assert_eq!(start_of(&w, merge), start);
    assert_eq!(
        w.returns(merge, hands_in(&[("merged", None)])),
        vec![Effect::Release {
            end: decide::End::Merged
        }]
    );
    let merged = w.last_finished();
    assert_eq!(
        w.work.contents.get(&("merged".into(), merged)),
        Some(&"the merged".to_string())
    );
    assert_eq!(w.decision(), decide::Decision::Ended(decide::End::Merged));
}

#[test]
fn a_write_from_outside_said_not_to_matter_runs_nothing() {
    let (mut w, write, _) = W::requested();
    let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
    w.returns(judge, hands_in(&[]));
    let first = w.on(wrote("main", Outsider::Outside, true, None));
    let (again, ..) = round(&first);
    w.returns(again, hands_in(&[]));
    assert_eq!(w.on(wrote("main", Outsider::Outside, false, None)), vec![]);
}

/// What the outside says of main for the writer's round, as a carrier does for what it read of main itself.
fn main_for(run: RunId, content: &str) -> Event {
    Event::WroteForRound {
        run,
        block: "main".into(),
        content: content.to_string(),
    }
}

// What the outside says for a round where the block says it already, as the check says the head of main it takes in
// after the watch of main wrote that head, is no new version: the round is told of the version that says it, once,
// and its run consumes it, so that its cell is not run again for it.
#[test]
fn a_write_for_a_round_of_what_the_block_says_already_puts_that_version_in_and_writes_nothing() {
    let (mut w, write, _) = W::requested();
    let (_, moved) = w.on_and_place(wrote("main", Outsider::Outside, true, None));
    let written = w.work.records.written.len();
    w.on(main_for(write, "main, as written by Outside"));
    w.on(main_for(write, "main, as written by Outside"));
    assert_eq!(w.work.records.written.len(), written, "nothing is written again");
    let put: Vec<(RunId, &str, Seq)> = (w.work.records.put_in.iter())
        .map(|p| (p.run, p.block.as_str(), p.version))
        .collect();
    assert_eq!(put, [(write, "main", moved)]);
    let (_, cell, ..) = round(&w.returns(write, hands_in(&[("text", None)])));
    assert_eq!(cell, "judge", "the writer consumed main as it was told of it");
}

// A run abandoned while its round is under way consumes nothing: what the outside says for its round is the outside's
// word all the same, written and put into no round.
#[test]
fn a_write_for_the_round_of_a_run_abandoned_is_written_and_put_in_nowhere() {
    let (mut w, write, _) = W::requested();
    w.on(Event::Cancelled { why: None });
    w.on(main_for(write, "Main is at c0ffee."));
    let main = w.last_written();
    assert_eq!(
        w.work.contents.get(&("main".into(), main)),
        Some(&"Main is at c0ffee.".to_string())
    );
    assert_eq!(w.work.records.put_in, Vec::new());
}

// What the host says of the pull request each check finds again is kept once for each pull request, at a place of its
// own and a moment of its own, and runs no cell.
#[test]
fn a_pull_request_the_host_found_is_kept_once_for_each_and_starts_nothing() {
    let (mut w, ..) = W::requested();
    let pulled = |number: u64| Event::Pulled {
        number,
        url: format!("https://github.com/o/r/pull/{number}"),
    };
    let started = w.work.records.started.len();
    let moments = w.work.moments.len();
    assert_eq!(w.on(pulled(7)), vec![]);
    let first = w.work.tick().0 - 1;
    assert_eq!(w.work.moments.len(), moments + 1, "kept as a moment of its own");
    let before = w.work.clone();
    assert_eq!(w.on(pulled(7)), vec![]);
    assert_eq!(w.work, before, "found again, it is kept once");
    w.on(pulled(8));
    let kept: Vec<(u64, u64, &str)> = (w.work.pulls.iter())
        .map(|p| (p.at.0, p.number, p.url.as_str()))
        .collect();
    let second = w.work.tick().0 - 1;
    assert_eq!(
        kept,
        [
            (first, 7, "https://github.com/o/r/pull/7"),
            (second, 8, "https://github.com/o/r/pull/8")
        ]
    );
    assert_eq!(w.work.records.started.len(), started, "no cell runs for it");
}

// A fork of a round's session that ended is kept with the run and the round it was asked of and what it took, at a place
// and a moment of its own, and runs no cell: for a work under way and for one that has ended, whose fork cost as much.
// A fork of a round that never began, or of no run, is refused and changes nothing.
#[test]
fn a_fork_of_a_round_is_kept_with_what_it_took_for_a_work_under_way_and_for_one_that_has_ended() {
    let (mut w, write, _) = W::requested();
    let spent = Spent {
        usd_micros: 400_000,
        unpriced: true,
        uncached_tokens: 120,
        cache_written_tokens: Some(100),
        cached_tokens: 18_100,
        output_tokens: 30,
        context_tokens: Some(9_100),
        compactions: 1,
    };
    let fork = |run: RunId, round: usize| Event::Forked { run, round, spent };
    let started = w.work.records.started.len();
    let moments = w.work.moments.len();
    assert_eq!(w.on(fork(write, 1)), vec![]);
    let first = w.work.tick().0 - 1;
    assert_eq!(w.work.moments.len(), moments + 1, "kept as a moment of its own");
    assert_eq!(w.work.records.started.len(), started, "no cell runs for it");
    // What is kept of what it took is its cost, whether some of it is not known, and its counts of tokens: the rest of
    // what a round keeps is not a fork's.
    assert_eq!(
        w.work.forked,
        [Forked {
            at: Seq(first),
            run: write,
            round: 1,
            usd_micros: 400_000,
            unpriced: true,
            uncached_tokens: 120,
            cache_written_tokens: Some(100),
            cached_tokens: 18_100,
            output_tokens: 30,
        }]
    );

    let before = w.work.clone();
    assert_eq!(w.refused(fork(write, 2)), Refused::NoSuchRound { run: write, round: 2 });
    assert_eq!(
        w.refused(fork(RunId(99), 1)),
        Refused::NoSuchRound {
            run: RunId(99),
            round: 1
        }
    );
    assert_eq!(w.work, before);

    w.on(Event::Cancelled { why: None });
    w.on(fork(write, 1));
    let kept = &w.work.forked;
    assert_eq!(kept.len(), 2, "kept for a work that has ended too");
    assert!(kept[1].at > kept[0].at, "at a place after the first's");
    assert_eq!(kept[1].run, write);
    assert_eq!(kept[1].usd_micros, 400_000);
}
