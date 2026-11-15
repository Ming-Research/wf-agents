//! What the owner says to a work, and what its cells answer (design document `data-and-context`, 30 to 38).
//!
//! The owner speaks about a question, or to a line of the work's flow, and the engine alone chooses the cell the words
//! reach ([`receiver`]): words about a question go to the cell that asked it, or, for a question of the engine's, to
//! the cell its answer goes to; words to a line go to the cell the line has reached (`decide::reached`), of those of
//! it the owner's words run. Where that cell cannot take them, as one the owner's words do not run or one that does
//! not apply now, or where the work has ended, the words are refused, saying why, and go to no other cell.
//!
//! In one act the words are the owner's entry of the record of what is said, the datum the cell's trigger of the
//! owner's words has its agent read (`discussion` in the engineer's flow), in the words of that datum's `owner`, which
//! name no cell, and kept said to the cell; and the trigger's block of that cell, which the engine's source
//! [`flow::WORDS`] writes, has a new version, which runs the cell, stops a round under way of a later cell and enters
//! the cell while it waits, as the owner turning to a cell does. Where a round of the cell is under way, its carrier is
//! given the entry to hand to the round after its next use of a tool ([`Effect::Tell`]); only once it says the round
//! was given it ([`Event::SaidToRound`]) are the two versions put into the round, so that a round consumes only words
//! that reached it and a session is not given again what its round was told. A cell answers by writing the record in
//! its return's `outputs`, as it writes any block, and the answer is its entry of the record as its run finishes, in
//! the words of that datum's `reply` ([`entry`]), which name no cell either: the run that wrote it is the cell's.
//!
//! [`Event::SaidToRound`]: crate::Event::SaidToRound

use decide::questions::questions;
use decide::{BlockId, CellId, Decision, Outsider, PutIn, RunId, Seq, Written, decide};
use flow::agents::fill;
use flow::{CellSpec, Compiled, Trigger};
use work::Work;

use crate::asked;
use crate::event::{Effect, Refused};

/// The text of the record of what is said that is the owner's entry: `{words}` the words.
pub const OWNER: &str = "owner";
/// The text of the record of what is said that is a cell's answer: `{reply}` its answer.
pub const REPLY: &str = "reply";

/// What the owner said to a round under way, as its carrier is to hand it in ([`Effect::Tell`]): the owner's entry of
/// the record of what is said, the record and the entry's version, the trigger of the cell's it is new to the round as
/// and what the cell does for it, and the versions that are put into the round once it was given them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Telling {
    pub entry: String,
    pub block: BlockId,
    pub number: Seq,
    pub trigger: String,
    pub does: Option<String>,
    pub versions: Vec<(BlockId, Seq)>,
}

/// What the owner's words are said to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum To {
    /// A line of the work's flow, by its name.
    Line(String),
    /// A question of the work, by its name: the words are about it.
    Question(String),
}

/// Why no cell can take the owner's words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoOne {
    /// The work has ended.
    Ended,
    /// The flow has no cell on such a line.
    NoSuchLine(String),
    /// The work has no question of this name that stands.
    NoSuchQuestion(String),
    /// The owner's words run no cell of the line.
    NoneOfTheLine(String),
    /// No cell of the line that the owner's words run has run, and the work stands at none of them.
    NotReached(String),
    /// The words would go to this cell, and the owner's words do not run it.
    NotSpokenTo(CellId),
    /// The words would go to this cell, and its condition is not met now: what runs it would not run it.
    Skipped(CellId),
}

/// The cell's trigger of the owner's words, where it has one, with the block of it the engine writes as the owner
/// speaks to the cell ([`Trigger::of_source`]).
pub fn trigger<'f>(flow: &Compiled, cell: &'f CellSpec) -> Option<(&'f Trigger, &'f BlockId)> {
    (cell.triggers.iter()).find_map(|t| t.of_source(flow, flow::WORDS).map(|block| (t, block)))
}

/// The cell the owner's words said to this reach, as the engine chooses it, or why no cell can take them.
pub fn receiver(flow: &Compiled, work: &Work, to: &To) -> Result<CellId, NoOne> {
    let records = &work.records;
    if matches!(decide(&flow.flow, records), Decision::Ended(_)) {
        return Err(NoOne::Ended);
    }
    let cell = match to {
        To::Question(name) => {
            let standing = questions(records);
            let asked = (standing.get(name.as_str())).filter(|q| q.stands()).and_then(|q| {
                records
                    .questions
                    .iter()
                    .find(|x| x.name == *name && x.at == q.latest.at)
            });
            let Some(asked) = asked else {
                return Err(NoOne::NoSuchQuestion(name.clone()));
            };
            asked::answering(flow, asked)
        }
        To::Line(line) => {
            let of_line: Vec<&CellSpec> = (flow.cells.iter())
                .filter(|c| c.line.as_deref() == Some(line.as_str()))
                .collect();
            if of_line.is_empty() {
                return Err(NoOne::NoSuchLine(line.clone()));
            }
            if !of_line.iter().any(|c| trigger(flow, c).is_some()) {
                return Err(NoOne::NoneOfTheLine(line.clone()));
            }
            // Only a cell the owner's words run counts as where the line stands: one they do not run, as the reading
            // of the tree a reopened session begins with, is passed over.
            let can: Vec<CellId> = (of_line.into_iter())
                .filter(|c| trigger(flow, c).is_some())
                .map(|c| c.id.clone())
                .collect();
            decide::reached(&flow.flow, records, &can).ok_or_else(|| NoOne::NotReached(line.clone()))?
        }
    };
    let spoken_to = (flow.cells.iter())
        .find(|c| c.id == cell)
        .and_then(|c| trigger(flow, c));
    if spoken_to.is_none() {
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
    let spec = (flow.cells.iter())
        .find(|c| c.id == cell)
        .expect("the receiver is a cell of the flow");
    let (trigger, own) = trigger(flow, spec).expect("the receiver is one the owner's words run");
    let mut put_in = Vec::new();
    let mut told = None;
    for block in trigger.records(flow, flow::WORDS) {
        let record = &flow.blocks[block];
        let entry = fill(
            record.writes.get(OWNER).unwrap_or_default(),
            &[("words", words.as_str())],
        );
        let version = write(work, block, entry.clone(), Some(&cell));
        put_in.push((block.clone(), version));
        told.get_or_insert((block.clone(), version, entry));
    }
    let content = flow.blocks[own].writes.get("text").unwrap_or_default().to_string();
    put_in.push((own.clone(), write(work, own, content, None)));
    // A round of the cell under way is told of them as it runs; they are put into it once it was.
    if let (Some(run), Some((block, number, entry))) = (under_way(work, &cell), told) {
        let telling = Telling {
            entry,
            block,
            number,
            trigger: trigger.name.clone(),
            does: trigger.does.clone(),
            versions: put_in,
        };
        effects.push(Effect::Tell {
            run,
            telling: Box::new(telling),
        });
    }
    Ok(())
}

/// Puts into the round of the run that is under way these versions, which the engine gave its carrier to tell the
/// round ([`Effect::Tell`]) and the carrier says the round was given; or why they cannot be: no round of the run is
/// under way, or one of them is no version the owner's words wrote. Of a run abandoned meanwhile, whose round is being
/// stopped, they are put in all the same, so that its session is known to hold them; its return is dropped, and they
/// stay new to the cell.
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
        let words = (flow.blocks.get(block)).is_some_and(|b| b.writers.outside.as_deref() == Some(flow::WORDS));
        let written = r.written.iter().any(|w| w.block == *block && w.at == *version);
        if !words || !written {
            return Err(Refused::NoSuchBlock(block.clone()));
        }
    }
    for (block, version) in versions {
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

/// Carries into the record of what is said the owner's words of a work kept before the record held them: each version a
/// person wrote, with words, of a cell's block of the owner's words, which the engine alone writes now, and with none,
/// is the owner's entry said to that cell, written once, after everything the work holds, at the time of the last thing
/// that happened to it. It runs no cell: the words ran it as they were written. Gives whether it wrote any.
pub fn carry(flow: &Compiled, work: &mut Work) -> bool {
    let mut carried = false;
    for spec in &flow.cells {
        let Some((trigger, own)) = trigger(flow, spec) else {
            continue;
        };
        let said: Vec<String> = (work.records.written.iter())
            .filter(|w| w.block == *own && w.by == Outsider::Person)
            .filter_map(|w| work.contents.get(&(own.clone(), w.at)))
            .filter(|words| !words.trim().is_empty())
            .cloned()
            .collect();
        for block in trigger.records(flow, flow::WORDS) {
            let record = &flow.blocks[block];
            for words in &said {
                let entry = fill(
                    record.writes.get(OWNER).unwrap_or_default(),
                    &[("words", words.as_str())],
                );
                let held = (work.records.written.iter()).any(|w| {
                    w.block == *block
                        && w.said_to.as_ref() == Some(&spec.id)
                        && work.contents.get(&(block.clone(), w.at)) == Some(&entry)
                });
                if !held {
                    write(work, block, entry, Some(&spec.id));
                    carried = true;
                }
            }
        }
    }
    carried
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

/// Writes a version of the block with this content, as the engine's source of the owner's words, said to the cell where
/// it is said to one, and gives its place.
fn write(work: &mut Work, block: &BlockId, content: String, said_to: Option<&CellId>) -> Seq {
    let at = work.tick();
    let empty = decide::holds_nothing(None, &content);
    work.contents.insert((block.clone(), at), content);
    work.records.written.push(Written {
        block: block.clone(),
        by: Outsider::Outside,
        matters: true,
        value: None,
        empty,
        said_to: said_to.cloned(),
        at,
    });
    at
}

/// What a cell's run writes of a block it hands in, as its content: of a record of what is said, which the owner's
/// words write ([`flow::WORDS`]), the cell's answer as its entry, in the words of that datum's `reply`; of any other
/// block, what it handed in. The cell is the one whose run writes it.
pub(crate) fn entry(flow: &Compiled, block: &BlockId, content: String) -> String {
    match flow.blocks.get(block) {
        Some(record) if record.writers.outside.as_deref() == Some(flow::WORDS) => fill(
            record.writes.get(REPLY).unwrap_or_default(),
            &[("reply", content.as_str())],
        ),
        _ => content,
    }
}
