//! What an agent is told at the start of a round: a function of the flow, the work's records, the round, and whether
//! the round begins its line's session, with no I/O, so that a round given again is told the same bytes.
//!
//! A run's first round, and any round that begins a session, is given the cell's task and a list: each block the cell
//! reads, with its current version, who wrote it, its value where it is a choice, and the version the cell last worked
//! from, those that changed marked; and each block it may write, with its values where it is a choice and the version
//! at which its meaning last changed. No content of any block is in the message: the agent reads what it needs with
//! the tool `read`, and the list ends with the input that reads all of it at once. Nothing is left out for what a
//! session is thought to hold. A later round is given the results of the calls it
//! asked for, or why what it handed in was sent back; one that begins a session, or goes on in one that never saw the
//! run's earlier rounds, is given, after the task and the list, what each earlier round of the run was given, in their
//! order, and then its own. Where the work's branch was to take main in as the run started ([`crate::catch_up`]), the
//! task and the list are followed by what came of it, unless the branch held main already.
//!
//! Every round is told last what ran it: each trigger of its cell new to it, by the name its return names it by in
//! `dealt_with`, with what the cell does for it (design document `data-and-context`, 9 and 12). What ran it, and why
//! what a round returned was sent back, are told in the words of the files beside the flows
//! (`agents/message/message.toml` and `agents/return/sent_back.toml`); every other fixed word an agent is told is in the
//! constants below, in the order they come in a message, so that each sentence is found and changed in one place; how
//! to return is the part on returning of the system prompt of every line, made from the files beside the flows
//! ([`crate::system`]).

use std::collections::BTreeMap;

use decide::run::Unmet;
use decide::{BlockId, CallId, Cause, RunId, Seq, Started, VersionView, Writer};
use flow::agents::{self, Agents, Part, SentBack};
use flow::{CellSpec, Compiled};
use work::{Ask, Call, Given, Work};

use crate::catch_up::{CaughtUp, Outcome};

/// The heading of every message: the cell's title, its id and the round's number.
fn heading(cell: &CellSpec, number: usize) -> String {
    format!("# {} (cell `{}`), round {number}", cell.title, cell.id)
}

const TASK: &str = "## Your task";
const READS: &str = "## What it reads";
const FIRST_RUN: &str = "This is the cell's first run in this work.";
/// An input at its current version, written by someone.
fn input(block: &BlockId, version: Seq, by: &str) -> String {
    format!("- `{block}`: version {}, written by {by}", version.0)
}
fn with_value(value: &str) -> String {
    format!(", with the value `{value}`")
}
const SAME: &str = "; you last worked from this version.";
fn changed(was: Option<Seq>) -> String {
    match was {
        Some(was) => format!("; changed since you last worked from version {}.", was.0),
        None => "; changed since you last worked, when it was not written.".to_string(),
    }
}
fn same_meaning(was: Option<Seq>) -> String {
    let was = was.map_or_else(|| "none".to_string(), |w| format!("version {}", w.0));
    format!("; a new version since you last worked from {was}, said not to change what it means.")
}
fn not_written(block: &BlockId) -> String {
    format!("- `{block}`: not written yet.")
}
const BY_A_PERSON: &str = "a person";
fn by_a_cell(cell: &str) -> String {
    format!("the cell `{cell}`")
}
fn by_the_outside(source: &str) -> String {
    format!("the source `{source}` outside the work")
}
const MAY_WRITE: &str = "## What it may write";
fn values(values: &[String]) -> String {
    let values: Vec<String> = values.iter().map(|v| format!("`{v}`")).collect();
    format!(", a choice, one of {}", values.join(", "))
}
const OWNER_READS: &str = " (the owner reads it)";
const EVERY_TIME: &str = " (hand it in every time)";
/// An output's current version, its value where it has one, and the version at which its meaning last changed.
fn output_version(version: Seq, value: Option<&str>, meaning: Seq) -> String {
    let value = value.map(with_value).unwrap_or_default();
    if version == meaning {
        format!(
            "version {}{value}; its meaning last changed at this version.",
            version.0
        )
    } else {
        format!(
            "version {}{value}; its meaning last changed at version {}.",
            version.0, meaning.0
        )
    }
}
const OUTPUT_NOT_WRITTEN: &str = "not written yet.";
fn read_all(input: &str) -> String {
    format!("To read all of it at once, ask for the tool `read` with the input `{input}`.")
}
const NOTHING_TO_READ: &str = "Nothing it reads or may write is written yet.";
/// What came of taking main in as the run started; nothing where the branch held main already.
fn caught_up(caught: &CaughtUp) -> Option<String> {
    let name = &caught.name;
    Some(match &caught.outcome {
        Outcome::Holds { .. } => return None,
        Outcome::TookIn { main, merge, files } => format!(
            "## {name} taken in\nThe engine merged {name} at {main} into the work's branch as this run started, as \
             the commit {merge}. The files {name} changed since the branch last took it in:\n{}",
            files.join("\n")
        ),
        Outcome::Conflict { main, files } => format!(
            "## {name} not taken in\n{name} at {main} does not merge into the work's branch without a conflict, so \
             the engine took nothing in. The files in conflict:\n{}\nMerge it yourself, with `git merge {main}`, \
             before you change the code.",
            files.join("\n")
        ),
        Outcome::Uncommitted {
            main,
            changes,
            merging: true,
        } => format!(
            "## {name} not taken in\nA merge is under way in the worktree and is not committed, so the engine took \
             nothing in:\n{changes}\nFinish it and commit it, then merge {name} yourself, with `git merge {main}`."
        ),
        Outcome::Uncommitted { main, changes, .. } => format!(
            "## {name} not taken in\nThe worktree holds changes not committed, which taking {name} at {main} in \
             could lose, so the engine took nothing in:\n{changes}\nCommit them, then merge it yourself, with \
             `git merge {main}`."
        ),
        Outcome::Failed { why } => {
            format!("## {name} not taken in\nThe engine could not take {name} in as this run started: {why}")
        }
    })
}
const RESULTS: &str = "## The results of what you asked for";
fn tool(name: &str, input: &str) -> String {
    if input.is_empty() {
        format!("### The tool `{name}`")
    } else {
        format!("### The tool `{name}`, on: {input}")
    }
}
fn question(question: &str) -> String {
    format!("### Your question: {question}")
}
const AT_LIMIT: &str = "### The owner let the run go on at its limit";
fn unknown_call(call: CallId) -> String {
    format!("### Call {}, which the work does not hold", call.0)
}
const NO_RESULT: &str = "(It has no result.)";
fn earlier_results(round: usize) -> String {
    format!("## Round {round} of this run: the results of what you asked for")
}
fn read_before(input: &str) -> String {
    let names: Vec<&str> = input.split_whitespace().collect();
    format!("### You read {}: read again what you need.", names.join(", "))
}
const GO_ON: &str = "Go on with the task.";

/// How a round opens, beside what it is given.
#[derive(Clone, Copy, Debug, Default)]
pub struct Opening<'a> {
    /// The round begins its line's session, or goes on in one that holds nothing of its run's earlier rounds.
    pub begins: bool,
    /// What came of taking main in as the run started, where the work's branch was to take it in.
    pub main: Option<&'a CaughtUp>,
    /// The cell's task as it is sent, each setting it names with its value beside it; the cell's own, as written,
    /// where none is given.
    pub task: Option<&'a str>,
}

/// What the agent is told at the start of the round of this number of the run of the cell, which is given this, and
/// which opens so, in the words of the files beside the flows where they say it.
#[allow(clippy::too_many_arguments)]
pub fn message(
    flow: &Compiled,
    work: &Work,
    cell: &CellSpec,
    run: RunId,
    number: usize,
    given: &Given,
    opening: &Opening<'_>,
    words: &Agents,
) -> String {
    let begins = opening.begins;
    let told = &words.sent_back;
    let mut parts = vec![heading(cell, number)];
    if begins || *given == Given::Task {
        task(flow, work, cell, run, opening.task, &mut parts);
        parts.extend(opening.main.and_then(caught_up));
    }
    // A new session holds nothing of the run: it is given what the run's earlier rounds were, in their order.
    if begins {
        for earlier in 2..number {
            match work.given.get(&(run, earlier)) {
                Some(Given::Results(calls)) => {
                    parts.push(earlier_results(earlier));
                    calls_and_results(work, calls, true, &mut parts);
                }
                Some(Given::SentBack(unmet)) => {
                    parts.push(told.earlier.replace("{round}", &earlier.to_string()));
                    parts.push(sent_back(flow, cell, unmet, told));
                }
                Some(Given::Task) | None => {}
            }
        }
    }
    match given {
        Given::Task => {}
        Given::Results(calls) => {
            parts.push(RESULTS.to_string());
            calls_and_results(work, calls, false, &mut parts);
            parts.push(GO_ON.to_string());
        }
        Given::SentBack(unmet) => {
            parts.push(told.heading.clone());
            parts.push(sent_back(flow, cell, unmet, told));
            parts.push(told.again.clone());
        }
    }
    parts.extend(triggers(flow, work, cell, run, words));
    let mut text = parts.join("\n\n");
    text.push('\n');
    text
}

/// The task, as it is sent where it is given, and the list of what the cell reads and may write.
fn task(flow: &Compiled, work: &Work, cell: &CellSpec, run: RunId, sent: Option<&str>, parts: &mut Vec<String>) {
    parts.push(TASK.to_string());
    let task = sent.or(cell.task.as_deref()).unwrap_or_default();
    parts.push(task.trim_end().to_string());
    let started = work.records.started.iter().find(|s| s.run == run);
    let current = |block: &BlockId| started.and_then(|s| s.basis.get(block).copied().flatten());
    let place = flow.flow.place(&cell.id);
    let (inputs, outputs) = place.map_or((&[][..], &[][..]), |p| {
        let c = &flow.flow.cells()[p];
        (&c.runs_on[..], &c.outputs[..])
    });
    let versions = decide::versions(&work.records);
    let record = started.and_then(|s| record_before(work, s));
    // What the line at the end reads: everything the list names.
    let mut reads = Vec::new();

    parts.push(READS.to_string());
    if record.is_none() {
        parts.push(FIRST_RUN.to_string());
    }
    let mut lines = Vec::new();
    for block in inputs {
        let Some(at) = current(block) else {
            lines.push(not_written(block));
            continue;
        };
        reads.push(format!("{block}@{}", at.0));
        let written = version(&versions, block, at);
        let mut line = input(block, at, &writer(flow, work, block, written));
        if let Some(value) = written.and_then(|v| v.value.as_deref()) {
            line.push_str(&with_value(value));
        }
        match record.map(|record| last_worked_from(work, record, block)) {
            None => line.push('.'),
            Some(was) if was == Some(at) => line.push_str(SAME),
            Some(was) => {
                let because = started.map(|s| s.because.as_slice()).unwrap_or_default();
                let in_meaning = (because.iter()).any(|c| matches!(c, Cause::Changed { block: b, .. } if b == block));
                line.push_str(&if in_meaning { changed(was) } else { same_meaning(was) });
                if let Some(was) = was {
                    reads.push(format!("{block}@{}..{}", was.0, at.0));
                }
            }
        }
        lines.push(line);
    }
    parts.push(lines.join("\n"));

    parts.push(MAY_WRITE.to_string());
    let mut lines = Vec::new();
    // A block the cell writes through a field of its own is no block it writes as one.
    let as_blocks = (outputs.iter()).filter(|b| flow.blocks.get(*b).is_none_or(|d| d.field.is_none()));
    for block in as_blocks {
        let declared = flow.blocks.get(block);
        let mut line = format!("- `{block}`");
        if let Some(choice) = declared.and_then(|b| b.values.as_deref()) {
            line.push_str(&values(choice));
        }
        if declared.is_some_and(|b| b.readers.shown) {
            line.push_str(OWNER_READS);
        }
        if cell.must_write.contains(block) {
            line.push_str(EVERY_TIME);
        }
        line.push_str(": ");
        match current(block) {
            Some(at) => {
                let written = version(&versions, block, at);
                let meaning = written.map_or(at, |v| v.meaning);
                line.push_str(&output_version(at, written.and_then(|v| v.value.as_deref()), meaning));
                if !inputs.contains(block) {
                    reads.push(format!("{block}@{}", at.0));
                }
                if meaning != at {
                    reads.push(format!("{block}@{}..{}", meaning.0, at.0));
                }
            }
            None => line.push_str(OUTPUT_NOT_WRITTEN),
        }
        lines.push(line);
    }
    parts.push(lines.join("\n"));
    parts.push(if reads.is_empty() {
        NOTHING_TO_READ.to_string()
    } else {
        read_all(&reads.join(" "))
    });
}

/// Each call with its result; of an earlier round, a `read` only named.
fn calls_and_results(work: &Work, calls: &[CallId], earlier: bool, parts: &mut Vec<String>) {
    for call in calls {
        // What an earlier round read is named, not given again: the blocks may have changed since, and the agent reads
        // again what it needs.
        if let Some(Call::Asked(Ask::Tool { name, input })) = work.calls.get(call)
            && earlier
            && name == crate::read::READ
        {
            parts.push(read_before(input));
            continue;
        }
        let head = match work.calls.get(call) {
            Some(Call::Asked(Ask::Tool { name, input })) => tool(name, input),
            Some(Call::Asked(Ask::Person { question: q, .. })) => question(q),
            Some(Call::AtLimit(_)) => AT_LIMIT.to_string(),
            // The engine's wait before a round is tried again is no result a round is given.
            Some(Call::Wait { .. }) | None => unknown_call(*call),
        };
        parts.push(head);
        let result = work.results.get(call).map(|r| r.trim_end().to_string());
        parts.push(result.unwrap_or_else(|| NO_RESULT.to_string()));
    }
}

/// What ran the round: each trigger of the cell that is new to it, in the order the cell names them, with what the cell
/// does for it, as the part of a message that says so is written; none where nothing new ran it, or the message names
/// no such part. What happened is not said: the engine fills no place of a trigger's yet.
fn triggers(flow: &Compiled, work: &Work, cell: &CellSpec, run: RunId, words: &Agents) -> Option<String> {
    let part = words.message.parts.iter().find(|p| p.part == Part::Triggers)?;
    let ran_by = decide::told(&flow.flow, &work.records, run)?.ran_by;
    let new: Vec<_> = cell.triggers.iter().filter(|t| ran_by.contains(&t.block)).collect();
    if new.is_empty() {
        return None;
    }
    let mut text = part.text.clone().unwrap_or_default();
    let each = part.each.as_deref().unwrap_or_default();
    for trigger in new {
        let does = match (&trigger.does, &part.does) {
            (Some(words), Some(does)) => does.replace("{do}", words.trim()),
            _ => String::new(),
        };
        // The words of what the cell does are put in last, so that nothing in them is filled.
        let line = (each.replace("{trigger}", &trigger.name)).replace("{detail}", "");
        text.push_str(&line.replace("{do}", &does));
    }
    text.push_str(part.after.as_deref().unwrap_or_default());
    Some(text.trim_end().to_string())
}

/// Why what the round returned was sent back, a line for each thing wrong with it, in the words of the file.
fn sent_back(flow: &Compiled, cell: &CellSpec, unmet: &[Unmet], words: &SentBack) -> String {
    let why: Vec<String> = (unmet.iter())
        .map(|u| words.item.replace("{text}", &fault(flow, cell, u, words)))
        .collect();
    why.join("\n")
}

/// One thing wrong with a return, in the words of the file, its places filled: the engine's own first, and what the
/// return wrote last, so that nothing in that is filled.
fn fault(flow: &Compiled, cell: &CellSpec, unmet: &Unmet, words: &SentBack) -> String {
    let f = &words.faults;
    let values = |block: &BlockId| {
        let values = flow.blocks.get(block).and_then(|b| b.values.as_ref());
        let ticked: Vec<String> = values.into_iter().flatten().map(|v| format!("`{v}`")).collect();
        ticked.join(&words.between)
    };
    // What the return dealt with is named by the cell's trigger that stands on the block.
    let trigger = |block: &BlockId| {
        let named = cell.triggers.iter().find(|t| &t.block == block);
        named.map_or_else(|| block.to_string(), |t| t.name.clone())
    };
    let block = |block: &BlockId| ("block", block.to_string());
    let (text, places): (&str, Vec<(&str, String)>) = match unmet {
        Unmet::AsksNothing => (&f.asks_nothing, vec![]),
        Unmet::NotAnOutput { block: b } => (&f.not_an_output, vec![block(b)]),
        Unmet::WrittenTwice { block: b } => (&f.written_twice, vec![block(b)]),
        Unmet::NoValue { block: b } => (&f.no_value, vec![block(b), ("values", values(b))]),
        Unmet::NotAValue { block: b, value } => (
            &f.not_a_value,
            vec![block(b), ("values", values(b)), ("value", value.clone())],
        ),
        Unmet::NotAChoice { block: b } => (&f.not_a_choice, vec![block(b)]),
        Unmet::NotHandedIn { block: b } => (&f.not_handed_in, vec![block(b)]),
        Unmet::Unreadable { why } => (&f.unreadable, vec![("why", why.clone())]),
        Unmet::NotRunBy { block: b } => (&f.not_run_by, vec![("name", trigger(b))]),
        Unmet::DealtWithTwice { block: b } => (&f.dealt_with_twice, vec![("name", trigger(b))]),
        Unmet::DealtWithNothing => (&f.dealt_with_nothing, vec![]),
        Unmet::WrittenOver { block: b, version } => {
            (&f.written_over, vec![block(b), ("version", version.0.to_string())])
        }
        Unmet::ChoiceChanged { block: b } => (&f.choice_changed, vec![block(b)]),
        Unmet::AsksAndWorks => (&f.asks_and_works, vec![]),
        Unmet::NotATrigger { name } => (&f.not_a_trigger, vec![("name", name.clone())]),
        Unmet::NotATool { name } => (&f.not_a_tool, vec![("name", name.clone())]),
        Unmet::UnchangedNotWritten { block: b } => (&f.unchanged_not_written, vec![block(b)]),
    };
    let places: Vec<(&str, &str)> = places.iter().map(|(name, value)| (*name, value.as_str())).collect();
    agents::fill(text.trim_end(), &places)
}

/// The version of the block written at this place.
pub(crate) fn version<'a>(
    versions: &'a BTreeMap<BlockId, Vec<VersionView>>,
    block: &BlockId,
    at: Seq,
) -> Option<&'a VersionView> {
    versions.get(block).and_then(|vs| vs.iter().find(|v| v.at == at))
}

/// Who wrote the version, in words.
pub(crate) fn writer(flow: &Compiled, work: &Work, block: &BlockId, written: Option<&VersionView>) -> String {
    match written.map(|v| &v.by) {
        Some(Writer::Run(run)) => {
            let cell = work
                .records
                .started
                .iter()
                .find(|s| s.run == *run)
                .map(|s| s.cell.as_str());
            by_a_cell(cell.unwrap_or_default())
        }
        Some(Writer::Outside) => {
            let source = flow.blocks.get(block).and_then(|b| b.writers.outside.clone());
            by_the_outside(&source.unwrap_or_default())
        }
        Some(Writer::Person) | None => BY_A_PERSON.to_string(),
    }
}

/// The cell's record as this run started: the latest finished run of the cell that started before this one, with
/// the place it finished at.
fn record_before<'a>(work: &'a Work, this: &Started) -> Option<(&'a Started, Seq)> {
    (work.records.finished.iter())
        .filter_map(|f| {
            let started = work.records.started.iter().find(|s| s.run == f.run)?;
            (started.cell == this.cell && started.at < this.at).then_some((started, f.at))
        })
        .max_by_key(|(_, at)| *at)
}

/// The version of the block the record knows: the one it wrote itself, or else the one its run started from.
fn last_worked_from(work: &Work, (started, finished): (&Started, Seq), block: &BlockId) -> Option<Seq> {
    let wrote = (work.records.finished.iter())
        .find(|f| f.at == finished)
        .is_some_and(|f| f.writes.iter().any(|w| w.block == *block));
    if wrote {
        Some(finished)
    } else {
        started.basis.get(block).copied().flatten()
    }
}

#[cfg(test)]
pub(crate) mod tests;
