//! Whether a cell has handed its work in, as the loop records it (design document `data-and-context`, 2).
//!
//! A cell may have a datum of its own of the source [`HANDED_IN`], made where a cell's condition or preconditions name
//! it: the engine writes yes as the first run of the cell that dealt with something finishes. A round that fails never
//! finishes, a return sent back is not taken, and a run that only asks for calls deals with nothing, so none of them
//! counts. The records hold the same, and the datum is written all the same because a condition reads data alone, as
//! the check's wait reads `asked` (decisions the-merge-gate-is-a-datum-the-engine-writes and
//! the-better-way-is-looked-for-once).

use decide::{BlockId, CellId, Outsider, Written};
use flow::Compiled;
use work::Work;

/// The source of the datum that says whether a cell has handed its work in. Such a block is a choice of no and
/// [`YES`].
pub const HANDED_IN: &str = "handed_in";
/// The value of a [`HANDED_IN`] block once its cell has handed its work in.
pub const YES: &str = "yes";

/// The cell's own datum of the source [`HANDED_IN`]; none where the flow made none for it.
fn own<'f>(flow: &'f Compiled, cell: &CellId) -> Option<&'f BlockId> {
    (flow.blocks.iter())
        .find(|(_, b)| b.writers.outside.as_deref() == Some(HANDED_IN) && b.of_cell.as_ref() == Some(cell))
        .map(|(id, _)| id)
}

/// Writes yes in the cell's own datum of [`HANDED_IN`], with the words its `writes` gives as `text`, as a run of the
/// cell that dealt with something finishes; nothing where the cell has no such datum or it says yes already.
pub(crate) fn tell(flow: &Compiled, work: &mut Work, cell: &CellId) {
    let Some(block) = own(flow, cell) else {
        return;
    };
    let says = (work.records.written.iter()).any(|w| &w.block == block && w.value.as_deref() == Some(YES));
    if says {
        return;
    }
    let content = flow.blocks[block].writes.get("text").unwrap_or_default().to_string();
    let at = work.tick();
    work.contents.insert((block.clone(), at), content);
    work.records.written.push(Written {
        block: block.clone(),
        by: Outsider::Outside,
        matters: true,
        value: Some(YES.to_string()),
        empty: false,
        said_to: None,
        at,
    });
}
