//! Keeping the sessions warm. The agents' program keeps a session's prompt in its cache a while after
//! its last call (an hour for `claude`); a round after that writes the whole session to the cache again. A write kept
//! an hour costs forty reads on Opus 5.5 and Sonnet 5.5, twenty on Haiku 5.5 and eighty on Fable 5.1, so a session read
//! once by a minimal message a little before the cache lets it go costs a small part of what its next round would pay
//! otherwise.
//!
//! So in a work that has not ended, the session of each of its lines that can come back is sent a keep-warm ping
//! `after_ms` after its own last call, and again as long as it goes on without a call, up to as many pings as cost,
//! each reading the session, no more than writing it again would beyond reading it, on its model ([`Warm::most`]). A
//! line can come back where a cell of it is where the work stands, or is reached from there by the data the cells
//! write: a ping of a line the work has passed for good would be paid for nothing. A ping that failed is tried again a
//! minute after, while the cache keeps the session.
//!
//! A session is kept warm by its own last call alone, whatever else the work does: what the work's other lines run, or
//! what a call of the work's own tools takes, changes nothing of when it is pinged. Only the session's own program
//! stops its ping: a round of its line under way, or being stopped, is the session's last call not yet made, and the
//! cache is kept by the calls it makes (`line_busy`).
//!
//! Whoever holds the work, its lines that can come back are pinged all the same: the engine as it opened, the owner's
//! pause of everything or of the work alone, or a question the work waits on the owner for. A hold is on the work
//! going on, and a session's cache is what the work has already paid for, which a hold is not there to let go of
//! (principles `no-unseen-spending-loop` and `keep-what-was-paid-for`). Two exceptions, each for a reason of its own:
//! not while the wait every work shares holds the agents' rounds, since a ping runs with the command line a round runs
//! with and would fail as they do ([`crate::Driver::pings`]); and not for a work the engine paused because it spent
//! more than it may, since a ping counts in the work's spend and the budget is the refusal to let that work spend
//! more. Whether sessions are kept warm at all is the engine's own state, which the owner switches
//! ([`crate::Driver::keep_warm`]): switched off, no ping is given, whatever a work's state.
//!
//! A session's cache is the account's its calls ran under, and a ping runs under the account in use, as every round
//! handed out does: only a session whose last call ran under the account in use is pinged. One whose last call ran
//! under another account, or under none, is not, a ping under the one in use reading nothing of its cache; and with
//! none in use none is, a ping having no account to run under (decision no-default-credential).
//!
//! A ping and a round of one line never run beside each other, as they would be of one session: none is due while a
//! round of its line is under way, and the loop holds back the rounds of the line while its ping is (crate `service`,
//! [`crate::Driver::gate_of`]); a round of another line begins and runs beside it. What is due is worked out from the
//! work's records and the time alone: a session's last call is the end of the latest of its rounds that called the
//! model, or of its latest ping that did not fail, and each ping is recorded as it ends, so that after a stop a ping
//! is given again only where one is still due. Which pings are under way the driver keeps in memory, so that none is
//! given twice while it runs.

use std::collections::{BTreeMap, BTreeSet};

use decide::{CellId, Decision, Pauser, RunId, decide};
use flow::Compiled;
use store::{Item, WorkNumber};
use work::{Time, Work};

use crate::accounts::Accounts;
use crate::sessions::{self, Policy, Warm};

/// How long after a ping that failed the next is tried, in milliseconds, while the cache still keeps the session: a
/// failure that passes is most often over in a minute, and a minute is a sixtieth of what the cache keeps.
pub const RETRY_MS: u64 = 60_000;

/// A ping due: the line, and which of its sessions.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Due {
    pub line: String,
    pub session: u32,
}

/// A line's session the work keeps warm: the ping it is due, when, and when the cache lets the session go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    pub due: Due,
    pub at: Time,
    pub life: Time,
}

/// Each line's session the work of this number keeps warm as of `now`, in the order of the walk, whoever holds the
/// work: none where `keep_warm`, the engine's own state, says sessions are not kept warm; none for a work that has
/// ended, one with no policy of keeping sessions warm, or one the engine paused for spending more than it may; none of a line with a round under way (`line_busy`), whatever the work's other lines run; none whose
/// last call ran under another account than the one in use, or under none, and none while none is in use, as the
/// accounts say; none of a line that is to begin a new session at its next round; and none whose cache has let it go
/// by `now`, which a ping would write again whole, for nothing.
#[allow(clippy::too_many_arguments)]
pub fn kept(
    flow: &Compiled,
    number: WorkNumber,
    work: &Work,
    item: &Item,
    policy: &Policy,
    keep_warm: bool,
    accounts: &Accounts,
    now: Time,
) -> Vec<Kept> {
    let Some(warm) = policy.warm.as_ref().filter(|_| keep_warm) else {
        return Vec::new();
    };
    let decision = decide(&flow.flow, &work.records);
    if matches!(decision, Decision::Ended(_) | Decision::Fault(_)) || over_budget(work, warm) {
        return Vec::new();
    }
    let back = coming_back(flow, work, &decision);
    (lines(flow))
        .filter(|line| back.contains(*line))
        .filter(|line| !line_busy(flow, work, line))
        .filter_map(|line| next(flow, number, work, item, line, policy, accounts))
        .filter(|kept| now < kept.life)
        .collect()
}

/// The pings due for the work of this number at `now`: those [`kept`] whose time has come.
#[allow(clippy::too_many_arguments)]
pub fn due(
    flow: &Compiled,
    number: WorkNumber,
    work: &Work,
    item: &Item,
    policy: &Policy,
    keep_warm: bool,
    accounts: &Accounts,
    now: Time,
) -> Vec<Due> {
    (kept(flow, number, work, item, policy, keep_warm, accounts, now).into_iter())
        .filter(|kept| kept.at <= now)
        .map(|kept| kept.due)
        .collect()
}

/// Whether a round of a run of a cell of the line has begun and not ended: the line's program is running, or, where
/// the run was abandoned as its round was stopped, has not yet ended. Its session is then in use, and is not pinged.
fn line_busy(flow: &Compiled, work: &Work, line: &str) -> bool {
    let r = &work.records;
    let on_line: BTreeSet<RunId> = (r.started.iter())
        .filter(|s| line_of_run(flow, work, s.run) == Some(line))
        .map(|s| s.run)
        .collect();
    let mut open: BTreeMap<RunId, isize> = BTreeMap::new();
    for round in r.rounds.iter().filter(|x| on_line.contains(&x.run)) {
        *open.entry(round.run).or_default() += 1;
    }
    for ended in r.returned.iter().filter(|x| on_line.contains(&x.run)) {
        *open.entry(ended.run).or_default() -= 1;
    }
    open.values().any(|n| *n > 0)
}

/// Whether the engine paused the work because it spent more than it may: the pause that holds the work is the
/// engine's, and what the work has spent in all, its pings with its rounds, is past the bar its budget sets
/// ([`Warm::budget`]). A ping counts in that spend, and the bar its budget sets is the refusal to let the work spend
/// more; let on by the owner, who alone lets a work on, the work is kept warm as any other, to the bar that
/// letting-on raised ([`crate::budget`]). A round judges its spend by the pings as the work stood when it began, so
/// that the pings of other lines that end while it runs count at the next judgment, and not before.
fn over_budget(work: &Work, warm: &Warm) -> bool {
    let Some(budget) = warm.budget else {
        return false;
    };
    let engine = (decide::pause(&work.records)).is_some_and(|pause| pause.by == Pauser::Engine);
    engine && crate::history::totals(work).0 > budget.bar(work) as f64
}

/// How a line's session stands for keeping it warm: the session, when its next ping is due, whether or not that time
/// has come, and when the cache lets the session go; none where it is not to be kept warm.
fn next(
    flow: &Compiled,
    number: WorkNumber,
    work: &Work,
    item: &Item,
    line: &str,
    policy: &Policy,
    accounts: &Accounts,
) -> Option<Kept> {
    let warm = policy.warm.as_ref()?;
    let on_line = |run: RunId| line_of_run(flow, work, run) == Some(line);
    let session = work::sessions::number_at(work, line, work.tick());
    // The session's latest round that called the model, by the place it ended.
    let r = &work.records;
    let mut counted: Vec<RunId> = Vec::new();
    let mut last_round: Option<(decide::Seq, RunId, usize)> = None;
    for round in &r.rounds {
        counted.push(round.run);
        let k = counted.iter().filter(|x| **x == round.run).count();
        if !on_line(round.run) {
            continue;
        }
        let ended = (r.returned.iter()).filter(|e| e.run == round.run).nth(k - 1);
        let called = work
            .spent
            .get(&(round.run, k))
            .is_some_and(|s| s.cached_tokens + s.uncached_tokens > 0);
        let began = work::sessions::start(work, round.run, k);
        if let Some(ended) = ended
            && called
            && work::sessions::number_at(work, line, began) == session
            && last_round.is_none_or(|(at, ..)| at < ended.at)
        {
            last_round = Some((ended.at, round.run, k));
        }
    }
    let (last_round, run, k) = last_round?;
    // As many pings as the session's model makes worth it.
    let model = (work.ran.iter().rev()).find(|t| t.run == run && t.round == k)?;
    // The session's cache is the account's its last call ran under, which a ping under the account in use must be;
    // with none in use, a ping has none to run under.
    if accounts.in_use().is_none() || model.on.account.as_deref() != accounts.in_use() {
        return None;
    }
    let most = warm.most.get(&model.on.model).copied().unwrap_or(0);
    let pings: Vec<&work::Pinged> = (work.pinged.iter())
        .filter(|p| p.line == line && p.session == session && p.at > last_round)
        .collect();
    if pings.len() >= most as usize {
        return None;
    }
    // The line is not to begin a new session at its next round, which holds nothing of this one.
    if sessions::due(flow, number, work, item, line, policy).is_some() {
        return None;
    }
    let read = pings.iter().filter(|p| p.failed.is_none()).map(|p| p.at).max();
    let last_call = work.time_of(read.unwrap_or(last_round).max(last_round))?;
    let mut at = Time(last_call.0.saturating_add(warm.after_ms));
    if let Some(failed) = pings.last().filter(|p| p.failed.is_some()) {
        at = at.max(Time(work.time_of(failed.at)?.0.saturating_add(RETRY_MS)));
    }
    let life = Time(last_call.0.saturating_add(warm.life_ms));
    let due = Due {
        line: line.to_string(),
        session,
    };
    (at < life).then_some(Kept { due, at, life })
}

/// The lines the work can come back to from where it stands: the lines of the cells where it stands, those of its open
/// runs and the one it runs, waits or is held at next, or, where it stands at none, as a work stalled with nothing of
/// it under way, the cell it last ran; and of every cell a block those cells write runs, and on, as far as the data
/// reach.
fn coming_back(flow: &Compiled, work: &Work, decision: &Decision) -> BTreeSet<String> {
    let r = &work.records;
    let open = (r.started.iter())
        .filter(|s| !r.finished.iter().any(|f| f.run == s.run) && !r.abandoned.iter().any(|a| a.run == s.run))
        .map(|s| s.cell.clone());
    let mut at: Vec<CellId> = open.collect();
    match decision {
        Decision::Run { cell, .. }
        | Decision::GoOn { cell, .. }
        | Decision::Hurry { cell, .. }
        | Decision::Abandon { cell, .. }
        | Decision::Paused { cell }
        | Decision::Wait { at: cell, .. } => at.push(cell.clone()),
        _ => {}
    }
    if at.is_empty() {
        at.extend(r.started.last().map(|s| s.cell.clone()));
    }
    let cells = flow.flow.cells();
    let mut reached: Vec<CellId> = Vec::new();
    while let Some(cell) = at.pop() {
        if reached.contains(&cell) {
            continue;
        }
        let Some(place) = flow.flow.place(&cell) else {
            continue;
        };
        for output in &cells[place].outputs {
            at.extend(
                (cells.iter())
                    .filter(|c| c.runs_on.contains(output))
                    .map(|c| c.id.clone()),
            );
        }
        reached.push(cell);
    }
    (flow.cells.iter())
        .filter(|c| reached.contains(&c.id))
        .filter_map(|c| c.line.clone())
        .collect()
}

/// The lines of the flow's cells, each once, in the order of the walk.
fn lines(flow: &Compiled) -> impl Iterator<Item = &str> {
    let mut seen = Vec::new();
    for line in flow.cells.iter().filter_map(|cell| cell.line.as_deref()) {
        if !seen.contains(&line) {
            seen.push(line);
        }
    }
    seen.into_iter()
}

/// The line of the run's cell.
fn line_of_run<'a>(flow: &'a Compiled, work: &Work, run: RunId) -> Option<&'a str> {
    let started = work.records.started.iter().find(|s| s.run == run)?;
    (flow.cells.iter())
        .find(|c| c.id == started.cell)
        .and_then(|c| c.line.as_deref())
}

#[cfg(test)]
mod tests {
    //! Which line a round that has not ended keeps from being pinged.

    use decide::{Abandoned, Returned, Round, Seq, Started};
    use flow::{Has, Source, compile};

    use super::*;

    /// Two lines: `write` on `main`, `judge` on `second`, and the merge.
    fn flow() -> Compiled {
        let text = r#"
            name = "lines"
            order = ["write", "judge", "merge"]
            [limits]
            runs = 3
            returns = 2
            calls = 3
            [lines.main]
            [lines.second]
            [blocks.request]
            people = true
            [blocks.text]
            [blocks.verdict]
            [blocks.merged]
            shown = true
        "#;
        let cell = |line: &str, watches: &str, outputs: &str| {
            format!(
                "title = \"A cell\"\nkind = \"agent\"\neffort = \"high\"\nline = \"{line}\"\ntask = \"Do it.\"\n\
                 watches = [\"{watches}\"]\noutputs = [\"{outputs}\"]\n[kinds]\n{watches} = \"last_and_latest\"\n"
            )
        };
        let merge =
            "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nwatches = [\"verdict\"]\noutputs = [\"merged\"]\n";
        let source = Source {
            flow: text.to_string(),
            cells: [
                ("write", cell("main", "request", "text")),
                ("judge", cell("second", "text", "verdict")),
                ("merge", merge.to_string()),
            ]
            .into_iter()
            .map(|(id, text)| (id.to_string(), text))
            .collect(),
            files: [(
                flow::waits_file(),
                include_str!("../../../flow/agents/waits.toml").to_string(),
            )]
            .into(),
            ..Source::default()
        };
        let has = Has {
            acts: ["merge".to_string()].into(),
            ..Has::default()
        };
        compile(&source, &has).expect("the definition compiles")
    }

    // A round is under way from its beginning to its end whether or not its run was abandoned meanwhile, as a round
    // stopped for a cell the owner turned to is: its program still runs, and its line's session is in use. A line
    // beside it is not. Fails where only the open runs are looked at, which an abandoned run is not, or where a round of
    // another line counts. Microseconds.
    #[test]
    fn a_line_is_busy_from_the_beginning_of_a_round_of_it_to_its_end_though_its_run_was_abandoned() {
        let flow = flow();
        let mut work = Work::default();
        let run = RunId(1);
        assert!(!line_busy(&flow, &work, "main"), "no round has begun");
        work.records.started.push(Started {
            run,
            cell: CellId::from("write"),
            at: Seq(1),
            because: Vec::new(),
            basis: BTreeMap::new(),
        });
        work.records.rounds.push(Round { run, at: Seq(2) });
        assert!(line_busy(&flow, &work, "main"), "its round is under way");
        assert!(
            !line_busy(&flow, &work, "second"),
            "another line's session is not in use"
        );
        work.records.abandoned.push(Abandoned { run, at: Seq(3) });
        assert!(line_busy(&flow, &work, "main"), "its round is being stopped");
        work.records.returned.push(Returned { run, at: Seq(4) });
        assert!(!line_busy(&flow, &work, "main"), "its round has ended");
    }
}
