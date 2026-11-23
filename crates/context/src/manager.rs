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

use decide::{BlockId, CallId, InputView, Records, RunId, Seq, VersionView, Writer};
use flow::agents::{Agents, Part};
use flow::{CellSpec, Compiled, InputKind, Listed};
use work::sessions::{began_at, number_at, of_line, of_round, start};
use work::{Ask, Back, Call, Fill, Given, Why, Work};

use crate::diff;
use crate::message::{self, By, Called, Datum, Form, Main, NewVersions, Run, Sent, Situation, Stands, Values, Version};
use crate::text::as_written;
use decide::run::Unmet;

/// Which work a round is of, and the item it is done for, by their numbers as the engine writes them (`W1`, `#1`),
/// with the item's title.
#[derive(Clone, Copy, Debug)]
pub struct For<'a> {
    pub work: &'a str,
    pub item: &'a str,
    pub title: &'a str,
}

/// What changed from an earlier version of a datum to its latest, of a datum whose versions the carrier compares
/// itself, by the datum and the earlier version.
pub type Changes = BTreeMap<(BlockId, Seq), String>;

/// What the carrier brings to an entry that neither the flow nor the work's records hold: which work the round is of;
/// what came of taking main in as the run started, where the cell takes main in and the branch did not hold it
/// already; what changed in the data whose versions the carrier compares itself; what a version is given as in place
/// of its own content; the settings the flow's words may name, with their values; and each result given otherwise than
/// the records hold it, as a result of the tool `query` is given cut.
#[derive(Clone, Copy, Debug)]
pub struct Carried<'a> {
    pub about: For<'a>,
    pub main: Option<&'a Main>,
    /// The blocks whose versions the carrier compares itself, as git compares the design tree's and the head's: what
    /// changed in one is what the carrier says, and a session that holds none of its versions is told it has read none
    /// of it.
    pub compared: &'a BTreeSet<BlockId>,
    pub changes: &'a Changes,
    /// What a version is given as in place of its own content, by the block and the version's place: the entries of
    /// the design tree a block's lines name, whole (`names_tree_entries`), as the carrier reads them. The records keep
    /// the version as it was written.
    pub given_as: &'a Changes,
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
    // The versions as the walk reads them by the flow, so that a point the owner ruled on is new to no cell here as
    // there (design document `data-and-context`, 39).
    let versions = decide::ruled_versions(&flow.flow, &records);
    let own = |v: &VersionView| matches!(v.by, Writer::Run(r) if of_cell(&records, r, cell));
    // A version is told by its number among the versions of its datum (decision
    // a-version-is-numbered-within-its-datum).
    let number_of = |block: &BlockId, at: Seq| numbered(&records, [block], at);
    let mut situation = Situation {
        after_hand_in: !held.taken.is_empty(),
        ..Situation::default()
    };
    let taken = (held.taken.iter())
        .map(|(block, at)| -> Result<(BlockId, u64), String> { Ok((block.clone(), number_of(block, *at)?)) })
        .collect::<Result<_, String>>()?;
    let mut values = Run {
        taken,
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
    let data = message::given(flow, words, cell);
    for given in &data {
        let block = &given.block;
        let all = versions.get(block).map_or(&[][..], Vec::as_slice);
        let Some(latest) = all.last() else {
            situation.data.insert(block.to_string(), Stands::Unwritten);
            continue;
        };
        let version = |v: &VersionView| -> Result<Version, String> {
            Ok(Version {
                number: number_of(block, v.at)?,
                value: v.value.clone(),
                content: given_as(work, carried, block, v.at).to_string(),
                by: by(flow, &records, block, v),
            })
        };
        let last = last_worked_from(flow, &records, cell, block, all);
        let holds = |at: Seq| held.versions.get(block).is_some_and(|h| h.contains(&at));
        let new = all
            .iter()
            .any(|v| Some(v.at) > last && v.meaning == Some(v.at) && !own(v) && !v.ruled);
        let mut datum = Datum {
            latest: version(latest)?,
            last: last.map(|at| number_of(block, at)).transpose()?,
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
            datum.versions = lacking.iter().map(|v| version(v)).collect::<Result<_, String>>()?;
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
                    (datum.last, datum.diff) = (Some(number_of(block, since)?), Some(diff));
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
    // the version the cell last worked from and each version new to it that changed its meaning, as the walk reads
    // them (design document `data-and-context`, 10 and 11).
    let ran_by = decide::told(&flow.flow, &records, run)
        .map(|t| t.ran_by)
        .unwrap_or_default();
    let seen = decide::view(&flow.flow, &records);
    let inputs: Vec<&InputView> = (seen.cells.iter())
        .find(|c| c.cell == cell.id)
        .map_or_else(Vec::new, |c| c.runs_on.iter().collect());
    for watched in cell.ran(&ran_by) {
        let of = (inputs.iter()).filter(|i| watched.blocks.contains(&i.block));
        let last = of.clone().filter_map(|i| i.worked).max();
        let mut changed: Vec<Seq> = of.flat_map(|i| i.new_versions.iter().copied()).collect();
        changed.sort();
        changed.dedup();
        // Of a group, among the versions of the group: each place that wrote any of its blocks is one.
        let new = NewVersions {
            last: last.map(|at| numbered(&records, &watched.blocks, at)).transpose()?,
            changed: (changed.into_iter())
                .map(|at| numbered(&records, &watched.blocks, at))
                .collect::<Result<_, String>>()?,
        };
        values.new.insert(watched.name.clone(), new);
        situation.new.push(watched.name.clone());
    }
    // The results the run's rounds are given, and why what its earlier rounds returned was sent back, that the session
    // lacks; the round's own reasons are always sent.
    let mut results: Vec<CallId> = Vec::new();
    let mut sent_back: Vec<usize> = Vec::new();
    // Each version given as written over, by its block and place.
    let mut over: Vec<(BlockId, Seq)> = Vec::new();
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
                for u in unmet {
                    if let Unmet::WrittenOver { block, version } = u {
                        values
                            .numbers
                            .insert((block.clone(), *version), number_of(block, *version)?);
                    }
                }
                // The version the round's own return would have written over, of a block the cell is not given, is
                // given with it where the session lacks it, so that the agent sees what it is to write over; but where
                // the context gives it already, as the latest of what the cell writes.
                for u in unmet.iter().filter(|_| k == number) {
                    let Unmet::WrittenOver { block, version } = u else {
                        continue;
                    };
                    let whole = Fill::Whole {
                        block: block.clone(),
                        version: *version,
                    };
                    let in_context = filling.get(block).is_some_and(|f| f.contains(&whole));
                    let given = in_context || cell.given.iter().any(|g| &g.block == block);
                    let held = held.versions.get(block).is_some_and(|h| h.contains(version));
                    let of = versions.get(block).and_then(|vs| vs.iter().find(|v| v.at == *version));
                    if let (false, false, Some(v)) = (given, held, of) {
                        let written = Version {
                            number: number_of(block, v.at)?,
                            value: v.value.clone(),
                            content: given_as(work, carried, block, v.at).to_string(),
                            by: by(flow, &records, block, v),
                        };
                        values.written_over.push((block.clone(), written));
                        over.push((block.clone(), v.at));
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
                fills.extend(over.iter().map(|(block, at)| Fill::Whole {
                    block: block.clone(),
                    version: *at,
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

/// The number of the datum's version written at this place, by which a round is told it: its place among the
/// datum's versions, counted from one ([`Records::number`]); of a group, among the versions of the group.
fn numbered<'b>(records: &Records, datum: impl IntoIterator<Item = &'b BlockId>, at: Seq) -> Result<u64, String> {
    let datum: Vec<&BlockId> = datum.into_iter().collect();
    let named: Vec<&str> = datum.iter().map(|b| b.as_str()).collect();
    (records.number(datum.iter().copied(), at)).ok_or_else(|| {
        format!(
            "the work has no version of {} written at place {}",
            named.join(", "),
            at.0
        )
    })
}

/// Whether the part of this name is one the session keeps: one with a `kind`.
fn kept_part(words: &Agents, name: &str) -> bool {
    (words.message.parts.iter()).any(|p| p.part.name() == name && p.kind.is_some() && p.part != Part::Context)
}

/// The content a version of a block is given as: what the carrier gives in its place, where it gives something
/// ([`Carried::given_as`]), or else its own.
fn given_as<'a>(work: &'a Work, carried: &'a Carried<'_>, block: &BlockId, at: Seq) -> &'a str {
    match carried.given_as.get(&(block.clone(), at)) {
        Some(text) => text,
        None => content(work, block, at),
    }
}

/// The content of a version of a block; nothing where the work keeps none.
fn content<'a>(work: &'a Work, block: &BlockId, at: Seq) -> &'a str {
    work.contents.get(&(block.clone(), at)).map_or("", String::as_str)
}

/// Whether the run is one of the cell's.
fn of_cell(records: &Records, run: RunId, cell: &CellSpec) -> bool {
    (records.started.iter()).any(|s| s.run == run && s.cell == cell.id)
}

/// Who wrote a version: a run, the owner, or nobody. A round is told the run's line, and the work's page the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Author {
    Run(RunId),
    Owner,
    Nobody,
}

/// Who wrote a version, worked out from the records and the flow, never stored (design document `data-and-context`,
/// 29): a run's version by its run; a person's by the owner. Of what the outside writes, the owner's words by the owner;
/// a version of a cell's questions by its own record, which the engine writes in the same act, just before it: an ask by
/// the asking run, an answer by the owner, and the engine's own question by nobody. Anything else the outside wrote,
/// read of the branch and put into a round or not, and what an act of the engine wrote, by nobody (decision
/// a-version-read-at-a-rounds-end-is-that-rounds).
pub fn author(flow: &Compiled, records: &Records, block: &BlockId, version: &VersionView) -> Author {
    let from = |block: &BlockId| (flow.blocks.get(block)).and_then(|b| b.writers.outside.clone());
    author_from(&from, records, block, version)
}

/// Who wrote a version, as [`author`] says, where each block written from outside the work comes from as `from` says:
/// the flow of the version a work is on, read where that version is no longer installed.
pub fn author_from(
    from: &dyn Fn(&BlockId) -> Option<String>,
    records: &Records,
    block: &BlockId,
    version: &VersionView,
) -> Author {
    match version.by {
        Writer::Run(run) => Author::Run(run),
        Writer::Person => Author::Owner,
        Writer::Outside => {
            let source = from(block);
            match source.as_deref() {
                Some(flow::WORDS) => Author::Owner,
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
                            true => Author::Nobody,
                            false => Author::Run(q.run),
                        },
                        _ if answered.is_some() => Author::Owner,
                        _ => Author::Nobody,
                    }
                }
                _ => Author::Nobody,
            }
        }
    }
}

/// Who wrote a version, as a round is told it: the line of the cell of the run that wrote it, as the flow declares it,
/// nobody where the cell is on no line; the owner; or nobody ([`author`]).
fn by(flow: &Compiled, records: &Records, block: &BlockId, version: &VersionView) -> By {
    match author(flow, records, block, version) {
        Author::Run(run) => {
            let cell = (records.started.iter()).find(|s| s.run == run).map(|s| &s.cell);
            let line = cell.and_then(|cell| flow.cells.iter().find(|c| &c.id == cell)?.line.clone());
            line.map_or(By::Nobody, By::Line)
        }
        Author::Owner => By::Owner,
        Author::Nobody => By::Nobody,
    }
}

/// The version of the datum the cell last worked from (design document `data-and-context`, 10). Where the datum runs
/// the cell, it is the walk's own ([`decide::InputView::worked`]), which a cause records as a run begins: the latest of
/// the one the cell consumed, one a run of it wrote and one the owner ruled on. Of another datum it is the latest its
/// last finished run was given, which is the one written last before that run's last round began, every round being
/// filled with the latest; or the latest a run of the cell wrote, where that is later, a cell's own version being no
/// change to it; or the latest of the block's `versions` the owner ruled on, where that is later, as the walk reads
/// them: a point the owner ruled on is new to no cell (39), so what came before it is no change to the cell.
fn last_worked_from(
    flow: &Compiled,
    records: &Records,
    cell: &CellSpec,
    block: &BlockId,
    versions: &[VersionView],
) -> Option<Seq> {
    let finished = (records.finished.iter()).filter(|f| of_cell(records, f.run, cell));
    let runs_it = (flow.flow.place(&cell.id)).is_some_and(|p| flow.flow.cells()[p].runs_on.contains(block));
    if runs_it {
        // The walk's own reading, which a cause records alike.
        let view = decide::view(&flow.flow, records);
        let of = view.cells.into_iter().find(|c| c.cell == cell.id);
        return of
            .and_then(|c| c.runs_on.into_iter().find(|i| &i.block == block))
            .and_then(|i| i.worked);
    }
    let wrote = (finished.clone())
        .filter(|f| f.writes.iter().any(|w| &w.block == block))
        .map(|f| f.at)
        .max();
    let last = finished.max_by_key(|f| f.at);
    let round = last.and_then(|f| (records.rounds.iter()).filter(|r| r.run == f.run).map(|r| r.at).max());
    let all = decide::versions(records).remove(block).unwrap_or_default();
    let given = round.and_then(|round| all.iter().map(|v| v.at).filter(|at| *at < round).max());
    let ruled = (versions.iter()).filter(|v| v.ruled).map(|v| v.at).max();
    given.max(wrote).max(ruled)
}

/// A call whose result the round is given, as the carrier gives it, or else as the work holds it.
fn result(work: &Work, call: CallId, given: &BTreeMap<CallId, String>) -> message::Result {
    let called = match work.calls.get(&call) {
        Some(Call::Asked(Ask::Tool { name, input })) => Called::Tool {
            name: name.clone(),
            input: input.clone(),
        },
        Some(Call::AtLimit(_)) => Called::AtLimit,
        // The engine's wait before a round is tried again is no result a round is given, nor is a question for a person,
        // which no round is given a result of now.
        Some(Call::Asked(Ask::Person { .. }) | Call::Wait { .. }) | None => Called::Unknown(call),
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
    // What the session took in at a place before a move it holds only where each move since left the datum's name as it
    // was: a datum a move renamed, or whose name it gave another's, is given again whole (decision
    // a-datum-a-move-renamed-is-given-again-whole). What was filled keeps the name it was filled under; what was put in
    // and handed in is read through the moves, by the name it has now.
    let since = |at: Seq| {
        (work.readings.iter())
            .filter(move |read| at < read.at)
            .map(|read| &read.reading)
    };
    let given_to = |reading: &decide::Reading, block: &BlockId| reading.blocks.values().any(|b| b == block);
    let filled_kept = |block: &BlockId, at: Seq| since(at).all(|r| !r.renames(block) && !given_to(r, block));
    let read_kept = |block: &BlockId, at: Seq| since(at).all(|r| !given_to(r, block));
    for f in &work.filled {
        let in_it = rounds.iter().any(|(r, k, ..)| *r == f.run && *k == f.round);
        if !in_it || f.at <= top || f.at >= began || !reached(f.run, f.round) {
            continue;
        }
        for fill in &f.fills {
            match fill {
                Fill::Whole { block, version } | Fill::Change { block, version, .. } => {
                    if filled_kept(block, f.at) {
                        hold(&mut held, block, *version);
                    }
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
            for p in records.put_in.iter().filter(into).filter(|p| read_kept(&p.block, p.at)) {
                hold(&mut held, &p.block, p.version);
            }
        }
        let begun = records.rounds.iter().filter(|x| x.run == *r).count();
        let finished = (records.finished.iter()).find(|f| f.run == *r && *k == begun && f.at > *ended);
        let mut taken = Vec::new();
        if let Some(f) = finished.filter(|f| f.at > top && f.at < began) {
            for w in f.writes.iter().filter(|w| read_kept(&w.block, f.at)) {
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
