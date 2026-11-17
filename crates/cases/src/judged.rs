//! What a case's cell did, read from its work's records, and how it is judged mechanically (design document `testing`,
//! 14): first by the engine's own checks of a return, each return of the cell sent back failing the case with the
//! engine's reasons; then by each thing the case says the cell owes; then by what any cell owes in any case: its answer
//! in a dialogue it holds as the maker, and in its own words, written only where that dialogue or those words ran the
//! round (design document `data-and-context`, 36, 41), and a judge's pass written as its verdict alone (principle
//! the-one-woken-owes-the-next-word). What only reading can judge is the reading judge's.

use decide::run::Unmet;
use decide::{BlockId, CellId, RunId, Write};
use flow::{CellSpec, Compiled};
use serde::{Deserialize, Serialize};
use work::{Given, Work};

use crate::checked::WRITTEN;
use crate::written::Owes;

/// What the cell's runs did, from the first given; kept with a try's result, so that it is judged again where only
/// what the case owes changed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Did {
    /// Its runs, in order.
    pub runs: Vec<RunId>,
    /// Each return of it sent back, by run and round, with the engine's reasons.
    pub sent_back: Vec<(RunId, usize, Vec<Unmet>)>,
    /// Each block its runs wrote as they finished, in order, with the run.
    pub writes: Vec<(RunId, Write)>,
    /// What ran each run: the blocks new to it as it started.
    pub ran_by: Vec<(RunId, Vec<BlockId>)>,
    /// Each question its runs asked or withdrew: its name, whether the work waits for it, whether it withdraws it.
    pub questions: Vec<(String, bool, bool)>,
}

/// What the cell's runs did, from this run on.
pub fn did(flow: &Compiled, work: &Work, cell: &CellId, from: RunId) -> Did {
    let records = &work.records;
    let runs: Vec<RunId> = (records.started.iter())
        .filter(|s| &s.cell == cell && s.run >= from)
        .map(|s| s.run)
        .collect();
    let mut did = Did {
        runs: runs.clone(),
        ..Did::default()
    };
    for ((run, round), given) in &work.given {
        if let (true, Given::SentBack(unmet)) = (runs.contains(run), given) {
            did.sent_back.push((*run, round - 1, unmet.clone()));
        }
    }
    for finished in records.finished.iter().filter(|f| runs.contains(&f.run)) {
        did.writes
            .extend(finished.writes.iter().map(|w| (finished.run, w.clone())));
    }
    for run in &runs {
        let told = decide::told(&flow.flow, records, *run)
            .map(|t| t.ran_by)
            .unwrap_or_default();
        did.ran_by.push((*run, told));
    }
    for question in records.questions.iter().filter(|q| runs.contains(&q.run) && !q.engine) {
        did.questions
            .push((question.name.clone(), question.waits, question.withdrawn));
    }
    did
}

/// Each block of this name the cell's runs wrote, with its run, in order.
fn written<'a>(did: &'a Did, block: &'a str) -> impl Iterator<Item = &'a (RunId, Write)> + 'a {
    did.writes.iter().filter(move |(_, w)| w.block.as_str() == block)
}

/// Every way what the cell did falls short of what it owes, each said once; none where it owes nothing it did not do.
pub fn judge(flow: &Compiled, cell: &CellSpec, owes: &Owes, did: &Did) -> Vec<String> {
    let mut found = Vec::new();
    for (run, round, unmet) in &did.sent_back {
        found.push(format!(
            "the engine sent back round {round} of run {}: {unmet:?}",
            run.0
        ));
    }
    let place = flow.flow.place(&cell.id);
    let walked = place.map(|p| &flow.flow.cells()[p]);
    let outputs = walked.map_or(&[][..], |c| &c.outputs[..]);
    // A datum, or each block of a group.
    let blocks = |name: &str| -> Vec<String> {
        match flow.groups.get(name) {
            Some(group) => group.clone(),
            None => vec![name.to_string()],
        }
    };
    for (block, value) in &owes.writes {
        let last = written(did, block).last();
        match (last, value.as_str()) {
            (None, _) => found.push(format!("owes `{block}` written, and wrote none")),
            (Some((_, w)), want) if want != WRITTEN && w.value.as_deref() != Some(want) => {
                let wrote = w.value.as_deref().unwrap_or("no value");
                found.push(format!("owes `{block}` = {want}, and wrote {wrote}"));
            }
            _ => {}
        }
    }
    for block in &owes.not_writes {
        if written(did, block).next().is_some() {
            found.push(format!("owes no `{block}`, and wrote it"));
        }
    }
    let changes = |name: &str| {
        blocks(name)
            .iter()
            .any(|b| written(did, b).any(|(_, w)| !w.meaning_unchanged))
    };
    for name in &owes.changes {
        if !changes(name) {
            found.push(format!(
                "owes `{name}` brought up to date, and handed in no change of it"
            ));
        }
    }
    for name in &owes.keeps {
        if changes(name) {
            found.push(format!("owes `{name}` kept as it is, and handed in a change of it"));
        }
    }
    for block in &owes.answers {
        if written(did, block).next().is_none() {
            found.push(format!("owes its answer in `{block}`, and wrote none"));
        }
    }
    let asked = did.questions.iter().filter(|(_, _, withdrawn)| !withdrawn);
    match owes.question.as_deref() {
        Some("none") if asked.clone().next().is_some() => {
            let names: Vec<&str> = asked.map(|(n, _, _)| n.as_str()).collect();
            found.push(format!("owes no question, and asked {}", names.join(", ")));
        }
        Some("waits") if !asked.clone().any(|(_, waits, _)| *waits) => {
            found.push("owes a question the work waits for, and asked none".to_string());
        }
        _ => {}
    }
    // A run commits where it writes a block the cell does not write itself: what the engine read of the branch.
    let committed = did.writes.iter().any(|(_, w)| !outputs.contains(&w.block));
    match owes.commits {
        Some(true) if !committed => found.push("owes commits on the work's branch, and made none".to_string()),
        Some(false) if committed => found.push("owes no commit, and committed".to_string()),
        _ => {}
    }
    // What any cell owes in any case.
    let own_words = engine::spoken::words_of(flow, &cell.id);
    for (run, w) in &did.writes {
        let ran = (did.ran_by.iter())
            .find(|(r, _)| r == run)
            .map_or(&[][..], |(_, b)| b.as_slice());
        let answer = walked.is_some_and(|c| c.dialogues.contains_key(&w.block)) || own_words == Some(&w.block);
        if answer && !ran.contains(&w.block) {
            found.push(format!("wrote `{}` where nothing of it ran the round", w.block));
        }
    }
    for (verdict, of) in walked.map(|c| &c.verdicts).into_iter().flatten() {
        for run in &did.runs {
            let passed = (did.writes.iter())
                .any(|(r, w)| r == run && &w.block == verdict && w.value.as_deref() != Some(of.fails.as_str()));
            let points = did.writes.iter().any(|(r, w)| r == run && w.block == of.dialogue);
            if passed && points {
                found.push(format!("wrote `{}` with a pass in `{verdict}`", of.dialogue));
            }
        }
    }
    found
}
