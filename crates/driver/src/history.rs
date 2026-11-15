//! What a measured quantity is judged by: the history of its own kind, never a number set by hand (principle
//! `judged-by-own-history`). The driver alone sees every work, so it gathers the history from the records of every
//! work on a flow of the same name, and gives each judgment its line.
//!
//! A line stands past the largest value of the history by as much as that is past its median: `2 × largest − median`,
//! in the quantity's own units, so that it reaches as far out as the history itself does. A round is judged against
//! the rounds of its own cell where at least as many of them as the settings say (`history.samples`) tell the quantity,
//! with a median above nothing, since a history with no scale gives no line; otherwise against every round of the
//! flow, where a median of nothing is usual and only a history of nothing at all gives no line; and with fewer there
//! too, it is not judged. A round that went past the lines of a signal that stops a round, stopped or let run, never
//! enters the history, nor does one whose session was replaced for its size: what nothing explained never becomes the
//! measure of what is usual, which it would raise for good.

use decide::run::Failure;
use decide::{CellId, RunId};
use flow::Compiled;
use work::{Back, Measures, Spent, Work};

/// The line of a history: past its largest value by as much as that is past its median. None for an empty one.
pub fn line(values: &[f64]) -> Option<f64> {
    line_of_sorted(&sorted(values))
}

/// The median of values, the mean of the middle two for an even number of them; none for no value.
pub fn median(values: &[f64]) -> Option<f64> {
    median_of_sorted(&sorted(values))
}

/// The values, the smallest first.
fn sorted(values: &[f64]) -> Vec<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted
}

/// The line of a history whose values are sorted, the smallest first.
fn line_of_sorted(sorted: &[f64]) -> Option<f64> {
    Some(2.0 * sorted.last()? - median_of_sorted(sorted)?)
}

/// The median of values sorted, the smallest first.
fn median_of_sorted(sorted: &[f64]) -> Option<f64> {
    let n = sorted.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => Some(sorted[n / 2]),
        _ => Some((sorted[n / 2 - 1] + sorted[n / 2]) / 2.0),
    }
}

/// What a line was worked out from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// The rounds of the cell itself.
    Cell,
    /// Every round of the flow.
    Flow,
    /// The other works of the flow.
    Works,
    /// The rounds of the line.
    Line,
    /// The calls of the tool.
    Tool,
}

/// A line, with how many values gave it and from where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub value: f64,
    pub samples: usize,
    pub scope: Scope,
}

/// The line of values of one's own, where at least `least` of them have a median above nothing; else of everyone's,
/// from at least `least`, where a line above nothing; else none.
fn judged(own: &[f64], any: &[f64], least: usize, own_scope: Scope, any_scope: Scope) -> Option<Line> {
    judged_sorted(&sorted(own), &sorted(any), least, own_scope, any_scope)
}

/// [`judged`] of values each sorted, the smallest first.
pub(crate) fn judged_sorted(
    own: &[f64],
    any: &[f64],
    least: usize,
    own_scope: Scope,
    any_scope: Scope,
) -> Option<Line> {
    if own.len() >= least.max(1) && median_of_sorted(own).is_some_and(|m| m > 0.0) {
        return line_of_sorted(own).map(|value| Line {
            value,
            samples: own.len(),
            scope: own_scope,
        });
    }
    if any.len() < least.max(1) {
        return None;
    }
    let value = line_of_sorted(any).filter(|v| *v > 0.0)?;
    Some(Line {
        value,
        samples: any.len(),
        scope: any_scope,
    })
}

/// A round of the history: the cell it was of, its line, what its calls came to and what it took, the model it ran
/// on, how long it took, in milliseconds, and the prompt its session last had before it, in tokens, where the records
/// tell.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    /// The place where the round ended, in its work's order.
    pub at: decide::Seq,
    /// When it ended, where the records tell.
    pub ended: Option<work::Time>,
    pub cell: CellId,
    pub line: Option<String>,
    /// Which of its line's sessions it ran in, counted from none.
    pub session: Option<u32>,
    /// Whether the session it ran in was replaced for the context a round of it ended with: a session past the limit
    /// of its time is no measure of what is usual, in any of its rounds.
    pub cut: bool,
    pub measures: Measures,
    pub spent: Spent,
    pub model: Option<String>,
    pub ms: Option<u64>,
    pub before: Option<u64>,
}

/// Whether the line's session of this number, counted from none, was replaced for the context a round of it ended
/// with: the session after it began for that.
fn cut(work: &Work, line: &str, session: u32) -> bool {
    let next = (work::sessions::of_line(work, line))
        .filter(|s| s.why.own_id())
        .nth(session as usize);
    next.is_some_and(|s| matches!(s.why, work::Why::Limit { .. }))
}

/// The rounds of a work that are history: those that ended and whose carrier said what their calls came to, at least
/// one, but for those the run guard stopped.
pub fn samples(flow: &Compiled, work: &Work) -> Vec<Sample> {
    let mut out = Vec::new();
    let mut counted: Vec<RunId> = Vec::new();
    let r = &work.records;
    for round in &r.rounds {
        counted.push(round.run);
        let number = counted.iter().filter(|run| **run == round.run).count();
        let key = (round.run, number);
        let guarded = matches!(
            work.back.get(&key),
            Some(Back::Failed {
                failure: Failure::Guarded,
                ..
            })
        );
        let (Some(spent), Some(measures)) = (
            work.spent.get(&key),
            work::measured::of_round(&work.measured, key.0, key.1),
        ) else {
            continue;
        };
        // A round past the lines of a signal that stops a round, stopped or let run, is no measure of what is usual.
        if guarded || measures.crossed || measures.calls == 0 {
            continue;
        }
        let Some(started) = r.started.iter().find(|s| s.run == round.run) else {
            continue;
        };
        let line = (flow.cells.iter())
            .find(|c| c.id == started.cell)
            .and_then(|c| c.line.clone());
        let ended = (r.returned.iter()).filter(|e| e.run == round.run).nth(number - 1);
        let ms = ended.and_then(|ended| {
            let (from, to) = (work.time_of(round.at)?, work.time_of(ended.at)?);
            Some(to.0.saturating_sub(from.0))
        });
        let model = (work.ran.iter().rev())
            .find(|t| t.run == round.run && t.round == number)
            .map(|t| t.on.model.clone());
        let before = line
            .as_deref()
            .and_then(|line| last_prompt(flow, work, line, round.run, number));
        let Some(ended) = ended else {
            continue;
        };
        let on_line = |run: RunId| line_of_run(flow, work, run) == line.as_deref();
        let session = (line.as_deref()).map(|l| work::sessions::of_round(work, l, on_line, round.run, number).0);
        let cut = (line.as_deref().zip(session)).is_some_and(|(l, n)| cut(work, l, n));
        out.push(Sample {
            at: ended.at,
            ended: work.time_of(ended.at),
            cell: started.cell.clone(),
            line,
            session,
            cut,
            measures: measures.clone(),
            spent: *spent,
            model,
            ms,
            before,
        });
    }
    out
}

/// Whether two names are of one model: a model named with its context window, as `[1m]`, is the model.
fn same_model(a: &str, b: &str) -> bool {
    model_of(a) == model_of(b)
}

/// The model a name is of, its context window left out.
pub(crate) fn model_of(name: &str) -> &str {
    name.split('[').next().unwrap_or(name)
}

/// The lines a round of a cell is judged by, each none where too few rounds tell it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lines {
    /// The tokens its context grows by over the round.
    pub added: Option<Line>,
    /// The tokens its context grows by for each call after the first.
    pub rate: Option<Line>,
    /// The rise of its first call's prompt over the session's last before the round.
    pub jump: Option<Line>,
    /// That rise for each character of the message the round was given.
    pub ratio: Option<Line>,
    /// What it costs, in millionths of a dollar, among rounds on the same model.
    pub cost: Option<Line>,
    /// How long it takes, in milliseconds.
    pub time: Option<Line>,
    /// What a work of the flow spends in all, in millionths of a dollar, as the flow's other works have.
    pub work_spent: Option<Line>,
    /// How long the rounds of a work of the flow take in all, in milliseconds, as the flow's other works' have.
    pub work_time: Option<Line>,
    /// How many runs of a cell in a row ask, for a tool or for tests, before a run of it hands in its work, as the
    /// rows of asking of the cell, else of the flow, in every work of the flow have ([`asking_rows`]).
    pub asking: Option<Line>,
}

impl Lines {
    /// The lines of a round of the cell on the model, from the history, a cell's own where at least `least` of its
    /// rounds tell it, else the flow's.
    pub fn of(history: &[Sample], cell: &CellId, model: &str, least: usize) -> Lines {
        Lines::of_refs(&history.iter().collect::<Vec<_>>(), cell, model, least)
    }

    /// [`Lines::of`] a history held by reference.
    pub fn of_refs(history: &[&Sample], cell: &CellId, model: &str, least: usize) -> Lines {
        let own: Vec<&Sample> = history.iter().copied().filter(|s| s.cell == *cell).collect();
        let every: Vec<&Sample> = history.to_vec();
        let values = |of: &[&Sample], f: &dyn Fn(&Sample) -> Option<f64>| -> Vec<f64> {
            of.iter().filter_map(|s| f(s)).collect()
        };
        let whole = |f: &dyn Fn(&Sample) -> Option<f64>| {
            judged(&values(&own, f), &values(&every, f), least, Scope::Cell, Scope::Flow)
        };
        let on_model = |of: &[&Sample]| -> Vec<f64> {
            (of.iter())
                .filter(|s| s.model.as_deref().is_some_and(|m| same_model(m, model)))
                .map(|s| s.spent.usd_micros as f64)
                .collect()
        };
        Lines {
            added: whole(&|s| Some(s.measures.added as f64)),
            rate: whole(&|s| s.measures.rate().map(|r| r as f64)),
            jump: whole(&|s| s.jump().map(|j| j as f64)),
            ratio: whole(&Sample::ratio),
            cost: judged(&on_model(&own), &on_model(&every), least, Scope::Cell, Scope::Flow),
            time: whole(&|s| s.ms.map(|ms| ms as f64)),
            work_spent: None,
            work_time: None,
            asking: None,
        }
    }
}

/// What a work has spent in all, in millionths of a dollar, its keep-warm pings with its rounds, and how long its rounds
/// have taken in all, in milliseconds, as its records say.
pub fn totals(work: &Work) -> (f64, f64) {
    let pinged: u64 = work.pinged.iter().map(|p| p.spent.usd_micros).sum();
    let spent: u64 = work.spent.values().map(|s| s.usd_micros).sum::<u64>() + pinged;
    let r = &work.records;
    let mut ms = 0u64;
    let mut counted: Vec<RunId> = Vec::new();
    for round in &r.rounds {
        counted.push(round.run);
        let k = counted.iter().filter(|x| **x == round.run).count();
        let ended = (r.returned.iter()).filter(|e| e.run == round.run).nth(k - 1);
        if let Some(ended) = ended
            && let (Some(from), Some(to)) = (work.time_of(round.at), work.time_of(ended.at))
        {
            ms += to.0.saturating_sub(from.0);
        }
    }
    (spent as f64, ms as f64)
}

impl Sample {
    /// How far its first call's prompt rose over the session's last before the round, where the records tell; none for
    /// a round that began its session.
    fn jump(&self) -> Option<u64> {
        Some(self.measures.first_context?.saturating_sub(self.before?))
    }

    /// The rise for each character of its message.
    fn ratio(&self) -> Option<f64> {
        let jump = self.jump()? as f64;
        (self.measures.given > 0).then(|| jump / self.measures.given as f64)
    }
}

/// The prompt the session that the round of this number of the run, on the line, runs in had last before the round,
/// in tokens: the context of the latest of the session's earlier rounds and keep-warm pings that tells one. None where
/// the round begins its session, or nothing before it in the session tells its context.
pub fn last_prompt(flow: &Compiled, work: &Work, line: &str, run: RunId, number: usize) -> Option<u64> {
    let on_line = |r: RunId| line_of_run(flow, work, r) == Some(line);
    let (session, begins) = work::sessions::of_round(work, line, on_line, run, number);
    if begins {
        return None;
    }
    let at = work::sessions::start(work, run, number);
    let r = &work.records;
    let mut latest: Option<(decide::Seq, u64)> = None;
    let mut counted: Vec<RunId> = Vec::new();
    for round in &r.rounds {
        counted.push(round.run);
        let k = counted.iter().filter(|x| **x == round.run).count();
        if !on_line(round.run) {
            continue;
        }
        let began = work::sessions::start(work, round.run, k);
        let ended = (r.returned.iter()).filter(|e| e.run == round.run).nth(k - 1);
        let (Some(ended), Some(context)) = (ended, work.spent.get(&(round.run, k)).and_then(|s| s.context_tokens))
        else {
            continue;
        };
        if began < at && ended.at < at && work::sessions::number_at(work, line, began) == session {
            latest = latest.max(Some((ended.at, context)));
        }
    }
    for ping in work
        .pinged
        .iter()
        .filter(|p| p.line == line && p.session == session && p.at < at)
    {
        if let Some(context) = ping.spent.context_tokens {
            latest = latest.max(Some((ping.at, context)));
        }
    }
    latest.map(|(_, context)| context)
}

/// What the latest of the earlier rounds of the session that the round of this number of the run, on the line, runs
/// in came back with, where one came back with anything: the session has held it only since that round, and the round
/// is given it beside its message. None where the round begins its session.
pub fn last_return<'a>(flow: &Compiled, work: &'a Work, line: &str, run: RunId, number: usize) -> Option<&'a Back> {
    let on_line = |r: RunId| line_of_run(flow, work, r) == Some(line);
    let (session, begins) = work::sessions::of_round(work, line, on_line, run, number);
    if begins {
        return None;
    }
    let at = work::sessions::start(work, run, number);
    let r = &work.records;
    let mut latest: Option<(decide::Seq, &Back)> = None;
    let mut counted: Vec<RunId> = Vec::new();
    for round in &r.rounds {
        counted.push(round.run);
        let k = counted.iter().filter(|x| **x == round.run).count();
        if !on_line(round.run) {
            continue;
        }
        let began = work::sessions::start(work, round.run, k);
        let ended = (r.returned.iter()).filter(|e| e.run == round.run).nth(k - 1);
        let (Some(ended), Some(back)) = (ended, work.back.get(&(round.run, k))) else {
            continue;
        };
        if matches!(back, Back::Failed { .. }) {
            continue;
        }
        if began < at
            && ended.at < at
            && work::sessions::number_at(work, line, began) == session
            && latest.is_none_or(|(was, _)| was < ended.at)
        {
            latest = Some((ended.at, back));
        }
    }
    latest.map(|(_, back)| back)
}

/// The line of the run's cell.
fn line_of_run<'a>(flow: &'a Compiled, work: &Work, run: RunId) -> Option<&'a str> {
    let started = work.records.started.iter().find(|s| s.run == run)?;
    (flow.cells.iter())
        .find(|c| c.id == started.cell)
        .and_then(|c| c.line.as_deref())
}

/// The line of the context the rounds of a line end with, from the history: where a line's session is past it as its
/// last round ends, its next round begins a new one. The rounds of a session replaced for its size are no part of it,
/// and the caller leaves out those of the session judged. None where fewer than `least` of the line's rounds tell it.
pub fn context_line<'a>(history: impl IntoIterator<Item = &'a Sample>, line: &str, least: usize) -> Option<Line> {
    let values: Vec<f64> = (history.into_iter())
        .filter(|s| s.line.as_deref() == Some(line) && !s.cut)
        .filter_map(|s| s.spent.context_tokens.map(|c| c as f64))
        .collect();
    judged(&values, &[], least, Scope::Line, Scope::Line)
}

/// The line of what works of a flow come to, from what each of the others came to: none where fewer than `least`
/// tell it.
pub fn works_line(others: &[f64], least: usize) -> Option<Line> {
    judged(others, &[], least, Scope::Works, Scope::Works)
}

/// The line of how many runs of a cell in a row ask before one hands in its work: of the cell's own rows where at least
/// `least` of them tell it, else of every cell's of the flow ([`asking_rows`]).
pub fn asking_line(own: &[f64], any: &[f64], least: usize) -> Option<Line> {
    judged(own, any, least, Scope::Cell, Scope::Flow)
}

/// Of each run of the work that finished, in the order they finished, its cell, whether it asked and where it
/// finished: it asked for calls, or for tests in a record its cell asks in ([`decide::Cell::asks`]). A run that asked
/// handed in no work of its cell's, whatever else it wrote.
fn asked(flow: &Compiled, work: &Work) -> Vec<(CellId, bool, decide::Seq)> {
    let r = &work.records;
    let mut finished: Vec<&decide::Finished> = r.finished.iter().collect();
    finished.sort_by_key(|f| f.at);
    (finished.into_iter())
        .filter_map(|f| {
            let cell = &r.started.iter().find(|s| s.run == f.run)?.cell;
            let records = flow
                .flow
                .place(cell)
                .map_or(&[][..], |i| &flow.flow.cells()[i].asks[..]);
            let called = (r.called.iter())
                .any(|c| c.run == f.run && matches!(work.calls.get(&c.call), Some(work::Call::Asked(_))));
            let entry = (f.writes.iter()).any(|w| records.iter().any(|a| a.record == w.block));
            Some((cell.clone(), called || entry, f.at))
        })
        .collect()
}

/// The rows of asking of the work, each with its cell: each count of a cell's finished runs in a row that asked before
/// a run of it that handed in its work, which ended the row. What a measure of asking is judged by is made of them, as
/// the rows of every work on the flow give them; the row a cell is in now is no part of it ([`asking_now`]).
pub fn asking_rows(flow: &Compiled, work: &Work) -> Vec<(CellId, f64)> {
    let mut rows = Vec::new();
    let mut counts: std::collections::BTreeMap<CellId, u32> = std::collections::BTreeMap::new();
    for (cell, asked, _) in asked(flow, work) {
        let count = counts.entry(cell.clone()).or_default();
        if asked {
            *count += 1;
        } else if *count > 0 {
            rows.push((cell, f64::from(*count)));
            *count = 0;
        }
    }
    rows
}

/// How many of the cell's latest finished runs in a row asked, for calls or for tests, of those that finished after
/// `since`, where the work was last let on after the engine paused it, as its spend and time are counted: the row it is
/// in now, which a run that hands in its work ends, and the owner letting the work on begins anew.
pub fn asking_now(flow: &Compiled, work: &Work, cell: &CellId, since: decide::Seq) -> u32 {
    let runs = asked(flow, work);
    let of_cell = runs.iter().rev().filter(|(c, _, at)| c == cell && *at > since);
    of_cell.take_while(|(_, asked, _)| *asked).count() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The engineer's flow as the repository defines it, whose R1, D1, I1 and T2 ask for tests in `run_tests`.
    fn engineer() -> Compiled {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
        let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
        let has = flow::Has {
            sources: names(&[
                "answer",
                "asked",
                "dependencies",
                "main",
                "session",
                "start",
                "head",
                "tree",
                "words",
            ]),
            acts: names(&["start", "check", "merge", "run_tests"]),
            tools: names(&["query"]),
            details: names(&["files"]),
            ..Default::default()
        };
        flow::compile(&flow::read(&dir).expect("the flow is read"), &has).expect("the flow compiles")
    }

    /// A work whose runs finished in this order, each of the cell named, each asking for a tool (`call`), asking for
    /// tests in `run_tests` (`entry`) or handing in its work (`work`); with where each finished.
    fn ran(runs: &[(&str, &str)]) -> (Work, Vec<decide::Seq>) {
        let mut work = Work::default();
        let mut finished = Vec::new();
        for (n, (cell, how)) in runs.iter().enumerate() {
            let run = RunId(n as u64 + 1);
            let at = work.tick();
            work.records.started.push(decide::Started {
                run,
                cell: CellId::from(*cell),
                at,
                because: Vec::new(),
                basis: Default::default(),
            });
            if *how == "call" {
                let call = decide::CallId(n as u64 + 100);
                let at = work.tick();
                work.records.called.push(decide::Called {
                    call,
                    run,
                    on: None,
                    at,
                });
                let ask = work::Ask::Tool(work::Tool {
                    name: "query".to_string(),
                    input: "{}".to_string(),
                });
                work.calls.insert(call, work::Call::Asked(ask));
            }
            let block = if *how == "entry" { "run_tests" } else { "code" };
            let at = work.tick();
            work.records.finished.push(decide::Finished {
                run,
                at,
                writes: vec![decide::Write {
                    block: block.into(),
                    meaning_unchanged: false,
                    empty: false,
                    value: None,
                }],
                dealt_with: Some(Vec::new()),
                waits_for: None,
            });
            finished.push(at);
        }
        (work, finished)
    }

    // A row of asking is the runs of one cell in a row that asked, for a tool or for tests, ended by a run of it that
    // hands in its work; the rows of each cell apart, and the row a cell is in now no part of them. Well under a
    // millisecond alone; no other test holds the rows.
    #[test]
    fn a_row_of_asking_is_a_cells_runs_in_a_row_that_asked_ended_by_one_that_handed_in() {
        let flow = engineer();
        let (work, _) = ran(&[
            ("I1", "call"),
            ("I1", "entry"),
            ("T2", "entry"),
            ("I1", "work"),
            ("T2", "work"),
            ("I1", "call"),
            ("I1", "work"),
            ("I1", "entry"),
        ]);
        let rows = asking_rows(&flow, &work);
        let rows: Vec<(&str, f64)> = rows.iter().map(|(cell, n)| (cell.as_str(), *n)).collect();
        assert_eq!(rows, [("I1", 2.0), ("T2", 1.0), ("I1", 1.0)]);
        assert_eq!(asking_now(&flow, &work, &"I1".into(), decide::Seq(0)), 1);
        assert_eq!(asking_now(&flow, &work, &"T2".into(), decide::Seq(0)), 0);
        // A run that handed in nothing of the kind, as a cell that never asked, makes no row.
        let (none, _) = ran(&[("I1", "work"), ("I1", "work")]);
        assert!(asking_rows(&flow, &none).is_empty());
    }

    // The row a cell is in now is counted from where the work was last let on after the engine paused it, as its spend
    // and time are: the runs that asked before it are no part of it. Well under a millisecond alone.
    #[test]
    fn the_row_a_cell_is_in_now_is_counted_from_where_the_work_was_last_let_on() {
        let flow = engineer();
        let (work, at) = ran(&[("I1", "call"), ("I1", "entry"), ("I1", "call")]);
        assert_eq!(asking_now(&flow, &work, &"I1".into(), decide::Seq(0)), 3);
        assert_eq!(asking_now(&flow, &work, &"I1".into(), at[1]), 1);
        assert_eq!(asking_now(&flow, &work, &"I1".into(), at[2]), 0);
    }

    // The line of asking is of the cell's own rows where enough of them tell it, else of every cell's of the flow;
    // with too few rows in all, there is none, and nothing is judged. Well under a millisecond alone.
    #[test]
    fn the_line_of_asking_is_the_cells_own_else_the_flows_and_none_from_too_few_rows() {
        let own = [1.0, 2.0, 2.0];
        let any = [1.0, 2.0, 2.0, 3.0, 9.0];
        let line = asking_line(&own, &any, 3).expect("a line");
        assert_eq!((line.value, line.samples, line.scope), (2.0, 3, Scope::Cell));
        let line = asking_line(&own[..2], &any, 3).expect("a line");
        assert_eq!((line.samples, line.scope), (5, Scope::Flow));
        assert_eq!(asking_line(&own[..1], &any[..2], 3), None);
    }

    #[test]
    fn a_line_stands_past_the_largest_value_by_as_much_as_that_is_past_the_median() {
        assert_eq!(line(&[1.0, 2.0, 3.0, 10.0]), Some(17.5));
        assert_eq!(line(&[4.0]), Some(4.0));
        assert_eq!(line(&[]), None);
    }

    #[test]
    fn a_cells_own_rounds_give_its_line_once_there_are_enough_of_them_with_a_scale() {
        let own = [1.0, 2.0, 3.0];
        let any = [1.0, 2.0, 3.0, 50.0];
        let line = judged(&own, &any, 3, Scope::Cell, Scope::Flow).unwrap();
        assert_eq!((line.value, line.samples, line.scope), (4.0, 3, Scope::Cell));
        let line = judged(&own[..2], &any, 3, Scope::Cell, Scope::Flow).unwrap();
        assert_eq!(line.scope, Scope::Flow, "too few of its own");
        let nothing = [0.0, 0.0, 0.0];
        let line = judged(&nothing, &any, 3, Scope::Cell, Scope::Flow).unwrap();
        assert_eq!(line.scope, Scope::Flow, "its own have no scale");
        assert!(judged(&own[..2], &any[..2], 3, Scope::Cell, Scope::Flow).is_none());
        assert!(
            judged(&[], &nothing, 3, Scope::Cell, Scope::Flow).is_none(),
            "nothing at all"
        );
    }

    /// A round of the cell on the model, whose context grew by `added` over `calls` calls, which cost `usd` millionths
    /// of a dollar and began with a prompt `before` past the session's last.
    fn sample(cell: &str, model: &str, calls: u32, added: u64, usd: u64) -> Sample {
        Sample {
            at: decide::Seq(1),
            ended: Some(work::Time(1)),
            cell: CellId::from(cell),
            line: Some("engineer".to_string()),
            session: Some(0),
            cut: false,
            measures: Measures {
                calls,
                added,
                first_context: Some(1_100),
                given: 50,
                ..Measures::default()
            },
            spent: Spent {
                usd_micros: usd,
                context_tokens: Some(added + 1_000),
                ..Spent::default()
            },
            model: Some(model.to_string()),
            ms: Some(60_000),
            before: Some(1_000),
        }
    }

    #[test]
    fn a_rounds_lines_are_its_cells_and_its_cost_is_judged_among_rounds_on_its_model() {
        let history = vec![
            sample("plan", "opus", 3, 100, 10),
            sample("plan", "opus", 5, 200, 20),
            sample("plan", "sonnet", 4, 300, 1),
            sample("build", "opus", 9, 9_000, 90),
        ];
        let lines = Lines::of(&history, &CellId::from("plan"), "opus[1m]", 3);
        let added = lines.added.expect("a line of growth");
        assert_eq!((added.value, added.samples, added.scope), (400.0, 3, Scope::Cell));
        let cost = lines.cost.expect("a line of cost");
        assert_eq!(
            (cost.samples, cost.scope),
            (3, Scope::Flow),
            "two of its own on the model are too few"
        );
        let jump = lines.jump.expect("a line of the jump");
        assert_eq!(jump.value, 100.0);
        assert_eq!(lines.ratio.expect("a line of the jump per character").value, 2.0);
        // A cell with too few rounds of its own is judged by the flow's.
        let build = Lines::of(&history, &CellId::from("build"), "opus", 3);
        assert_eq!(build.added.expect("the flow's line").scope, Scope::Flow);
        assert_eq!(
            context_line(&history, "engineer", 3).map(|l| l.value),
            Some(2.0 * 10_000.0 - 1_250.0)
        );
        assert_eq!(context_line(&history, "reviewer", 1), None);
    }
}
