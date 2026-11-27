//! What a work's time went to, from its first round's begin to the end of its last round or call of a tool: what ran
//! it, a model or the tools, and between, what the work waited for (decision `a-work-keeps-what-its-time-went-to`).
//!
//! What ran is read from the records, with whether each run's cell is an agent's. What a work waited for is read on
//! the flow that decides it, as the records stood then, by the one definition of what waits on the owner
//! ([`crate::owner`]): so it is worked out for a work on the installed version, and kept as such a work ends; a work
//! that ended without keeping it has what ran read from its records alone, and what it waited for is not known.
//!
//! What holds every work at once then is read with it: the pause of everything, the stop of the engine, a wait of the
//! agents' service in which their command line could not run, every stretch in which a cause every work shares held
//! the rounds of agents, and, for the turn of the work's project to check and merge, each stretch another work of the
//! project held it. The walk is asked in the room of that moment, so that a wait for the turn is the walk's own, as it
//! was when the work waited. A wait for a slot is read from the walk too: the scheduler gives a slot to every work that
//! waits for one while any is free (module `driver`), so a work the walk would run, where nothing holds every work and
//! no round of it began, waited for the slots.

use std::collections::{BTreeMap, BTreeSet};

use decide::{CallId, Decision, Holder, Pauser, Records, Room, RunId, Seq, Waited};
use work::{Ask, Call, Stretch, Time, TurnSpan, Went, Work};

use crate::attention::on_items;
use crate::outage::{CannotRun, Down};
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
    let returns = returns_of(r);
    let mut counted: BTreeMap<RunId, usize> = BTreeMap::new();
    for round in &r.rounds {
        let number = counted.entry(round.run).or_default();
        *number += 1;
        let ended = returns.get(&round.run).and_then(|returned| returned.get(*number - 1));
        let (Some(from), Some(until)) = (work.time_of(round.at), end_of(ended.copied())) else {
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
        if matches!(work.calls.get(&called.call), Some(Call::Asked(Ask::Tool { .. }))) {
            out.extend(call_stretches(work, called.call, called.at, now));
        }
    }
    out
}

/// The places where each run's rounds returned, in order: the nth round of a run ended at the nth.
fn returns_of(r: &Records) -> BTreeMap<RunId, Vec<Seq>> {
    let mut returns: BTreeMap<RunId, Vec<Seq>> = BTreeMap::new();
    for returned in &r.returned {
        returns.entry(returned.run).or_default().push(returned.at);
    }
    returns
}

/// The stretches of the tools' time of a call of a tool, made at this place: from when it was made to when it was done,
/// one under way to `now`, less each wait the engine made for the host. A call whose times the records do not tell has
/// none.
fn call_stretches(work: &Work, call: CallId, made: Seq, now: Time) -> Vec<Stretch> {
    let r = &work.records;
    let end_of = |at: Option<Seq>| match at {
        Some(at) => work.time_of(at),
        None => Some(now),
    };
    let done = r.done.iter().find(|done| done.call == call);
    let (Some(from), Some(until)) = (work.time_of(made), end_of(done.map(|d| d.at))) else {
        return Vec::new();
    };
    let until = until.max(from);
    // The engine's wait for a call the host could not carry out for now is no time of the tools: nothing runs in
    // it, so it is cut out of the call's stretch and read as a wait like any other ([`Waits::at`]).
    let mut cuts: Vec<(Time, Time)> = (r.held.iter())
        .filter(|held| held.call == call)
        .filter_map(|held| {
            let wait = r.called.iter().find(|c| c.call == held.wait)?;
            let ended = r.done.iter().find(|d| d.call == held.wait);
            Some((work.time_of(wait.at)?, end_of(ended.map(|d| d.at))?))
        })
        .collect();
    let tools = |from, until| Stretch {
        from,
        until,
        went: Went::Tools,
    };
    if cuts.is_empty() {
        return vec![tools(from, until)];
    }
    cuts.sort();
    let mut out = Vec::new();
    let mut at = from;
    for (cut_from, cut_until) in cuts {
        if cut_from > at {
            out.push(tools(at, cut_from.min(until)));
        }
        at = at.max(cut_until);
    }
    if at < until {
        out.push(tools(at, until));
    }
    out
}

/// The stretches of the work's time in which a call of a tool or a round of an engine's cell waited for a run of the
/// project's CI: each call or round that a run of the CI was read for, by what asked for the run ([`work::Timed`]:
/// `call/<call>`, `run/<run>/round/<round>`), as the tools' time of it is ([`ran`]), less the engine's wait for the host.
/// Nothing names a tool or a cell: a run the CI was read for says what asked for it. Each stretch says whether the call
/// or round it is of has ended; one under way is counted to `now`.
pub fn ci_waits(work: &Work, now: Time) -> Vec<(Stretch, bool)> {
    let askings: BTreeSet<&str> = work.timed.iter().map(|t| t.reading.asking.as_str()).collect();
    if askings.is_empty() {
        return Vec::new();
    }
    let r = &work.records;
    let mut out = Vec::new();
    for called in &r.called {
        if askings.contains(format!("call/{}", called.call.0).as_str()) {
            let done = r.done.iter().any(|done| done.call == called.call);
            out.extend(
                call_stretches(work, called.call, called.at, now)
                    .into_iter()
                    .map(|s| (s, done)),
            );
        }
    }
    let returns = returns_of(r);
    let mut counted: BTreeMap<RunId, usize> = BTreeMap::new();
    for round in &r.rounds {
        let number = counted.entry(round.run).or_default();
        *number += 1;
        if !askings.contains(format!("run/{}/round/{number}", round.run.0).as_str()) {
            continue;
        }
        let ended = returns.get(&round.run).and_then(|returned| returned.get(*number - 1));
        let until = match ended {
            Some(at) => work.time_of(*at),
            None => Some(now),
        };
        if let (Some(from), Some(until)) = (work.time_of(round.at), until) {
            let stretch = Stretch {
                from,
                until: until.max(from),
                went: Went::Tools,
            };
            out.push((stretch, ended.is_some()));
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
    down: &[Down],
    turns: Vec<TurnSpan>,
    now: Time,
) -> Vec<Stretch> {
    stretches_to(
        kept,
        spans,
        stops,
        cannot_run,
        down,
        turns,
        now,
        None,
        &mut Decided::default(),
    )
}

/// What the work's time went to ([`stretches`]), its waits carried on after the last of what ran up to `to`, where one
/// is given and is later: what a work that has not ended waits for now, which `stretches` leaves out because it reads
/// only the time from the first round's begin to the last of what ran. What an earlier reading of the same work decided
/// is in `decided`, and what this one decides is left in it ([`Decided`]).
#[allow(clippy::too_many_arguments)]
pub fn stretches_to(
    kept: &Kept<'_>,
    spans: &[Span],
    stops: &[StopSpan],
    cannot_run: &[CannotRun],
    down: &[Down],
    turns: Vec<TurnSpan>,
    now: Time,
    to: Option<Time>,
    decided: &mut Decided,
) -> Vec<Stretch> {
    let mut waits = Waits::new(*kept, spans, stops, cannot_run, down, turns, now);
    waits.recall(decided);
    if let Some(to) = to {
        waits.until = waits.until.map(|until| until.max(to));
    }
    let mut out = waits.ran.clone();
    let (Some(from), Some(until)) = (waits.from, waits.until) else {
        waits.remember(decided);
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
    points.extend(down.iter().flat_map(|wait| [Some(wait.from), wait.until]).flatten());
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
    waits.remember(decided);
    out
}

/// What a work waited for at each time of its span that nothing of it ran, as the flow installed decides on its records
/// as they stood then, with the pause of everything as `spans` say it held, the stop of the engine as `stops` say it
/// stood, the agents' wait where their command line could not run, in each stretch of `cannot_run` with what it said,
/// the agents' rounds held by any cause every work shares, in each stretch of `down`, and the turn of the work's project
/// as `turns` say another work of the project held it ([`waited`]). The times of one wait share the place of the first
/// entry after them, who set the pause of everything then, whether a stop stood, whether the agents could run, whether
/// a cause every work shares held their rounds and whether another work held the turn, and each such wait is decided
/// once: the bar of
/// a work going on asks it at the middle of each of its cells, as many as it has, and what it kept as it ended at the
/// start of each stretch, so that the two read alike.
pub struct Waits<'a> {
    kept: Kept<'a>,
    spans: &'a [Span],
    stops: &'a [StopSpan],
    cannot_run: &'a [CannotRun],
    /// Each stretch a cause every work shares held the rounds of agents.
    down: &'a [Down],
    /// Each stretch another work of the work's project held the turn to check and merge, as the work's own is none.
    turns: Vec<TurnSpan>,
    /// What ran ([`ran`]), as of the time given.
    pub ran: Vec<Stretch>,
    /// What ran, as stretches that do not overlap or touch, in order.
    busy: Vec<(Time, Time)>,
    /// Whether the times of the work's moments do not go back, so that the first moment after a time is found by halving.
    ordered: bool,
    /// When the work's first round began.
    pub from: Option<Time>,
    /// When the last of what ran ended.
    pub until: Option<Time>,
    decided: BTreeMap<Then, Went>,
}

/// What a wait is decided by: the place of the first entry after the time, who set the pause of everything then (whether
/// the engine), whether a stop stood, whether the agents' command line could not run, whether a cause every work shares
/// held the rounds of agents, and whether another work of the project held the turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Then {
    place: Option<Seq>,
    everything: Option<bool>,
    stopping: bool,
    could_not_run: bool,
    shared: bool,
    held: bool,
}

/// What a work waited for at the places of its records, kept from one reading of its stretches to the next. A wait is
/// decided by the flow installed, the records before the place it is decided at, and the rest of what [`Then`] says;
/// records are only added, so what was decided at a place that records follow is what a later reading would decide there
/// again, and is not decided again. What was decided after the latest record is not kept: the records that follow it
/// will change what it was decided on. Kept for the version of the flow it was decided on, and dropped for another.
#[derive(Clone, Debug, Default)]
pub struct Decided {
    version: Option<String>,
    by: BTreeMap<Then, Went>,
}

impl<'a> Waits<'a> {
    /// Takes what an earlier reading of the work decided on this version of the flow, leaving none in `decided`.
    fn recall(&mut self, decided: &mut Decided) {
        let version = self.kept.on.version();
        if version.is_some() && decided.version.as_deref() == version {
            self.decided = std::mem::take(&mut decided.by);
        } else {
            *decided = Decided::default();
        }
    }

    /// Leaves in `decided` what was decided at a place that records follow, for the next reading.
    fn remember(&mut self, decided: &mut Decided) {
        decided.version = self.kept.on.version().map(str::to_string);
        decided.by = std::mem::take(&mut self.decided)
            .into_iter()
            .filter(|(then, _)| then.place.is_some())
            .collect();
    }

    /// The waits of the work `kept` holds, as of `now`.
    pub fn new(
        kept: Kept<'a>,
        spans: &'a [Span],
        stops: &'a [StopSpan],
        cannot_run: &'a [CannotRun],
        down: &'a [Down],
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
            down,
            turns,
            from: ran.iter().map(|s| s.from).min(),
            until: ran.iter().map(|s| s.until).max(),
            ran,
            busy: merged,
            ordered: work.moments.is_sorted_by_key(|moment| moment.at),
            decided: BTreeMap::new(),
        }
    }

    /// What the work waited for at this time: none where something of it ran then, or the time is outside its span.
    pub fn at(&mut self, at: Time) -> Option<Went> {
        let (Some(from), Some(until)) = (self.from, self.until) else {
            return None;
        };
        // The stretches of `busy` are in order and do not touch, so only the last that began by `at` can hold it.
        let began = self.busy.partition_point(|(x, _)| *x <= at);
        if at < from || at >= until || began.checked_sub(1).is_some_and(|last| at < self.busy[last].1) {
            return None;
        }
        let work = self.kept.work;
        let place = if self.ordered {
            let after = work.moments.partition_point(|m| m.at <= at);
            work.moments.get(after).map(|m| m.from)
        } else {
            (work.moments.iter()).find(|m| m.at > at).map(|m| m.from)
        };
        let everything = (self.spans.iter())
            .find(|span| span.from <= at && span.until.is_none_or(|until| at < until))
            .map(|span| span.by);
        let could_not_run = (self.cannot_run.iter())
            .find(|wait| wait.from <= at && wait.until.is_none_or(|until| at < until))
            .map(|wait| wait.why.as_str());
        let stopping = (self.stops.iter()).any(|stop| stop.from <= at && stop.until.is_none_or(|until| at < until));
        let shared = (self.down.iter()).any(|wait| wait.from <= at && wait.until.is_none_or(|until| at < until));
        let held = (self.turns.iter()).any(|span| span.from <= at && span.until.is_none_or(|until| at < until));
        let key = Then {
            place,
            everything: everything.map(|by| by == Pauser::Engine),
            stopping,
            could_not_run: could_not_run.is_some(),
            shared,
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
                Some(place) => waited(&kept, &work.records.before(place), then, stopping, shared, room),
                None => waited(&kept, &work.records, then, stopping, shared, room),
            }
        }))
    }
}

/// What a work on these records waited for, with what held every work at once then: the owner, where the one
/// definition of that says it waited on them ([`owner::waits`]); a pause, its own or the pause of everything; the stop
/// of the engine, where `stopping`, which holds a work that would begin a round or waits for a slot or for the turn;
/// the turn of its project, which another work holds; the items its item depends on; the slots, where the walk would run
/// a cell of an agent in a room with every slot free and nothing held back every agent's round (`shared`), so that
/// only the slots can have kept the round from beginning; anything else, a stalled work among it; as the flow
/// installed decides on them, in the room the engine left the work.
fn waited(
    kept: &Kept<'_>,
    records: &Records,
    then: owner::Shared<'_>,
    stopping: bool,
    shared: bool,
    room: Room,
) -> Went {
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
        Decision::Run { cell, .. } | Decision::GoOn { cell, .. } if !shared && takes_slot(kept, &cell) => Went::Slot,
        _ => Went::Other,
    }
}

/// Whether a round of the cell takes a slot, as the flow that decides the work has it.
fn takes_slot(kept: &Kept<'_>, cell: &decide::CellId) -> bool {
    let flow = &kept.flow.flow;
    flow.place(cell).is_some_and(|i| flow.cells()[i].slot)
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
        waited_in(&flow(), work, everything)
    }

    /// The same, as this flow decides the work.
    fn waited_in(flow: &Compiled, work: &Work, everything: Option<Pauser>) -> Went {
        let then = owner::Shared {
            everything,
            cannot_run: None,
        };
        with_kept(flow, work, |kept| {
            waited(kept, &work.records, then, false, false, Room::FREE)
        })
    }

    /// A flow of the merge alone, which the owner's request runs: a cell of the engine's own act, which takes no slot.
    fn act_flow() -> Compiled {
        let text = r#"
            name = "small"
            order = ["merge"]
            [limits]
            runs = 3
            returns = 2
            calls = 3
            [lines.main]
            [blocks.request]
            people = true
            [blocks.merged]
            shown = true
        "#;
        let merge =
            "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nwatches = [\"request\"]\noutputs = [\"merged\"]\n";
        let source = Source {
            flow: text.to_string(),
            cells: [("merge".to_string(), merge.to_string())].into(),
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

    // A work whose request the writer, a cell of an agent, runs, with nothing holding it, would begin its round: that no
    // round began is the slots', where every slot is free for it in the room the walk is asked in. A cell of the engine's
    // own act takes no slot, so a wait before one is nobody's. Fails where the wait for a slot is read as any other, or a
    // cell that takes none is read as waiting for one.
    #[test]
    fn a_work_that_would_begin_a_round_of_an_agent_s_cell_that_did_not_begin_waited_for_a_slot() {
        assert_eq!(waited_on(&requested(None), None), Went::Slot);
        let flow = flow();
        with_kept(&flow, &requested(None), |kept| {
            assert!(takes_slot(kept, &decide::CellId::from("write")));
            assert!(!takes_slot(kept, &decide::CellId::from("merge")));
        });
        // The same request run by a cell that takes no slot: the walk says it would begin that, and no slot kept it.
        let act = act_flow();
        with_kept(&act, &requested(None), |kept| {
            let decision = decide::decide_in(&kept.flow.flow, &requested(None).records, Room::FREE);
            assert!(matches!(decision, Decision::Run { .. }), "{decision:?}");
        });
        assert_eq!(waited_in(&act, &requested(None), None), Went::Other);
    }

    #[test]
    fn the_pause_of_every_work_is_a_pause_whoever_set_it() {
        // A pause held the work, which is what it is, whether the owner set it over every work or the engine did as it
        // started again: a work it holds waits on the owner for nothing of its own (module `owner`).
        for by in [Pauser::Engine, Pauser::Owner] {
            assert_eq!(waited_on(&requested(None), Some(by)), Went::Paused, "{by:?}");
        }
        assert_eq!(waited_on(&requested(None), None), Went::Slot);
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
            stretches(kept, &[], &[], &[], &[], Vec::new(), Time(9000))
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
    // What was decided at a place that records follow is kept for the next reading of the work's stretches and is not
    // decided again, and what was decided after the latest record is not kept, as the records that follow it change what
    // it waited for: a work paused after a reading waits for no one after it. Fails where nothing is kept (a decision put
    // wrong in `decided` is not read back), and where what was decided after the latest record is kept (it then reads as
    // the wrong decision put in, or as the owner's wait after the pause).
    #[test]
    fn what_was_decided_at_a_place_records_follow_is_kept_for_the_next_reading_and_the_rest_is_not() {
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
        let read = |work: &Work, decided: &mut Decided| {
            with_kept(&flow, work, |kept| {
                stretches_to(
                    kept,
                    &[],
                    &[],
                    &[],
                    &[],
                    Vec::new(),
                    Time(9000),
                    Some(Time(9000)),
                    decided,
                )
            })
        };
        let model = |from, until| stretch(from, until, Went::Model);
        let mut decided = Decided::default();
        assert_eq!(
            read(&work, &mut decided),
            [
                model(0, 1000),
                model(3000, 4000),
                stretch(1000, 3000, Went::Owner),
                stretch(4000, 9000, Went::Owner)
            ]
        );
        // Whatever was decided at a place a record follows is read back, and nothing decided after the last record is: a
        // decision put wrong everywhere in what was kept reads back before the last record only.
        let mut put_wrong = decided.clone();
        put_wrong.by.values_mut().for_each(|went| *went = Went::Items);
        assert_eq!(
            read(&work, &mut put_wrong),
            [
                model(0, 1000),
                model(3000, 4000),
                stretch(1000, 3000, Went::Items),
                stretch(4000, 9000, Went::Owner)
            ]
        );
        // The owner pauses the work after the reading: what the stretch from the pause on waited for is read anew, and what
        // came before it is as it was.
        let paused = Event::Paused {
            by: Pauser::Owner,
            why: None,
            waits_for: Vec::new(),
        };
        engine::handle(&flow, &mut work, Time(5000), paused).expect("the owner pauses it");
        let again = read(&work, &mut decided);
        assert_eq!(again, read(&work, &mut Decided::default()));
        assert_eq!(again.last(), Some(&stretch(5000, 9000, Went::Paused)));
        // Another version of the flow decided nothing that is kept.
        let mut of_another = decided.clone();
        of_another.version = Some("another".to_string());
        of_another.by.values_mut().for_each(|went| *went = Went::Items);
        assert_eq!(read(&work, &mut of_another), again);
    }

    // What a work waited for at a time is the same where the moment after the time and the round it falls in are found
    // by halving the moments and the stretches that ran, and where the moments are looked through one by one, as they must
    // be where their times go back; and no time a round ran in is a wait. Fails where a time is put in the wrong moment,
    // or in a stretch of what ran that it is outside of.
    #[test]
    fn a_wait_is_the_same_found_by_halving_the_moments_and_found_one_by_one() {
        let flow = flow();
        let work = held_back(&flow);
        with_kept(&flow, &work, |kept| {
            let mut halved = Waits::new(*kept, &[], &[], &[], &[], Vec::new(), Time(9000));
            let mut by_one = Waits::new(*kept, &[], &[], &[], &[], Vec::new(), Time(9000));
            assert!(halved.ordered, "the times of these moments do not go back");
            by_one.ordered = false;
            let mut waited = 0;
            for at in (0u64..9500).step_by(25).map(Time) {
                let ran_in = halved.ran.iter().any(|s| s.from <= at && at < s.until);
                let found = halved.at(at);
                assert_eq!(found, by_one.at(at), "{at:?}");
                assert_eq!(found.is_none(), ran_in || at >= halved.until.unwrap(), "{at:?}");
                waited += usize::from(found.is_some());
            }
            assert!(waited > 0, "the work waited at some time that was looked at");
        });
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
            stretches(kept, &spans, &[], &[], &[], Vec::new(), Time(9000))
        });
        assert_eq!(
            stretches,
            [
                stretch(0, 1000, Went::Model),
                stretch(6000, 7000, Went::Model),
                stretch(1000, 3000, Went::Owner),
                stretch(3000, 4000, Went::Slot),
                stretch(4000, 5000, Went::Paused),
                stretch(5000, 5500, Went::Slot),
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
            stretches(kept, &[], &stops, &[], &[], Vec::new(), Time(9000))
        });
        assert_eq!(
            stretches,
            [
                stretch(0, 1000, Went::Model),
                stretch(6000, 7000, Went::Model),
                stretch(1000, 3000, Went::Owner),
                stretch(3000, 3500, Went::Slot),
                stretch(3500, 4500, Went::Stopping),
                stretch(4500, 6000, Went::Slot),
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
        let at = |turns: Vec<TurnSpan>| {
            with_kept(&flow, &work, |kept| {
                stretches(kept, &[], &[], &[], &[], turns, Time(9000))
            })
        };
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
        // Nothing holds it: the turn is free, and no round began, which is the slots'.
        assert_eq!(at(Vec::new()), with(&[stretch(3000, 6000, Went::Slot)]));
        // Held until the holder let go before the work began, and so for no longer.
        assert_eq!(
            at(vec![held(2000, Some(4000))]),
            with(&[stretch(3000, 4000, Went::Turn), stretch(4000, 6000, Went::Slot)])
        );
        // Held from after the work was let on.
        assert_eq!(
            at(vec![held(4000, None)]),
            with(&[stretch(3000, 4000, Went::Slot), stretch(4000, 6000, Went::Turn)])
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
        let got = with_kept(&flow, &work, |kept| {
            stretches(kept, &[], &[], &[], &[], turns, Time(9000))
        });
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
            stretches(kept, &[], &stops, &[], &[], turns, Time(9000))
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
        // Every wait of this kind is a stretch in which a cause every work shares held the rounds of agents.
        let at = |waits: &[CannotRun]| {
            let down: Vec<Down> = (waits.iter())
                .map(|wait| Down {
                    from: wait.from,
                    until: wait.until,
                })
                .collect();
            with_kept(&flow, &work, |kept| {
                stretches(kept, &[], &[], waits, &down, Vec::new(), Time(9000))
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
            with(&[stretch(1000, 4500, Went::Owner), stretch(4500, 6000, Went::Slot)])
        );
        // Nothing says it held: the pause the engine made, then the slots'.
        assert_eq!(
            at(&[]),
            with(&[stretch(1000, 3000, Went::Owner), stretch(3000, 6000, Went::Slot)])
        );
        // The wait that stands has no end.
        assert_eq!(
            at(&[wait(4000, None)]),
            with(&[
                stretch(1000, 3000, Went::Owner),
                stretch(3000, 4000, Went::Slot),
                stretch(4000, 6000, Went::Owner)
            ])
        );
    }

    // While a cause every work shares held the rounds of agents, a usage limit, no network or a service that did not serve
    // as much as a command line that could not run, no round of this work could begin whatever the slots were: the wait
    // is nobody's, which is `Other`, and the slots' only from when it was over. The work here would begin its round from
    // 3000 to 6000; the stretch said to have held is from 2000 to 4500. Fails where such a wait is read as a wait for a
    // slot, or one still standing as over.
    #[test]
    fn a_wait_in_which_a_shared_cause_held_every_round_is_no_wait_for_a_slot_while_it_lasted() {
        let flow = flow();
        let work = held_back(&flow);
        let at = |down: &[Down]| {
            with_kept(&flow, &work, |kept| {
                stretches(kept, &[], &[], &[], down, Vec::new(), Time(9000))
            })
        };
        let wait = |from: u64, until: Option<u64>| Down {
            from: Time(from),
            until: until.map(Time),
        };
        let ran = [stretch(0, 1000, Went::Model), stretch(6000, 7000, Went::Model)];
        let with = |waited: &[Stretch]| [&ran[..], waited].concat();
        assert_eq!(
            at(&[wait(2000, Some(4500))]),
            with(&[
                stretch(1000, 3000, Went::Owner),
                stretch(3000, 4500, Went::Other),
                stretch(4500, 6000, Went::Slot)
            ])
        );
        // The wait that stands has no end.
        assert_eq!(
            at(&[wait(4000, None)]),
            with(&[
                stretch(1000, 3000, Went::Owner),
                stretch(3000, 4000, Went::Slot),
                stretch(4000, 6000, Went::Other)
            ])
        );
        // Nothing held every round: the whole of it is the slots'.
        assert_eq!(
            at(&[]),
            with(&[stretch(1000, 3000, Went::Owner), stretch(3000, 6000, Went::Slot)])
        );
    }

    // What waited for a run of the project's CI: a call of a tool, and a round, that a run was read for, by what asked for
    // it, as the tools' time of each less the engine's wait for the host, and nothing named: a call no run was read for is
    // not one, and a call or round under way is told so. Fails where a call is counted that no run was read for, or the
    // wait the engine made for the host is counted, or the round of an engine's cell is left out.
    #[test]
    fn what_a_call_or_a_round_waited_for_a_run_of_the_ci_is_told_by_what_asked_for_the_run() {
        use decide::{Called, Done, Held, Obstacle, Returned, Round};
        use work::{Reading, Timed};
        let mut work = Work::default();
        let called = |call, at| Called {
            call: CallId(call),
            run: RunId(1),
            on: None,
            at: Seq(at),
        };
        work.records.called = vec![called(2, 1), called(3, 3), called(4, 6)];
        work.records.held = vec![Held {
            call: CallId(2),
            wait: CallId(3),
            at: Seq(2),
            obstacle: Obstacle::Limit,
        }];
        let done = |call, at| Done {
            call: CallId(call),
            at: Seq(at),
        };
        work.records.done = vec![done(3, 4), done(2, 5), done(4, 7)];
        for call in [2, 4] {
            work.calls.insert(
                CallId(call),
                Call::Asked(Ask::Tool {
                    name: "a_tool".to_string(),
                    input: String::new(),
                }),
            );
        }
        work.records.rounds = vec![Round {
            run: RunId(2),
            at: Seq(8),
        }];
        work.records.returned = vec![Returned {
            run: RunId(2),
            at: Seq(9),
        }];
        work.moments = [
            (1, 1000),
            (3, 5000),
            (4, 65_000),
            (5, 70_000),
            (6, 71_000),
            (7, 72_000),
            (8, 73_000),
            (9, 80_000),
        ]
        .iter()
        .map(|(from, at)| work::Moment {
            from: Seq(*from),
            at: Time(*at),
        })
        .collect();
        let reading = |asking: &str| Timed {
            at: Seq(10),
            reading: Reading {
                asking: asking.to_string(),
                source: work::Source::Fast,
                commit: "c".to_string(),
                workflow: "w.yml".to_string(),
                jobs: Vec::new(),
                tests: Vec::new(),
            },
        };
        // A run was read for call 2 and for the first round of run 2; none for call 4.
        work.timed = vec![reading("call/2"), reading("run/2/round/1")];
        let tools = |from: u64, until: u64| Stretch {
            from: Time(from),
            until: Time(until),
            went: Went::Tools,
        };
        let waits: Vec<(Stretch, bool)> = ci_waits(&work, Time(90_000));
        assert_eq!(
            waits,
            [
                // Call 2: the engine's wait for the host, from 5000 to 65_000, is no time of the tools.
                (tools(1000, 5000), true),
                (tools(65_000, 70_000), true),
                (tools(73_000, 80_000), true),
            ]
        );
        // No run was read for any: nothing waited for CI.
        work.timed.clear();
        assert!(ci_waits(&work, Time(90_000)).is_empty());
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
