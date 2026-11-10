//! What an agent is told at the start of a round: a function of the flow, the work's records and the round alone, with
//! no I/O, so that a round given again is told the same bytes.
//!
//! A run's first round is given the cell's task, why it runs now, each block it reads and each block it may hand in.
//! A version of a block the cell's line has read before in this work, or wrote itself, is not sent again: the session
//! of the line holds it. Which it is is read from the records: the earlier runs of the line's cells, the versions they
//! started from and the versions they wrote. A later round is given the results of the calls it asked for, or why
//! what it handed in was sent back.
//!
//! Every fixed word an agent is told is in the constants below, in the order they come in a message, so that each
//! sentence is found and changed in one place; how to return is in `protocol.md`, which every round is given as its
//! system prompt.

use decide::{BlockId, CallId, RunId, Seq, Started, Writer};
use flow::{CellSpec, Compiled};
use work::{Ask, Call, Given, Work};

/// The heading of every message: the cell's title, its id and the round's number.
fn heading(cell: &CellSpec, number: usize) -> String {
    format!("# {} (cell `{}`), round {number}", cell.title, cell.id)
}

const TASK: &str = "## Your task";
const WHY: &str = "## Why it runs now";
const FIRST_RUN: &str = "This is the cell's first run in this work.";
/// A block it reads changed: the block, the version it last worked from or none, and the version now.
fn changed(block: &BlockId, was: Option<Seq>, now: Option<Seq>) -> String {
    let was = match was {
        Some(was) => format!("you last worked from version {}", was.0),
        None => "it was not written when you last worked".to_string(),
    };
    let now = match now {
        Some(now) => format!("it is now version {}", now.0),
        None => "it is not written now".to_string(),
    };
    format!("- `{block}` changed: {was}, and {now}.")
}
const CHANGED_AFTER: &str = " A cell after this one changed it.";
const READS: &str = "## What it reads";
/// A block it reads at a version: the block, the version and who wrote it.
fn version(block: &BlockId, version: Seq, by: &str) -> String {
    format!("### `{block}`, version {}, written by {by}", version.0)
}
fn with_value(value: &str) -> String {
    format!(", with the value `{value}`")
}
const AS_YOU_WROTE_IT: &str = ": as you wrote it.";
const UNCHANGED: &str = ": unchanged since you read it.";
fn not_written(block: &BlockId) -> String {
    format!("### `{block}`: not written yet.")
}
const BY_A_PERSON: &str = "a person";
fn by_a_cell(cell: &str) -> String {
    format!("the cell `{cell}`")
}
fn by_the_outside(source: &str) -> String {
    format!("the source `{source}` outside the work")
}
const MAY_HAND_IN: &str = "## What it may hand in";
fn output(block: &BlockId) -> String {
    format!("- `{block}`")
}
fn values(values: &[String]) -> String {
    let values: Vec<String> = values.iter().map(|v| format!("`{v}`")).collect();
    format!(": a choice, one of {}", values.join(", "))
}
const OWNER_READS: &str = " (the owner reads it)";
const RESULTS: &str = "## The results of what you asked for";
fn tool(name: &str, input: &str) -> String {
    format!("### The tool `{name}`, on: {input}")
}
fn question(question: &str) -> String {
    format!("### Your question: {question}")
}
const AT_LIMIT: &str = "### The owner let the run go on at its limit";
const NO_RESULT: &str = "(It has no result.)";
const GO_ON: &str = "Go on with the task.";
const SENT_BACK: &str = "## What you handed in was sent back";
const HAND_IN_AGAIN: &str = "Hand in again: everything the cell hands in, in one return.";

/// What the agent is told at the start of the round of this number of the run of the cell, which is given this.
pub fn message(flow: &Compiled, work: &Work, cell: &CellSpec, run: RunId, number: usize, given: &Given) -> String {
    let mut parts = vec![heading(cell, number)];
    match given {
        Given::Task => task(flow, work, cell, run, &mut parts),
        Given::Results(calls) => results(work, calls, &mut parts),
        Given::SentBack(unmet) => {
            parts.push(SENT_BACK.to_string());
            let why: Vec<String> = unmet.iter().map(|u| format!("- {}", service::unmet_words(u))).collect();
            parts.push(why.join("\n"));
            parts.push(HAND_IN_AGAIN.to_string());
        }
    }
    let mut text = parts.join("\n\n");
    text.push('\n');
    text
}

/// The first round: the task, why the cell runs, what it reads and what it may hand in.
fn task(flow: &Compiled, work: &Work, cell: &CellSpec, run: RunId, parts: &mut Vec<String>) {
    parts.push(TASK.to_string());
    parts.push(cell.task.clone().unwrap_or_default().trim_end().to_string());
    parts.push(WHY.to_string());
    let started = work.records.started.iter().find(|s| s.run == run);
    let because = started.map(|s| s.because.as_slice()).unwrap_or_default();
    if because.iter().any(|cause| matches!(cause, decide::Cause::NoRecord)) || because.is_empty() {
        parts.push(FIRST_RUN.to_string());
    } else {
        let lines: Vec<String> = (because.iter())
            .filter_map(|cause| match cause {
                decide::Cause::NoRecord => None,
                decide::Cause::Changed { block, was, behind } => {
                    let now = started.and_then(|s| s.basis.get(block).copied().flatten());
                    let after = if *behind { CHANGED_AFTER } else { "" };
                    Some(format!("{}{after}", changed(block, *was, now)))
                }
            })
            .collect();
        parts.push(lines.join("\n"));
    }
    let place = flow.flow.place(&cell.id);
    let (inputs, outputs) = place.map_or((&[][..], &[][..]), |p| {
        let c = &flow.flow.cells()[p];
        (&c.inputs[..], &c.outputs[..])
    });
    parts.push(READS.to_string());
    let versions = decide::versions(&work.records);
    for block in inputs {
        let Some(at) = started.and_then(|s| s.basis.get(block).copied().flatten()) else {
            parts.push(not_written(block));
            continue;
        };
        let written = versions.get(block).and_then(|vs| vs.iter().find(|v| v.at == at));
        let by = match written.map(|v| &v.by) {
            Some(Writer::Run(writer)) => by_a_cell(cell_of(work, *writer).unwrap_or_default()),
            Some(Writer::Outside) => {
                let source = flow.blocks.get(block).and_then(|b| b.writers.outside.clone());
                by_the_outside(&source.unwrap_or_default())
            }
            Some(Writer::Person) | None => BY_A_PERSON.to_string(),
        };
        let mut head = version(block, at, &by);
        if let Some(value) = written.and_then(|v| v.value.as_deref()) {
            head.push_str(&with_value(value));
        }
        let line = cell.line.as_deref();
        let wrote = matches!(written.map(|v| &v.by), Some(Writer::Run(writer)) if line.is_some() && line_of(flow, work, *writer) == line);
        if wrote {
            parts.push(format!("{head}{AS_YOU_WROTE_IT}"));
        } else if let (Some(line), Some(started)) = (line, started)
            && read_before(flow, work, line, started, block, at)
        {
            parts.push(format!("{head}{UNCHANGED}"));
        } else {
            parts.push(head);
            let content = work.contents.get(&(block.clone(), at)).map(String::as_str);
            parts.push(content.unwrap_or_default().trim_end().to_string());
        }
    }
    parts.push(MAY_HAND_IN.to_string());
    let lines: Vec<String> = (outputs.iter())
        .map(|block| {
            let declared = flow.blocks.get(block);
            let mut line = output(block);
            if let Some(choice) = declared.and_then(|b| b.values.as_deref()) {
                line.push_str(&values(choice));
            }
            if declared.is_some_and(|b| b.readers.shown) {
                line.push_str(OWNER_READS);
            }
            line
        })
        .collect();
    parts.push(lines.join("\n"));
}

/// A later round given the results of the calls the round before asked for.
fn results(work: &Work, calls: &[CallId], parts: &mut Vec<String>) {
    parts.push(RESULTS.to_string());
    for call in calls {
        let head = match work.calls.get(call) {
            Some(Call::Asked(Ask::Tool { name, input })) => tool(name, input),
            Some(Call::Asked(Ask::Person { question: q, .. })) => question(q),
            Some(Call::AtLimit(_)) | None => AT_LIMIT.to_string(),
        };
        parts.push(head);
        let result = work.results.get(call).map(|r| r.trim_end().to_string());
        parts.push(result.unwrap_or_else(|| NO_RESULT.to_string()));
    }
    parts.push(GO_ON.to_string());
}

/// The cell of the run.
fn cell_of(work: &Work, run: RunId) -> Option<&str> {
    let started = work.records.started.iter().find(|s| s.run == run)?;
    Some(started.cell.as_str())
}

/// The line of the run's cell.
fn line_of<'a>(flow: &'a Compiled, work: &Work, run: RunId) -> Option<&'a str> {
    let cell = cell_of(work, run)?;
    let spec = flow.cells.iter().find(|spec| spec.id.as_str() == cell)?;
    spec.line.as_deref()
}

/// Whether the line was given this version of the block in an earlier run of this work: a run of one of its cells
/// that started before this one, whose cell reads the block, which started from this version, and whose first round
/// began.
fn read_before(flow: &Compiled, work: &Work, line: &str, this: &Started, block: &BlockId, at: Seq) -> bool {
    (work.records.started.iter())
        .filter(|s| s.at < this.at && line_of(flow, work, s.run) == Some(line))
        .filter(|s| work.given.contains_key(&(s.run, 1)))
        .filter(|s| {
            let place = flow.flow.place(&s.cell);
            place.is_some_and(|p| flow.flow.cells()[p].inputs.contains(block))
        })
        .any(|s| s.basis.get(block) == Some(&Some(at)))
}

#[cfg(test)]
mod tests;
