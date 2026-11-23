//! What the owner says to a work, and what its cells answer (design document `data-and-context`, 35).
//!
//! A cell that may ask has a datum of its own of the owner's words, a dialogue with the owner, which it watches: the
//! block among its outputs that the engine's source of the owner's words writes ([`flow::WORDS`], [`words_of`]). The
//! owner speaks to a cell, or to a line of the work's flow, and the engine alone chooses the cell the words reach
//! ([`receiver`]): words to a cell go to that cell; words to a line go to the cell the line has reached
//! (`decide::reached`), of those of it that have words of their own. Where that cell cannot take them, as one with no
//! words of its own or one that does not apply now, or where the work has ended, the words are refused, saying why,
//! and go to no other cell.
//!
//! The words are the next version of the cell's words, in the words of that datum's `owner`, which name no cell, and
//! which runs the cell, tells a round under way of a later cell to hand in what it has and enters the cell while it
//! waits, as the owner turning to a cell does. Where a round of the cell is under way, its carrier is given the version to hand to the round after
//! its next use of a tool ([`Effect::Tell`]); only once it says the round was given it ([`Event::SaidToRound`]) is the
//! version put into the round, so that a round consumes only words that reached it and a session is not given again
//! what its round was told. A cell answers by writing its words in its return's `outputs`, as it writes any block, and
//! the answer is their version as its run finishes, in the words of that datum's `reply` ([`entry`]), which name no
//! cell either: the run that wrote it is the cell's.
//!
//! [`Event::SaidToRound`]: crate::Event::SaidToRound

use std::borrow::Cow;

use decide::{BlockId, CellId, Decision, Hurried, Outsider, PutIn, RunId, Seq, Written, decide};
use flow::Compiled;
use flow::agents::fill;
use work::Work;

use crate::asked;
use crate::event::{Effect, Refused};

/// The text of a cell's words that is the owner's version: `{words}` the words.
pub const OWNER: &str = "owner";
/// The text of a cell's words that is the cell's answer: `{reply}` its answer.
pub const REPLY: &str = "reply";

/// What the owner said to a round under way, as its carrier is to hand it in ([`Effect::Tell`]): the owner's version of
/// the cell's words, the datum and the version's number among its versions, which the round is told, what the cell
/// does when its words are new, and the versions, by their places, that are put into the round once it was given them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Telling {
    pub entry: String,
    pub block: BlockId,
    pub number: u64,
    pub does: Option<String>,
    pub versions: Vec<(BlockId, Seq)>,
    /// The round is told to hand in what it has now, because the owner turned to a cell before its own, and not to
    /// answer words said to its own cell.
    pub hurry: bool,
}

/// What the owner's words are said to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum To {
    /// A line of the work's flow, by its name.
    Line(String),
    /// A cell of the work's flow, by its id.
    Cell(CellId),
}

/// Why no cell can take the owner's words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoOne {
    /// The work has ended.
    Ended,
    /// The flow has no cell on such a line.
    NoSuchLine(String),
    /// The flow has no cell of this id.
    NoSuchCell(CellId),
    /// No cell of the line has words of its own.
    NoneOfTheLine(String),
    /// No cell of the line that has words of its own has run, and the work stands at none of them.
    NotReached(String),
    /// The words would go to this cell, and it has no words of its own.
    NotSpokenTo(CellId),
    /// The words would go to this cell, and its condition is not met now: what runs it would not run it.
    Skipped(CellId),
}

/// The cell's own words, where it has them: the block among its outputs that the engine's source of the owner's words
/// writes.
pub fn words_of<'f>(flow: &'f Compiled, cell: &CellId) -> Option<&'f BlockId> {
    asked::own(flow, cell, flow::WORDS)
}

/// The cell the owner's words said to this reach, as the engine chooses it, or why no cell can take them.
pub fn receiver(flow: &Compiled, work: &Work, to: &To) -> Result<CellId, NoOne> {
    let records = &work.records;
    if matches!(decide(&flow.flow, records), Decision::Ended(_)) {
        return Err(NoOne::Ended);
    }
    let cell = match to {
        To::Cell(cell) => {
            if flow.flow.place(cell).is_none() {
                return Err(NoOne::NoSuchCell(cell.clone()));
            }
            cell.clone()
        }
        To::Line(line) => {
            let of_line: Vec<&CellId> = (flow.cells.iter())
                .filter(|c| c.line.as_deref() == Some(line.as_str()))
                .map(|c| &c.id)
                .collect();
            if of_line.is_empty() {
                return Err(NoOne::NoSuchLine(line.clone()));
            }
            // Only a cell with words of its own counts as where the line stands: one with none, as the reading of the
            // tree a reopened session begins with, is passed over.
            let can: Vec<CellId> = (of_line.into_iter())
                .filter(|c| words_of(flow, c).is_some())
                .cloned()
                .collect();
            if can.is_empty() {
                return Err(NoOne::NoneOfTheLine(line.clone()));
            }
            decide::reached(&flow.flow, records, &can).ok_or_else(|| NoOne::NotReached(line.clone()))?
        }
    };
    if words_of(flow, &cell).is_none() {
        return Err(NoOne::NotSpokenTo(cell));
    }
    if !decide::applies(&flow.flow, records, &cell) {
        return Err(NoOne::Skipped(cell));
    }
    Ok(cell)
}

/// Takes the owner's words: written as the module says, to the cell [`receiver`] chooses, and, where a round of that
/// cell is under way, given to be told it; or why no cell can take them.
pub(crate) fn said(
    flow: &Compiled,
    work: &mut Work,
    to: To,
    words: String,
    effects: &mut Vec<Effect>,
) -> Result<(), Refused> {
    let cell = receiver(flow, work, &to).map_err(Refused::NoOneSpokenTo)?;
    let block = words_of(flow, &cell)
        .expect("the receiver has words of its own")
        .clone();
    let entry = fill(
        flow.blocks[&block].writes.get(OWNER).unwrap_or_default(),
        &[("words", words.as_str())],
    );
    let number = work.records.next_number([&block]);
    let at = write(work, &block, entry.clone());
    // A round of the cell under way is told of them as it runs; they are put into it once it was.
    if let Some(run) = under_way(work, &cell) {
        let watched = (flow.cells.iter())
            .find(|c| c.id == cell)
            .and_then(|c| c.watches.iter().find(|w| w.name == block.as_str()));
        let telling = Telling {
            entry,
            block: block.clone(),
            number,
            does: watched.and_then(|w| w.does.clone()),
            versions: vec![(block, at)],
            hurry: false,
        };
        effects.push(Effect::Tell {
            run,
            telling: Box::new(telling),
        });
    }
    Ok(())
}

/// Begins to tell the round of the run, which is under way, to hand in what it has now, for this version of a block the
/// owner wrote to turn to a cell before the run's: records that it is told, which the walk reads so that a round is
/// told once for a version, and gives its carrier the version to hand to the round after its next use of a tool
/// ([`Effect::Tell`]). The round is put the version only once its carrier says it was given it ([`heard`]).
pub(crate) fn hurry(work: &mut Work, run: RunId, block: BlockId, version: Seq, effects: &mut Vec<Effect>) {
    let at = work.tick();
    work.records.hurried.push(Hurried {
        run,
        block: block.clone(),
        version,
        at,
    });
    // The version is one the walk read among the writes of the block, so its content and its number are kept.
    let entry = (work.contents.get(&(block.clone(), version)))
        .cloned()
        .expect("the content of a version written is kept");
    let number = (work.records.number([&block], version)).expect("a version written has a number among its block's");
    let telling = Telling {
        entry,
        block: block.clone(),
        number,
        does: None,
        versions: vec![(block, version)],
        hurry: true,
    };
    effects.push(Effect::Tell {
        run,
        telling: Box::new(telling),
    });
}

/// Puts into the round of the run that is under way these versions, which the engine gave its carrier to tell the
/// round ([`Effect::Tell`]) and the carrier says the round was given; or why they cannot be: no round of the run is
/// under way, or one of them is no version the owner wrote to turn to a cell, words or an answer. Of a run abandoned
/// meanwhile, whose round is being stopped, they are put in all the same, so that its session is known to hold them;
/// its return is dropped, and they stay new to the cell. A version put into the run already is not put in again, as
/// where the round is given again after the process of the engine that said it ended, and its carrier says it again
/// from what the round's hook kept.
pub(crate) fn heard(
    flow: &Compiled,
    work: &mut Work,
    run: RunId,
    versions: Vec<(BlockId, Seq)>,
) -> Result<(), Refused> {
    let r = &work.records;
    let began = r.rounds.iter().filter(|x| x.run == run).count();
    let ended = r.returned.iter().filter(|x| x.run == run).count();
    if began == ended || r.finished.iter().any(|f| f.run == run) {
        return Err(Refused::NoRoundUnderWay(run));
    }
    for (block, version) in &versions {
        let turning = flow.flow.interrupts(block);
        let written = r.written.iter().any(|w| w.block == *block && w.at == *version);
        if !turning || !written {
            return Err(Refused::NoSuchBlock(block.clone()));
        }
    }
    for (block, version) in versions {
        let put = |p: &PutIn| p.run == run && p.block == block && p.version == version;
        if work.records.put_in.iter().any(put) {
            continue;
        }
        let at = work.tick();
        work.records.put_in.push(PutIn {
            run,
            block,
            version,
            at,
        });
    }
    Ok(())
}

/// The run of the cell whose round is under way, where one is.
fn under_way(work: &Work, cell: &CellId) -> Option<RunId> {
    let r = &work.records;
    (r.started.iter())
        .filter(|s| s.cell == *cell)
        .map(|s| s.run)
        .find(|run| {
            let open = !r.finished.iter().any(|f| f.run == *run) && !r.abandoned.iter().any(|a| a.run == *run);
            let began = r.rounds.iter().filter(|x| x.run == *run).count();
            let ended = r.returned.iter().filter(|x| x.run == *run).count();
            open && began > ended
        })
}

/// Writes a version of the block with this content, as the engine's source of the owner's words, and gives its place.
fn write(work: &mut Work, block: &BlockId, content: String) -> Seq {
    let at = work.tick();
    let empty = decide::holds_nothing(None, &content);
    work.contents.insert((block.clone(), at), content);
    work.records.written.push(Written {
        block: block.clone(),
        by: Outsider::Outside,
        matters: true,
        value: None,
        empty,
        said_to: None,
        at,
    });
    at
}

/// What a cell's run writes of a block it hands in, as its content: of a cell's words, which the owner's words write
/// ([`flow::WORDS`]), the cell's answer, in the words of that datum's `reply`; of any other block, what it handed in.
/// The cell is the one whose run writes it.
pub(crate) fn entry<'a>(flow: &Compiled, block: &BlockId, content: &'a str) -> Cow<'a, str> {
    match flow.blocks.get(block) {
        Some(record) if record.writers.outside.as_deref() == Some(flow::WORDS) => Cow::Owned(fill(
            record.writes.get(REPLY).unwrap_or_default(),
            &[("reply", content)],
        )),
        _ => Cow::Borrowed(content),
    }
}
