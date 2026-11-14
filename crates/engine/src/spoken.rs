//! What the owner says to a work, and what its cells answer (design document `data-and-context`, 37 to 44).
//!
//! The owner speaks to a line: the words go to the cell of that line the work has reached, of the cells the owner's
//! words run, those with a trigger whose block the engine's source [`flow::WORDS`] writes. In one act the words are
//! the owner's entry of each datum the cell's agent reads for that trigger, `discussion` in the engineer's flow, and a
//! new version of the trigger's block, which runs the cell, stops a round under way of a later cell and enters the cell
//! while it waits for the results of its calls or the answer to a question, as the owner turning to a cell does. Where
//! a round of the cell is under way, the trigger's version is put into it, so that it is told of it and may deal with
//! it, and its carrier is given the words to hand to the round after its next use of a tool ([`Effect::Tell`]); words
//! that reach no round before it ends stay new to the cell, which runs again for them. A cell answers in the field of its return that the
//! datum it reads for the trigger is written through (`reply`), and the answer is its entry of that datum as its run
//! finishes (`handle`).

use decide::{BlockId, CellId, Outsider, PutIn, Written, spoken_to};
use flow::{CellSpec, Compiled, Trigger};
use work::Work;

use crate::event::{Effect, Refused};

/// The cell's trigger of the owner's words, where it has one.
pub(crate) fn trigger<'f>(flow: &'f Compiled, cell: &'f CellSpec) -> Option<&'f Trigger> {
    let source = |block: &BlockId| flow.blocks.get(block).and_then(|b| b.writers.outside.as_deref());
    (cell.triggers.iter()).find(|t| source(&t.block) == Some(flow::WORDS))
}

/// Takes the owner's words to the line: written as above, and, where a round of the cell spoken to is under way, put
/// into it and given to be told it; or why the words cannot be taken: the flow has no such line, or no cell of it that
/// the owner's words run applies now.
pub(crate) fn said(
    flow: &Compiled,
    work: &mut Work,
    line: String,
    words: String,
    effects: &mut Vec<Effect>,
) -> Result<(), Refused> {
    if !flow.cells.iter().any(|c| c.line.as_deref() == Some(line.as_str())) {
        return Err(Refused::NoSuchLine(line));
    }
    let speakable: Vec<CellId> = (flow.cells.iter())
        .filter(|c| c.line.as_deref() == Some(line.as_str()) && trigger(flow, c).is_some())
        .map(|c| c.id.clone())
        .collect();
    let Some(cell) = spoken_to(&flow.flow, &work.records, &speakable) else {
        return Err(Refused::NoOneSpokenTo(line));
    };
    let Some(spec) = flow.cells.iter().find(|c| c.id == cell) else {
        return Err(Refused::NoOneSpokenTo(line));
    };
    let Some(trigger) = trigger(flow, spec) else {
        return Err(Refused::NoOneSpokenTo(line));
    };
    for block in trigger.read.iter().filter(|b| **b != trigger.block) {
        if flow.blocks.get(block).is_some_and(|b| b.writers.people) {
            write(work, block, Outsider::Person, words.clone());
        }
    }
    let content = flow.blocks[&trigger.block]
        .writes
        .get("text")
        .unwrap_or_default()
        .to_string();
    let (block, version) = write(work, &trigger.block, Outsider::Outside, content);
    // A round of the cell under way is told of them as it runs.
    let r = &work.records;
    let under_way = (r.started.iter())
        .filter(|s| s.cell == cell)
        .map(|s| s.run)
        .find(|run| {
            let open = !r.finished.iter().any(|f| f.run == *run) && !r.abandoned.iter().any(|a| a.run == *run);
            let began = r.rounds.iter().filter(|x| x.run == *run).count();
            let ended = r.returned.iter().filter(|x| x.run == *run).count();
            open && began > ended
        });
    // Only the trigger is put in: what the words are written into stays for the session's next filling, since the
    // round may end before its hook hands them in.
    if let Some(run) = under_way {
        let at = work.tick();
        work.records.put_in.push(PutIn {
            run,
            block,
            version,
            at,
        });
        effects.push(Effect::Tell {
            run,
            trigger: trigger.name.clone(),
            words,
            does: trigger.does.clone(),
        });
    }
    Ok(())
}

/// Writes a version of the block with this content, by this writer, and gives it.
fn write(work: &mut Work, block: &BlockId, by: Outsider, content: String) -> (BlockId, decide::Seq) {
    let at = work.tick();
    work.contents.insert((block.clone(), at), content);
    work.records.written.push(Written {
        block: block.clone(),
        by,
        matters: true,
        value: None,
        at,
    });
    (block.clone(), at)
}
