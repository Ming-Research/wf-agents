//! What a work's time went to, from its first round's begin to the end of its last round or call of a tool: what ran
//! it, a model or the tools, and between, what the work waited for (decision `a-work-keeps-what-its-time-went-to`).
//!
//! What ran is read from the records, with whether each run's cell is an agent's. What a work waited for is read on
//! the flow that decides it, as the records stood then, by the one definition of what waits on the owner
//! ([`crate::owner`]): so it is worked out for a work on the installed version, and kept as such a work ends; a work
//! that ended without keeping it has what ran read from its records alone, and what it waited for is not known.
//!
//! What holds every work at once then is read with it: the pause of everything, the stop of the engine, a wait of the
//! agents' service in which their command line could not run, and, for the turn of the work's project to check and
//! merge, each stretch another work of the project held it. The walk is asked in the room of that moment, so that a
//! wait for the turn is the walk's own, as it was when the work waited.

use std::collections::BTreeMap;

use decide::{Decision, Holder, Pauser, Records, Room, RunId, Seq, Waited};
use work::{Ask, Call, Stretch, Time, TurnSpan, Went, Work};

use crate::attention::on_items;
use crate::outage::CannotRun;
use crate::{Kept, Span, StopSpan, owner};

/// Each stretch of the work's time that ran, from its begin to its end, one under way to `now`: each round, a model's
/// where `model` says its run is an agent's, else the tools'; and each call of a tool, the tools', from when it was made
/// to when it was done. A round or a call whose times the records do not tell is left out.
pub fn ran(work: &Work, model: &dyn Fn(RunId) -> bool, now: Time) -> Vec<Stretch> {
    let r = &work.records;
    // When what ended at this place ended; now for what has not ended.
    let end_of = |at: Option<decide::Seq>| match at {
        Some(at) => work.time_of(at),
        None => Some(now),
    };
    let mut out = Vec::new();
    let mut counted: BTreeMap<RunId, usize> = BTreeMap::new();
    for round in &r.rounds {
        let number = counted.entry(round.run).or_default();
        *number += 1;
        let ended = (r.returned.iter()).filter(|e| e.run == round.run).nth(*number - 1);
        let (Some(from), Some(until)) = (work.time_of(round.at), end_of(ended.map(|e| e.at))) else {
            continue;
        };
        let went = if model(round.run) { Went::Model } else { Went::Tools };
        out.push(Stretch {
            from,
            until: until.max(from),
            went,
        });
    }
    for called in &r.called {
        if !matches!(work.calls.get(&called.call), Some(Call::Asked(Ask::Tool { .. }))) {
            continue;
        }
        let done = r.done.iter().find(|done| done.call == called.call);
        let (Some(from), Some(until)) = (work.time_of(called.at), end_of(done.map(|d| d.at))) else {
            continue;
        };
        let until = until.max(from);
        // The engine's wait for a call the host could not carry out for now is no time of the tools: nothing runs in
        // it, so it is cut out of the call's stretch and read as a wait like any other ([`Waits::at`]).
        let mut cuts: Vec<(Time, Time)> = (r.held.iter())
            .filter(|held| held.call == called.call)
            .filter_map(|held| {
                let wait = r.called.iter().find(|c| c.call == held.wait)?;
                let ended = r.done.iter().find(|d| d.call == held.wait);
                Some((work.time_of(wait.at)?, end_of(ended.map(|d| d.at))?))
            })
            .collect();
        if cuts.is_empty() {
            out.push(Stretch {
                from,
                until,
                went: Went::Tools,
            });
            continue;
        }
        cuts.sort();
        let mut at = from;
        for (cut_from, cut_until) in cuts {
            if cut_from > at {
                out.push(Stretch {
                    from: at,
                    until: cut_from.min(until),
                    went: Went::Tools,
                });
            }
            at = at.max(cut_until);
        }
        if at < until {
            out.push(Stretch {
                from: at,
                until,
                went: Went::Tools,
            });
        }
    }
    out
}

/// What the work's time went to as the flow installed decides it, as `kept` holds it, as of `now`: what ran ([`ran`]),
/// and between, each stretch nothing ran all through, what the work waited for then ([`Waits::at`]). None for a work no
/// round of which began.
pub fn stretches(
    kept: &Kept<'_>,
    spans: &[Span],
    stops: &[StopSpan],
    cannot_run: &[CannotRun],
    turns: Vec<TurnSpan>,
    now: Time,
) -> Vec<Stretch> {
    let mut waits = Waits::new(*kept, spans, stops, cannot_run, turns, now);
    let mut out = waits.ran.clone();
    let (Some(from), Some(until)) = (waits.from, waits.until) else {
        return out;
    };
    // Where what holds the work may change: as something happened to it, and as what holds every work changed.
    let mut points: Vec<Time> = vec![from, until];
    points.extend(kept.work.moments.iter().map(|m| m.at));
    points.extend(spans.iter().flat_map(|span| [Some(span.from), span.until]).flatten());
    points.extend(stops.iter().flat_map(|stop| [Some(stop.from), stop.until]).flatten());
    points.extend(
        cannot_run
            .iter()
            .flat_map(|wait| [Some(wait.from), wait.until])
            .flatten(),
    );
    points.extend(
        waits
            .turns
            .iter()
            .flat_map(|span| [Some(span.from), span.until])
            .flatten(),
    );
    points.retain(|at| *at >= from && *at <= until);
    points.sort();
    points.dedup();
    let mut stretches: Vec<Stretch> = Vec::new();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        // Nothing that holds the work changes within the stretch, so what it waited for at its start it waited for
        // all through.
        let Some(went) = waits.at(a) else {
            continue;
        };
        match stretches.last_mut() {
            Some(last) if last.until == a && last.went == went => last.until = b,
            _ => stretches.push(Stretch {
                from: a,
                until: b,
                went,
            }),
        }
    }
    out.extend(stretches);
    out
}

/// What a work waited for at each time of its span that nothing of it ran, as the flow installed decides on its records
/// as they stood then, with the pause of everything as `spans` say it held, the stop of the engine as `stops` say it
/// stood, the agents' wait where their command line could not run, in each stretch of `cannot_run` with what it said,
/// and the turn of the work's project as `turns` say another work of the project held it ([`waited`]). The times of
/// one wait share the place of the first entry after them, who set the pause of everything then, whether a stop stood,
/// whether the agents could run and whether another work held the turn, and each such wait is decided once: the bar of
/// a work going on asks it at the middle of each of its cells, as many as it has, and what it kept as it ended at the
/// start of each stretch, so that the two read alike.
pub struct Waits<'a> {
    kept: Kept<'a>,
    spans: &'a [Span],
    stops: &'a [StopSpan],
    cannot_run: &'a [CannotRun],
    /// Each stretch another work of the work's project held the turn to check and merge, as the work's own is none.
    turns: Vec<TurnSpan>,
    /// What ran ([`ran`]), as of the time given.
    pub ran: Vec<Stretch>,
    /// What ran, as stretches that do not overlap, in order.
    busy: Vec<(Time, Time)>,
    /// When the work's first round began.
    pub from: Option<Time>,
    /// When the last of what ran ended.
    pub until: Option<Time>,
    decided: BTreeMap<Then, Went>,
}

/// What a wait is decided by: the place of the first entry after the time, who set the pause of everything then (whether
/// the engine), whether a stop stood, whether the agents' command line could not run, and whether another work of the
/// project held the turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Then {
    place: Option<Seq>,
    everything: Option<bool>,
    stopping: bool,
    could_not_run: bool,
    held: bool,
}

impl<'a> Waits<'a> {
    /// The waits of the work `kept` holds, as of `now`.
    pub fn new(
        kept: Kept<'a>,
        spans: &'a [Span],
        stops: &'a [StopSpan],
        cannot_run: &'a [CannotRun],
        turns: Vec<TurnSpan>,
        now: Time,
    ) -> Waits<'a> {
        let (flow, work) = (kept.flow, kept.work);
        let agent = |run: RunId| {
            let cell = work.records.started.iter().find(|s| s.run == run).map(|s| &s.cell);
            (flow.cells.iter()).any(|spec| Some(&spec.id) == cell && spec.act.is_none())
        };
        let ran = ran(work, &agent, now);
        let mut busy: Vec<(Time, Time)> = ran.iter().map(|s| (s.from, s.until)).collect();
        busy.sort();
        let mut merged: Vec<(Time, Time)> = Vec::new();
        for (a, b) in busy {
            match merged.last_mut() {
                Some(last) if a <= last.1 => last.1 = last.1.max(b),
                _ => merged.push((a, b)),
            }
        }
        Waits {
            kept,
            spans,
            stops,
            cannot_run,
            turns,
            from: ran.iter().map(|s| s.from).min(),
            until: ran.iter().map(|s| s.until).max(),
            ran,
            busy: merged,
            decided: BTreeMap::new(),
        }
    }

    /// What the work waited for at this time: none where something of it ran then, or the time is outside its span.
    pub fn at(&mut self, at: Time) -> Option<Went> {
        let (Some(from), Some(until)) = (self.from, self.until) else {
            return None;
        };
        if at < from || at >= until || self.busy.iter().any(|(x, y)| *x <= at && at < *y) {
            return None;
        }
        let work = self.kept.work;
        let place = (work.moments.iter()).find(|m| m.at > at).map(|m| m.from);
        let everything = (self.spans.iter())
            .find(|span| span.from <= at && span.until.is_none_or(|until| at < until))
            .map(|span| span.by);
        let could_not_run = (self.cannot_run.iter())
            .find(|wait| wait.from <= at && wait.until.is_none_or(|until| at < until))
            .map(|wait| wait.why.as_str());
        let stopping = (self.stops.iter()).any(|stop| stop.from <= at && stop.until.is_none_or(|until| at < until));
        let held = (self.turns.iter()).any(|span| span.from <= at && span.until.is_none_or(|until| at < until));
        let key = Then {
            place,
            everything: everything.map(|by| by == Pauser::Engine),
            stopping,
            could_not_run: could_not_run.is_some(),
            held,
        };
        let kept = self.kept;
        Some(*self.decided.entry(key).or_insert_with(|| {
            let then = owner::Shared {
                everything,
                cannot_run: could_not_run,
            };
            // Another work holding the turn is the room the work was left, the walk deciding it as the driver did.
            let room = Room {
                turn: !held,
                ..Room::FREE
            };
            match place {
                Some(place) => waited(&kept, &work.records.before(place), then, stopping, room),
                None => waited(&kept, &work.records, then, stopping, room),
            }
        }))
    }
}

/// What a work on these records waited for, with what held every work at once then: the owner, where the one
/// definition of that says it waited on them ([`owner::waits`]); a pause, its own or the pause of everything; the stop
/// of the engine, where `stopping`, which holds a work that would begin a round or waits for a slot or for the turn;
/// the turn of its project, which another work holds; the items its item depends on; anything else, a stalled work
/// among it; as the flow installed decides on them, in the room the engine left the work.
fn waited(kept: &Kept<'_>, records: &Records, then: owner::Shared<'_>, stopping: bool, room: Room) -> Went {
    let decision = decide::decide_in(&kept.flow.flow, records, room);
    let paused = decide::paused(records);
    let begins = matches!(&decision, Decision::Run { .. } | Decision::GoOn { .. })
        || matches!(&decision, Decision::Wait { on, .. } if on == &[Waited::Turn]);
    // The pause of everything holds a work that would begin a round, where its own pause does not.
    let held = (then.everything.is_some() && begins && !paused).then_some(Holder::Everything);
    if !owner::waits(kept, records, &decision, held, then).is_empty() {
        return Went::Owner;
    }
    match decision {
        Decision::Paused { .. } => Went::Paused,
        Decision::Stalled { .. } | Decision::Run { .. } | Decision::GoOn { .. } if paused => Went::Paused,
        _ if held.is_some() => Went::Paused,
        Decision::Run { .. } | Decision::GoOn { .. } if stopping => Went::Stopping,
        Decision::Wait { on, .. } if stopping && (on == [Waited::Slot] || on == [Waited::Turn]) => Went::Stopping,
        Decision::Wait { on, .. } if on == [Waited::Turn] => Went::Turn,
        Decision::GoOn { cell, .. } if !room.turn && takes_turn(kept, &cell) => Went::Turn,
        Decision::Wait { on, .. } if on.iter().any(on_items) => Went::Items,
        _ => Went::Other,
    }
}

/// Whether the cell takes the turn of its work's project, as the flow that decides the work has it.
fn takes_turn(kept: &Kept<'_>, cell: &decide::CellId) -> bool {
    let flow = &kept.flow.flow;
    flow.place(cell).is_some_and(|i| flow.cells()[i].turn)
}

#[cfg(test)]
mod tests {
    //! What a stretch of a work's time no round ran in went to, on the records as they stood then, with what held every
    //! work at once then.

    use decide::run::Failure;
    use decide::{Outsider, Pause, Pauser, Seq, Written};
    use engine::{Effect, Event};
    use flow::{Compiled, Has, Source, compile};
    use store::{First, Item, ItemNumber, WorkHead, WorkNumber};
    use work::Spent;

    use super::*;
    use crate::On;

    /// A writer the owner's request runs, and the merge, with the repository's words beside it.
    fn flow() -> Compiled {
        flow_with("")
    }

    /// The same, the writer's file holding these lines besides.
    fn flow_with(own: &str) -> Compiled {
        let text = r#"
            name = "small"
            order = ["write", "merge"]
            [limits]
            runs = 3
            returns = 2
            calls = 3
            [lines.main]
            [blocks.request]
            people = true
            [blocks.text]
            [blocks.merged]
            shown = true
        "#;
        let write = format!(
            "title = \"Write\"\nkind = \"agent\"\neffort = \"high\"\nline = \"main\"\ntask = \"Write.\"\n{own}\
             watches = [\"request\"]\noutputs = [\"text\"]\n[kinds]\nrequest = \"last_and_latest\"\n"
        );
        let merge =
            "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nwatches = [\"text\"]\noutputs = [\"merged\"]\n";
        let source = Source {
            flow: text.to_string(),
            cells: [("write", write.as_str()), ("merge", merge)]
                .iter()
                .map(|(id, text)| (id.to_string(), text.to_string()))
                .collect(),
            files: [(
                flow::waits_file(),
                include_str!("../../../flow/agents/waits.toml").to_string(),
            )]
            .into(),
            ..Source::default()
        };
        let has = Has {
            acts: ["merge".to_string()].into(),
            ..Has::default()
        };
        compile(&source, &has).expect("the definition compiles")
    }

    /// What reads the work on the flow, held on the version it is.
    fn with_kept<T>(flow: &Compiled, work: &Work, read: impl FnOnce(&Kept<'_>) -> T) -> T {
        let head = WorkHead {
            number: WorkNumber(1),
            item: ItemNumber(1),
            flow: "small".to_string(),
            added: Time(0),
        };
        let first = First {
            title: "Greet".to_string(),
            level: None,
            project: None,
            depends_on: Vec::new(),
            tags: Vec::new(),
            continues: None,
        };
        let item = Item::new(ItemNumber(1), Time(0), first, Vec::new());
        let on = On::installed_flow(flow);
        let kept = Kept {
            head: &head,
            item: &item,
            flow,
            on: &on,
            passed: &[],
            work,
        };
        read(&kept)
    }

    /// What a work on these records waited for, with what held every work at once then.
    fn waited_on(work: &Work, everything: Option<Pauser>) -> Went {
        let then = owner::Shared {
            everything,
            cannot_run: None,
        };
        with_kept(&flow(), work, |kept| {
            waited(kept, &work.records, then, false, Room::FREE)
        })
    }

    /// A work the owner's request runs the writer of, paused by this pauser where one is given.
    fn requested(paused: Option<Pauser>) -> Work {
        let mut work = Work::default();
        work.records.written.push(Written {
            block: "request".into(),
            by: Outsider::Person,
            matters: true,
            value: None,
            empty: false,
            said_to: None,
            at: Seq(1),
        });
        if let Some(by) = paused {
            work.records.pauses.push(Pause {
                paused: true,
                at: Seq(2),
                by,
                why: Some("it spent more than it may".to_string()),
                waits_for: Vec::new(),
            });
        }
        work
    }

    #[test]
    fn a_pause_the_engine_made_is_the_owners_and_a_stalled_work_is_nobodys() {
        assert_eq!(waited_on(&requested(Some(Pauser::Engine)), None), Went::Owner);
        assert_eq!(waited_on(&requested(Some(Pauser::Owner)), None), Went::Paused);
        // Nothing runs a cell of a work nothing was written to: it is stalled, which waits on no one.
        assert_eq!(waited_on(&Work::default(), None), Went::Other);
    }

    #[test]
    fn the_pause_of_every_work_is_a_pause_whoever_set_it() {
        // A pause held the work, which is what it is, whether the owner set it over every work or the engine did as it
        // started again: a work it holds waits on the owner for nothing of its own (module `owner`).
        for by in [Pauser::Engine, Pauser::Owner] {
            assert_eq!(waited_on(&requested(None), Some(by)), Went::Paused, "{by:?}");
        }
        assert_eq!(waited_on(&requested(None), None), Went::Other);
    }

    // A round that fails pauses its work for the owner: from its end to the round that begins as the owner lets the
    // work on, the work waited on the owner, and each round was a model's, the writer being an agent's cell.
    // A call the host could not carry out for now ran as the tools from when it was made to when it was done, but for
    // the engine's wait it was held until: nothing ran then, so that time is no stretch of the tools.
    #[test]
    fn the_wait_a_call_was_held_until_is_no_time_of_the_tools() {
        use decide::{CallId, Called, Done, Held, Obstacle};
        let mut work = Work::default();
        let called = |call, at| Called {
            call: CallId(call),
            run: RunId(1),
            on: None,
            at: Seq(at),
        };
        work.records.called = vec![called(2, 1), called(3, 3)];
        work.records.held = vec![Held {
            call: CallId(2),
            wait: CallId(3),
            at: Seq(2),
            obstacle: Obstacle::Limit,
        }];
        work.records.done = vec![
            Done {
                call: CallId(3),
                at: Seq(4),
            },
            Done {
                call: CallId(2),
                at: Seq(5),
            },
        ];
        work.calls.insert(
            CallId(2),
            Call::Asked(Ask::Tool {
                name: "run_tests".to_string(),
                input: String::new(),
            }),
        );
        work.moments = [(1, 1000), (3, 5000), (4, 65_000), (5, 70_000)]
            .iter()
            .map(|(from, at)| work::Moment {
                from: Seq(*from),
                at: Time(*at),
            })
            .collect();
        let tools = |from: u64, until: u64| Stretch {
            from: Time(from),
            until: Time(until),
            went: Went::Tools,
        };
        assert_eq!(
            ran(&work, &|_| true, Time(80_000)),
            [tools(1000, 5000), tools(65_000, 70_000)]
        );
        // A call held and not yet done: the wait under way is cut out up to now, and what came before is kept.
        work.records.done.clear();
        assert_eq!(ran(&work, &|_| true, Time(20_000)), [tools(1000, 5000)]);
    }

    #[test]
    fn what_ran_is_read_from_the_records_and_what_the_work_waited_for_between_as_they_stood_then() {
        let flow = flow();
        let mut work = Work::default();
        let wrote = Event::Wrote {
            block: "request".into(),
            by: Outsider::Person,
            content: "Greet.".to_string(),
            matters: true,
            value: None,
        };
        let effects = engine::handle(&flow, &mut work, Time(0), wrote).expect("the request is taken");
        let run = effects
            .iter()
            .find_map(|effect| match effect {
                Effect::Round { run, .. } => Some(*run),
                _ => None,
            })
            .expect("the writer's round begins");
        let failed = || Event::Failed {
            run,
            why: "it broke".to_string(),
            failure: Failure::Other,
            resets: None,
            hindrance: None,
            spent: Spent::default(),
        };
        engine::handle(&flow, &mut work, Time(1000), failed()).expect("the round fails");
        engine::handle(&flow, &mut work, Time(3000), Event::Resumed).expect("the owner lets it on");
        engine::handle(&flow, &mut work, Time(4000), failed()).expect("the round fails again");
        let stretches = with_kept(&flow, &work, |kept| {
            stretches(kept, &[], &[], &[], Vec::new(), Time(9000))
        });
        let stretch = |from: u64, until: u64, went: Went| Stretch {
            from: Time(from),
            until: Time(until),
            went,
        };
        assert_eq!(
            stretches,
            [
                stretch(0, 1000, Went::Model),
                stretch(3000, 4000, Went::Model),
                stretch(1000, 3000, Went::Owner)
            ]
        );
    }
    // The pause of everything holds a work that would begin a round from the time it was set, and no longer from the
    // time it was lifted, a pause not lifted holding it to the end, whoever set it: between, the work waited for nothing
    // that holds it. The work here, let on as the agents' wait held its round back, would begin the round all along.
    // Fails where a pause is read as holding at the time it was lifted, or not at the time it was set, or one not lifted
    // as over. Milliseconds; the one test that reads the pause of everything through its stretches.
    #[test]
    fn the_pause_of_everything_holds_a_work_from_the_time_it_was_set_up_to_the_time_it_was_lifted() {
        let flow = flow();
        let work = held_back(&flow);
        let span = |from: u64, until: Option<u64>, by: Pauser| Span {
            from: Time(from),
            until: until.map(Time),
            by,
        };
        let spans = [span(4000, Some(5000), Pauser::Owner), span(5500, None, Pauser::Engine)];
        let stretches = with_kept(&flow, &work, |kept| {
            stretches(kept, &spans, &[], &[], Vec::new(), Time(9000))
        });
        assert_eq!(
            stretches,
            [
                stretch(0, 1000, Went::Model),
                stretch(6000, 7000, Went::Model),
                stretch(1000, 3000, Went::Owner),
                stretch(3000, 4000, Went::Other),
                stretch(4000, 5000, Went::Paused),
                stretch(5000, 5500, Went::Other),
                stretch(5500, 6000, Went::Paused),
            ]
        );
    }

    // A stop of the engine holds a work that would begin a round from the time it was asked for up to the time the
    // engine started again, as a stretch of its own, and not the pause's: what the time went to keeps the two apart,
    // though the bar draws them alike. Fails where the stop is read as holding past its end or not from its start, or
    // as a pause. Milliseconds; the one test that reads a stop through its stretches.
    #[test]
    fn a_stop_holds_a_work_from_the_time_it_was_asked_for_up_to_the_engine_s_start_after_it() {
        let flow = flow();
        let work = held_back(&flow);
        let stops = [StopSpan {
            from: Time(3500),
            until: Some(Time(4500)),
        }];
        let stretches = with_kept(&flow, &work, |kept| {
            stretches(kept, &[], &stops, &[], Vec::new(), Time(9000))
        });
        assert_eq!(
            stretches,
            [
                stretch(0, 1000, Went::Model),
                stretch(6000, 7000, Went::Model),
                stretch(1000, 3000, Went::Owner),
                stretch(3000, 3500, Went::Other),
                stretch(3500, 4500, Went::Stopping),
                stretch(4500, 6000, Went::Other),
            ]
        );
    }

    // Where another work of the project held the turn to check and merge, a work that would begin a round of a cell that
    // takes it waited for it, as the walk says it in that room; a cell that takes none is not held back by it. The work
    // here is paused from 1000 to 3000 and would begin its round from then to 6000. Fails where the wait is read with
    // the turn free, or not stopped at the time the holder let go.
    #[test]
    fn a_work_that_would_begin_a_round_of_a_cell_that_takes_the_turn_waits_for_it_while_another_work_holds_it() {
        let flow = flow_with("takes_turn = true\n");
        let work = held_back(&flow);
        let at =
            |turns: Vec<TurnSpan>| with_kept(&flow, &work, |kept| stretches(kept, &[], &[], &[], turns, Time(9000)));
        let held = |from: u64, until: Option<u64>| TurnSpan {
            from: Time(from),
            until: until.map(Time),
        };
        let rest = [
            stretch(0, 1000, Went::Model),
            stretch(6000, 7000, Went::Model),
            stretch(1000, 3000, Went::Owner),
        ];
        let with = |last: &[Stretch]| [&rest[..], last].concat();
        assert_eq!(
            at(vec![held(2000, Some(6000))]),
            with(&[stretch(3000, 6000, Went::Turn)])
        );
        // Nothing holds it: nothing says what the work waited for.
        assert_eq!(at(Vec::new()), with(&[stretch(3000, 6000, Went::Other)]));
        // Held until the holder let go before the work began, and so for no longer.
        assert_eq!(
            at(vec![held(2000, Some(4000))]),
            with(&[stretch(3000, 4000, Went::Turn), stretch(4000, 6000, Went::Other)])
        );
        // Held from after the work was let on.
        assert_eq!(
            at(vec![held(4000, None)]),
            with(&[stretch(3000, 4000, Went::Other), stretch(4000, 6000, Went::Turn)])
        );
    }

    #[test]
    fn a_cell_that_takes_no_turn_is_not_held_back_by_another_work_holding_it() {
        let flow = flow();
        let work = held_back(&flow);
        let turns = vec![TurnSpan {
            from: Time(2000),
            until: Some(Time(6000)),
        }];
        let got = with_kept(&flow, &work, |kept| stretches(kept, &[], &[], &[], turns, Time(9000)));
        assert!(!got.iter().any(|s| s.went == Went::Turn), "{got:?}");
    }

    #[test]
    fn a_stop_of_the_engine_is_said_before_the_turn() {
        let flow = flow_with("takes_turn = true\n");
        let work = held_back(&flow);
        let turns = vec![TurnSpan {
            from: Time(2000),
            until: Some(Time(6000)),
        }];
        let stops = [StopSpan {
            from: Time(3500),
            until: Some(Time(4500)),
        }];
        let got = with_kept(&flow, &work, |kept| {
            stretches(kept, &[], &stops, &[], turns, Time(9000))
        });
        assert_eq!(
            got,
            [
                stretch(0, 1000, Went::Model),
                stretch(6000, 7000, Went::Model),
                stretch(1000, 3000, Went::Owner),
                stretch(3000, 3500, Went::Turn),
                stretch(3500, 4500, Went::Stopping),
                stretch(4500, 6000, Went::Turn),
            ]
        );
    }

    // A wait in which the agents' command line could not run is a wait on the owner from the time its first round failed
    // so up to the time the next round of an agent began, whether it stands or is over: the work here would begin its
    // round from 3000 to 6000, and the wait is said to have held from 2000 to 4500, after which nothing says what the
    // work waited for. Fails where only the wait that stands is read, or one that is over is read as not holding.
    #[test]
    fn a_wait_in_which_the_command_line_could_not_run_is_a_wait_on_the_owner_while_it_lasted_and_nothing_after() {
        let flow = flow();
        let work = held_back(&flow);
        let at = |waits: &[CannotRun]| {
            with_kept(&flow, &work, |kept| {
                stretches(kept, &[], &[], waits, Vec::new(), Time(9000))
            })
        };
        let wait = |from: u64, until: Option<u64>| CannotRun {
            from: Time(from),
            until: until.map(Time),
            why: "the agents' program could not run".to_string(),
        };
        let ran = [stretch(0, 1000, Went::Model), stretch(6000, 7000, Went::Model)];
        let with = |waited: &[Stretch]| [&ran[..], waited].concat();
        assert_eq!(
            at(&[wait(2000, Some(4500))]),
            with(&[stretch(1000, 4500, Went::Owner), stretch(4500, 6000, Went::Other)])
        );
        // Nothing says it held: the pause the engine made, then nothing.
        assert_eq!(
            at(&[]),
            with(&[stretch(1000, 3000, Went::Owner), stretch(3000, 6000, Went::Other)])
        );
        // The wait that stands has no end.
        assert_eq!(
            at(&[wait(4000, None)]),
            with(&[
                stretch(1000, 3000, Went::Owner),
                stretch(3000, 4000, Went::Other),
                stretch(4000, 6000, Went::Owner)
            ])
        );
    }

    /// A stretch of the work's time, in milliseconds.
    fn stretch(from: u64, until: u64, went: Went) -> Stretch {
        Stretch {
            from: Time(from),
            until: Time(until),
            went,
        }
    }

    /// A work whose round failed, paused it, and was let on at 3000 while the agents' wait held its round back, then
    /// began the round at 6000 and failed again at 7000: from 3000 to 6000 it would begin the round all along.
    fn held_back(flow: &Compiled) -> Work {
        let mut work = Work::default();
        let wrote = Event::Wrote {
            block: "request".into(),
            by: Outsider::Person,
            content: "Greet.".to_string(),
            matters: true,
            value: None,
        };
        let effects = engine::handle(flow, &mut work, Time(0), wrote).expect("the request is taken");
        let run = effects
            .iter()
            .find_map(|effect| match effect {
                Effect::Round { run, .. } => Some(*run),
                _ => None,
            })
            .expect("the writer's round begins");
        let failed = || Event::Failed {
            run,
            why: "it broke".to_string(),
            failure: Failure::Other,
            resets: None,
            hindrance: None,
            spent: Spent::default(),
        };
        engine::handle(flow, &mut work, Time(1000), failed()).expect("the round fails");
        let held = |work: &mut Work, at: u64, event: Event, gate: engine::Gate| {
            engine::handle_held(flow, work, Time(at), event, decide::Room::FREE, gate).expect("it is taken")
        };
        held(&mut work, 3000, Event::Resumed, engine::Gate::Agents);
        held(&mut work, 6000, Event::Look, engine::Gate::Open);
        engine::handle(flow, &mut work, Time(7000), failed()).expect("the round fails again");
        work
    }
}
