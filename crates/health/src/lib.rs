//! The system's health numbers. One registry ([`measures`]) says what is measured, each measure once: its name, what
//! it is, its unit, where its values come from, how the values within a step fold into one, and what it can be split
//! by. [`series`] gives a measure's values over any period at any step, each point with how many values it folds: a
//! point with nothing to fold has no value rather than a zero standing for one, and a measure nothing records yet says
//! why.
//!
//! Most measures are computed from the records as they stand, each time they are read (the rounds, the tool runs, the
//! blockers, the changes, the architect's matters, the reverts), so that a definition put right puts their whole
//! history right. Two are computed from the records and the commits on a project's main, which the caller reads from
//! git and gives ([`MainLog`], in [`Scope::main`]). A state of the engine at a moment, which no record holds (its CPU,
//! the page's reads, the slots, the works running, the architect's queue), the engine samples once a minute into
//! `samples` ([`sample`]).
//!
//! An event falls in the point that holds the moment it was done: a round when it ended, a merge when it merged, an
//! item when it was made, a commit on main when it was made by its committer time; the one exception is what a round
//! cost, which falls when the round started, as the System view counts it. A stop is a span: it counts once, in the
//! point it started in, and lasts in each point for the part of it there, one still open lasting until now. An amount
//! the engine samples over the time between two of its samples (its CPU, the page's reads) falls in the minute that
//! time began. A point runs from its start up to its end, which belongs to the next.

mod records;
mod round;
mod samples;

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

use db::rusqlite::types::Value as Sql;
use db::rusqlite::{self, Connection, Row, params_from_iter};

pub use round::{Outcome, outcome};
pub use samples::{Beat, MINUTE, PageReads, cpu_ms, minute, sample};

/// The most points a series may have: a year at a step of a minute, twice over.
const MOST_POINTS: i64 = 1_051_200;

/// What marks a row of the merge tool (`tool_runs.tool` `merge`, in `args.step`) as a wait for the full CI, which
/// [`measure`] `ci.wait` counts: the tool's other rows look it up on GitHub or call the merge, and a row that failed
/// shows by nothing else which it was. Its writer and its reader use this one name.
pub const FULL_CI_STEP: &str = "full_ci";

/// Whose a measure is to act on when it goes wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// The system's own: only the architect of the project that develops wf-agents can mend it.
    System,
    /// The project's own, such as its CI: the project's architect mends it.
    Project,
}

/// What a measure's values count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Ms,
    Tokens,
    Usd,
    Count,
    /// A part of a whole, from 0 to 1.
    Ratio,
    /// A share of one processor's time: 1 is one processor busy all the time.
    Share,
}

impl Unit {
    pub fn name(self) -> &'static str {
        match self {
            Unit::Ms => "ms",
            Unit::Tokens => "tokens",
            Unit::Usd => "usd",
            Unit::Count => "count",
            Unit::Ratio => "ratio",
            Unit::Share => "share",
        }
    }
}

/// How the values that fall in one point fold into the point's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fold {
    /// How many there are; for spans, how many started in the point.
    Count,
    /// Their sum; for spans, how long they lasted within the point.
    Sum,
    /// The middle one, or the mean of the two middle ones when there is an even number of them.
    Median,
    /// The largest.
    Max,
    /// The part of them that holds, each value being 1 when it holds and 0 when not.
    Ratio,
}

/// What a measure can be split by: each of its values belongs to one subject of that kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum By {
    /// Nothing: every value belongs to the subject "".
    Nothing,
    /// The cell, as `<graph>/<cell code>`.
    Cell,
    /// Why the work stopped.
    Reason,
    /// The graph of the work: the items' works (`engineer`) apart from the architect's.
    Graph,
}

impl By {
    pub fn name(self) -> Option<&'static str> {
        match self {
            By::Nothing => None,
            By::Cell => Some("cell"),
            By::Reason => Some("reason"),
            By::Graph => Some("graph"),
        }
    }
}

/// What a measure's values are read from, between two times and for one project or all of them.
pub type Gather = fn(&Reader<'_>, Option<&str>, i64, i64) -> Result<Vec<Fact>, String>;

/// What a measure's values are read from with the commits on the project's main, between two times.
pub type MainGather = fn(&Reader<'_>, Option<&str>, &MainLog, i64, i64) -> Result<Vec<Fact>, String>;

/// Where a measure's values come from.
#[derive(Debug, Clone, Copy)]
pub enum Source {
    /// Computed from the records each time it is read.
    Records(Gather),
    /// Computed from the records and the commits on the project's main, which the caller gives ([`Scope::main`]); not
    /// measured when it gives none.
    Main(MainGather),
    /// Sampled by the engine each minute, its own and no project's.
    Engine,
    /// Sampled by the engine each minute, one value for each project.
    Projects,
    /// Nothing records it yet: why.
    Missing(&'static str),
}

/// A measure of the system's health.
#[derive(Debug, Clone, Copy)]
pub struct Measure {
    /// Its name, which every reader uses, such as `work.to_merge`.
    pub name: &'static str,
    /// What it measures, in a sentence.
    pub what: &'static str,
    pub layer: Layer,
    pub unit: Unit,
    pub source: Source,
    pub fold: Fold,
    pub by: By,
    /// The subjects of its kind it does not measure yet, each with why.
    pub missing: &'static [(&'static str, &'static str)],
    /// Why a point of it has no value, when nothing fell in it.
    pub empty: &'static str,
}

impl Measure {
    /// Why it has no values for a subject (all of them when none is given), when nothing records them.
    pub fn not_measured(&self, subject: Option<&str>) -> Option<&'static str> {
        if let Source::Missing(why) = self.source {
            return Some(why);
        }
        let s = subject?;
        self.missing.iter().find(|(m, _)| *m == s).map(|(_, why)| *why)
    }

    /// Whether a point with nothing in it is a zero, as a count of events the records hold is, and not a value unknown.
    fn zero_when_empty(&self) -> bool {
        matches!(self.source, Source::Records(_) | Source::Main(_)) && matches!(self.fold, Fold::Count | Fold::Sum)
    }

    /// Why it has no values for the scope read, when nothing records them or what it needs was not given.
    fn unmeasured_in(&self, scope: &Scope<'_>) -> Option<&'static str> {
        // One project's main says nothing of another's merges, so reading them for every project measures nothing.
        let read = scope.project.is_some() && scope.main.is_some();
        self.not_measured(scope.subject)
            .or_else(|| (matches!(self.source, Source::Main(_)) && !read).then_some(MAIN_UNREAD))
    }
}

/// Why waiting for a free slot is not among the stops yet.
const SLOT_UNRECORDED: &str = "a wait for a free slot is recorded in slot_waits, but not counted among the stops yet";

/// Why a measure of the commits on main has no values when the reader named no project or gave no commits.
const MAIN_UNREAD: &str = "the commits on one project's main were not read";

/// What the engine measures, each measure once.
static MEASURES: [Measure; 24] = [
    Measure {
        name: "merge.broken",
        what: "The checks of main's red CI (the architect's cell M1) that found a merge made main red; the merges among \
               them not in merge.reverted are those the architect chose to fix forward",
        layer: Layer::System,
        unit: Unit::Count,
        source: Source::Records(records::broken_merges),
        fold: Fold::Count,
        by: By::Nothing,
        missing: &[],
        empty: "",
    },
    Measure {
        name: "merge.reverted",
        what: "Merges the engine reverted, as the architect asked once it judged they made main red",
        layer: Layer::System,
        unit: Unit::Count,
        source: Source::Records(records::reverted_merges),
        fold: Fold::Count,
        by: By::Nothing,
        missing: &[],
        empty: "",
    },
    Measure {
        name: "merge.defects",
        what: "Defects found later in an item already merged",
        layer: Layer::System,
        unit: Unit::Count,
        source: Source::Missing(
            "a defect found in an item already merged is not recorded in a form the engine can count",
        ),
        fold: Fold::Count,
        by: By::Nothing,
        missing: &[],
        empty: "",
    },
    Measure {
        name: "round.context",
        what: "The context of an agent's round as it ended, the median of the rounds: the tokens of its last call's \
               prompt, read from the cache, written to it or neither",
        layer: Layer::System,
        unit: Unit::Tokens,
        source: Source::Records(records::round_context),
        fold: Fold::Median,
        by: By::Cell,
        missing: &[],
        empty: "no agent's round that reported its usage ended",
    },
    Measure {
        name: "round.context_max",
        what: "The context of an agent's round as it ended, the largest of the rounds",
        layer: Layer::System,
        unit: Unit::Tokens,
        source: Source::Records(records::round_context),
        fold: Fold::Max,
        by: By::Cell,
        missing: &[],
        empty: "no agent's round that reported its usage ended",
    },
    Measure {
        name: "round.sent_back",
        what: "The part of the agents' rounds whose return the engine sent back; a run that failed or was interrupted \
               is not sent back",
        layer: Layer::System,
        unit: Unit::Ratio,
        source: Source::Records(records::sent_back),
        fold: Fold::Ratio,
        by: By::Nothing,
        missing: &[],
        empty: "no agent's round ended",
    },
    Measure {
        name: "round.failed",
        what: "The part of the agents' rounds whose run failed or was interrupted before it handed in a return",
        layer: Layer::System,
        unit: Unit::Ratio,
        source: Source::Records(records::failed),
        fold: Fold::Ratio,
        by: By::Nothing,
        missing: &[],
        empty: "no agent's round ended",
    },
    Measure {
        name: "owner.reply",
        what: "How long the owner's words to an architect waited, from when they came to the end of the architect's \
               round that handled them, the median",
        layer: Layer::System,
        unit: Unit::Ms,
        source: Source::Records(records::owner_reply),
        fold: Fold::Median,
        by: By::Nothing,
        missing: &[],
        empty: "no round handled the owner's words",
    },
    Measure {
        name: "work.stops",
        what: "How many times a work stopped to wait for someone, by why: a blocker of each kind (question, \
               tool_failure, owner_ask, items, ruling), or a pause (paused)",
        layer: Layer::System,
        unit: Unit::Count,
        source: Source::Records(records::stops),
        fold: Fold::Count,
        by: By::Reason,
        missing: &[("slot", SLOT_UNRECORDED)],
        empty: "",
    },
    Measure {
        name: "work.stopped",
        what: "How long the works stood stopped, all together, by why, as work.stops counts them",
        layer: Layer::System,
        unit: Unit::Ms,
        source: Source::Records(records::stops),
        fold: Fold::Sum,
        by: By::Reason,
        missing: &[("slot", SLOT_UNRECORDED)],
        empty: "",
    },
    Measure {
        name: "engine.cpu",
        what: "The processor time the engine's process used, the page it serves included, as a share of the time \
               that passed, the median of the minutes",
        layer: Layer::System,
        unit: Unit::Share,
        source: Source::Engine,
        fold: Fold::Median,
        by: By::Nothing,
        missing: &[],
        empty: "the engine took no sample of its CPU: it was not running, or had just started",
    },
    Measure {
        name: "work.to_merge",
        what: "How long an item took from the first round of its first work to its merge, the median of the items \
               merged",
        layer: Layer::System,
        unit: Unit::Ms,
        source: Source::Records(records::to_merge),
        fold: Fold::Median,
        by: By::Nothing,
        missing: &[],
        empty: "no item merged",
    },
    Measure {
        name: "ci.wait",
        what: "How long an item waited for CI before its merge, the median of the items merged: the time covered by \
               the runs of ci, ci_before and full_ci of all its works and by the merge's waits for the full CI, each \
               from queued to ended, time when several ran counted once",
        layer: Layer::Project,
        unit: Unit::Ms,
        source: Source::Records(records::ci_wait),
        fold: Fold::Median,
        by: By::Nothing,
        missing: &[],
        empty: "no item merged",
    },
    Measure {
        name: "items.made",
        what: "Items made here, those imported from outside the flow left out",
        layer: Layer::System,
        unit: Unit::Count,
        source: Source::Records(records::items_made),
        fold: Fold::Count,
        by: By::Nothing,
        missing: &[],
        empty: "",
    },
    Measure {
        name: "items.merged",
        what: "Pull requests the flow merged (cell M2), each merge it recorded, each one commit on main; a merge whose \
               commit main does not hold counts here too, and again in merge.not_on_main",
        layer: Layer::System,
        unit: Unit::Count,
        source: Source::Records(records::items_merged),
        fold: Fold::Count,
        by: By::Nothing,
        missing: &[],
        empty: "",
    },
    Measure {
        name: "merge.not_on_main",
        what: "Merges the flow recorded (cell M2) whose commit cannot be reached from the project's main, by when the \
               round ended; a merge that ended after the commit of main read was made is judged once main has moved on",
        layer: Layer::System,
        unit: Unit::Count,
        source: Source::Main(records::not_on_main),
        fold: Fold::Count,
        by: By::Nothing,
        missing: &[],
        empty: "",
    },
    Measure {
        name: "main.outside_flow",
        what: "Commits that came onto the project's main without the flow, each one counted, all that a merge by hand \
               brought and the merge itself: every commit main reaches but the merge of a pull request the flow merged \
               (cell M2) and the engine's revert of a merge. Each falls when it was made, by its committer time: for a \
               commit pushed by hand, when it was made, not when it was pushed; so a week already past can still grow",
        layer: Layer::Project,
        unit: Unit::Count,
        source: Source::Main(records::outside_flow),
        fold: Fold::Count,
        by: By::Nothing,
        missing: &[],
        empty: "",
    },
    Measure {
        name: "items.cancelled",
        what: "Items cancelled",
        layer: Layer::System,
        unit: Unit::Count,
        source: Source::Records(records::items_cancelled),
        fold: Fold::Count,
        by: By::Nothing,
        missing: &[],
        empty: "",
    },
    Measure {
        name: "spend.usd",
        what: "What the rounds cost, by the graph of their work: the items' works (engineer) and the architect's",
        layer: Layer::System,
        unit: Unit::Usd,
        source: Source::Records(records::spend),
        fold: Fold::Sum,
        by: By::Graph,
        missing: &[],
        empty: "",
    },
    Measure {
        name: "page.reads",
        what: "The requests the owner's page read with, the event stream left out",
        layer: Layer::System,
        unit: Unit::Count,
        source: Source::Engine,
        fold: Fold::Sum,
        by: By::Nothing,
        missing: &[],
        empty: "the engine took no sample: it was not running",
    },
    Measure {
        name: "page.read_ms",
        what: "How long serving the owner's page's reads took, all together",
        layer: Layer::System,
        unit: Unit::Ms,
        source: Source::Engine,
        fold: Fold::Sum,
        by: By::Nothing,
        missing: &[],
        empty: "the engine took no sample: it was not running",
    },
    Measure {
        name: "slots.used",
        what: "The works holding a slot, the median of the minutes",
        layer: Layer::System,
        unit: Unit::Count,
        source: Source::Engine,
        fold: Fold::Median,
        by: By::Nothing,
        missing: &[],
        empty: "the engine took no sample: it was not running",
    },
    Measure {
        name: "works.running",
        what: "The works with a step running, the median of the minutes",
        layer: Layer::System,
        unit: Unit::Count,
        source: Source::Engine,
        fold: Fold::Median,
        by: By::Nothing,
        missing: &[],
        empty: "the engine took no sample: it was not running",
    },
    Measure {
        name: "queue.oldest",
        what: "How long the oldest matter in the architect's queue had waited, the longest of the minutes",
        layer: Layer::System,
        unit: Unit::Ms,
        source: Source::Projects,
        fold: Fold::Max,
        by: By::Nothing,
        missing: &[],
        empty: "the engine took no sample: it was not running",
    },
];

/// Every measure the engine knows.
pub fn measures() -> &'static [Measure] {
    &MEASURES
}

/// A measure by its name.
pub fn measure(name: &str) -> Option<&'static Measure> {
    MEASURES.iter().find(|m| m.name == name)
}

/// Which values a series reads: one project's, or every project's and the engine's own; and one subject's, or all of
/// them together; with the commits on the project's main, for the measures that read them.
#[derive(Debug, Clone, Copy, Default)]
pub struct Scope<'a> {
    pub project: Option<&'a str>,
    pub subject: Option<&'a str>,
    pub main: Option<&'a MainLog>,
}

/// The commits a project's main holds: all that the commit read can reach through any parent, as git gives them, each
/// by its full id with when it was made, by its committer time; and when the commit read was made. Reaching a commit
/// through any parent, and not along first parents alone, is what holding it means: a merge made by hand may put main
/// as its second parent, and what it brought from elsewhere is on main all the same.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainLog {
    /// Each commit's committer time, in milliseconds, by its full id.
    at: HashMap<String, i64>,
    /// The committer time of the commit read, main's tip, in milliseconds.
    tip_at: i64,
}

impl MainLog {
    /// The git command that gives what [`MainLog::parse`] reads, the commit to read from after it: every commit it
    /// reaches, itself first, as its full id and its committer time in seconds. It reads them all, not since a time,
    /// since git's `--since` stops at a commit made before the time, and a commit pushed by hand can have been made
    /// before one under it.
    pub const GIT_LOG: [&'static str; 2] = ["log", "--format=%H %ct"];

    /// The commits of main as [`MainLog::GIT_LOG`] gives them.
    pub fn parse(log: &str) -> Result<MainLog, String> {
        let mut at = HashMap::new();
        let mut tip_at = None;
        for line in log.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let made = line
                .split_once(' ')
                .and_then(|(id, s)| Some((id, s.trim().parse::<i64>().ok()?)));
            let Some((id, seconds)) = made else {
                return Err(format!(
                    "main's log has a line that is no commit with its time: {line:?}"
                ));
            };
            let ms = seconds.saturating_mul(1000);
            tip_at.get_or_insert(ms);
            at.insert(id.to_string(), ms);
        }
        let tip_at = tip_at.ok_or_else(|| "main's log has no commit".to_string())?;
        Ok(MainLog { at, tip_at })
    }

    /// Whether main reaches a commit, by its full id.
    pub fn holds(&self, commit: &str) -> bool {
        self.at.contains_key(commit)
    }
}

/// One point of a series: the time from `from` up to `to`, the values that fell in it, and what they fold to; none
/// when nothing fell in it and nothing is no zero.
#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub from: i64,
    pub to: i64,
    pub n: i64,
    pub value: Option<f64>,
}

/// A measure's values over a period, a point at each step.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub points: Vec<Point>,
    /// Why it has no values at all: nothing records them.
    pub not_measured: Option<&'static str>,
}

/// One value a measure is computed from: an event at a time, or a span from one time to another.
#[derive(Debug, Clone, PartialEq)]
pub struct Fact {
    /// When it happened; for a span, when it started, or [`i64::MIN`] for one already going on before the time read.
    at: i64,
    /// For a span, when it ended, or now for one still going on; none for an event.
    until: Option<i64>,
    subject: String,
    value: f64,
}

impl Fact {
    fn event(at: i64, subject: impl Into<String>, value: f64) -> Fact {
        Fact {
            at,
            until: None,
            subject: subject.into(),
            value,
        }
    }

    fn span(at: i64, until: i64, subject: impl Into<String>) -> Fact {
        Fact {
            at,
            until: Some(until.max(at)),
            subject: subject.into(),
            value: 1.0,
        }
    }
}

/// Reads the records for a series, keeping each statement it ran with its arguments, so that a test can see how
/// SQLite reads each.
pub struct Reader<'c> {
    conn: &'c Connection,
    /// Now, until which a span still going on lasts.
    now: i64,
    ran: RefCell<Vec<(String, Vec<Sql>)>>,
}

impl<'c> Reader<'c> {
    fn new(conn: &'c Connection) -> Reader<'c> {
        Reader {
            conn,
            now: db::now(),
            ran: RefCell::default(),
        }
    }

    /// The rows a statement gives, each as `f` reads it.
    fn rows<T>(
        &self,
        sql: &str,
        args: Vec<Sql>,
        f: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Vec<T>, String> {
        let failed = |e: rusqlite::Error| format!("reading the health numbers: {e}");
        let mut st = self.conn.prepare_cached(sql).map_err(failed)?;
        let rows = st
            .query_map(params_from_iter(args.iter()), f)
            .map_err(failed)?
            .collect::<rusqlite::Result<Vec<T>>>()
            .map_err(failed)?;
        self.ran.borrow_mut().push((sql.to_string(), args));
        Ok(rows)
    }
}

/// The arguments every statement of the records takes: the project (none for all of them), and the time from which
/// and up to which it reads.
fn args(project: Option<&str>, from: i64, to: i64) -> Vec<Sql> {
    vec![project_arg(project), Sql::Integer(from), Sql::Integer(to)]
}

/// The project a statement reads (`?1`), none for all of them.
fn project_arg(project: Option<&str>) -> Sql {
    project.map_or(Sql::Null, |p| Sql::Text(p.to_string()))
}

/// The points of a period at a step: each from its start up to the next one's, the last up to the period's end.
fn points(from: i64, to: i64, step: i64) -> Result<Vec<(i64, i64)>, String> {
    if step <= 0 {
        return Err(format!("a step of {step} ms is no step"));
    }
    let span = to
        .checked_sub(from)
        .filter(|s| *s > 0)
        .ok_or_else(|| format!("the period from {from} to {to} holds no time"))?;
    let n = span / step + i64::from(span % step != 0);
    if n > MOST_POINTS {
        return Err(format!(
            "a series of {n} points is more than the {MOST_POINTS} a series may have"
        ));
    }
    // A point's end is at most the period's end, which a last point that is not a whole step would pass.
    let at = |k: i64| from.saturating_add(k.saturating_mul(step)).min(to);
    Ok((0..n).map(|k| (at(k), at(k + 1))).collect())
}

/// The values a measure is computed from, between two times, for the scope's project or all of them; every subject's.
fn facts(r: &Reader<'_>, m: &Measure, scope: &Scope<'_>, from: i64, to: i64) -> Result<Vec<Fact>, String> {
    let project = scope.project;
    match (m.source, scope.main) {
        (Source::Records(gather), _) => gather(r, project, from, to),
        (Source::Main(gather), Some(main)) => gather(r, project, main, from, to),
        (Source::Engine, _) => samples::sampled(r, m.name, Some(""), from, to),
        (Source::Projects, _) => samples::sampled(r, m.name, project, from, to),
        (Source::Main(_), None) | (Source::Missing(_), _) => Ok(Vec::new()),
    }
}

/// The values of each point of a period, from the facts given.
fn fold(m: &Measure, facts: &[&Fact], bounds: &[(i64, i64)], step: i64) -> Vec<Point> {
    let (Some(first), Some(last)) = (bounds.first(), bounds.last()) else {
        return Vec::new();
    };
    let (from, to) = (first.0, last.1);
    let index = |t: i64| usize::try_from((t - from) / step).unwrap_or(0);
    let mut values: Vec<Vec<f64>> = vec![Vec::new(); bounds.len()];
    for f in facts {
        match f.until {
            None => {
                if (from..to).contains(&f.at) {
                    values[index(f.at)].push(f.value);
                }
            }
            // A span counts once, where it started.
            Some(_) if m.fold == Fold::Count => {
                if (from..to).contains(&f.at) {
                    values[index(f.at)].push(1.0);
                }
            }
            // And lasts, in each point, for the part of it there.
            Some(until) => {
                let (a, b) = (f.at.max(from), until.min(to));
                if a >= b {
                    continue;
                }
                let (i, j) = (index(a), index(b - 1));
                for (k, &(p, q)) in bounds.iter().enumerate().take(j + 1).skip(i) {
                    values[k].push((b.min(q) - a.max(p)) as f64);
                }
            }
        }
    }
    let zero = m.zero_when_empty();
    bounds
        .iter()
        .zip(values)
        .map(|(&(a, b), mut vs)| Point {
            from: a,
            to: b,
            n: vs.len() as i64,
            value: if vs.is_empty() {
                zero.then_some(0.0)
            } else {
                Some(match m.fold {
                    Fold::Count => vs.len() as f64,
                    Fold::Sum => vs.iter().sum(),
                    Fold::Ratio => vs.iter().sum::<f64>() / vs.len() as f64,
                    Fold::Max => vs.iter().copied().fold(f64::MIN, f64::max),
                    Fold::Median => {
                        vs.sort_by(f64::total_cmp);
                        let mid = vs.len() / 2;
                        if vs.len() % 2 == 1 {
                            vs[mid]
                        } else {
                            (vs[mid - 1] + vs[mid]) / 2.0
                        }
                    }
                })
            },
        })
        .collect()
}

/// A series with no values, since nothing records them: why.
fn unmeasured(bounds: &[(i64, i64)], why: &'static str) -> Series {
    Series {
        points: bounds
            .iter()
            .map(|&(from, to)| Point {
                from,
                to,
                n: 0,
                value: None,
            })
            .collect(),
        not_measured: Some(why),
    }
}

/// A measure's values from `from` up to `to`, a point at each `step` milliseconds (the last up to `to`), for the scope
/// given: the project's, or all of them; the subject's, or all of them together.
pub fn series(
    conn: &Connection,
    m: &Measure,
    scope: &Scope<'_>,
    from: i64,
    to: i64,
    step: i64,
) -> Result<Series, String> {
    series_read(&Reader::new(conn), m, scope, from, to, step)
}

fn series_read(
    r: &Reader<'_>,
    m: &Measure,
    scope: &Scope<'_>,
    from: i64,
    to: i64,
    step: i64,
) -> Result<Series, String> {
    let bounds = points(from, to, step)?;
    if let Some(why) = m.unmeasured_in(scope) {
        return Ok(unmeasured(&bounds, why));
    }
    let all = facts(r, m, scope, from, to)?;
    let facts: Vec<&Fact> = all
        .iter()
        .filter(|f| scope.subject.is_none_or(|s| f.subject == s))
        .collect();
    Ok(Series {
        points: fold(m, &facts, &bounds, step),
        not_measured: None,
    })
}

/// A measure's series for each of its subjects that has values in the period, as [`series`] gives one, and for each
/// subject it does not measure yet, saying why; for the scope's project, or all of them, whatever subject it names.
pub fn series_by_subject(
    conn: &Connection,
    m: &Measure,
    scope: &Scope<'_>,
    from: i64,
    to: i64,
    step: i64,
) -> Result<Vec<(String, Series)>, String> {
    let bounds = points(from, to, step)?;
    let every = Scope {
        subject: None,
        ..*scope
    };
    if let Some(why) = m.unmeasured_in(&every) {
        return Ok(vec![(String::new(), unmeasured(&bounds, why))]);
    }
    let r = Reader::new(conn);
    let all = facts(&r, m, &every, from, to)?;
    let mut by: BTreeMap<&str, Vec<&Fact>> = BTreeMap::new();
    for f in &all {
        by.entry(f.subject.as_str()).or_default().push(f);
    }
    let mut out: Vec<(String, Series)> = by
        .into_iter()
        .map(|(s, fs)| {
            let series = Series {
                points: fold(m, &fs, &bounds, step),
                not_measured: None,
            };
            (s.to_string(), series)
        })
        .collect();
    for (s, why) in m.missing {
        out.push(((*s).to_string(), unmeasured(&bounds, why)));
    }
    Ok(out)
}

/// The items made here, imported ones left out, each with when it was made, from `from` up to `to`, the earliest
/// first: of one project, or of all of them.
pub fn items_made(conn: &Connection, project: Option<&str>, from: i64, to: i64) -> Result<Vec<(i64, i64)>, String> {
    records::made(&Reader::new(conn), project, from, to)
}

#[cfg(test)]
mod tests;
