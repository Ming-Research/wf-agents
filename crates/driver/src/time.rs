//! What a work's time went to, from its first round's begin to the end of its last round or call of a tool: what ran
//! it, a model or the tools, and between, what the work waited for (decision `a-work-keeps-what-its-time-went-to`).
//!
//! What ran is read from the records, with whether each run's cell is an agent's. What a work waited for is read on
//! the flow that decides it, as the records stood then, by the one definition of what waits on the owner
//! ([`crate::owner`]): so it is worked out for a work on the installed version, and kept as such a work ends; a work
//! that ended without keeping it has what ran read from its records alone, and what it waited for is not known.

use std::collections::BTreeMap;

use decide::{Decision, Holder, Pauser, Records, RunId, Seq};
use work::{Ask, Call, Stretch, Time, Went, Work};

use crate::attention::on_items;
use crate::{Kept, Span, owner};

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
        out.push(Stretch {
            from,
            until: until.max(from),
            went: Went::Tools,
        });
    }
    out
}

/// What the work's time went to as the flow installed decides it, as `kept` holds it, as of `now`: what ran ([`ran`]),
/// and between, each stretch nothing ran all through, what the work waited for as its records stood then, with the
/// pause of everything as `spans` say it held, and the agents' wait where their command line could not run, since the
/// time `cannot_run` gives, with what it said ([`waited`]). None for a work no round of which began.
pub fn stretches(kept: &Kept<'_>, spans: &[Span], cannot_run: Option<(Time, &str)>, now: Time) -> Vec<Stretch> {
    let (flow, work) = (kept.flow, kept.work);
    let agent = |run: RunId| {
        let cell = work.records.started.iter().find(|s| s.run == run).map(|s| &s.cell);
        (flow.cells.iter()).any(|spec| Some(&spec.id) == cell && spec.act.is_none())
    };
    let mut out = ran(work, &agent, now);
    let (Some(from), Some(until)) = (out.iter().map(|s| s.from).min(), out.iter().map(|s| s.until).max()) else {
        return out;
    };
    // What ran, as stretches that do not overlap, in order.
    let mut busy: Vec<(Time, Time)> = out.iter().map(|s| (s.from, s.until)).collect();
    busy.sort();
    let mut merged: Vec<(Time, Time)> = Vec::new();
    for (a, b) in busy {
        match merged.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => merged.push((a, b)),
        }
    }
    // Where what holds the work may change: as something happened to it, and as what holds every work changed.
    let mut points: Vec<Time> = vec![from, until];
    points.extend(work.moments.iter().map(|m| m.at));
    points.extend(spans.iter().flat_map(|span| [Some(span.from), span.until]).flatten());
    points.extend(cannot_run.map(|(since, _)| since));
    points.retain(|at| *at >= from && *at <= until);
    points.sort();
    points.dedup();
    let everything_at = |at: Time| {
        (spans.iter())
            .find(|span| span.from <= at && span.until.is_none_or(|until| at <= until))
            .map(|span| span.by)
    };
    // What the work waited for in a stretch, by the place of the first entry after its start, the records as they stood
    // then being those before it, by who set the pause of everything then, and by whether the agents' command line could
    // run: the stretches of one wait share these, and each is decided once.
    let mut decided: BTreeMap<(Option<Seq>, Option<bool>, bool), Went> = BTreeMap::new();
    let mut waits: Vec<Stretch> = Vec::new();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if merged.iter().any(|(x, y)| *x <= a && b <= *y) {
            continue;
        }
        let place = (work.moments.iter()).find(|m| m.at > a).map(|m| m.from);
        let everything = everything_at(a);
        let could_not_run = cannot_run.filter(|(since, _)| *since <= a).map(|(_, why)| why);
        let key = (
            place,
            everything.map(|by| by == Pauser::Engine),
            could_not_run.is_some(),
        );
        let went = *decided.entry(key).or_insert_with(|| {
            let then = owner::Shared {
                everything,
                cannot_run: could_not_run,
            };
            match place {
                Some(place) => waited(kept, &work.records.before(place), then),
                None => waited(kept, &work.records, then),
            }
        });
        match waits.last_mut() {
            Some(last) if last.until == a && last.went == went => last.until = b,
            _ => waits.push(Stretch {
                from: a,
                until: b,
                went,
            }),
        }
    }
    out.extend(waits);
    out
}

/// What a work on these records waited for, with what held every work at once then: the owner, where the one
/// definition of that says it waited on them ([`owner::waits`]); a pause, its own or the pause of everything; the items
/// its item depends on; anything else, a stalled work among it; as the flow installed decides on them.
fn waited(kept: &Kept<'_>, records: &Records, then: owner::Shared<'_>) -> Went {
    let decision = decide::decide(&kept.flow.flow, records);
    let paused = decide::paused(records);
    let begins = matches!(decision, Decision::Run { .. } | Decision::GoOn { .. });
    // The pause of everything holds a work that would begin a round, where its own pause does not.
    let held = (then.everything.is_some() && begins && !paused).then_some(Holder::Everything);
    if !owner::waits(kept, records, &decision, held, then).is_empty() {
        return Went::Owner;
    }
    match decision {
        Decision::Paused { .. } => Went::Paused,
        Decision::Stalled { .. } | Decision::Run { .. } | Decision::GoOn { .. } if paused => Went::Paused,
        _ if held.is_some() => Went::Paused,
        Decision::Wait { on, .. } if on.iter().any(on_items) => Went::Items,
        _ => Went::Other,
    }
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
        let write = "title = \"Write\"\nkind = \"agent\"\neffort = \"high\"\nline = \"main\"\ntask = \"Write.\"\n\
                     watches = [\"request\"]\noutputs = [\"text\"]\n[kinds]\nrequest = \"last_and_latest\"\n";
        let merge =
            "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nwatches = [\"text\"]\noutputs = [\"merged\"]\n";
        let source = Source {
            flow: text.to_string(),
            cells: [("write", write), ("merge", merge)]
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
        with_kept(&flow(), work, |kept| waited(kept, &work.records, then))
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
            spent: Spent::default(),
        };
        engine::handle(&flow, &mut work, Time(1000), failed()).expect("the round fails");
        engine::handle(&flow, &mut work, Time(3000), Event::Resumed).expect("the owner lets it on");
        engine::handle(&flow, &mut work, Time(4000), failed()).expect("the round fails again");
        let stretches = with_kept(&flow, &work, |kept| stretches(kept, &[], None, Time(9000)));
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
            spent: Spent::default(),
        };
        engine::handle(&flow, &mut work, Time(1000), failed()).expect("the round fails");
        let held = |work: &mut Work, at: u64, event: Event, gate: engine::Gate| {
            engine::handle_held(&flow, work, Time(at), event, decide::Room::FREE, gate).expect("it is taken")
        };
        held(&mut work, 3000, Event::Resumed, engine::Gate::Agents);
        held(&mut work, 6000, Event::Look, engine::Gate::Open);
        engine::handle(&flow, &mut work, Time(7000), failed()).expect("the round fails again");
        let span = |from: u64, until: Option<u64>, by: Pauser| Span {
            from: Time(from),
            until: until.map(Time),
            by,
        };
        let spans = [span(4000, Some(5000), Pauser::Owner), span(5500, None, Pauser::Engine)];
        let stretches = with_kept(&flow, &work, |kept| stretches(kept, &spans, None, Time(9000)));
        let stretch = |from: u64, until: u64, went: Went| Stretch {
            from: Time(from),
            until: Time(until),
            went,
        };
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

    // A measurement kept for one run (research/flow-versions/bar-cost.md): what drawing a live work's bar takes, as the
    // stretches decide each wait exactly, against main's 96 cells, each idle one decided at its middle, cached by place.
    #[test]
    fn measure_what_the_bar_takes() {
        use std::time::{Duration, Instant};

        let flow = flow();
        // A work whose writer's round fails `cycles` times, the owner letting it on a second after each failure.
        let make = |cycles: u64| {
            let mut work = Work::default();
            let wrote = Event::Wrote {
                block: "request".into(),
                by: Outsider::Person,
                content: "Greet.".to_string(),
                matters: true,
                value: None,
            };
            let round_of = |effects: &[Effect]| {
                effects.iter().find_map(|effect| match effect {
                    Effect::Round { run, .. } => Some(*run),
                    _ => None,
                })
            };
            let effects = engine::handle(&flow, &mut work, Time(0), wrote).expect("the request is taken");
            let mut run = round_of(&effects).expect("a round");
            let mut t = 0;
            for _ in 0..cycles {
                t += 1000;
                let failed = Event::Failed {
                    run,
                    why: "it broke".to_string(),
                    failure: Failure::Other,
                    resets: None,
                    spent: Spent::default(),
                };
                engine::handle(&flow, &mut work, Time(t), failed).expect("the round fails");
                t += 1000;
                let effects = engine::handle(&flow, &mut work, Time(t), Event::Resumed).expect("it is let on");
                run = round_of(&effects).expect("a round again");
            }
            (work, Time(t + 1000))
        };
        let median = |mut times: Vec<Duration>| {
            times.sort();
            times[times.len() / 2]
        };
        let mut out = String::new();
        for cycles in [20, 100, 200] {
            let (work, now) = make(cycles);
            let (head, places) = with_kept(&flow, &work, |kept| {
                let mut times = Vec::new();
                let mut drawn = Vec::new();
                for _ in 0..9 {
                    let begun = Instant::now();
                    drawn = stretches(kept, &[], None, now);
                    times.push(begun.elapsed());
                }
                let waits = drawn
                    .iter()
                    .filter(|s| !matches!(s.went, Went::Model | Went::Tools))
                    .count();
                (times, waits)
            });
            let (main, decided) = with_kept(&flow, &work, |kept| {
                let mut times = Vec::new();
                let mut decided = 0;
                for _ in 0..9 {
                    let begun = Instant::now();
                    let runs = ran(kept.work, &|_| true, now);
                    let from = runs.iter().map(|s| s.from.0).min().unwrap();
                    let until = runs.iter().map(|s| s.until.0).max().unwrap();
                    let span = until - from;
                    let mut cache: BTreeMap<Option<Seq>, Went> = BTreeMap::new();
                    for i in 0..96_u64 {
                        let middle = from + (span * (2 * i + 1)) / (2 * 96);
                        if runs.iter().any(|s| s.from.0 <= middle && middle < s.until.0) {
                            continue;
                        }
                        let place = (kept.work.moments.iter()).find(|m| m.at.0 > middle).map(|m| m.from);
                        cache.entry(place).or_insert_with(|| {
                            let then = owner::Shared {
                                everything: None,
                                cannot_run: None,
                            };
                            match place {
                                Some(place) => waited(kept, &kept.work.records.before(place), then),
                                None => waited(kept, &kept.work.records, then),
                            }
                        });
                    }
                    times.push(begun.elapsed());
                    decided = cache.len();
                }
                (times, decided)
            });
            let head_median = median(head.clone());
            out += &format!(
                "cycles={cycles} moments={} entries={} head: waits={places} median={:?} min={:?} max={:?}; main: decided={decided} median={:?} min={:?} max={:?}\n",
                work.moments.len(),
                work.tick().0,
                head_median,
                head.iter().min().unwrap(),
                head.iter().max().unwrap(),
                median(main.clone()),
                main.iter().min().unwrap(),
                main.iter().max().unwrap(),
            );
            // The smallest sample first: a larger one only where it took under a second.
            if head_median > Duration::from_secs(1) {
                break;
            }
        }
        panic!("{out}");
    }
}
