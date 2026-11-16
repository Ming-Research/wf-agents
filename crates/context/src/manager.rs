//! The context manager (design document `data-and-context`, 12 to 20): for a round of an agent's cell, the session it
//! runs in, which versions of which data that session holds, and what is to be filled into it, as a function of the
//! flow and the work's records alone.
//!
//! What a session holds is read from what was filled into it, as each try of a round recorded it, and from what its
//! rounds handed in, since the session last held nothing: since it began, since the agents' program lost it and it was
//! begun again under its id, or since a round of it was compacted, which counts as the session reopened and leaves it
//! holding only what that round then handed in. Only a round whose program put its message into the session put
//! anything into it: one that reached the model, and one whose program said it had taken the message in
//! (`work::sessions::wrote_into`). A version is held by what it is, never by its content: a new version is new, though
//! it says what an earlier one said, as a person saying the same twice, or a datum changed and changed back.
//! Everything is read as the records stood when the round began, so that a round given again is sent the same.
//!
//! The round is filled with what its cell is given and its session lacks, in the order of the message: which work it
//! is, each datum of the cell's context by the kind the cell gives it in, in the order the `context` part names, then
//! the latest of each block the cell writes that nothing else gives it, what the cell returns, its task, and the
//! results and the reasons for sending back that the round is given. A datum with no version yet is left out, and so
//! is one the session holds, but for the line that says so where it is new to the cell.
//!
//! What changed in a datum given `last_and_latest` is what differs in its text, but for a datum whose versions the
//! carrier compares itself, as git compares the versions of the design tree: the carrier gives what changed from each
//! earlier version to the latest ([`Changes`]), which is sent as it is, and from the version the session holds where the
//! cell last worked from none, since such a datum stands for what was read into the session; where the session holds
//! none of it, its latest is sent whole, saying so (design document `data-and-context`, 23).

use std::collections::{BTreeMap, BTreeSet};

use decide::{BlockId, CallId, Records, RunId, Seq, VersionView, Writer};
use flow::agents::{Agents, Part};
use flow::{CellSpec, Compiled, InputKind, Listed};
use work::sessions::{began_at, number_at, of_line, of_round, start};
use work::{Ask, Back, Call, Fill, Given, Why, Work};

use crate::diff;
use crate::message::{self, By, Called, Datum, Form, Main, NewVersions, Run, Sent, Situation, Stands, Values, Version};
use crate::text::as_written;
use decide::run::Unmet;

/// Which work a round is of, and the item it is done for, by their numbers, with the item's title.
#[derive(Clone, Copy, Debug)]
pub struct For<'a> {
    pub work: u64,
    pub item: u64,
    pub title: &'a str,
}

/// What changed from an earlier version of a datum to its latest, of a datum whose versions the carrier compares
/// itself, by the datum and the earlier version.
pub type Changes = BTreeMap<(BlockId, Seq), String>;

/// What the carrier brings to an entry that neither the flow nor the work's records hold: which work the round is of;
/// what came of taking main in as the run started, where the cell takes main in and the branch did not hold it
/// already; what changed in the data whose versions the carrier compares itself; the settings the flow's words may
/// name, with their values; and each result given otherwise than the records hold it, as a result of the tool `query`
/// is given cut.
#[derive(Clone, Copy, Debug)]
pub struct Carried<'a> {
    pub about: For<'a>,
    pub main: Option<&'a Main>,
    /// The blocks whose versions the carrier compares itself, as git compares the design tree's: what changed in one
    /// is what the carrier says, and a session that holds none of its versions is told it has read none of it.
    pub compared: &'a BTreeSet<BlockId>,
    pub changes: &'a Changes,
    pub named: &'a flow::system::Settings,
    pub results: &'a BTreeMap<CallId, String>,
}

/// What a round is sent as its cell is entered, and what that fills into its session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entered {
    /// The situation the entry is in.
    pub situation: Situation,
    /// What fills the places of its message.
    pub run: Run,
    /// The message, part by part.
    pub sent: Vec<Sent>,
    /// The message as one text.
    pub text: String,
    /// What the message fills into the session, in the order it does.
    pub fills: Vec<Fill>,
}

/// What the round of this number of the run is sent, in the session it runs in as that session stood when the round
/// began; or, `anew`, in a session that holds nothing, as one begun again under the round, or compacted during it,
/// does.
pub fn enter(
    flow: &Compiled,
    words: &Agents,
    work: &Work,
    run: RunId,
    number: usize,
    carried: &Carried<'_>,
    anew: bool,
) -> Result<Entered, String> {
    let (about, changes) = (carried.about, carried.changes);
    let started = (work.records.started.iter())
        .find(|s| s.run == run)
        .ok_or_else(|| format!("the work has no run {}", run.0))?;
    let cell = (flow.cells.iter())
        .find(|c| c.id == started.cell)
        .ok_or_else(|| format!("the flow has no cell {}", started.cell))?;
    let line = (cell.line.as_deref()).ok_or_else(|| format!("the cell {} names no line", cell.id))?;
    let began = began_at(work, run, number).unwrap_or_else(|| work.tick());
    let records = work.records.before(began);
    let held = match anew {
        true => Held::default(),
        false => held(flow, work, line, run, number, began),
    };
    let versions = decide::versions(&records);
    let own = |v: &VersionView| matches!(v.by, Writer::Run(r) if of_cell(&records, r, cell));
    let mut situation = Situation {
        after_hand_in: !held.taken.is_empty(),
        ..Situation::default()
    };
    let mut values = Run {
        taken: held.taken.clone(),
        work: about.work.to_string(),
        item: about.item.to_string(),
        item_title: about.title.to_string(),
        main: carried.main.cloned(),
        named: carried.named.clone(),
        ..Run::default()
    };
    situation.main = values.main.as_ref().map(|m| m.key().to_string());
    // What each datum fills, by its name.
    let mut filling: BTreeMap<&BlockId, Vec<Fill>> = BTreeMap::new();
    for given in &message::given(flow, words, cell) {
        let block = &given.block;
        let all = versions.get(block).map_or(&[][..], Vec::as_slice);
        let Some(latest) = all.last() else {
            situation.data.insert(block.to_string(), Stands::Unwritten);
            continue;
        };
        let version = |v: &VersionView| Version {
            number: v.at,
            value: v.value.clone(),
            content: content(work, block, v.at).to_string(),
            by: by(flow, &records, block, v),
        };
        let last = last_worked_from(flow, &records, cell, block);
        let holds = |at: Seq| held.versions.get(block).is_some_and(|h| h.contains(&at));
        let new = all
            .iter()
            .any(|v| Some(v.at) > last && v.meaning == Some(v.at) && !own(v));
        let mut datum = Datum {
            latest: version(latest),
            last,
            ..Datum::default()
        };
        let whole = |at: Seq| {
            vec![Fill::Whole {
                block: block.clone(),
                version: at,
            }]
        };
        let lacking: Vec<&VersionView> = all.iter().filter(|v| !holds(v.at)).collect();
        let compared = carried.compared.contains(block);
        // A latest version the session holds is never sent again, though the session lacks the version the cell last
        // worked from: the change could not be told against it, and the whole would give the session what it holds.
        let (stands, fills) = if given.list == Listed::Outputs {
            // What the cell writes and is given by nothing else is sent only where the session lacks its latest.
            match holds(latest.at) {
                true => (Stands::Holds, Vec::new()),
                false => (Stands::Lacks, whole(latest.at)),
            }
        } else if given.kind == InputKind::All && !lacking.is_empty() {
            datum.versions = lacking.iter().map(|v| version(v)).collect();
            let fills = lacking.iter().flat_map(|v| whole(v.at)).collect();
            let stands = if holds(all[0].at) {
                Stands::HoldsLast
            } else {
                Stands::Lacks
            };
            (stands, fills)
        } else if holds(latest.at) {
            // A datum the cell never worked from is new to it whole, the line that it is held naming its version.
            let stands = match last {
                None => Stands::HoldsNew,
                Some(_) if new => Stands::HoldsLatest,
                Some(_) => Stands::Holds,
            };
            (stands, Vec::new())
        } else {
            let held_before = || all.iter().rev().map(|v| v.at).find(|at| *at < latest.at && holds(*at));
            let since = match last.filter(|l| holds(*l) && *l < latest.at) {
                None if compared => held_before(),
                since => since,
            };
            let change = match (given.kind, since) {
                // What the carrier gives is sent as it is, however long: the versions' contents do not say it.
                (InputKind::LastAndLatest, Some(since)) if compared => {
                    changes.get(&(block.clone(), since)).map(|c| (since, c.clone()))
                }
                (InputKind::LastAndLatest, Some(since)) => {
                    diff::diff(content(work, block, since), &datum.latest.content)
                        .filter(|d| d.len() <= datum.latest.content.len())
                        .map(|d| (since, d))
                }
                _ => None,
            };
            match change {
                Some((since, diff)) => {
                    // The change is told from the version it is from.
                    (datum.last, datum.diff) = (Some(since), Some(diff));
                    let fill = Fill::Change {
                        block: block.clone(),
                        version: latest.at,
                        since,
                    };
                    (Stands::HoldsLast, vec![fill])
                }
                // Where the change is not sent, the latest is sent whole, as where the session lacks what the cell
                // last worked from; of a datum the carrier compares, saying so where the session holds none of it.
                None if given.kind == InputKind::LastAndLatest && compared && lacking.len() == all.len() => {
                    (Stands::Unread, whole(latest.at))
                }
                None if given.kind == InputKind::LastAndLatest => (Stands::Lacks, whole(latest.at)),
                None if since.is_some() => (Stands::HoldsLast, whole(latest.at)),
                None => (Stands::Lacks, whole(latest.at)),
            }
        };
        situation.data.insert(block.to_string(), stands);
        values.data.insert(block.clone(), datum);
        filling.insert(block, fills);
    }
    // What the cell watches that is new to it this entry: each datum, or group of which a block, ran the round, with
    // the version the cell last worked from and the latest (design document `data-and-context`, 11).
    let ran_by = decide::told(&flow.flow, &records, run)
        .map(|t| t.ran_by)
        .unwrap_or_default();
    for watched in cell.ran(&ran_by) {
        let latest = (watched.blocks.iter())
            .filter_map(|b| versions.get(b).and_then(|vs| vs.last()).map(|v| v.at))
            .max();
        let last = (watched.blocks.iter())
            .filter_map(|b| last_worked_from(flow, &records, cell, b))
            .max();
        if let Some(latest) = latest {
            values.new.insert(watched.name.clone(), NewVersions { last, latest });
        }
        situation.new.push(watched.name.clone());
    }
    // The results the run's rounds are given, and why what its earlier rounds returned was sent back, that the session
    // lacks; the round's own reasons are always sent.
    let mut results: Vec<CallId> = Vec::new();
    let mut sent_back: Vec<usize> = Vec::new();
    for k in 1..=number {
        let given = if k == number {
            work.given.get(&(run, k))
        } else {
            // A round that failed is given again by the next, which tells what it was given.
            let failed = matches!(work.back.get(&(run, k)), Some(Back::Failed { .. }));
            work.given.get(&(run, k)).filter(|_| !failed)
        };
        match given {
            Some(Given::Results(calls)) => {
                for call in calls {
                    if !results.contains(call) && !held.results.contains(call) {
                        results.push(*call);
                    }
                }
            }
            Some(Given::SentBack(unmet)) if k == number || !held.sent_back.contains(&k) => {
                sent_back.push(k);
                let round = (k < number).then_some(k);
                values.sent_back.push((round, unmet.clone()));
                // The version the round's own return would have written over, of a block the cell is not given, is
                // given with it where the session lacks it, so that the agent sees what it is to write over.
                for u in unmet.iter().filter(|_| k == number) {
                    let Unmet::WrittenOver { block, version } = u else {
                        continue;
                    };
                    let given = cell.given.iter().any(|g| &g.block == block);
                    let held = held.versions.get(block).is_some_and(|h| h.contains(version));
                    let of = versions.get(block).and_then(|vs| vs.iter().find(|v| v.at == *version));
                    if let (false, false, Some(v)) = (given, held, of) {
                        let written = Version {
                            number: v.at,
                            value: v.value.clone(),
                            content: content(work, block, v.at).to_string(),
                            by: by(flow, &records, block, v),
                        };
                        values.written_over.push((block.clone(), written));
                    }
                }
            }
            _ => {}
        }
    }
    values.results = results.iter().map(|c| result(work, *c, carried.results)).collect();
    situation.results = values.results.iter().map(|r| r.call.key().to_string()).collect();
    situation.sent_back = !sent_back.is_empty();
    // The parts the session keeps that it holds, by their text as the round fills them.
    for (part, text) in message::kept(flow, words, cell, Values::Run(&values)) {
        if held.parts.contains(&Fill::part(&part, &text)) {
            situation.holds.push(part);
        }
    }
    let sent = message::message(flow, words, cell, &situation, Values::Run(&values));
    let mut fills = Vec::new();
    for s in &sent {
        match s.part.as_str() {
            name if name == Part::Context.name() => {
                let filled = s.block.as_ref().and_then(|b| filling.get(&BlockId::from(b.as_str())));
                fills.extend(filled.into_iter().flatten().cloned());
            }
            name if name == Part::Results.name() => fills.push(Fill::Results(results.clone())),
            name if name == Part::SentBack.name() => {
                fills.extend(sent_back.iter().map(|round| Fill::SentBack { round: *round }));
                fills.extend(values.written_over.iter().map(|(block, v)| Fill::Whole {
                    block: block.clone(),
                    version: v.number,
                }));
            }
            name if s.form == Form::Text && kept_part(words, name) => {
                fills.push(Fill::part(name, &as_written(&s.pieces)));
            }
            _ => {}
        }
    }
    Ok(Entered {
        text: message::text(&sent),
        situation,
        run: values,
        sent,
        fills,
    })
}

/// Whether the part of this name is one the session keeps: one with a `kind`.
fn kept_part(words: &Agents, name: &str) -> bool {
    (words.message.parts.iter()).any(|p| p.part.name() == name && p.kind.is_some() && p.part != Part::Context)
}

/// The content of a version of a block; nothing where the work keeps none.
fn content<'a>(work: &'a Work, block: &BlockId, at: Seq) -> &'a str {
    work.contents.get(&(block.clone(), at)).map_or("", String::as_str)
}

/// Whether the run is one of the cell's.
fn of_cell(records: &Records, run: RunId, cell: &CellSpec) -> bool {
    (records.started.iter()).any(|s| s.run == run && s.cell == cell.id)
}

/// The version of the datum the cell last worked from: the latest it consumed, where the datum runs the cell (design
/// document `data-and-context`, 10); of another, the latest its last finished run was given, which is the one written
/// last before that run's last round began, every round being filled with the latest; and in either case the latest
/// a run of the cell wrote, where that is later, a cell's own version being no change to it.
/// Who wrote a version, as a round is told it, worked out from the records and the flow, never stored (design document
/// `data-and-context`, 29): a run's version by its cell's line, as the flow declares it, nobody where the cell is on no
/// line; a person's by the owner. Of what the outside writes, the owner's words by the owner; a version of a cell's
/// questions by its own record, which the engine writes in the same act, just before it: an ask by the asking run's
/// line, an answer by the owner, and the engine's own question by nobody; and what a round's end read, the design tree
/// and the work's head among it, by that round's line, as the version put into it says. Anything else the outside
/// wrote, and what an act of the engine wrote, by nobody.
fn by(flow: &Compiled, records: &Records, block: &BlockId, version: &VersionView) -> By {
    let line_of_run = |run: RunId| {
        let cell = (records.started.iter()).find(|s| s.run == run).map(|s| &s.cell);
        let line = cell.and_then(|cell| flow.cells.iter().find(|c| &c.id == cell)?.line.clone());
        line.map_or(By::Nobody, By::Line)
    };
    match version.by {
        Writer::Run(run) => line_of_run(run),
        Writer::Person => By::Owner,
        Writer::Outside => {
            let source = (flow.blocks.get(block)).and_then(|b| b.writers.outside.as_deref());
            match source {
                Some(flow::WORDS) => By::Owner,
                Some(flow::ANSWER) => {
                    let asked = (records.questions.iter())
                        .filter(|q| q.at < version.at)
                        .max_by_key(|q| q.at);
                    let answered = (records.answers.iter())
                        .filter(|a| a.at < version.at)
                        .map(|a| a.at)
                        .max();
                    match asked {
                        Some(q) if answered.is_none_or(|a| a < q.at) => match q.engine {
                            true => By::Nobody,
                            false => line_of_run(q.run),
                        },
                        _ if answered.is_some() => By::Owner,
                        _ => By::Nobody,
                    }
                }
                _ => (records.put_in.iter())
                    .find(|p| &p.block == block && p.version == version.at)
                    .map_or(By::Nobody, |p| line_of_run(p.run)),
            }
        }
    }
}

fn last_worked_from(flow: &Compiled, records: &Records, cell: &CellSpec, block: &BlockId) -> Option<Seq> {
    let finished = (records.finished.iter()).filter(|f| of_cell(records, f.run, cell));
    let wrote = (finished.clone())
        .filter(|f| f.writes.iter().any(|w| &w.block == block))
        .map(|f| f.at)
        .max();
    let runs_it = (flow.flow.place(&cell.id)).is_some_and(|p| flow.flow.cells()[p].runs_on.contains(block));
    let worked = if runs_it {
        let view = decide::view(&flow.flow, records);
        let of = view.cells.into_iter().find(|c| c.cell == cell.id);
        of.and_then(|c| c.runs_on.into_iter().find(|i| &i.block == block))
            .and_then(|i| i.consumed)
    } else {
        let last = finished.max_by_key(|f| f.at);
        let round = last.and_then(|f| (records.rounds.iter()).filter(|r| r.run == f.run).map(|r| r.at).max());
        let versions = decide::versions(records).remove(block).unwrap_or_default();
        round.and_then(|round| versions.iter().map(|v| v.at).filter(|at| *at < round).max())
    };
    worked.max(wrote)
}

/// A call whose result the round is given, as the carrier gives it, or else as the work holds it.
fn result(work: &Work, call: CallId, given: &BTreeMap<CallId, String>) -> message::Result {
    let called = match work.calls.get(&call) {
        Some(Call::Asked(Ask::Tool { name, input })) => Called::Tool {
            name: name.clone(),
            input: input.clone(),
        },
        Some(Call::Asked(Ask::Person { question, .. })) => Called::Question(question.clone()),
        Some(Call::AtLimit(_)) => Called::AtLimit,
        // The engine's wait before a round is tried again is no result a round is given.
        Some(Call::Wait { .. }) | None => Called::Unknown(call),
    };
    message::Result {
        call: called,
        result: given.get(&call).or_else(|| work.results.get(&call)).cloned(),
    }
}

/// What a session holds as a round of it begins.
#[derive(Clone, Debug, Default)]
struct Held {
    /// The versions of each block.
    versions: BTreeMap<BlockId, BTreeSet<Seq>>,
    /// The parts of the message it keeps, as they were filled.
    parts: Vec<Fill>,
    /// The calls whose results it was given.
    results: BTreeSet<CallId>,
    /// The rounds of the round's own run whose reasons for sending back it was given, by their numbers.
    sent_back: BTreeSet<usize>,
    /// What the last return in it handed in: each block, with the version it is kept as.
    taken: Vec<(BlockId, Seq)>,
}

/// What the session the round of this number of the run runs in, on this line, holds as the round begins, at this
/// place.
fn held(flow: &Compiled, work: &Work, line: &str, run: RunId, number: usize, began: Seq) -> Held {
    let records = &work.records;
    let on_line = |r: RunId| {
        let cell = records.started.iter().find(|s| s.run == r).map(|s| &s.cell);
        let spec = cell.and_then(|cell| flow.cells.iter().find(|c| c.id == *cell));
        spec.and_then(|c| c.line.as_deref()) == Some(line)
    };
    let (session, begins) = of_round(work, line, on_line, run, number);
    if begins {
        return Held::default();
    }
    // Each round of the line that ran in the session and ended before this one began, with where it began and ended.
    let mut rounds: Vec<(RunId, usize, Seq, Seq)> = Vec::new();
    let mut counted: BTreeMap<RunId, usize> = BTreeMap::new();
    for round in &records.rounds {
        let k = counted.entry(round.run).or_default();
        *k += 1;
        let ended = (records.returned.iter()).filter(|r| r.run == round.run).nth(*k - 1);
        if let Some(ended) = ended
            && ended.at < began
            && on_line(round.run)
            && number_at(work, line, start(work, round.run, *k)) == session
        {
            rounds.push((round.run, *k, round.at, ended.at));
        }
    }
    // Where the session last held nothing: where it began, or where it was reopened under its id, begun again or
    // compacted, a compaction during a round leaving it what that round then handed in.
    let own: Vec<Seq> = (of_line(work, line)).filter(|s| s.why.own_id()).map(|s| s.at).collect();
    let mut top = match session {
        0 => Seq(0),
        n => own.get(n as usize - 1).copied().unwrap_or(Seq(0)),
    };
    for s in of_line(work, line) {
        let under_its_id = matches!(s.why, Why::Lost | Why::Compacted);
        if under_its_id && s.at < began && number_at(work, line, s.at) == session {
            top = top.max(s.at);
        }
    }
    let reached = |r: RunId, k: usize| work::sessions::wrote_into(work, r, k);
    let mut held = Held::default();
    let hold = |held: &mut Held, block: &BlockId, at: Seq| {
        held.versions.entry(block.clone()).or_default().insert(at);
    };
    for f in &work.filled {
        let in_it = rounds.iter().any(|(r, k, ..)| *r == f.run && *k == f.round);
        if !in_it || f.at <= top || f.at >= began || !reached(f.run, f.round) {
            continue;
        }
        for fill in &f.fills {
            match fill {
                Fill::Whole { block, version } | Fill::Change { block, version, .. } => {
                    hold(&mut held, block, *version);
                }
                Fill::Part { .. } => held.parts.push(fill.clone()),
                Fill::Results(calls) => held.results.extend(calls.iter().copied()),
                Fill::SentBack { round } if f.run == run => {
                    held.sent_back.insert(*round);
                }
                Fill::SentBack { .. } => {}
            }
        }
    }
    // What the rounds handed in, and what was put into each that reached the model while it ran; and what the last
    // return in the session handed in, a round that failed having returned nothing.
    let mut last: Option<(Seq, Vec<(BlockId, Seq)>)> = None;
    for (r, k, from, ended) in &rounds {
        let into = |p: &&decide::PutIn| p.run == *r && p.at > *from && p.at < *ended && p.at > top;
        if reached(*r, *k) {
            for p in records.put_in.iter().filter(into) {
                hold(&mut held, &p.block, p.version);
            }
        }
        let begun = records.rounds.iter().filter(|x| x.run == *r).count();
        let finished = (records.finished.iter()).find(|f| f.run == *r && *k == begun && f.at > *ended);
        let mut taken = Vec::new();
        if let Some(f) = finished.filter(|f| f.at > top && f.at < began) {
            for w in &f.writes {
                hold(&mut held, &w.block, f.at);
                taken.push((w.block.clone(), f.at));
            }
        }
        let returned = !matches!(work.back.get(&(*r, *k)), Some(Back::Failed { .. }) | None);
        if returned && *ended >= top && last.as_ref().is_none_or(|(at, _)| at < ended) {
            last = Some((*ended, taken));
        }
    }
    held.taken = last.map(|(_, taken)| taken).unwrap_or_default();
    held
}
