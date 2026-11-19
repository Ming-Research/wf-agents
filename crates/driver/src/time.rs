//! What a work's time went to, from its first round's begin to the end of its last round or call of a tool: what ran
//! it, a model or the tools, and between, what the work waited for (decision `a-work-keeps-what-its-time-went-to`).
//!
//! What ran is read from the records, with whether each run's cell is an agent's. What a work waited for is read on
//! the flow that decides it, as the records stood then, by the one definition of what waits on the owner
//! ([`crate::owner`]): so it is worked out for a work on the installed version, and kept as such a work ends; a work
//! that ended without keeping it has what ran read from its records alone, and what it waited for is not known.

use std::collections::BTreeMap;

use decide::{Decision, Holder, Maker, Records, RunId, Waited};
use work::{Ask, Call, Stretch, Time, Went, Work};

use crate::{DEPENDENCIES, Kept, Span, owner};

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
            .find(|span| span.from <= at && span.until.is_none_or(|until| at < until))
            .map(|span| span.by)
    };
    let mut waits: Vec<Stretch> = Vec::new();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if merged.iter().any(|(x, y)| *x <= a && b <= *y) {
            continue;
        }
        let then = owner::Shared {
            everything: everything_at(a),
            cannot_run: cannot_run.filter(|(since, _)| *since <= a).map(|(_, why)| why),
        };
        let place = (work.moments.iter()).find(|m| m.at > a).map(|m| m.from);
        let went = match place {
            Some(place) => waited(kept, &work.records.before(place), then),
            None => waited(kept, &work.records, then),
        };
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

/// Whether this is a wait for the items the work's item depends on: a precondition only they make hold, as they merge.
fn on_items(waited: &Waited) -> bool {
    matches!(waited, Waited::Precondition { by, .. }
        if !by.is_empty()
            && by.iter().all(|maker| matches!(maker, Maker::Outside { source } if source == DEPENDENCIES)))
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
}
