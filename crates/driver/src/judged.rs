//! How each round and each call of a tool went against the lines of their history (principle
//! `judged-by-own-history`): what an agent's round cost and how long it took, how long a round of an act of the
//! engine's own took, such as the check before the merge, which waits for the project's CI, and how long a call of a
//! tool took, such as the one that runs the project's fast tests. None of it stops anything, and none of it is stored:
//! it is worked out from the records whenever it is read (principle `state-from-records`), for the owner's page and
//! the command line to say what went past its line. What stops a round is the run guard's alone ([`crate::history`]).
//!
//! A thing is judged by the line its history gave as it began: of the other works on its flow, the rounds and calls
//! that had ended by then, and of its own work, those that ended before it in the work's order. So a round is judged by
//! the lines the run guard judged it by as it ran, and a judgment reads the same however the history has grown since;
//! what is under way is judged by how long it has taken so far.
//!
//! - An agent's round: its cost among the rounds on its model, and its time, each against its cell's rounds where at
//!   least as many of them as the settings say tell it, else against every agent's round of the flow, as the run
//!   guard's lines are; the rounds that are history are those the run guard's are. Its cost is judged once it ended,
//!   its program saying only then what it took.
//! - A round of an act: its time, against the rounds of its cell that handed something in and nothing else, the flow's
//!   other cells being agents' or other acts'. An act costs nothing.
//! - A call of a tool: its time, from when it was made until it had its result, against the calls of the same tool
//!   that had theirs.

use std::collections::BTreeMap;

use decide::{CallId, CellId, RunId, Seq};
use flow::Compiled;
use store::WorkNumber;
use work::{Ask, Back, Call, Time, Work};

use crate::history::{Line, Lines, Sample, Scope, judged_sorted, model_of};

/// A work whose rounds and calls are judged, or are history.
#[derive(Clone, Copy)]
pub struct Of<'a> {
    pub number: WorkNumber,
    /// The name of the flow it runs on: the works on flows of one name are one history.
    pub flow: &'a str,
    /// The flow as the work runs on it.
    pub compiled: &'a Compiled,
    pub work: &'a Work,
    /// Its rounds that are history, as the run guard takes them ([`crate::history::samples`]).
    pub samples: &'a [Sample],
}

/// A measure against the line of its history.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Against {
    /// The measure, in the line's units: millionths of a dollar for a cost, milliseconds for a time.
    pub value: f64,
    pub line: Line,
}

impl Against {
    /// Whether the measure went past its line.
    pub fn past(&self) -> bool {
        self.value > self.line.value
    }
}

/// A round against the lines of its history.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Round {
    /// What it cost; none while it is under way, for a round of an act, for one whose model was not recorded, and where
    /// too few rounds tell the line.
    pub cost: Option<Against>,
    /// How long it took, or has taken so far while it is under way; none where too few rounds tell the line.
    pub time: Option<Against>,
    /// For an agent's round under way, every line it is judged by as it runs, the run guard's among them, its cost's
    /// among the rounds on the model its try runs on; none for any other round.
    pub lines: Option<Lines>,
}

/// The rounds and the calls of tools of the works judged, each against the lines of its history.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Judged {
    /// Each round its history tells a line of, by its work, its run and its number in the run, the first being one.
    pub rounds: BTreeMap<(WorkNumber, RunId, usize), Round>,
    /// Each call of a tool its history tells a line of, by its work and its name.
    pub calls: BTreeMap<(WorkNumber, CallId), Against>,
}

/// Each round and each call of a tool of the works `picked` names, against the line its history gave as it began,
/// from every work on its flow: each line worked out from at least `least` values, and what is under way judged as
/// having taken until `now`.
pub fn judged(works: &[Of<'_>], picked: impl Fn(WorkNumber) -> bool, least: usize, now: Time) -> Judged {
    let mut out = Judged::default();
    let mut flows: BTreeMap<&str, Vec<&Of<'_>>> = BTreeMap::new();
    for of in works {
        flows.entry(of.flow).or_default().push(of);
    }
    for works in flows.values() {
        if !works.iter().any(|of| picked(of.number)) {
            continue;
        }
        let mut ended: Vec<Value<'_>> = works.iter().flat_map(|of| values(of)).collect();
        ended.sort_by_key(|v| (v.ended, v.work, v.at));
        let mut asked: Vec<Judging<'_>> = (works.iter())
            .filter(|of| picked(of.number))
            .flat_map(|of| judging(of, now))
            .collect();
        asked.sort_by_key(|a| (a.began, a.work, a.at));
        let mut history = History::default();
        let mut next = 0;
        for one in &asked {
            while ended.get(next).is_some_and(|v| v.ended < one.began) {
                history.add(&ended[next]);
                next += 1;
            }
            // What ended as it began: of another work, it had ended by then; of its own, only what ended before it in
            // the work's order, which leaves out the thing itself, however short.
            let ties: Vec<&Value<'_>> = (ended[next..].iter())
                .take_while(|v| v.ended == one.began)
                .filter(|v| v.work != one.work || v.at < one.at)
                .collect();
            for value in &ties {
                history.add(value);
            }
            history.judge(one, least, &mut out);
            for value in ties.iter().rev() {
                history.take(value);
            }
        }
    }
    out
}

/// A value of a history, of a round or a call that ended: when, where in its work's order, and what it is.
struct Value<'a> {
    ended: u64,
    work: WorkNumber,
    at: Seq,
    kind: Kind<'a>,
}

#[derive(Clone, Copy)]
enum Kind<'a> {
    /// An agent's round that is history.
    Agent(&'a Sample),
    /// A round of an act's cell that handed something in, with how long it took.
    Act(&'a CellId, f64),
    /// A call of a tool that had its result, with how long it took.
    Tool(&'a str, f64),
}

/// A round or a call to be judged: when it began, where in its work's order, and what it is.
struct Judging<'a> {
    began: u64,
    work: WorkNumber,
    at: Seq,
    what: What<'a>,
}

enum What<'a> {
    Round {
        run: RunId,
        number: usize,
        cell: &'a CellId,
        act: bool,
        /// The model its latest try ran on, where one was recorded.
        model: Option<&'a str>,
        /// What it cost, once it ended.
        cost: Option<f64>,
        /// How long it took, or has taken so far.
        time: f64,
        under_way: bool,
    },
    Call {
        call: CallId,
        tool: &'a str,
        time: f64,
    },
}

/// Each round of the work: its run, its number in the run, where it began, and where it ended, where it has.
fn rounds(work: &Work) -> Vec<(RunId, usize, Seq, Option<Seq>)> {
    let r = &work.records;
    let mut ends: BTreeMap<RunId, Vec<Seq>> = BTreeMap::new();
    for returned in &r.returned {
        ends.entry(returned.run).or_default().push(returned.at);
    }
    let mut counted: BTreeMap<RunId, usize> = BTreeMap::new();
    (r.rounds.iter())
        .map(|round| {
            let number = counted.entry(round.run).or_default();
            *number += 1;
            let ended = ends.get(&round.run).and_then(|ends| ends.get(*number - 1)).copied();
            (round.run, *number, round.at, ended)
        })
        .collect()
}

/// The cell of each run of the work, and whether an act of the engine's own carries it out, as the work's flow says.
fn cells<'a>(of: &Of<'a>) -> BTreeMap<RunId, (&'a CellId, bool)> {
    let (work, compiled) = (of.work, of.compiled);
    (work.records.started.iter())
        .filter_map(|started| {
            let spec = compiled.cells.iter().find(|spec| spec.id == started.cell)?;
            Some((started.run, (&spec.id, spec.act.is_some())))
        })
        .collect()
}

/// The tool a call runs, where it is a call of a tool.
fn tool(work: &Work, call: CallId) -> Option<&str> {
    match work.calls.get(&call) {
        Some(Call::Asked(Ask::Tool { name, .. })) => Some(name),
        _ => None,
    }
}

/// The work's values of the history: its agents' rounds that are history, the rounds of its acts that handed something
/// in, and its calls of tools that had their results, each where the records tell when.
fn values<'a>(of: &Of<'a>) -> Vec<Value<'a>> {
    let (work, samples) = (of.work, of.samples);
    let ms = |from: Seq, to: Seq| Some(work.time_of(to)?.0.saturating_sub(work.time_of(from)?.0) as f64);
    let mut out: Vec<Value<'a>> = (samples.iter())
        .filter_map(|sample| {
            Some(Value {
                ended: sample.ended?.0,
                work: of.number,
                at: sample.at,
                kind: Kind::Agent(sample),
            })
        })
        .collect();
    let cells = cells(of);
    for (run, number, began, ended) in rounds(work) {
        let (Some(&(cell, true)), Some(ended)) = (cells.get(&run), ended) else {
            continue;
        };
        let handed = matches!(
            work.back.get(&(run, number)),
            Some(Back::HandedIn(_) | Back::Returned { .. })
        );
        if let (true, Some(took), Some(at)) = (handed, ms(began, ended), work.time_of(ended)) {
            out.push(Value {
                ended: at.0,
                work: of.number,
                at: ended,
                kind: Kind::Act(cell, took),
            });
        }
    }
    for called in &work.records.called {
        let Some(name) = tool(work, called.call) else {
            continue;
        };
        let Some(done) = work.records.done.iter().find(|done| done.call == called.call) else {
            continue;
        };
        if let (Some(took), Some(at)) = (ms(called.at, done.at), work.time_of(done.at)) {
            out.push(Value {
                ended: at.0,
                work: of.number,
                at: done.at,
                kind: Kind::Tool(name, took),
            });
        }
    }
    out
}

/// The work's rounds and calls of tools to judge, each where the records tell when it began.
fn judging<'a>(of: &Of<'a>, now: Time) -> Vec<Judging<'a>> {
    let work = of.work;
    let cells = cells(of);
    let mut out = Vec::new();
    for (run, number, began_at, ended_at) in rounds(work) {
        let (Some(&(cell, act)), Some(began)) = (cells.get(&run), work.time_of(began_at)) else {
            continue;
        };
        let ended = ended_at.and_then(|at| work.time_of(at));
        let model = (work.ran.iter().rev())
            .find(|ran| ran.run == run && ran.round == number)
            .map(|ran| ran.on.model.as_str());
        let cost = (ended.and(work.spent.get(&(run, number)))).map(|spent| spent.usd_micros as f64);
        out.push(Judging {
            began: began.0,
            work: of.number,
            at: began_at,
            what: What::Round {
                run,
                number,
                cell,
                act,
                model,
                cost,
                time: ended.unwrap_or(now).0.saturating_sub(began.0) as f64,
                under_way: ended.is_none(),
            },
        });
    }
    for called in &work.records.called {
        let (Some(name), Some(made)) = (tool(work, called.call), work.time_of(called.at)) else {
            continue;
        };
        let done = (work.records.done.iter())
            .find(|done| done.call == called.call)
            .and_then(|done| work.time_of(done.at));
        out.push(Judging {
            began: made.0,
            work: of.number,
            at: called.at,
            what: What::Call {
                call: called.call,
                tool: name,
                time: done.unwrap_or(now).0.saturating_sub(made.0) as f64,
            },
        });
    }
    out
}

/// The history as of a moment: each kind of value apart, sorted, the smallest first.
#[derive(Default)]
struct History<'a> {
    /// The agents' rounds that are history, for the lines a round under way is judged by.
    agents: Vec<&'a Sample>,
    /// How long the agents' rounds took, by cell, and of every cell.
    time_of_cell: BTreeMap<&'a CellId, Vec<f64>>,
    time: Vec<f64>,
    /// What the agents' rounds cost, by cell and model, and by model of every cell.
    cost_of_cell: BTreeMap<(&'a CellId, &'a str), Vec<f64>>,
    cost: BTreeMap<&'a str, Vec<f64>>,
    /// How long the rounds of acts took, by cell.
    acts: BTreeMap<&'a CellId, Vec<f64>>,
    /// How long the calls of tools took, by tool.
    tools: BTreeMap<&'a str, Vec<f64>>,
}

impl<'a> History<'a> {
    /// The value joins the history.
    fn add(&mut self, value: &Value<'a>) {
        if let Kind::Agent(sample) = value.kind {
            self.agents.push(sample);
        }
        self.each(value, insert);
    }

    /// The value, which joined the history last of all of its kind, leaves it.
    fn take(&mut self, value: &Value<'a>) {
        if let Kind::Agent(sample) = value.kind
            && let Some(at) = self.agents.iter().rposition(|s| std::ptr::eq(*s, sample))
        {
            self.agents.remove(at);
        }
        self.each(value, remove);
    }

    /// Does this to each list the value counts in, with the value.
    fn each(&mut self, value: &Value<'a>, f: fn(&mut Vec<f64>, f64)) {
        match value.kind {
            Kind::Agent(sample) => {
                if let Some(ms) = sample.ms {
                    f(self.time_of_cell.entry(&sample.cell).or_default(), ms as f64);
                    f(&mut self.time, ms as f64);
                }
                if let Some(model) = sample.model.as_deref() {
                    let (model, usd) = (model_of(model), sample.spent.usd_micros as f64);
                    f(self.cost_of_cell.entry((&sample.cell, model)).or_default(), usd);
                    f(self.cost.entry(model).or_default(), usd);
                }
            }
            Kind::Act(cell, ms) => f(self.acts.entry(cell).or_default(), ms),
            Kind::Tool(tool, ms) => f(self.tools.entry(tool).or_default(), ms),
        }
    }

    /// Judges the round or the call against the history as it stands, into what was judged.
    fn judge(&self, one: &Judging<'a>, least: usize, out: &mut Judged) {
        let none: &[f64] = &[];
        fn of(list: Option<&Vec<f64>>) -> &[f64] {
            list.map(Vec::as_slice).unwrap_or_default()
        }
        match one.what {
            What::Round {
                run,
                number,
                cell,
                act: true,
                time,
                ..
            } => {
                if let Some(line) = judged_sorted(of(self.acts.get(cell)), none, least, Scope::Cell, Scope::Cell) {
                    let judged = Round {
                        time: Some(Against { value: time, line }),
                        ..Round::default()
                    };
                    out.rounds.insert((one.work, run, number), judged);
                }
            }
            What::Round {
                run,
                number,
                cell,
                act: false,
                model,
                cost,
                time,
                under_way,
            } => {
                let on = model.map(model_of);
                let cost_line = on.and_then(|on| {
                    let own = of(self.cost_of_cell.get(&(cell, on)));
                    judged_sorted(own, of(self.cost.get(on)), least, Scope::Cell, Scope::Flow)
                });
                let time_line = judged_sorted(
                    of(self.time_of_cell.get(cell)),
                    &self.time,
                    least,
                    Scope::Cell,
                    Scope::Flow,
                );
                let judged = Round {
                    cost: (cost.zip(cost_line)).map(|(value, line)| Against { value, line }),
                    time: time_line.map(|line| Against { value: time, line }),
                    lines: under_way.then(|| Lines::of_refs(&self.agents, cell, model.unwrap_or_default(), least)),
                };
                if judged != Round::default() {
                    out.rounds.insert((one.work, run, number), judged);
                }
            }
            What::Call { call, tool, time } => {
                if let Some(line) = judged_sorted(of(self.tools.get(tool)), none, least, Scope::Tool, Scope::Tool) {
                    out.calls.insert((one.work, call), Against { value: time, line });
                }
            }
        }
    }
}

/// Puts the value into the sorted list, after those as large.
fn insert(sorted: &mut Vec<f64>, value: f64) {
    let at = sorted.partition_point(|v| v.total_cmp(&value).is_le());
    sorted.insert(at, value);
}

/// Takes one of the value out of the sorted list, where it is there.
fn remove(sorted: &mut Vec<f64>, value: f64) {
    let at = sorted.partition_point(|v| v.total_cmp(&value).is_lt());
    if sorted.get(at).is_some_and(|v| v.total_cmp(&value).is_eq()) {
        sorted.remove(at);
    }
}
