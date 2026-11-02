//! The run guard: what a round's calls to the model come to while it runs, and whether that is past what its cell's
//! history explains. Nothing here is a number set by hand: a line is computed from the rounds kept in `round_measures`,
//! as the flow's rounds section says, once when a round starts; each call then costs a few additions and comparisons
//! ([`Run`]). The signals that stop a round take two conditions, so that a round that is long and a message that is
//! large, both normal, are told from growth nothing explains:
//!
//! - growth: the context grew by more than the line, and faster per call than the line;
//! - jump: the first call's prompt is past the session's last by more than the line, and by more for each character the
//!   engine gave than the line.
//!
//! Cost, calls, repeated tool uses and compactions past their lines are only recorded. Who runs the round, and what
//! happens to it, is the engine's; this module only judges and keeps what it judged.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use db::rusqlite::{self, Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use crate::choose;
use crate::cli::{Call, Seen};
use crate::cost;
use crate::round::{Round, RoundId};

/// How many of a round's last tool uses a stopped round's record keeps, for the round that runs after it.
const LAST_TOOLS: usize = 8;

/// The line of a distribution: past its largest value by as much as that is past its median, so that the line stands
/// as far out as the distribution itself reaches, in its own units. None for an empty one.
pub fn line(xs: &[f64]) -> Option<f64> {
    let largest = xs.iter().copied().reduce(f64::max)?;
    Some(2.0 * largest - median(xs)?)
}

/// The median of values, the mean of the middle two for an even number of them; none for no value.
pub fn median(xs: &[f64]) -> Option<f64> {
    let mut sorted = xs.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => Some(sorted[n / 2]),
        _ => Some((sorted[n / 2 - 1] + sorted[n / 2]) / 2.0),
    }
}

/// What a line was computed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// The cell's own rounds.
    Cell,
    /// Every round of the cell's graph.
    Graph,
}

impl Scope {
    pub fn name(self) -> &'static str {
        match self {
            Scope::Cell => "cell",
            Scope::Graph => "graph",
        }
    }
}

/// A line, with how many rounds gave it and from where.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line {
    pub value: f64,
    pub samples: i64,
    pub scope: Scope,
}

/// A round as `round_measures` keeps it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sample {
    pub cell: String,
    pub model: Option<String>,
    pub calls: i64,
    pub added: i64,
    pub jump: Option<i64>,
    pub given: Option<i64>,
    pub repeats: Option<i64>,
    pub compactions: i64,
    pub cost: Option<f64>,
    /// Kept call by call as it ran, not rebuilt from the first and last calls of its record.
    pub ledger: bool,
}

impl Sample {
    /// How much the context grew per call after the first.
    fn rate(&self) -> Option<f64> {
        (self.calls >= 2).then(|| self.added as f64 / (self.calls - 1) as f64)
    }

    /// How large the jump at its first call was, for each character the engine gave.
    fn ratio(&self) -> Option<f64> {
        match (self.jump, self.given) {
            (Some(j), Some(g)) if g > 0 => Some(j as f64 / g as f64),
            _ => None,
        }
    }
}

/// What of a kept round a line is computed from; none for a round that does not tell it.
type Measure = fn(&Sample) -> Option<f64>;

/// The line of a measure: from a cell's own rounds when they give one, else from those of its graph. A median of
/// nothing is usual among all a graph's rounds, so there only a line of nothing, from values that are all nothing,
/// is no line.
fn judged(own: &[&Sample], any: &[&Sample], least: usize, f: Measure) -> Option<Line> {
    let values = |of: &[&Sample]| -> Vec<f64> { of.iter().filter_map(|s| f(s)).collect() };
    let own = values(own);
    if own.len() >= least && median(&own).is_some_and(|m| m > 0.0) {
        return line(&own).map(|value| Line {
            value,
            samples: own.len() as i64,
            scope: Scope::Cell,
        });
    }
    let any = values(any);
    if any.len() < least {
        return None;
    }
    let value = line(&any).filter(|v| *v > 0.0)?;
    Some(Line {
        value,
        samples: any.len() as i64,
        scope: Scope::Graph,
    })
}

/// The lines a round is judged by, each none when too few rounds give it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lines {
    pub added: Option<Line>,
    pub rate: Option<Line>,
    pub jump: Option<Line>,
    pub ratio: Option<Line>,
    pub cost: Option<Line>,
    pub calls: Option<Line>,
    pub repeats: Option<Line>,
    pub compactions: Option<Line>,
}

impl Lines {
    /// The lines of a round of the cell `cell` on `model`, from every round of the cell's graph: a cell's own, from at
    /// least `least` of its rounds kept call by call and a median above nothing, since a distribution with no scale
    /// gives no line; else the graph's, from at least `least` rounds, those from before calls were kept included, so
    /// that a new cell is judged from its first day. What a round cost is judged among rounds on the same model. A
    /// cell is every id it has had.
    pub fn of<'a>(all: &'a [Sample], graph: &str, cell: &str, model: &str, least: usize) -> Lines {
        let code = db::code_of(graph, cell).unwrap_or(cell);
        let every: Vec<&Sample> = all.iter().collect();
        let mine: Vec<&Sample> = all
            .iter()
            .filter(|s| s.ledger && db::is_cell(graph, code, &s.cell))
            .collect();
        let on_model = |of: &[&'a Sample]| -> Vec<&'a Sample> {
            let same = |s: &&&Sample| s.model.as_deref().is_some_and(|m| same_model(m, model));
            of.iter().filter(same).copied().collect()
        };
        let whole = |f: Measure| judged(&mine, &every, least, f);
        Lines {
            added: whole(|s| Some(s.added as f64)),
            rate: whole(Sample::rate),
            jump: whole(|s| s.jump.map(|j| j as f64)),
            ratio: whole(Sample::ratio),
            cost: judged(&on_model(&mine), &on_model(&every), least, |s| s.cost),
            calls: whole(|s| Some(s.calls as f64)),
            repeats: whole(|s| s.repeats.map(|r| r as f64)),
            compactions: whole(|s| Some(s.compactions as f64)),
        }
    }

    /// The lines as a round's record keeps them, which tells afterwards what the round was judged by.
    pub fn record(&self) -> Value {
        let one = |l: &Option<Line>| {
            l.map_or(
                Value::Null,
                |l| json!({ "value": l.value, "samples": l.samples, "scope": l.scope.name() }),
            )
        };
        json!({
            "added": one(&self.added),
            "rate": one(&self.rate),
            "jump": one(&self.jump),
            "ratio": one(&self.ratio),
            "cost": one(&self.cost),
            "calls": one(&self.calls),
            "repeats": one(&self.repeats),
            "compactions": one(&self.compactions),
        })
    }
}

/// Whether two names are of one model: a model named with its context window, as `[1m]`, is the model.
fn same_model(a: &str, b: &str) -> bool {
    a.split('[').next() == b.split('[').next()
}

/// A signal past its line, as `guard_events` names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Signal {
    Growth,
    Jump,
    Cost,
    Calls,
    Repeats,
    Compactions,
}

impl Signal {
    pub fn name(self) -> &'static str {
        match self {
            Signal::Growth => "growth",
            Signal::Jump => "jump",
            Signal::Cost => "cost",
            Signal::Calls => "calls",
            Signal::Repeats => "repeats",
            Signal::Compactions => "compactions",
        }
    }
}

/// What came of a signal past its line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Recorded, and the round goes on.
    Noted,
    /// The round is stopped.
    Stopped,
    /// It would have been stopped, but the owner let the work go on after the cell's last round was stopped.
    Waived,
}

impl Action {
    pub fn name(self) -> &'static str {
        match self {
            Action::Noted => "noted",
            Action::Stopped => "stopped",
            Action::Waived => "waived",
        }
    }
}

/// A signal past its line, each once in a round.
#[derive(Debug, Clone, PartialEq)]
pub struct Crossing {
    pub signal: Signal,
    pub value: f64,
    pub line: f64,
    /// A signal of two conditions: its second value and that value's line.
    pub also: Option<(f64, f64)>,
    pub samples: i64,
    pub scope: Scope,
    pub action: Action,
}

impl Crossing {
    /// What happened, in a sentence for the owner and the round's record.
    pub fn words(&self) -> String {
        let (v, l) = (self.value, self.line);
        let two = self.also.unwrap_or_default();
        match self.signal {
            Signal::Growth => format!(
                "the context grew by {v:.0} tokens in the round, past the line of {l:.0}, and by {:.0} a call, past the \
                 line of {:.0}",
                two.0, two.1
            ),
            Signal::Jump => format!(
                "the context jumped by {v:.0} tokens at the round's first call, past the line of {l:.0}, and by {:.1} for \
                 each character the engine gave, past the line of {:.1}",
                two.0, two.1
            ),
            Signal::Cost => format!("the round cost ${v:.2}, past the line of ${l:.2}"),
            Signal::Calls => format!("the round made {v:.0} calls, past the line of {l:.0}"),
            Signal::Repeats => format!("the round repeated {v:.0} tool uses, past the line of {l:.0}"),
            Signal::Compactions => format!("the session was compacted {v:.0} times, past the line of {l:.0}"),
        }
    }
}

/// What a round's calls come to, however many lines of the stream told them: the calls of the session's own line, the
/// context at the first and the last, the tokens the context grew by, each rise between neighbouring calls counted so
/// that a compaction does not take it back, and what all of them cost, subagents' calls included, which are not in the
/// session's context.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Totals {
    told: BTreeMap<String, Call>,
    pub calls: i64,
    pub context: Option<i64>,
    pub added: i64,
    pub cost: f64,
}

impl Totals {
    /// Takes a call, new or told before with more output, of a round on `model`; whether it is a new call of the
    /// session's own.
    pub fn take(&mut self, c: &Call, model: &str) -> bool {
        let price = |x: &Call| cost::of_call(x.model.as_deref().unwrap_or(model), x).unwrap_or(0.0);
        let was = self.told.insert(c.id.clone(), c.clone());
        self.cost += price(c) - was.as_ref().map_or(0.0, price);
        if was.is_some() || c.subagent {
            return false;
        }
        self.calls += 1;
        let prompt = c.prompt();
        if let Some(last) = self.context {
            self.added += (prompt - last).max(0);
        }
        self.context = Some(prompt);
        true
    }

    /// How much the context grew per call after the first; none for a single call.
    pub fn rate(&self) -> Option<f64> {
        (self.calls >= 2).then(|| self.added as f64 / (self.calls - 1) as f64)
    }

    /// A usage record of what the calls come to, to stand for the one the command line gives at a run's end, which a
    /// stopped run never reaches: `usage` as the record of a whole run has it, marked as computed here. What the record
    /// holds so far, such as the first call, stays.
    pub fn usage(&self, mut record: Value) -> Value {
        let sum = |f: &dyn Fn(&Call) -> i64| self.told.values().map(f).sum::<i64>();
        let (written_5m, written_1h) = (sum(&|c| c.written_5m), sum(&|c| c.written_1h));
        if !record.is_object() {
            record = json!({ "started": true });
        }
        record["usage"] = json!({
            "input_tokens": sum(&|c| c.input),
            "cache_read_input_tokens": sum(&|c| c.read),
            "cache_creation_input_tokens": written_5m + written_1h,
            "cache_creation": {
                "ephemeral_5m_input_tokens": written_5m,
                "ephemeral_1h_input_tokens": written_1h,
            },
            "output_tokens": sum(&|c| c.output),
        });
        record["turns"] = json!(self.calls);
        record["cost_usd"] = json!(self.cost);
        record["estimated"] = json!(true);
        record
    }
}

/// How a round stands now, for the owner's page.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Live {
    pub context: i64,
    pub added: i64,
    pub cost_usd: f64,
}

/// A running round as the guard sees it.
#[derive(Debug, Clone)]
pub struct Run {
    lines: Lines,
    /// What comes of a signal past its line that stops a round.
    past: Action,
    model: String,
    /// The characters the engine gave the round: its message, and the return before it in its session.
    given: i64,
    /// The session's last prompt before this round; none for a new session.
    before: Option<i64>,
    totals: Totals,
    jump: Option<i64>,
    tools: VecDeque<(String, Option<String>)>,
    uses: BTreeSet<(String, u64)>,
    repeats: i64,
    compactions: i64,
    crossed: BTreeSet<Signal>,
    stopped: Option<Crossing>,
}

impl Run {
    pub fn new(lines: Lines, past: Action, model: &str, given: i64, before: Option<i64>) -> Run {
        Run {
            lines,
            past,
            model: model.to_string(),
            given,
            before,
            totals: Totals::default(),
            jump: None,
            tools: VecDeque::new(),
            uses: BTreeSet::new(),
            repeats: 0,
            compactions: 0,
            crossed: BTreeSet::new(),
            stopped: None,
        }
    }

    pub fn lines(&self) -> &Lines {
        &self.lines
    }

    pub fn totals(&self) -> &Totals {
        &self.totals
    }

    pub fn live(&self) -> Live {
        Live {
            context: self.totals.context.unwrap_or(0),
            added: self.totals.added,
            cost_usd: self.totals.cost,
        }
    }

    /// Takes what the run did; the signals it put past their lines, each the first time.
    pub fn see(&mut self, seen: &Seen) -> Vec<Crossing> {
        match seen {
            Seen::Doing(_) => Vec::new(),
            Seen::Call(c) => self.call(c),
            Seen::Tool { name, on, digest } => {
                self.tools.push_back((name.clone(), on.clone()));
                if self.tools.len() > LAST_TOOLS {
                    self.tools.pop_front();
                }
                if !self.uses.insert((name.clone(), *digest)) {
                    self.repeats += 1;
                }
                let line = self.lines.repeats;
                self.noted(Signal::Repeats, self.repeats as f64, line)
                    .into_iter()
                    .collect()
            }
            Seen::Compacted => {
                self.compactions += 1;
                let line = self.lines.compactions;
                self.noted(Signal::Compactions, self.compactions as f64, line)
                    .into_iter()
                    .collect()
            }
        }
    }

    fn call(&mut self, c: &Call) -> Vec<Crossing> {
        let new = self.totals.take(c, &self.model);
        let mut out = Vec::new();
        if new {
            let added = self.totals.added as f64;
            if let (Some(rate), Some(a), Some(r)) = (self.totals.rate(), self.lines.added, self.lines.rate)
                && added > a.value
                && rate > r.value
            {
                out.extend(self.past(Signal::Growth, added, a, (rate, r.value)));
            }
            if self.totals.calls == 1 {
                self.jump = self.before.map(|b| (c.prompt() - b).max(0));
                let ratio = (self.given > 0).then(|| self.jump.unwrap_or(0) as f64 / self.given as f64);
                if let (Some(jump), Some(ratio), Some(j), Some(r)) =
                    (self.jump, ratio, self.lines.jump, self.lines.ratio)
                    && jump as f64 > j.value
                    && ratio > r.value
                {
                    out.extend(self.past(Signal::Jump, jump as f64, j, (ratio, r.value)));
                }
            }
            let (calls, line) = (self.totals.calls as f64, self.lines.calls);
            out.extend(self.noted(Signal::Calls, calls, line));
        }
        let (spent, line) = (self.totals.cost, self.lines.cost);
        out.extend(self.noted(Signal::Cost, spent, line));
        out
    }

    /// The signal that stops a round, the first time it is past.
    fn past(&mut self, signal: Signal, value: f64, l: Line, also: (f64, f64)) -> Option<Crossing> {
        if !self.crossed.insert(signal) {
            return None;
        }
        let c = Crossing {
            signal,
            value,
            line: l.value,
            also: Some(also),
            samples: l.samples,
            scope: l.scope,
            action: self.past,
        };
        if c.action == Action::Stopped {
            self.stopped.get_or_insert_with(|| c.clone());
        }
        Some(c)
    }

    /// A signal only recorded, the first time it is past its line.
    fn noted(&mut self, signal: Signal, value: f64, line: Option<Line>) -> Option<Crossing> {
        let l = line.filter(|l| value > l.value)?;
        self.crossed.insert(signal).then_some(Crossing {
            signal,
            value,
            line: l.value,
            also: None,
            samples: l.samples,
            scope: l.scope,
            action: Action::Noted,
        })
    }

    /// Why the round was stopped, as its record keeps it for the round that runs after it: the signal, its value, its
    /// line, how many rounds gave the line and from where, and the last tools the round used. None for a round not
    /// stopped.
    pub fn stop_record(&self) -> Option<Value> {
        let c = self.stopped.as_ref()?;
        let tools: Vec<Value> = self
            .tools
            .iter()
            .map(|(name, on)| json!({ "tool": name, "on": on }))
            .collect();
        let mut record = json!({
            "signal": c.signal.name(),
            "says": c.words(),
            "value": c.value,
            "line": c.line,
            "samples": c.samples,
            "scope": c.scope.name(),
            "last_tools": tools,
        });
        if let Some((value, line)) = c.also {
            record["also"] = json!({ "value": value, "line": line });
        }
        Some(record)
    }

    /// The row `round_measures` keeps of the run, once it ended; none for a run that made no call.
    pub fn sample(&self, cell: &str, model: Option<&str>, cost: Option<f64>) -> Option<Sample> {
        (self.totals.calls > 0).then(|| Sample {
            cell: cell.to_string(),
            model: Some(model.unwrap_or(&self.model).to_string()),
            calls: self.totals.calls,
            added: self.totals.added,
            jump: self.jump,
            given: Some(self.given),
            repeats: Some(self.repeats),
            compactions: self.compactions,
            cost: cost.or(Some(self.totals.cost)),
            ledger: true,
        })
    }
}

/// The session's last prompt before a round, from its rounds, oldest first: its last call kept, or, for rounds from
/// before calls were kept, the last prompt a round's record tells. A failed round's calls count, so that what it wrote
/// in the session counts as the session's when kept; none for a session that has made no call.
pub fn last_prompt(conn: &Connection, rounds: &[Round]) -> Result<Option<i64>, String> {
    for r in rounds.iter().rev() {
        let kept: Option<i64> = conn
            .query_row(
                "SELECT input + read + written_5m + written_1h FROM round_calls
                 WHERE work = ?1 AND round = ?2 AND subagent = 0 ORDER BY seq DESC LIMIT 1",
                params![r.id.work, r.id.n],
                |x| x.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if kept.is_some() {
            return Ok(kept);
        }
    }
    Ok(choose::context(rounds))
}

/// The characters the engine gave a round in its session: its message, and the return before it, which the session
/// holds only since then.
pub fn given(message: &str, rounds: &[Round]) -> i64 {
    let before = rounds.iter().rev().find_map(|r| r.output.as_ref());
    (message.chars().count() + before.map_or(0, |o| o.to_string().chars().count())) as i64
}

/// Every round of a graph as `round_measures` keeps it, but those a stopped round leaves out, which never enter it.
pub fn samples(conn: &Connection, graph: &str) -> Result<Vec<Sample>, String> {
    let mut st = conn
        .prepare(
            "SELECT cell, model, calls, added, jump, given, repeats, compactions, cost_usd, ledger
             FROM round_measures WHERE graph = ?1",
        )
        .map_err(|e| e.to_string())?;
    st.query_map([graph], |r| {
        Ok(Sample {
            cell: r.get(0)?,
            model: r.get(1)?,
            calls: r.get(2)?,
            added: r.get(3)?,
            jump: r.get(4)?,
            given: r.get(5)?,
            repeats: r.get(6)?,
            compactions: r.get(7)?,
            cost: r.get(8)?,
            ledger: r.get::<_, i64>(9)? != 0,
        })
    })
    .map_err(|e| e.to_string())?
    .collect::<rusqlite::Result<_>>()
    .map_err(|e| e.to_string())
}

/// Keeps a call in the ledger: one row for a call however many times it is told, its output the most told.
pub fn keep_call(tx: &Transaction<'_>, round: RoundId, c: &Call, at: i64) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO round_calls (work, round, seq, message, model, input, read, written_5m, written_1h, output,
                                  subagent, at)
         VALUES (?1, ?2, (SELECT coalesce(max(seq), 0) + 1 FROM round_calls WHERE work = ?1 AND round = ?2), ?3, ?4, ?5,
                 ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT (work, round, message) DO UPDATE SET output = max(output, excluded.output)",
        params![
            round.work,
            round.n,
            c.id,
            c.model,
            c.input,
            c.read,
            c.written_5m,
            c.written_1h,
            c.output,
            i64::from(c.subagent),
            at
        ],
    )?;
    Ok(())
}

/// Keeps a signal past its line.
pub fn keep_crossing(tx: &Transaction<'_>, round: RoundId, c: &Crossing, at: i64) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO guard_events (work, round, signal, value, line, also, also_line, samples, scope, action, at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            round.work,
            round.n,
            c.signal.name(),
            c.value,
            c.line,
            c.also.map(|a| a.0),
            c.also.map(|a| a.1),
            c.samples,
            c.scope.name(),
            c.action.name(),
            at
        ],
    )?;
    Ok(())
}

/// Keeps what a round that ran came to, for the rounds judged after it.
pub fn keep_sample(tx: &Transaction<'_>, round: RoundId, graph: &str, s: &Sample) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO round_measures (work, round, graph, cell, model, calls, added, jump, given, repeats, compactions,
                                     cost_usd, ledger)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            round.work,
            round.n,
            graph,
            s.cell,
            s.model,
            s.calls,
            s.added,
            s.jump,
            s.given,
            s.repeats,
            s.compactions,
            s.cost,
            i64::from(s.ledger)
        ],
    )?;
    Ok(())
}

/// A running round's calls kept so far, oldest first, for the owner's page.
pub fn calls_of(conn: &Connection, round: RoundId) -> Result<Vec<Call>, String> {
    let mut st = conn
        .prepare(
            "SELECT message, model, input, read, written_5m, written_1h, output, subagent FROM round_calls
             WHERE work = ?1 AND round = ?2 ORDER BY seq",
        )
        .map_err(|e| e.to_string())?;
    st.query_map(params![round.work, round.n], |r| {
        Ok(Call {
            id: r.get(0)?,
            model: r.get(1)?,
            input: r.get(2)?,
            read: r.get(3)?,
            written_5m: r.get(4)?,
            written_1h: r.get(5)?,
            output: r.get(6)?,
            subagent: r.get::<_, i64>(7)? != 0,
        })
    })
    .map_err(|e| e.to_string())?
    .collect::<rusqlite::Result<_>>()
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(cell: &str, ledger: bool, added: i64, calls: i64) -> Sample {
        Sample {
            cell: cell.into(),
            model: Some("claude-opus-5-5".into()),
            calls,
            added,
            jump: Some(added / 10),
            given: Some(1000),
            repeats: ledger.then_some(1),
            compactions: 0,
            cost: Some(added as f64 / 1000.0),
            ledger,
        }
    }

    fn call(id: &str, prompt: i64) -> Seen {
        Seen::Call(Call {
            id: id.into(),
            read: prompt,
            ..Call::default()
        })
    }

    const OPUS: &str = "claude-opus-5-5";

    // A line stands past the largest value by as much as that is past the median.
    #[test]
    fn a_line_is_past_the_largest_value_by_its_distance_from_the_median() {
        assert_eq!(line(&[1.0, 2.0, 3.0, 4.0, 10.0]), Some(17.0));
        assert_eq!(line(&[1.0, 2.0, 3.0, 10.0]), Some(17.5));
        assert_eq!(line(&[]), None);
    }

    // Eighteen rounds of a cell give no line of its own, nineteen do; below that the graph's rounds do, those from
    // before calls were kept included; with fewer than nineteen there too, there is no line.
    #[test]
    fn a_cell_needs_nineteen_kept_rounds_and_a_graph_nineteen_of_any() {
        let own = |n: i64| (1..=n).map(|i| sample("I1", true, i * 100, 11)).collect::<Vec<_>>();
        let old = |n: i64| (1..=n).map(|i| sample("R1", false, i * 1000, 11)).collect::<Vec<_>>();
        let lines = |rows: Vec<Sample>| Lines::of(&rows, "engineer", "I1", OPUS, 19);
        let added = lines(own(19)).added.unwrap();
        assert_eq!((added.scope, added.samples, added.value), (Scope::Cell, 19, 2800.0));
        let added = lines([own(18), old(19)].concat()).added.unwrap();
        assert_eq!((added.scope, added.samples), (Scope::Graph, 37));
        let rows = [own(9), old(9)].concat();
        assert_eq!(lines(rows).added, None);
        assert_eq!(lines(own(18)).added, None);
    }

    // A cell whose rounds add nothing gives no scale: the graph's line stands instead.
    #[test]
    fn a_cell_with_a_median_of_nothing_is_judged_by_its_graph() {
        let mut rows: Vec<Sample> = (0..19).map(|_| sample("I1", true, 0, 2)).collect();
        rows.extend((1..=19).map(|i| sample("R1", false, i * 1000, 11)));
        let added = Lines::of(&rows, "engineer", "I1", OPUS, 19).added.unwrap();
        assert_eq!(added.scope, Scope::Graph);
    }

    // Most rounds never compact, so a graph's median of compactions is nothing: that one round in nineteen compacted
    // still gives the signal its line. Values that are all nothing give none, which is what the first jump needs.
    #[test]
    fn a_graph_gives_a_line_from_a_median_of_nothing_but_not_from_nothing_at_all() {
        let rows: Vec<Sample> = (1..=19)
            .map(|i| Sample {
                jump: Some(0),
                compactions: if i == 19 { 2 } else { 0 },
                ..sample("I1", true, i * 100, 11)
            })
            .collect();
        let lines = Lines::of(&rows, "engineer", "I1", OPUS, 19);
        let compactions = lines.compactions.unwrap();
        assert_eq!((compactions.scope, compactions.value), (Scope::Graph, 4.0));
        assert_eq!(lines.jump, None);
    }

    // The ids a cell had in earlier flow versions are the cell; what a round cost is judged among its own model only.
    #[test]
    fn a_cell_is_all_its_ids_and_cost_is_judged_by_model() {
        let mut rows: Vec<Sample> = (1..=10).map(|i| sample("I1", true, i * 100, 11)).collect();
        rows.extend((11..=19).map(|i| sample("implement", true, i * 100, 11)));
        let lines = Lines::of(&rows, "engineer", "I1", OPUS, 19);
        assert_eq!(lines.added.unwrap().scope, Scope::Cell);
        assert_eq!(lines.cost.unwrap().samples, 19);
        assert_eq!(Lines::of(&rows, "engineer", "I1", "claude-haiku-5-5", 19).cost, None);
        let named = Lines::of(&rows, "engineer", "I1", "claude-opus-5-5[1m]", 19);
        assert_eq!(named.cost.unwrap().samples, 19);
    }

    fn lines() -> Lines {
        let at = |value: f64| {
            Some(Line {
                value,
                samples: 19,
                scope: Scope::Cell,
            })
        };
        Lines {
            added: at(1000.0),
            rate: at(100.0),
            jump: at(5000.0),
            ratio: at(2.0),
            cost: at(1.0),
            calls: at(50.0),
            repeats: at(2.0),
            compactions: at(1.0),
        }
    }

    fn run() -> Run {
        Run::new(lines(), Action::Stopped, OPUS, 1000, Some(10_000))
    }

    // Growth past both lines stops the round; a compaction in between takes nothing back.
    #[test]
    fn growth_past_both_lines_stops_the_round_across_a_compaction() {
        let mut r = run();
        assert!(r.see(&call("a", 10_000)).is_empty());
        assert!(r.see(&call("b", 10_600)).is_empty());
        assert!(r.see(&call("c", 2_000)).is_empty());
        let past = r.see(&call("d", 2_600));
        assert_eq!(past.len(), 1, "{past:?}");
        assert_eq!(past[0].signal, Signal::Growth);
        assert_eq!(past[0].action, Action::Stopped);
        assert_eq!(past[0].value, 1200.0);
        assert_eq!(past[0].also, Some((400.0, 100.0)));
        assert_eq!(r.stop_record().unwrap()["signal"], "growth");
    }

    // What a stopped round's record keeps for the round after it: why, and its last tools, the oldest dropped.
    #[test]
    fn a_stopped_rounds_record_keeps_its_last_tools() {
        let mut r = run();
        for i in 0..(LAST_TOOLS as u64 + 2) {
            r.see(&Seen::Tool {
                name: format!("t{i}"),
                on: Some("x".into()),
                digest: i,
            });
        }
        r.see(&call("a", 10_000));
        r.see(&call("b", 12_000));
        let record = r.stop_record().unwrap();
        let tools = record["last_tools"].as_array().unwrap();
        assert_eq!(tools.len(), LAST_TOOLS);
        assert_eq!(tools[0], json!({ "tool": "t2", "on": "x" }));
        assert_eq!(record["also"], json!({ "value": 2000.0, "line": 100.0 }));
    }

    // A round that adds much but no faster than its cell's rounds is a long round: it is not stopped.
    #[test]
    fn growth_past_one_line_only_does_not_stop_the_round() {
        let mut r = run();
        let mut prompt = 10_000;
        for i in 0..40 {
            let past = r.see(&call(&format!("c{i}"), prompt));
            assert!(past.iter().all(|c| c.signal != Signal::Growth), "{past:?}");
            prompt += 90;
        }
        assert!(r.totals().added > 1000);
        assert!(r.stop_record().is_none());
    }

    // A jump needs both its lines, and only the first call has one.
    #[test]
    fn a_jump_past_both_lines_stops_the_round_at_its_first_call() {
        let mut r = run();
        let past = r.see(&call("a", 20_000));
        assert_eq!(past[0].signal, Signal::Jump);
        assert_eq!(past[0].also, Some((10.0, 2.0)));
        // As large a jump, with as much given to explain it, is a large message.
        let mut large = Run::new(lines(), Action::Stopped, OPUS, 100_000, Some(10_000));
        assert!(large.see(&call("a", 20_000)).is_empty());
        // A new session has no jump.
        let mut new = Run::new(lines(), Action::Stopped, OPUS, 1000, None);
        assert!(new.see(&call("a", 20_000)).is_empty());
    }

    // The same call told again counts once, however its output grows; a subagent's call is not in the context.
    #[test]
    fn a_call_counts_once_and_a_subagents_not_in_the_context() {
        let mut r = run();
        let c = Call {
            id: "a".into(),
            read: 10_000,
            output: 1000,
            ..Call::default()
        };
        r.see(&Seen::Call(c.clone()));
        r.see(&Seen::Call(Call {
            output: 2000,
            ..c.clone()
        }));
        assert_eq!(r.totals().calls, 1);
        // 10,000 tokens read at $0.20 a million, and 2,000 written at $20 a million.
        assert!((r.totals().cost - 0.042).abs() < 1e-9, "{}", r.totals().cost);
        r.see(&Seen::Call(Call {
            id: "b".into(),
            read: 900_000,
            subagent: true,
            ..Call::default()
        }));
        assert_eq!((r.totals().calls, r.totals().added), (1, 0));
    }

    // Cost, calls, repeats and compactions past their lines are noted once each, and stop nothing.
    #[test]
    fn the_signals_only_noted_are_noted_once_and_stop_nothing() {
        let mut r = run();
        let use_of = |d: u64| Seen::Tool {
            name: "Bash".into(),
            on: None,
            digest: d,
        };
        let mut signals = Vec::new();
        for _ in 0..5 {
            signals.extend(r.see(&use_of(7)));
        }
        for _ in 0..3 {
            signals.extend(r.see(&Seen::Compacted));
        }
        assert_eq!(
            signals
                .iter()
                .map(|c| (c.signal, c.value, c.action))
                .collect::<Vec<_>>(),
            [
                (Signal::Repeats, 3.0, Action::Noted),
                (Signal::Compactions, 2.0, Action::Noted)
            ]
        );
        assert!(r.stop_record().is_none());
    }

    // A round the owner let go on is judged and recorded as waived, and nothing stops it.
    #[test]
    fn a_waived_round_is_recorded_and_not_stopped() {
        let mut r = Run::new(lines(), Action::Waived, OPUS, 1000, Some(10_000));
        let past = r.see(&call("a", 20_000));
        assert_eq!(past[0].action, Action::Waived);
        assert!(r.stop_record().is_none());
    }

    // The longest round that ever ran, work 38's round 22, took 237 calls at some 1,173 tokens a call and was right to:
    // past any line of tokens added its cell's rounds give, it is at their pace, and is not stopped.
    #[test]
    fn a_round_of_the_shape_of_the_longest_that_ran_is_not_stopped() {
        let rows: Vec<Sample> = (1..=30).map(|i| sample("I1", false, i * 3000, 31)).collect();
        let lines = Lines::of(&rows, "engineer", "I1", OPUS, 19);
        assert_eq!(lines.added.unwrap().scope, Scope::Graph);
        let mut r = Run::new(lines, Action::Stopped, OPUS, 1000, None);
        for i in 0..237 {
            let past = r.see(&call(&format!("c{i}"), 100_000 + i * 1173));
            assert!(past.iter().all(|c| c.action == Action::Noted), "{past:?}");
        }
        assert!(r.totals().added as f64 > r.lines().added.unwrap().value);
        assert!(r.stop_record().is_none());
    }

    // A round with one call has no rate, and nothing divides by nothing.
    #[test]
    fn a_single_call_has_no_rate_and_nothing_given_no_ratio() {
        let mut r = Run::new(lines(), Action::Stopped, OPUS, 0, Some(10_000));
        assert!(r.see(&call("a", 900_000)).is_empty());
        assert_eq!(r.totals().rate(), None);
    }

    // A stopped run's record of what it used stands for the one the command line would have given.
    #[test]
    fn a_stopped_runs_usage_is_computed_from_its_calls_and_marked() {
        let mut r = run();
        r.see(&Seen::Call(Call {
            id: "a".into(),
            model: Some(OPUS.into()),
            read: 1_000_000,
            written_1h: 1000,
            output: 1000,
            ..Call::default()
        }));
        let u = r
            .totals()
            .usage(json!({ "started": true, "first_call": { "input_tokens": 2 } }));
        assert_eq!(u["estimated"], true);
        assert_eq!(u["first_call"]["input_tokens"], 2);
        assert_eq!(u["usage"]["cache_read_input_tokens"], 1_000_000);
        let cost = u["cost_usd"].as_f64().unwrap();
        assert!((cost - (0.2 + 0.008 + 0.02)).abs() < 1e-9, "{cost}");
    }
}
