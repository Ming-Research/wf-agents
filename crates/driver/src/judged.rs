//! How each round and each call of a tool went against the lines of their history (principle
//! `judged-by-own-history`): what an agent's round cost and how long it took, how long a round of an act of the
//! engine's own took, such as the check before the merge, which waits for the project's CI, and how long a call of a
//! tool took and how large its result was, such as the one that runs the project's fast tests. None of it stops
//! anything, and none of it is stored: what each work adds is worked out from its records each time the work grows,
//! and the driver keeps it with the work ([`Part`]); the judgments are worked out from what the works keep whenever
//! they are read (principle `state-from-records`), for the owner's page and the command line to say what went past its
//! line. What stops a round is the run guard's alone ([`crate::history`]).
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
//! - A call of a tool: its time, from when it was made until it had its result, and the size of its result in bytes,
//!   each against the calls of the same tool that had theirs. A call withdrawn, its run abandoned or its work's item
//!   cancelled before it had its result, is neither judged nor history: the withdrawal cut it short, not its tool.
//!
//! The judgments go through a flow's history once, in the order things ended, and each list of the history holds the
//! values it can come to hold as counts ([`Ordered`]), so that a value joins or leaves it, and its median and largest
//! are read, at a cost that grows with the logarithm of its length, not with its length.

use std::collections::BTreeMap;

use decide::{CallId, CellId, Reading, RunId, Seq};
use flow::Compiled;
use store::WorkNumber;
use work::{Ask, Back, Call, Time, Work};

use crate::history::{Line, Sample, Scope, Sorted, judged_sorted, median_of_sorted, model_of};

#[cfg(test)]
thread_local! {
    /// How many times what a work adds to the judgments was worked out on this thread ([`Part::of`]): what a test of
    /// what a read of the judgments costs reads.
    pub(crate) static WORKED_OUT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A work whose rounds and calls are judged, or are history: what it adds, as the driver keeps it.
#[derive(Clone, Copy)]
pub struct Of<'a> {
    pub number: WorkNumber,
    /// The name of the flow it runs on: the works on flows of one name are one history.
    pub flow: &'a str,
    pub part: &'a Part,
}

/// What a work adds to the judgments of its flow, worked out from its records alone: its values of the history, and
/// its rounds and calls of tools to judge, each where the records tell when. What is under way is judged by how long it
/// has taken as of the time the judgments are read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Part {
    values: Vec<Value>,
    judging: Vec<Judging>,
}

impl Part {
    /// What the work of this number adds: each cell's id, and whether an act of the engine's own carries it out, as the
    /// flow installed of its flow's name says, read through `reading` for a work on an earlier version, and none for a
    /// work on no version kept, whose cells no flow tells (decision `a-work-kept-before-the-versions-is-on-none`); its
    /// rounds that are history being `samples`, as the run guard takes them ([`crate::history::samples`]).
    pub fn of(
        number: WorkNumber,
        compiled: Option<&Compiled>,
        reading: Option<&Reading>,
        work: &Work,
        samples: &[Sample],
    ) -> Part {
        #[cfg(test)]
        WORKED_OUT.with(|worked_out| worked_out.set(worked_out.get() + 1));
        let cells = cells(compiled, reading, work);
        Part {
            values: values(number, work, samples, &cells),
            judging: judging(number, work, &cells),
        }
    }
}

/// A measure against the line of its history.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Against {
    /// The measure, in the line's units: millionths of a dollar for a cost, milliseconds for a time, bytes for a size.
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
    /// For a round under way, how much longer it is expected to take ([`rest`]); none for a round that ended.
    pub rest: Option<Rest>,
}

/// How much longer a round under way is expected to take, by the rounds of its history that took longer than it has so
/// far: the median of what each of them had left at that point; or that it has taken longer than every one of them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rest {
    /// About this many milliseconds more.
    Expected(f64),
    /// It has taken longer than any round of its history.
    Longer,
}

/// The values of a list longer than a time, each as what it had left past that time.
struct Left<'s> {
    sorted: &'s Ordered,
    /// How many values of the list are no longer than the time.
    from: usize,
    so_far: f64,
}

impl Left<'_> {
    fn of(sorted: &Ordered, so_far: f64) -> Left<'_> {
        Left {
            sorted,
            from: sorted.at_most(so_far),
            so_far,
        }
    }
}

impl Sorted for Left<'_> {
    fn count(&self) -> usize {
        self.sorted.count() - self.from
    }

    fn nth(&self, place: usize) -> f64 {
        self.sorted.nth(self.from + place) - self.so_far
    }
}

/// The rest of a round under way that has taken `so_far` milliseconds: from the times of its cell's rounds, `own`, where
/// at least `least` of them took longer, else from the times of every round its kind is judged among, `any`, where any
/// of them took longer, else from its cell's again.
fn rest(own: &Ordered, any: &Ordered, so_far: f64, least: usize) -> Rest {
    let own = Left::of(own, so_far);
    let from = if own.count() >= least.max(1) {
        own
    } else {
        Some(Left::of(any, so_far)).filter(|any| any.count() > 0).unwrap_or(own)
    };
    median_of_sorted(&from).map_or(Rest::Longer, Rest::Expected)
}

/// A call of a tool against the lines of its history.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CallAgainst {
    /// How long it took, or has taken so far while it is under way; none where too few calls of its tool tell the line.
    pub time: Option<Against>,
    /// How many bytes its result is, once it had one; none while it is under way and where too few calls of its tool
    /// had results to tell the line.
    pub size: Option<Against>,
}

/// The rounds and the calls of tools of the works judged, each against the lines of its history.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Judged {
    /// Each round its history tells a line of, by its work, its run and its number in the run, the first being one.
    pub rounds: BTreeMap<(WorkNumber, RunId, usize), Round>,
    /// Each call of a tool its history tells a line of, by its work and its name.
    pub calls: BTreeMap<(WorkNumber, CallId), CallAgainst>,
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
        let mut ended: Vec<&Value> = works.iter().flat_map(|of| &of.part.values).collect();
        ended.sort_by_key(|v| (v.ended, v.work, v.at));
        let mut asked: Vec<&Judging> = (works.iter())
            .filter(|of| picked(of.number))
            .flat_map(|of| &of.part.judging)
            .collect();
        asked.sort_by_key(|a| (a.began, a.work, a.at));
        let mut history = History::of(&ended);
        let mut next = 0;
        for one in &asked {
            while ended.get(next).is_some_and(|v| v.ended < one.began) {
                history.add(next);
                next += 1;
            }
            // What ended as it began: of another work, it had ended by then; of its own, only what ended before it in
            // the work's order, which leaves out the thing itself, however short.
            let ties: Vec<usize> = (next..ended.len())
                .take_while(|i| ended[*i].ended == one.began)
                .filter(|i| ended[*i].work != one.work || ended[*i].at < one.at)
                .collect();
            for value in &ties {
                history.add(*value);
            }
            history.judge(one, least, now, &mut out);
            for value in ties.iter().rev() {
                history.take(*value);
            }
        }
    }
    out
}

/// A value of a history, of a round or a call that ended: when, where in its work's order, and what it is.
#[derive(Clone, Debug, PartialEq)]
struct Value {
    ended: u64,
    work: WorkNumber,
    at: Seq,
    kind: Kind,
}

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    /// An agent's round that is history: its cell, where the flow installed has it, how long it took, where the records
    /// tell, and the model it ran on, by the name its kind goes by ([`model_of`]), with what it cost, where one was
    /// recorded.
    Agent {
        cell: Option<CellId>,
        ms: Option<f64>,
        cost: Option<(String, f64)>,
    },
    /// A round of an act's cell that handed something in, with how long it took.
    Act(CellId, f64),
    /// A call of a tool that had its result, with how long it took and how many bytes the result is, where the records
    /// hold it.
    Tool(String, f64, Option<f64>),
}

/// A round or a call to be judged: when it began, where in its work's order, and what it is.
#[derive(Clone, Debug, PartialEq)]
struct Judging {
    began: u64,
    work: WorkNumber,
    at: Seq,
    /// When it ended, where it has and the records tell when; none while it is under way.
    ended: Option<u64>,
    /// The waits a call was held over, as the host could not carry it out for now, each from when it began until when
    /// it was over, none while it is under way; none for a round.
    held: Vec<(u64, Option<u64>)>,
    what: What,
}

impl Judging {
    /// How long it took, or has taken until `now` while it is under way, the time it was held left out: that is no time
    /// of its own.
    fn time(&self, now: Time) -> f64 {
        let end = self.ended.unwrap_or(now.0);
        end.saturating_sub(self.began)
            .saturating_sub(held_time(&self.held, end)) as f64
    }
}

#[derive(Clone, Debug, PartialEq)]
enum What {
    Round {
        run: RunId,
        number: usize,
        cell: CellId,
        act: bool,
        /// The model its latest try ran on, where one was recorded.
        model: Option<String>,
        /// What it cost, once it ended.
        cost: Option<f64>,
    },
    Call {
        call: CallId,
        tool: String,
        /// How many bytes its result is, once it had one.
        size: Option<f64>,
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

/// The cell of each run of the work, as it is on the flow installed, and whether an act of the engine's own carries it
/// out, as that flow says: none of a run whose cell has no id there, and none of a work on no version kept.
fn cells(compiled: Option<&Compiled>, reading: Option<&Reading>, work: &Work) -> BTreeMap<RunId, (CellId, bool)> {
    let Some(compiled) = compiled else {
        return BTreeMap::new();
    };
    (work.records.started.iter())
        .filter_map(|started| {
            let cell = reading.map_or_else(|| started.cell.clone(), |r| r.cell(&started.cell));
            let spec = compiled.cells.iter().find(|spec| spec.id == cell)?;
            Some((started.run, (spec.id.clone(), spec.act.is_some())))
        })
        .collect()
}

/// Whether the call was withdrawn, as the records tell: its run was abandoned, or its work's item cancelled and not
/// reopened since, before it had its result, or by now where it has none. Whatever it then had came of the withdrawal.
pub fn withdrawn(work: &Work, call: CallId) -> bool {
    let r = &work.records;
    let Some(called) = r.called.iter().find(|called| called.call == call) else {
        return false;
    };
    let done = r.done.iter().find(|done| done.call == call).map(|done| done.at);
    let before = |at: Seq| done.is_none_or(|done| at < done);
    let abandoned = (r.abandoned.iter()).any(|a| a.run == called.run && before(a.at));
    let cancelled = (r.cancels.iter())
        .filter(|cancel| before(**cancel))
        .any(|cancel| !r.reopens.iter().any(|reopen| reopen > cancel && before(*reopen)));
    abandoned || cancelled
}

/// The tool a call runs, where it is a call of a tool.
fn tool(work: &Work, call: CallId) -> Option<&str> {
    match work.calls.get(&call) {
        Some(Call::Asked(Ask::Tool { name, .. })) => Some(name),
        _ => None,
    }
}

/// The waits a call was held over, each from when it began until when it was over, none while it is under way: what the
/// call took beside, as the host could not carry it out for now, is no time of the call's own.
fn held_waits(work: &Work, call: CallId) -> Vec<(u64, Option<u64>)> {
    (work.records.held.iter())
        .filter(|held| held.call == call)
        .filter_map(|held| {
            let wait = work.records.called.iter().find(|c| c.call == held.wait)?;
            let done = (work.records.done.iter().find(|d| d.call == held.wait)).and_then(|d| work.time_of(d.at));
            Some((work.time_of(wait.at)?.0, done.map(|done| done.0)))
        })
        .collect()
}

/// How long of the time up to `end` the waits `waits` took.
fn held_time(waits: &[(u64, Option<u64>)], end: u64) -> u64 {
    (waits.iter())
        .map(|(from, to)| to.unwrap_or(end).min(end).saturating_sub(*from))
        .sum()
}

/// The work's values of the history: its agents' rounds that are history, the rounds of its acts that handed something
/// in, and its calls of tools that had their results, each where the records tell when.
fn values(number: WorkNumber, work: &Work, samples: &[Sample], cells: &BTreeMap<RunId, (CellId, bool)>) -> Vec<Value> {
    let ms = |from: Seq, to: Seq| Some(work.time_of(to)?.0.saturating_sub(work.time_of(from)?.0) as f64);
    let mut out: Vec<Value> = (samples.iter())
        .filter_map(|sample| {
            Some(Value {
                ended: sample.ended?.0,
                work: number,
                at: sample.at,
                kind: Kind::Agent {
                    cell: sample.cell.clone(),
                    ms: sample.ms.map(|ms| ms as f64),
                    cost: (sample.model.as_deref())
                        .map(|model| (model_of(model).to_string(), sample.spent.usd_micros as f64)),
                },
            })
        })
        .collect();
    for (run, number_in_run, began, ended) in rounds(work) {
        let (Some((cell, true)), Some(ended)) = (cells.get(&run), ended) else {
            continue;
        };
        let handed = matches!(
            work.back.get(&(run, number_in_run)),
            Some(Back::HandedIn(_) | Back::Returned { .. })
        );
        if let (true, Some(took), Some(at)) = (handed, ms(began, ended), work.time_of(ended)) {
            out.push(Value {
                ended: at.0,
                work: number,
                at: ended,
                kind: Kind::Act(cell.clone(), took),
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
        if withdrawn(work, called.call) {
            continue;
        }
        if let (Some(took), Some(at)) = (ms(called.at, done.at), work.time_of(done.at)) {
            let took = took - held_time(&held_waits(work, called.call), at.0) as f64;
            out.push(Value {
                ended: at.0,
                work: number,
                at: done.at,
                kind: Kind::Tool(
                    name.to_string(),
                    took,
                    work.results.get(&called.call).map(|r| r.len() as f64),
                ),
            });
        }
    }
    out
}

/// The work's rounds and calls of tools to judge, each where the records tell when it began.
fn judging(number: WorkNumber, work: &Work, cells: &BTreeMap<RunId, (CellId, bool)>) -> Vec<Judging> {
    let mut out = Vec::new();
    for (run, number_in_run, began_at, ended_at) in rounds(work) {
        let (Some((cell, act)), Some(began)) = (cells.get(&run), work.time_of(began_at)) else {
            continue;
        };
        let ended = ended_at.and_then(|at| work.time_of(at));
        let model = (work.ran.iter().rev())
            .find(|ran| ran.run == run && ran.round == number_in_run)
            .map(|ran| ran.on.model.clone());
        let cost = (ended.and(work.spent.get(&(run, number_in_run)))).map(|spent| spent.usd_micros as f64);
        out.push(Judging {
            began: began.0,
            work: number,
            at: began_at,
            ended: ended.map(|ended| ended.0),
            held: Vec::new(),
            what: What::Round {
                run,
                number: number_in_run,
                cell: cell.clone(),
                act: *act,
                model,
                cost,
            },
        });
    }
    for called in &work.records.called {
        let (Some(name), Some(made)) = (tool(work, called.call), work.time_of(called.at)) else {
            continue;
        };
        if withdrawn(work, called.call) {
            continue;
        }
        let done = (work.records.done.iter()).find(|done| done.call == called.call);
        let ended = done.and_then(|done| work.time_of(done.at));
        let size = done
            .and_then(|done| work.results.get(&done.call))
            .map(|r| r.len() as f64);
        out.push(Judging {
            began: made.0,
            work: number,
            at: called.at,
            ended: ended.map(|ended| ended.0),
            held: held_waits(work, called.call),
            what: What::Call {
                call: called.call,
                tool: name.to_string(),
                size,
            },
        });
    }
    out
}

/// The values a list of a history may come to hold, each once, the smallest first, with how many times it holds each
/// of them now, those counts summed over the stretches of a Fenwick tree: so a value joins or leaves the list, and the
/// value at a place in it is found, at a cost that grows with the logarithm of how many values it may hold.
#[derive(Clone, Debug, Default)]
struct Ordered {
    distinct: Vec<f64>,
    held: Vec<usize>,
    /// The sums of `held` over the stretches of the tree, counted from one.
    sums: Vec<usize>,
    count: usize,
}

impl Ordered {
    /// A list that may come to hold these values, and holds none yet.
    fn over(mut values: Vec<f64>) -> Ordered {
        values.sort_by(f64::total_cmp);
        values.dedup_by(|a, b| a.total_cmp(b).is_eq());
        let n = values.len();
        Ordered {
            distinct: values,
            held: vec![0; n],
            sums: vec![0; n + 1],
            count: 0,
        }
    }

    /// The value joins the list: one of the values it may come to hold.
    fn add(&mut self, value: f64) {
        let Ok(rank) = self.distinct.binary_search_by(|v| v.total_cmp(&value)) else {
            debug_assert!(false, "{value} is not a value the list may hold");
            return;
        };
        self.held[rank] += 1;
        self.count += 1;
        let mut i = rank + 1;
        while i < self.sums.len() {
            self.sums[i] += 1;
            i += i.isolate_lowest_one();
        }
    }

    /// One of the value leaves the list, where the list holds it.
    fn take(&mut self, value: f64) {
        let Ok(rank) = self.distinct.binary_search_by(|v| v.total_cmp(&value)) else {
            return;
        };
        if self.held[rank] == 0 {
            return;
        }
        self.held[rank] -= 1;
        self.count -= 1;
        let mut i = rank + 1;
        while i < self.sums.len() {
            self.sums[i] -= 1;
            i += i.isolate_lowest_one();
        }
    }

    /// How many of the values the list holds are at most this one.
    fn at_most(&self, value: f64) -> usize {
        let mut i = self.distinct.partition_point(|v| *v <= value);
        let mut sum = 0;
        while i > 0 {
            sum += self.sums[i];
            i -= i.isolate_lowest_one();
        }
        sum
    }
}

impl Sorted for Ordered {
    fn count(&self) -> usize {
        self.count
    }

    /// The value at this place among those the list holds: the first value whose counts, with those of the values
    /// smaller than it, pass the place.
    fn nth(&self, place: usize) -> f64 {
        let n = self.distinct.len();
        let (mut at, mut left) = (0, place);
        let mut step = if n == 0 {
            0
        } else {
            1 << (usize::BITS - 1 - n.leading_zeros())
        };
        while step > 0 {
            let next = at + step;
            if next <= n && self.sums[next] <= left {
                at = next;
                left -= self.sums[next];
            }
            step >>= 1;
        }
        self.distinct[at]
    }
}

/// The history as of a moment: each kind of value apart, as a list of its own ([`Ordered`]).
struct History<'a> {
    lists: Vec<Ordered>,
    /// Each list a value of the history counts in, with what it counts there: those of the value at a place in the
    /// order the values ended are `slots[starts[place]..starts[place + 1]]`.
    slots: Vec<(usize, f64)>,
    starts: Vec<usize>,
    /// The list of how long the agents' rounds took, by cell, and of every cell.
    time_of_cell: BTreeMap<&'a CellId, usize>,
    /// The lists of what the agents' rounds cost, by cell and model, and by model of every cell.
    cost_of_cell: BTreeMap<(&'a CellId, &'a str), usize>,
    cost: BTreeMap<&'a str, usize>,
    /// The lists of how long the rounds of acts took, by cell.
    acts: BTreeMap<&'a CellId, usize>,
    /// The lists of how long the calls of tools took, by tool.
    tools: BTreeMap<&'a str, usize>,
    /// The lists of how many bytes the results of the calls of tools are, by tool.
    sizes: BTreeMap<&'a str, usize>,
    /// A list that holds nothing, for a kind of value the history has none of.
    empty: Ordered,
}

/// The list of how long every agent's round of the flow took.
const TIME: usize = 0;

impl<'a> History<'a> {
    /// The history of a flow whose values, in the order they ended, are these, holding none of them yet.
    fn of(ended: &[&'a Value]) -> History<'a> {
        let mut history = History {
            lists: Vec::new(),
            slots: Vec::new(),
            starts: Vec::with_capacity(ended.len() + 1),
            time_of_cell: BTreeMap::new(),
            cost_of_cell: BTreeMap::new(),
            cost: BTreeMap::new(),
            acts: BTreeMap::new(),
            tools: BTreeMap::new(),
            sizes: BTreeMap::new(),
            empty: Ordered::default(),
        };
        let mut can_hold: Vec<Vec<f64>> = vec![Vec::new()];
        for &value in ended {
            history.starts.push(history.slots.len());
            let (slots, hold) = (&mut history.slots, &mut can_hold);
            match &value.kind {
                // A round of a cell with no id on the flow installed counts in the flow's history alone.
                Kind::Agent { cell, ms, cost } => {
                    if let Some(ms) = *ms {
                        if let Some(cell) = cell {
                            let list = list_of(&mut history.time_of_cell, cell, hold);
                            put(slots, hold, list, ms);
                        }
                        put(slots, hold, TIME, ms);
                    }
                    if let Some((model, usd)) = cost {
                        if let Some(cell) = cell {
                            let list = list_of(&mut history.cost_of_cell, (cell, model.as_str()), hold);
                            put(slots, hold, list, *usd);
                        }
                        let list = list_of(&mut history.cost, model.as_str(), hold);
                        put(slots, hold, list, *usd);
                    }
                }
                Kind::Act(cell, ms) => {
                    let list = list_of(&mut history.acts, cell, hold);
                    put(slots, hold, list, *ms);
                }
                Kind::Tool(tool, ms, size) => {
                    let list = list_of(&mut history.tools, tool.as_str(), hold);
                    put(slots, hold, list, *ms);
                    if let Some(size) = size {
                        let list = list_of(&mut history.sizes, tool.as_str(), hold);
                        put(slots, hold, list, *size);
                    }
                }
            }
        }
        history.starts.push(history.slots.len());
        history.lists = can_hold.into_iter().map(Ordered::over).collect();
        history
    }

    /// The value at this place in the order the values ended joins the history.
    fn add(&mut self, place: usize) {
        for &(list, value) in &self.slots[self.starts[place]..self.starts[place + 1]] {
            self.lists[list].add(value);
        }
    }

    /// The value at this place in the order the values ended, which joined the history last of all of its kind, leaves
    /// it.
    fn take(&mut self, place: usize) {
        for &(list, value) in &self.slots[self.starts[place]..self.starts[place + 1]] {
            self.lists[list].take(value);
        }
    }

    /// The list at this index, where there is one; else one that holds nothing.
    fn list(&self, index: Option<&usize>) -> &Ordered {
        index.map_or(&self.empty, |index| &self.lists[*index])
    }

    /// Judges the round or the call against the history as it stands, what is under way as having taken until `now`,
    /// into what was judged.
    fn judge(&self, one: &'a Judging, least: usize, now: Time, out: &mut Judged) {
        let (time, under_way) = (one.time(now), one.ended.is_none());
        match &one.what {
            What::Round {
                run,
                number,
                cell,
                act: true,
                ..
            } => {
                // An act is judged among the rounds of its own cell alone.
                let own = self.list(self.acts.get(cell));
                let judged = Round {
                    time: (judged_sorted(own, &self.empty, least, Scope::Cell, Scope::Cell))
                        .map(|line| Against { value: time, line }),
                    rest: under_way.then(|| rest(own, own, time, least)),
                    ..Round::default()
                };
                if judged != Round::default() {
                    out.rounds.insert((one.work, *run, *number), judged);
                }
            }
            What::Round {
                run,
                number,
                cell,
                act: false,
                model,
                cost,
            } => {
                let on = model.as_deref().map(model_of);
                let cost_line = on.and_then(|on| {
                    let own = self.list(self.cost_of_cell.get(&(cell, on)));
                    judged_sorted(own, self.list(self.cost.get(on)), least, Scope::Cell, Scope::Flow)
                });
                let own = self.list(self.time_of_cell.get(cell));
                let every = &self.lists[TIME];
                let time_line = judged_sorted(own, every, least, Scope::Cell, Scope::Flow);
                let judged = Round {
                    cost: (cost.zip(cost_line)).map(|(value, line)| Against { value, line }),
                    time: time_line.map(|line| Against { value: time, line }),
                    rest: under_way.then(|| rest(own, every, time, least)),
                };
                if judged != Round::default() {
                    out.rounds.insert((one.work, *run, *number), judged);
                }
            }
            What::Call { call, tool, size } => {
                let line =
                    |list: Option<&usize>| judged_sorted(self.list(list), &self.empty, least, Scope::Tool, Scope::Tool);
                let judged = CallAgainst {
                    time: line(self.tools.get(tool.as_str())).map(|line| Against { value: time, line }),
                    size: size
                        .zip(line(self.sizes.get(tool.as_str())))
                        .map(|(value, line)| Against { value, line }),
                };
                if judged != CallAgainst::default() {
                    out.calls.insert((one.work, *call), judged);
                }
            }
        }
    }
}

/// The list of this key, made where there is none yet.
fn list_of<K: Ord>(lists: &mut BTreeMap<K, usize>, key: K, can_hold: &mut Vec<Vec<f64>>) -> usize {
    *lists.entry(key).or_insert_with(|| {
        can_hold.push(Vec::new());
        can_hold.len() - 1
    })
}

/// The value counts in the list: the list may come to hold it, and holds it whenever the value is in the history.
fn put(slots: &mut Vec<(usize, f64)>, can_hold: &mut [Vec<f64>], list: usize, value: f64) {
    can_hold[list].push(value);
    slots.push((list, value));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A list that holds these values.
    fn holding(values: &[f64]) -> Ordered {
        let mut list = Ordered::over(values.to_vec());
        for value in values {
            list.add(*value);
        }
        list
    }

    // A round under way is expected to take what the rounds of its history that took longer had left at the point it
    // has reached, the median of those: of its cell's where at least as many as the settings say took longer, else of
    // every round its kind is judged among, and past every one of them it is longer than any before. A round that took
    // just as long as this one has so far is not longer. Fails where the median is taken of the whole history, of what
    // the rounds took rather than what they had left, or of the cell's too few. Microseconds; the one test of the rest.
    #[test]
    fn a_round_s_rest_is_the_median_of_what_the_longer_rounds_had_left() {
        let own = holding(&[1000.0, 2000.0, 3000.0, 5000.0, 9000.0]);
        let any = holding(&[1000.0, 2000.0, 2500.0, 4000.0, 6000.0, 8000.0, 12_000.0]);
        assert_eq!(
            rest(&own, &any, 2000.0, 3),
            Rest::Expected(3000.0),
            "of the cell's 1000, 3000 and 7000 left"
        );
        assert_eq!(
            rest(&own, &any, 2000.0, 4),
            Rest::Expected(4000.0),
            "of the flow's 500, 2000, 4000, 6000 and 10000 left, the cell's three longer being too few"
        );
        assert_eq!(
            rest(&own, &any, 10_000.0, 3),
            Rest::Expected(2000.0),
            "of the one longer round of the flow"
        );
        assert_eq!(
            rest(&own, &any, 12_000.0, 3),
            Rest::Longer,
            "as long as the longest is not longer"
        );
        let none = Ordered::default();
        assert_eq!(rest(&none, &none, 0.0, 3), Rest::Longer, "with no history");
    }

    /// The next number of a sequence that is the same on every run, from its seed.
    fn next(seed: &mut u64) -> u64 {
        *seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *seed >> 33
    }

    /// One of these, as the sequence picks it.
    fn pick<T: Clone>(seed: &mut u64, of: &[T]) -> T {
        of[next(seed) as usize % of.len()].clone()
    }

    /// A number below `below`, or none, as the sequence picks it.
    fn maybe(seed: &mut u64, below: u64) -> Option<f64> {
        let value = (next(seed) % below) as f64;
        next(seed).is_multiple_of(2).then_some(value)
    }

    /// What six works add to the judgments of one flow, made up from the seed: few distinct times, so that many things
    /// end as others begin, of two agents' cells, an act's cell and two tools, on two models, one named with what it
    /// was run with.
    fn parts(seed: &mut u64) -> Vec<(WorkNumber, Part)> {
        let cells = ["a", "b"].map(CellId::from);
        let models = ["m1", "m1[1m]", "m2"];
        let tools = ["t1", "t2"];
        (1..=6)
            .map(|work| {
                let work = WorkNumber(work);
                let values = (0..40)
                    .map(|at| {
                        let kind = match next(seed) % 3 {
                            0 => {
                                let cell = pick(seed, &[Some(cells[0].clone()), Some(cells[1].clone()), None]);
                                let ms = maybe(seed, 9);
                                let model = model_of(pick(seed, &models)).to_string();
                                let cost = maybe(seed, 7).map(|usd| (model, usd));
                                Kind::Agent { cell, ms, cost }
                            }
                            1 => Kind::Act(CellId::from("x"), (next(seed) % 9) as f64),
                            _ => {
                                let tool = pick(seed, &tools).to_string();
                                let ms = (next(seed) % 9) as f64;
                                Kind::Tool(tool, ms, maybe(seed, 5))
                            }
                        };
                        Value {
                            ended: next(seed) % 30,
                            work,
                            at: Seq(at),
                            kind,
                        }
                    })
                    .collect();
                let judging = (0..30)
                    .map(|at| {
                        let began = next(seed) % 30;
                        let what = if next(seed).is_multiple_of(3) {
                            let tool = pick(seed, &tools).to_string();
                            What::Call {
                                call: CallId(at),
                                tool,
                                size: maybe(seed, 5),
                            }
                        } else {
                            let cell = pick(seed, &["a", "b", "x"]);
                            let model = pick(seed, &models).to_string();
                            let model = pick(seed, &[Some(model), None]);
                            What::Round {
                                run: RunId(at),
                                number: 1,
                                cell: CellId::from(cell),
                                act: cell == "x",
                                model,
                                cost: maybe(seed, 7),
                            }
                        };
                        let took = next(seed) % 9;
                        Judging {
                            began,
                            work,
                            at: Seq(at),
                            ended: pick(seed, &[Some(began + took), None]),
                            held: Vec::new(),
                            what,
                        }
                    })
                    .collect();
                (work, Part { values, judging })
            })
            .collect()
    }

    /// Each thing of the works judged as the module says it is, each from its history worked out apart: every value of
    /// another work that had ended as it began, and of its own work every value that ended before it began or as it
    /// began before it in the work's order, each list sorted and read as it stands.
    fn by_definition(works: &[(WorkNumber, Part)], least: usize, now: Time) -> Judged {
        let mut out = Judged::default();
        let every: Vec<&Value> = works.iter().flat_map(|(_, part)| &part.values).collect();
        for one in works.iter().flat_map(|(_, part)| &part.judging) {
            let history: Vec<&Value> = (every.iter().copied())
                .filter(|v| v.ended < one.began || (v.ended == one.began && (v.work != one.work || v.at < one.at)))
                .collect();
            let list = |of: &dyn Fn(&Kind) -> Option<f64>| -> Vec<f64> {
                let mut list: Vec<f64> = history.iter().filter_map(|v| of(&v.kind)).collect();
                list.sort_by(f64::total_cmp);
                list
            };
            let rest = |own: &[f64], any: &[f64], so_far: f64| {
                let left = |sorted: &[f64]| -> Vec<f64> {
                    let longer = sorted.partition_point(|v| *v <= so_far);
                    sorted[longer..].iter().map(|took| took - so_far).collect()
                };
                let own = left(own);
                let from = if own.len() >= least.max(1) {
                    own
                } else {
                    Some(left(any)).filter(|any| !any.is_empty()).unwrap_or(own)
                };
                crate::history::median(&from).map_or(Rest::Longer, Rest::Expected)
            };
            let none: &[f64] = &[];
            let (time, under_way) = (one.time(now), one.ended.is_none());
            match &one.what {
                What::Round {
                    run,
                    number,
                    cell,
                    act: true,
                    ..
                } => {
                    let own = list(&|kind| match kind {
                        Kind::Act(of, ms) if of == cell => Some(*ms),
                        _ => None,
                    });
                    let judged = Round {
                        time: judged_sorted(own.as_slice(), none, least, Scope::Cell, Scope::Cell)
                            .map(|line| Against { value: time, line }),
                        rest: under_way.then(|| rest(&own, &own, time)),
                        cost: None,
                    };
                    if judged != Round::default() {
                        out.rounds.insert((one.work, *run, *number), judged);
                    }
                }
                What::Round {
                    run,
                    number,
                    cell,
                    model,
                    cost,
                    ..
                } => {
                    let on = model.as_deref().map(model_of);
                    let cost_of = |of_cell: bool| {
                        list(&|kind| match kind {
                            Kind::Agent {
                                cell: of,
                                cost: Some((model, usd)),
                                ..
                            } if Some(model.as_str()) == on && (!of_cell || of.as_ref() == Some(cell)) => Some(*usd),
                            _ => None,
                        })
                    };
                    let time_of = |of_cell: bool| {
                        list(&|kind| match kind {
                            Kind::Agent {
                                cell: of, ms: Some(ms), ..
                            } if !of_cell || of.as_ref() == Some(cell) => Some(*ms),
                            _ => None,
                        })
                    };
                    let (own, every) = (time_of(true), time_of(false));
                    let cost_line = on.and_then(|_| {
                        judged_sorted(
                            cost_of(true).as_slice(),
                            cost_of(false).as_slice(),
                            least,
                            Scope::Cell,
                            Scope::Flow,
                        )
                    });
                    let judged = Round {
                        cost: (cost.zip(cost_line)).map(|(value, line)| Against { value, line }),
                        time: judged_sorted(own.as_slice(), every.as_slice(), least, Scope::Cell, Scope::Flow)
                            .map(|line| Against { value: time, line }),
                        rest: under_way.then(|| rest(&own, &every, time)),
                    };
                    if judged != Round::default() {
                        out.rounds.insert((one.work, *run, *number), judged);
                    }
                }
                What::Call { call, tool, size } => {
                    let times = list(&|kind| match kind {
                        Kind::Tool(of, ms, _) if of == tool => Some(*ms),
                        _ => None,
                    });
                    let sizes = list(&|kind| match kind {
                        Kind::Tool(of, _, Some(size)) if of == tool => Some(*size),
                        _ => None,
                    });
                    let line = |list: &[f64]| judged_sorted(list, none, least, Scope::Tool, Scope::Tool);
                    let judged = CallAgainst {
                        time: line(&times).map(|line| Against { value: time, line }),
                        size: size.zip(line(&sizes)).map(|(value, line)| Against { value, line }),
                    };
                    if judged != CallAgainst::default() {
                        out.calls.insert((one.work, *call), judged);
                    }
                }
            }
        }
        out
    }

    // Going through the flow's history once with its lists kept as counts judges each thing as its history worked out
    // apart judges it: in all, many works' values ending as others begin, of every kind, with lines of each scope, and
    // what is under way with its rest. Fails where a value joins the history after the thing it ended as, where a list
    // misreads its median or its largest, or where a tie of the thing's own work is read the wrong way round.
    #[test]
    fn judging_through_the_history_once_gives_what_each_things_own_history_gives() {
        let mut seed = 7;
        for _ in 0..20 {
            let works = parts(&mut seed);
            let of: Vec<Of<'_>> = (works.iter())
                .map(|(number, part)| Of {
                    number: *number,
                    flow: "flow",
                    part,
                })
                .collect();
            for least in [1, 3, 8] {
                let now = Time(45);
                let judged = judged(&of, |_| true, least, now);
                assert!(!judged.rounds.is_empty() && !judged.calls.is_empty(), "lines are given");
                assert_eq!(
                    judged,
                    by_definition(&works, least, now),
                    "with {least} values at least"
                );
            }
        }
    }
}
