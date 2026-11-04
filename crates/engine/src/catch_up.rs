//! A work's branch brought to the latest main before the first round of a cell that reads the engine's input
//! [`INPUT`] (cells `R1`, `D1` and `I1`) each time the work comes to it, so that no agent works on an old picture of
//! main: the round is told the files main changed since the branch last took it. The research and the deep research
//! commit only under `research/`, so taking main in does not conflict there; the implementation may meet a conflict,
//! which is undone and given as the files in conflict for the agent to merge itself. A worktree with changes not
//! committed is left as it is, and the round told why.

use std::path::Path;

use flow::format::Src;
use flow::{CompiledCell, CompiledGraph};
use serde_json::{Value, json};

use crate::Engine;
use crate::work::Work;

/// The engine's input that gives what taking main in brought.
pub const INPUT: &str = "main_changes";

/// Whether a cell is given what taking main in brought, and so has main taken in before its first round each time the
/// work comes to it.
pub fn reads(cell: &CompiledCell) -> bool {
    cell.inputs
        .iter()
        .flat_map(|x| &x.from)
        .any(|s| matches!(s, Src::Engine(n) if n == INPUT))
}

/// Whether the round about to run at `cell` is its first since the work came to it: the latest round of the work on
/// the cell's line was at another cell, or there is none. A round after tools, a look-up of the history or a pause
/// comes back to the same cell, and is none.
pub fn first_round(e: &Engine, w: &Work, g: &CompiledGraph, cell: &CompiledCell) -> Result<bool, String> {
    let line = crate::line_of(g, cell);
    let latest = crate::work::rounds(e.db.conn(), w.id)?
        .into_iter()
        .rev()
        .find(|r| {
            r.cell
                .as_deref()
                .and_then(|c| g.cell(c))
                .is_some_and(|c| crate::line_of(g, c) == line)
        });
    Ok(latest.is_none_or(|r| r.cell.as_deref() != Some(cell.id.as_str())))
}

/// Takes main in the worktree's branch: what it brought, as the input gives it. None when main changed nothing since
/// the branch last took it.
pub fn bring(wt: &Path, main: &str) -> Option<Value> {
    match try_bring(wt, main) {
        Ok(v) => v,
        Err(why) => Some(json!({ "merged": false, "why": format!("main could not be taken in: {why}") })),
    }
}

fn try_bring(wt: &Path, main: &str) -> Result<Option<Value>, String> {
    let dirty = crate::git::git(wt, &["status", "--porcelain"])?;
    if !dirty.trim().is_empty() {
        return Ok(Some(json!({
            "merged": false,
            "why": "the worktree has changes not committed, which taking main in could lose: commit them, then merge \
                    main yourself",
        })));
    }
    crate::git::git(wt, &["fetch", "-q", "origin", &config::main_refspec(main)])?;
    let theirs = format!("origin/{main}");
    let base = crate::git::git(wt, &["merge-base", "HEAD", &theirs])?
        .trim()
        .to_string();
    let changed = crate::git::changed(wt, &base, &theirs)?;
    if changed.is_empty() {
        return Ok(None);
    }
    Ok(Some(match crate::git::catch_up(wt, main)? {
        Ok(_) => json!({ "merged": true, "files": changed }),
        Err(conflicts) => json!({
            "merged": false,
            "why": "taking main in met conflicts, and was undone: merge main yourself before you change code",
            "conflicts": conflicts,
            "files": changed,
        }),
    }))
}
