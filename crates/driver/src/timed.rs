//! What the times of the project's tests and jobs say, worked out from the works' records whenever they are read and
//! stored nowhere (principle `state-from-records`), by the history of their own kind (principle
//! `judged-by-own-history`): the merged checks of the project, each the runs of the full workflows that passed and
//! merged a work.
//!
//! - A test that got slower: its time in a merged check against its own earlier times in merged checks, by the line of
//!   [`crate::history`], listed only where it is past the line in two merged checks in a row, which a reading made slow
//!   by the machine's load once is not ([`slowed`]).
//! - The series of the merges: for each, what its check took, job by job and tests in all, and the tests it added with
//!   what they take ([`series`]), from which the merge that brought a growth in is read.
//! - The growth of the CI: the median of the last `samples` merged checks of a job's time, and of the sum of the tests'
//!   times, against the median of the `samples` before them, and the merge after which the series stepped most
//!   ([`growth`]). The comparison is a sign test: the latest values above the earlier median must be as many as, with
//!   no step, they would be by chance less than one time in a thousand.
//! - What the work's own runs say of the tests it added, for the work's reviewers: the facts of the datum `test_times`
//!   ([`facts`], [`render`]). The engine judges no new test's time; the reviewer does, with these.
//! - What each call of `run_tests` asked for and gave, for the work's test review: the datum `test_runs`, one version to
//!   a call ([`runs`]). The engine reads nothing of a result.
//!
//! Nothing is judged where the history has fewer values than [`crate::history::HISTORY_SAMPLES`].

use std::collections::{BTreeMap, BTreeSet};

use flow::Texts;
use flow::agents::{ResultGiven, fill};
use store::WorkNumber;
use work::timed::Source;
use work::{Ask, Call, Time, Work};

use crate::history::{line, median};

/// The tool whose calls the work's times are told after.
pub const RUN_TESTS: &str = "run_tests";

/// The source of the datum that says what the work's runs took.
pub const TEST_TIMES: &str = "test_times";

/// The source of the datum that gives each call of `run_tests` of the work, with what it asked for and what it gave.
pub const TEST_RUNS: &str = "test_runs";

/// The words the datum `test_runs`' definition gives in its `writes`, each a text the engine fills.
pub const RUNS_WORDS: [&str; 2] = ["call", "cut"];

/// The words the datum's definition gives in its `writes`, each a text the engine fills.
pub const WORDS: [&str; 15] = [
    "no_times",
    "none_read",
    "basis",
    "not_compared",
    "covers",
    "new_title",
    "all_title",
    "slowed_title",
    "test_new",
    "test_plain",
    "test_slowed",
    "more",
    "sum",
    "sum_alone",
    "merges",
];

/// How many tests of a kind the datum lists, the slowest first; the rest are counted and summed.
pub const LISTED: usize = 20;

/// How many of the latest merges the series gives.
pub const SERIES_ROWS: usize = 20;

/// A test, by the unit it is in and its name.
pub type Key = (String, String);

/// A merged work's check: the full runs that passed and merged the work, with the time of each job and of each test
/// that passed.
#[derive(Clone, Debug, PartialEq)]
pub struct Merge {
    pub work: WorkNumber,
    /// When the last of the runs was read, where the work's records say.
    pub time: Option<Time>,
    /// Each job's time, by the name of the job under the workflow it ran in.
    pub jobs: BTreeMap<String, u64>,
    /// Each test's time; one that more than one workflow ran is at its slowest.
    pub tests: BTreeMap<Key, u64>,
}

/// The name a job goes by in a merged check: its own under the workflow it ran in, since two workflows may each have a
/// job of one name. A job of a run that is no workflow's goes by its own name.
pub fn job_name(workflow: &str, job: &str) -> String {
    if workflow.is_empty() {
        job.to_string()
    } else {
        format!("{workflow}:{job}")
    }
}

impl Merge {
    /// The check of a merged work: the runs of the full workflows that its latest check asked for, all of them, since
    /// the check runs every full workflow of the project under one asking on one commit; none where it has none. A
    /// check that failed, or was run again, was before the one that merged the work, which is the latest.
    pub fn of(number: WorkNumber, work: &Work) -> Option<Merge> {
        let latest = (work.timed.iter().rev()).find(|t| t.reading.source == Source::Full)?;
        let check = &latest.reading;
        let mut jobs = BTreeMap::new();
        let mut tests: BTreeMap<Key, u64> = BTreeMap::new();
        let runs = (work.timed.iter())
            .map(|t| &t.reading)
            .filter(|r| r.source == Source::Full && r.asking == check.asking && r.commit == check.commit);
        for reading in runs {
            for job in &reading.jobs {
                jobs.insert(job_name(&reading.workflow, &job.name), job.millis);
            }
            for test in reading.tests.iter().filter(|t| t.passed) {
                let slowest = tests.entry((test.unit.clone(), test.name.clone())).or_insert(0);
                *slowest = (*slowest).max(test.millis);
            }
        }
        Some(Merge {
            work: number,
            time: work.time_of(latest.at),
            jobs,
            tests,
        })
    }
}

/// The merged checks of the works that merged, in the order they were read: by their time, then by the work's number.
pub fn history<'a>(merged: impl Iterator<Item = (WorkNumber, &'a Work)>) -> Vec<Merge> {
    let mut checks: Vec<Merge> = merged.filter_map(|(number, work)| Merge::of(number, work)).collect();
    checks.sort_by_key(|m| (m.time, m.work));
    checks
}

/// The least count of the latest `n` values above the median of the `n` before them that has, with no step, a chance
/// under one in a thousand, as the count of `n` fair coins that come up heads: more than `n` where that chance is never
/// so small, so that a history that short never reports.
pub fn needed(n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    // The chance of each count, as the binomial coefficient over two to the n, worked out without overflow.
    let mut chance = vec![0.0_f64; n + 1];
    let mut coefficient = 1.0_f64;
    for (k, slot) in chance.iter_mut().enumerate() {
        *slot = coefficient * 0.5_f64.powi(n as i32);
        coefficient = coefficient * (n - k) as f64 / (k + 1) as f64;
    }
    let mut tail = 0.0;
    let mut least = n + 1;
    for k in (0..=n).rev() {
        tail += chance[k];
        if tail < 0.001 {
            least = k;
        } else {
            break;
        }
    }
    least
}

/// A test that took longer than its own history says in two merged checks in a row.
#[derive(Clone, Debug, PartialEq)]
pub struct Slowed {
    /// The work whose check measured it last.
    pub work: WorkNumber,
    pub unit: String,
    pub name: String,
    pub millis: u64,
    /// The median of its earlier times and the line they give, in milliseconds.
    pub median: f64,
    pub line: f64,
}

/// The times of a test in the merged checks that have it, the earliest first.
fn values(history: &[Merge], key: &Key) -> Vec<f64> {
    history
        .iter()
        .filter_map(|m| m.tests.get(key).map(|v| *v as f64))
        .collect()
}

/// The tests past their own line in the latest merged check and in the one before it: each judged against its own
/// times in the checks before those two, where at least `samples` of them tell it, so that a slowdown that stays is
/// not read into the line it is judged by.
pub fn slowed(history: &[Merge], samples: usize) -> Vec<Slowed> {
    let [baseline @ .., before, latest] = history else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for (key, millis) in &latest.tests {
        let Some(then) = before.tests.get(key) else {
            continue;
        };
        let own = values(baseline, key);
        if own.len() < samples.max(1) {
            continue;
        }
        let Some(limit) = line(&own) else {
            continue;
        };
        if (*millis as f64) > limit && (*then as f64) > limit {
            found.push(Slowed {
                work: latest.work,
                unit: key.0.clone(),
                name: key.1.clone(),
                millis: *millis,
                median: median(&own).unwrap_or_default(),
                line: limit,
            });
        }
    }
    found.sort_by(|a, b| {
        b.millis
            .cmp(&a.millis)
            .then_with(|| (&a.unit, &a.name).cmp(&(&b.unit, &b.name)))
    });
    found
}

/// What one merged check said, as a row of the series.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub work: WorkNumber,
    pub time: Option<Time>,
    /// Each job's time, in the order of the jobs' names.
    pub jobs: Vec<(String, u64)>,
    /// What all its tests took together.
    pub tests_millis: u64,
    /// How many tests it has that the check before it did not, and what they take.
    pub added: usize,
    pub added_millis: u64,
}

/// The latest `rows` merges, in order, each with what its check took and what it added to the check before it.
pub fn series(history: &[Merge], rows: usize) -> Vec<Row> {
    let from = history.len().saturating_sub(rows);
    (from..history.len())
        .map(|i| {
            let merge = &history[i];
            let before = i.checked_sub(1).map(|b| &history[b]);
            let new: Vec<u64> = (merge.tests.iter())
                .filter(|(key, _)| before.is_some_and(|b| !b.tests.contains_key(*key)))
                .map(|(_, millis)| *millis)
                .collect();
            Row {
                work: merge.work,
                time: merge.time,
                jobs: merge.jobs.iter().map(|(n, m)| (n.clone(), *m)).collect(),
                tests_millis: merge.tests.values().sum(),
                added: new.len(),
                added_millis: new.iter().sum(),
            }
        })
        .collect()
}

/// A series of the checks' times that grew: its latest `n` values are above the median of the `n` before them in as
/// many as no step makes unlikely, and the merge after which it stepped most.
#[derive(Clone, Debug, PartialEq)]
pub struct Growth {
    /// `tests` for the sum of the tests' times, or the name of a job.
    pub of: String,
    /// The work whose merge the series stepped at, the first after the step.
    pub work: WorkNumber,
    pub before: f64,
    pub after: f64,
    /// How many of the latest values are above the earlier median, of `n`, and how many it takes.
    pub above: usize,
    pub n: usize,
    pub needed: usize,
}

/// The name of the series of the sum of the tests' times.
pub const TESTS: &str = "tests";

/// The series that grew, each judged once it has 2 x `samples` values.
pub fn growth(history: &[Merge], samples: usize) -> Vec<Growth> {
    let n = samples;
    let mut series: BTreeMap<String, Vec<(usize, f64)>> = BTreeMap::new();
    for (i, merge) in history.iter().enumerate() {
        series
            .entry(TESTS.to_string())
            .or_default()
            .push((i, merge.tests.values().sum::<u64>() as f64));
        for (name, millis) in &merge.jobs {
            series.entry(name.clone()).or_default().push((i, *millis as f64));
        }
    }
    let least = needed(n);
    let mut found = Vec::new();
    for (of, points) in series {
        if n == 0 || points.len() < 2 * n {
            continue;
        }
        let values: Vec<f64> = points.iter().map(|(_, v)| *v).collect();
        let last = &values[values.len() - n..];
        let before = &values[values.len() - 2 * n..values.len() - n];
        let Some(earlier) = median(before) else {
            continue;
        };
        let above = last.iter().filter(|v| **v > earlier).count();
        if above < least {
            continue;
        }
        // The step: the split of the last 2 n values into the ones before and the ones after it whose means differ most,
        // weighted by how many each side has, which is the true step where the series has one.
        let window = &values[values.len() - 2 * n..];
        let total = window.len();
        let shortest = (n / 4).max(1);
        let mut best: Option<(usize, f64)> = None;
        for b in shortest..=total - shortest {
            let (low, high) = (&window[..b], &window[b..]);
            let rise = high.iter().sum::<f64>() / high.len() as f64 - low.iter().sum::<f64>() / low.len() as f64;
            if rise <= 0.0 {
                continue;
            }
            let score = (b * (total - b)) as f64 / total as f64 * rise * rise;
            if best.is_none_or(|(_, top)| score > top) {
                best = Some((b, score));
            }
        }
        let Some((boundary, _)) = best else {
            continue;
        };
        let merge = points[points.len() - 2 * n + boundary].0;
        found.push(Growth {
            of,
            work: history[merge].work,
            before: earlier,
            after: median(last).unwrap_or_default(),
            above,
            n,
            needed: least,
        });
    }
    found
}

/// What the merged checks of a project say: how many there are, the tests that got slower, the series of the merges, and
/// the series that grew.
#[derive(Clone, Debug, PartialEq)]
pub struct Tested {
    pub project: Option<String>,
    pub checks: usize,
    pub slowed: Vec<Slowed>,
    pub series: Vec<Row>,
    pub growth: Vec<Growth>,
}

/// What the merged checks of the project say, judged by the history where at least `samples` of them tell it: with
/// none, only the series.
pub fn tested(project: Option<String>, history: &[Merge], samples: usize) -> Tested {
    Tested {
        project,
        checks: history.len(),
        slowed: if samples > 0 {
            slowed(history, samples)
        } else {
            Vec::new()
        },
        series: series(history, SERIES_ROWS),
        growth: if samples > 0 {
            growth(history, samples)
        } else {
            Vec::new()
        },
    }
}

/// How long one test of the project's CI may run before it is stopped, in milliseconds: the line of every passed test's
/// time in the latest `samples` merged checks, past the largest of them by as much as that is past their median. None
/// with fewer merged checks than that, or none that read a test's time.
pub fn test_time(history: &[Merge], samples: usize) -> Option<f64> {
    if samples == 0 || history.len() < samples {
        return None;
    }
    let values: Vec<f64> = (history[history.len() - samples..].iter())
        .flat_map(|merge| merge.tests.values().map(|millis| *millis as f64))
        .collect();
    line(&values).filter(|line| *line > 0.0)
}

/// One test of the work's runs, for the datum.
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    pub unit: String,
    pub name: String,
    /// The median of its readings in the work's runs, in milliseconds, and how many readings there are.
    pub millis: u64,
    pub readings: usize,
    /// How many percent of the project's tests it is slower than, where the history tells.
    pub share: Option<u64>,
    /// For a test an earlier merged check has: the median of its times there and the line they give.
    pub before: Option<(u64, u64)>,
}

/// What the datum `test_times` says of a work, before it is put in words.
#[derive(Clone, Debug, PartialEq)]
pub enum Facts {
    /// The work's host gives no time of any test.
    NoTimes,
    /// No run of the work has reported a test's time.
    NoneRead,
    /// What the work's runs reported.
    Read {
        /// How many merged checks the project has, and how many the comparisons need.
        checks: usize,
        samples: usize,
        /// The tests the work's runs reported: those in no merged check, and so new, where the history tells; every
        /// one of them where it does not.
        new: Vec<Listed>,
        /// How many of `new` there are in all, and their time together.
        new_count: usize,
        new_millis: u64,
        /// The tests an earlier merged check has that took longer in the work's runs than their own line.
        slowed: Vec<Listed>,
        /// What the project's tests typically take together in a merged check, in milliseconds.
        suite: Option<u64>,
        /// What the latest merges added: how many, the median and the largest, in milliseconds.
        adds: Option<(usize, u64, u64)>,
    },
}

/// The facts of the datum for a work's records, against the history of the project's merged checks.
pub fn facts(work: &Work, history: &[Merge], samples: usize) -> Facts {
    let reading = |source: Source| work.timed.iter().filter(move |t| t.reading.source == source);
    let mut readings: BTreeMap<Key, Vec<u64>> = BTreeMap::new();
    for timed in reading(Source::Fast).chain(reading(Source::Full)) {
        for test in timed.reading.tests.iter().filter(|t| t.passed) {
            readings
                .entry((test.unit.clone(), test.name.clone()))
                .or_default()
                .push(test.millis);
        }
    }
    if readings.is_empty() {
        let none = work.timed.iter().any(|t| t.reading.source == Source::Command);
        return if none { Facts::NoTimes } else { Facts::NoneRead };
    }
    let compared = samples > 0 && history.len() >= samples;
    let last: &[Merge] = &history[history.len().saturating_sub(samples.max(1))..];
    // Each test's median over the latest merged checks, for where a test stands in the suite.
    let typical: Vec<u64> = if compared {
        let keys: BTreeSet<&Key> = history.last().map(|m| m.tests.keys().collect()).unwrap_or_default();
        keys.into_iter()
            .filter_map(|key| median(&values(last, key)).map(|m| m.round() as u64))
            .collect()
    } else {
        Vec::new()
    };
    let known: BTreeSet<&Key> = history.iter().flat_map(|m| m.tests.keys()).collect();
    let mut new = Vec::new();
    let mut slowed = Vec::new();
    for (key, millis) in &readings {
        let mut sorted = millis.clone();
        sorted.sort_unstable();
        let own = median(&sorted.iter().map(|m| *m as f64).collect::<Vec<_>>()).unwrap_or_default();
        let millis = own.round() as u64;
        let listed = |share: Option<u64>, before: Option<(u64, u64)>| Listed {
            unit: key.0.clone(),
            name: key.1.clone(),
            millis,
            readings: sorted.len(),
            share,
            before,
        };
        if !known.contains(key) || !compared {
            let share = compared.then(|| {
                let slower = typical.iter().filter(|t| **t < millis).count();
                (100 * slower / typical.len().max(1)) as u64
            });
            new.push(listed(share, None));
            continue;
        }
        let history_of = values(history, key);
        if history_of.len() >= samples.max(1)
            && let (Some(line), Some(before)) = (line(&history_of), median(&history_of))
            && own > line
        {
            slowed.push(listed(None, Some((before.round() as u64, line.round() as u64))));
        }
    }
    let by_time = |a: &Listed, b: &Listed| {
        b.millis
            .cmp(&a.millis)
            .then_with(|| (&a.unit, &a.name).cmp(&(&b.unit, &b.name)))
    };
    new.sort_by(by_time);
    slowed.sort_by(by_time);
    let new_count = new.len();
    let new_millis = new.iter().map(|l| l.millis).sum();
    new.truncate(LISTED);
    slowed.truncate(LISTED);
    let suite = compared.then(|| typical.iter().sum::<u64>());
    let adds = compared.then(|| {
        let rows = series(history, samples + 1);
        let added: Vec<f64> = rows.iter().skip(1).map(|r| r.added_millis as f64).collect();
        let largest = added.iter().cloned().fold(0.0, f64::max);
        (
            added.len(),
            median(&added).unwrap_or_default().round() as u64,
            largest.round() as u64,
        )
    });
    Facts::Read {
        checks: history.len(),
        samples,
        new,
        new_count,
        new_millis,
        slowed,
        suite,
        adds,
    }
}

/// The datum in the words its definition gives (`texts`, its `writes`), each filled with the facts and none written
/// here.
pub fn render(facts: &Facts, texts: &Texts) -> String {
    let says = |name: &str| texts.get(name).unwrap_or_default();
    match facts {
        Facts::NoTimes => says("no_times").to_string(),
        Facts::NoneRead => says("none_read").to_string(),
        Facts::Read {
            checks,
            samples,
            new,
            new_count,
            new_millis,
            slowed: slow,
            suite,
            adds,
        } => {
            let mut lines = vec![fill(says("basis"), &[("checks", &checks.to_string())])];
            let compared = suite.is_some();
            if !compared {
                lines.push(fill(
                    says("not_compared"),
                    &[("checks", &checks.to_string()), ("needed", &samples.to_string())],
                ));
            }
            lines.push(says("covers").to_string());
            lines.push(says(if compared { "new_title" } else { "all_title" }).to_string());
            for t in new {
                let slots = test_slots(t);
                lines.push(fill(
                    says(if t.share.is_some() { "test_new" } else { "test_plain" }),
                    &slots_ref(&slots),
                ));
            }
            if *new_count > new.len() {
                let left = new_count - new.len();
                let listed: u64 = new.iter().map(|t| t.millis).sum();
                lines.push(fill(
                    says("more"),
                    &[
                        ("left", &left.to_string()),
                        ("millis", &new_millis.saturating_sub(listed).to_string()),
                    ],
                ));
            }
            match suite {
                Some(suite) => {
                    let share = if *suite == 0 { 0 } else { 100 * new_millis / suite };
                    lines.push(fill(
                        says("sum"),
                        &[
                            ("millis", &new_millis.to_string()),
                            ("share", &share.to_string()),
                            ("suite", &suite.to_string()),
                        ],
                    ));
                }
                None => lines.push(fill(says("sum_alone"), &[("millis", &new_millis.to_string())])),
            }
            if let Some((merges, median, largest)) = adds {
                lines.push(fill(
                    says("merges"),
                    &[
                        ("merges", &merges.to_string()),
                        ("median", &median.to_string()),
                        ("largest", &largest.to_string()),
                    ],
                ));
            }
            if !slow.is_empty() {
                lines.push(says("slowed_title").to_string());
                for t in slow {
                    let slots = test_slots(t);
                    lines.push(fill(says("test_slowed"), &slots_ref(&slots)));
                }
            }
            lines.join("\n")
        }
    }
}

/// The versions of the datum `test_runs` as the work's records now say them: one for each call of `run_tests` that has
/// a result, in the order the results were recorded, in the words of the datum's definition (`texts`), each filled with
/// the call's number among them, what it asked for as the agent wrote it, and its result as a round of the engineer is
/// given it: its end of whitespace left off, and cut where the tool's words (`given`) say, with the datum's own `cut`
/// words, which name no file. Nothing of the result is read here.
pub fn runs(work: &Work, texts: &Texts, given: Option<&ResultGiven>) -> Vec<String> {
    let says = |name: &str| texts.get(name).unwrap_or_default();
    let asked = work
        .records
        .done
        .iter()
        .filter_map(|done| match work.calls.get(&done.call) {
            Some(Call::Asked(Ask::Tool { name, input })) if name == RUN_TESTS => Some((done.call, input)),
            _ => None,
        });
    asked
        .enumerate()
        .map(|(i, (call, input))| {
            let result = work.results.get(&call).map(String::as_str).unwrap_or_default();
            let result = match given {
                Some(given) => given.cut_with(result, |kept, size| {
                    fill(says("cut"), &[("given", kept), ("size", size)])
                }),
                None => result.trim_end().to_string(),
            };
            fill(
                says("call"),
                &[("n", &(i + 1).to_string()), ("request", input), ("result", &result)],
            )
        })
        .collect()
}

/// The slots a listed test fills its line with.
fn test_slots(t: &Listed) -> Vec<(&'static str, String)> {
    let (before, line) = t.before.unzip();
    vec![
        ("name", t.name.clone()),
        ("unit", t.unit.clone()),
        ("millis", t.millis.to_string()),
        ("readings", t.readings.to_string()),
        ("share", t.share.map(|s| s.to_string()).unwrap_or_default()),
        ("before", before.map(|b| b.to_string()).unwrap_or_default()),
        ("line", line.map(|l| l.to_string()).unwrap_or_default()),
    ]
}

fn slots_ref<'a>(slots: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    slots.iter().map(|(name, value)| (*name, value.as_str())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use decide::Seq;
    use work::Timed;
    use work::timed::{Job, Reading, Test};

    fn merge(work: u64, tests: &[(&str, u64)], jobs: &[(&str, u64)]) -> Merge {
        Merge {
            work: WorkNumber(work),
            time: Some(Time(work)),
            jobs: jobs.iter().map(|(n, m)| (n.to_string(), *m)).collect(),
            tests: (tests.iter())
                .map(|(n, m)| (("unit".to_string(), n.to_string()), *m))
                .collect(),
        }
    }

    // The count a history of n values needs is the least whose chance is under one in a thousand, and none at all where
    // even every value above would not be so unlikely.
    #[test]
    fn the_count_needed_is_the_least_that_chance_makes_unlikely_and_none_for_a_short_history() {
        assert_eq!(
            needed(19),
            17,
            "191 of 524288 is under a thousandth, 1160 of 524288 is not"
        );
        assert_eq!(needed(9), 10, "even all 9 above has a chance of 1 in 512");
        assert_eq!(needed(10), 10, "all 10 above has a chance under 1 in 1000");
    }

    #[test]
    fn a_tests_stop_is_the_line_of_every_tests_time_in_the_latest_checks_and_none_for_a_short_history() {
        let checks: Vec<Merge> = (1..=4)
            .map(|n| merge(n, &[("a", 1_000), ("b", 3_000), ("c", 2_000 * n), ("d", 5_000)], &[]))
            .collect();
        // The latest three checks: the values 1, 3, 4, 5, 1, 3, 6, 5, 1, 3, 8, 5 seconds: largest 8, median 3.5.
        assert_eq!(test_time(&checks, 3), Some(12_500.0));
        assert_eq!(test_time(&checks[..2], 3), None, "fewer checks than values asked for");
        assert_eq!(test_time(&checks, 0), None, "no number of values to judge by");
        assert_eq!(test_time(&[merge(1, &[], &[])], 1), None, "no test's time read");
    }

    // A series with a step is reported with the merge after which it stepped; the same noise with no step is not; and
    // fewer than 2 n values report nothing.
    #[test]
    fn a_series_with_a_step_names_its_merge_and_one_without_is_not_reported() {
        let n = 10;
        // Noise of +-20 around 100, then a step to 160 at the 21st merge.
        let noise = |i: u64| 100 + (i * 7 % 41) as i64 - 20;
        let stepped: Vec<Merge> = (1..=30)
            .map(|w| {
                let base = noise(w) + if w >= 21 { 60 } else { 0 };
                merge(w, &[], &[("test", base as u64)])
            })
            .collect();
        let found = growth(&stepped, n);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!((found[0].of.as_str(), found[0].work), ("test", WorkNumber(21)));
        let flat: Vec<Merge> = (1..=30).map(|w| merge(w, &[], &[("test", noise(w) as u64)])).collect();
        assert!(growth(&flat, n).is_empty(), "no step, no report");
        assert!(
            growth(&stepped[..19], n).is_empty(),
            "fewer than 2 n values report nothing"
        );
    }

    // A test is slowed only where it is past its own line in the latest merged check and in the one before, each by its
    // own earlier times; past in one alone, it is not.
    #[test]
    fn a_test_is_slowed_only_where_it_is_past_its_line_in_two_checks_in_a_row() {
        let history = |last: u64, before: u64| -> Vec<Merge> {
            let mut h: Vec<Merge> = (1..=12).map(|w| merge(w, &[("t", 100 + w % 3)], &[])).collect();
            h.push(merge(13, &[("t", before)], &[]));
            h.push(merge(14, &[("t", last)], &[]));
            h
        };
        assert_eq!(slowed(&history(300, 300), 10).len(), 1);
        assert!(slowed(&history(300, 100), 10).is_empty(), "past it once");
        assert!(slowed(&history(100, 300), 10).is_empty(), "past it before alone");
        assert!(slowed(&history(300, 300), 13).is_empty(), "fewer than the samples");
    }

    // What a merge added is the tests the check before it did not have, with what they take.
    #[test]
    fn the_series_gives_what_each_merge_added_to_the_check_before_it() {
        let h = vec![
            merge(1, &[("a", 10)], &[("test 1", 1000)]),
            merge(2, &[("a", 10), ("b", 30), ("c", 5)], &[("test 1", 2000)]),
        ];
        let rows = series(&h, 20);
        assert_eq!(
            (rows[0].added, rows[0].added_millis),
            (0, 0),
            "the first has nothing before it"
        );
        assert_eq!((rows[1].added, rows[1].added_millis, rows[1].tests_millis), (2, 35, 45));
        assert_eq!(rows[1].jobs, vec![("test 1".to_string(), 2000)]);
    }

    fn reading(source: Source, tests: &[(&str, u64)]) -> Timed {
        Timed {
            at: Seq(1),
            reading: Reading {
                asking: "call/1".to_string(),
                source,
                commit: "c".to_string(),
                workflow: "w.yml".to_string(),
                jobs: vec![Job {
                    name: "test".to_string(),
                    millis: 1,
                }],
                tests: (tests.iter())
                    .map(|(n, m)| Test {
                        unit: "unit".to_string(),
                        name: n.to_string(),
                        passed: true,
                        millis: *m,
                    })
                    .collect(),
            },
        }
    }

    /// A run the host read at a place of the work's records: of this asking, on this commit, of this workflow, with the
    /// time of each job and of each test, whether it passed.
    fn ran(
        at: u64,
        asking: &str,
        commit: &str,
        source: Source,
        workflow: &str,
        jobs: &[(&str, u64)],
        tests: &[(&str, bool, u64)],
    ) -> Timed {
        Timed {
            at: Seq(at),
            reading: Reading {
                asking: asking.to_string(),
                source,
                commit: commit.to_string(),
                workflow: workflow.to_string(),
                jobs: (jobs.iter())
                    .map(|(name, millis)| Job {
                        name: name.to_string(),
                        millis: *millis,
                    })
                    .collect(),
                tests: (tests.iter())
                    .map(|(name, passed, millis)| Test {
                        unit: "unit".to_string(),
                        name: name.to_string(),
                        passed: *passed,
                        millis: *millis,
                    })
                    .collect(),
            },
        }
    }

    // The check of a merged work is every run of the full workflows its latest check asked for, one for each workflow,
    // and not an earlier check that failed nor a run of the fast workflow: a job goes by its name under its workflow, a
    // test that failed is left out, and a test two workflows ran is at its slowest.
    #[test]
    fn a_merged_works_check_is_the_runs_of_every_full_workflow_of_its_latest_check() {
        let key = |name: &str| ("unit".to_string(), name.to_string());
        let mut work = Work::default();
        // The first check failed: it is no part of the merged check.
        work.timed.push(ran(
            1,
            "run/1/round/1",
            "a",
            Source::Full,
            "ci.yml",
            &[("test", 100)],
            &[("t1", true, 10), ("t2", false, 5)],
        ));
        // A fast run of the same asking, which reran what failed.
        work.timed.push(ran(
            2,
            "run/1/round/1",
            "a",
            Source::Fast,
            "fast.yml",
            &[],
            &[("t3", true, 1)],
        ));
        // The check that merged the work ran two workflows, each with a job named `test`.
        work.timed.push(ran(
            3,
            "run/1/round/2",
            "b",
            Source::Full,
            "ci.yml",
            &[("test", 120), ("lint", 30)],
            &[("t1", true, 12), ("t2", true, 7)],
        ));
        work.timed.push(ran(
            4,
            "run/1/round/2",
            "b",
            Source::Full,
            "slow.yml",
            &[("test", 200)],
            &[("t2", true, 9), ("t9", true, 50), ("t8", false, 1)],
        ));
        let check = Merge::of(WorkNumber(5), &work).expect("the work has a full check");
        assert_eq!(
            check.jobs.iter().map(|(n, m)| (n.as_str(), *m)).collect::<Vec<_>>(),
            [("ci.yml:lint", 30), ("ci.yml:test", 120), ("slow.yml:test", 200)],
            "both workflows' jobs, none of the first check's"
        );
        assert_eq!(
            check.tests,
            BTreeMap::from([(key("t1"), 12), (key("t2"), 9), (key("t9"), 50)]),
            "the tests of both workflows that passed, t2 at its slowest"
        );
        assert_eq!(
            Merge::of(WorkNumber(6), &Work::default()),
            None,
            "a work with no run has no check"
        );
        let mut fast = Work::default();
        fast.timed
            .push(ran(1, "call/1", "a", Source::Fast, "fast.yml", &[], &[("t1", true, 1)]));
        assert_eq!(Merge::of(WorkNumber(7), &fast), None, "a fast run is no check");
    }

    fn words() -> Texts {
        Texts(
            WORDS
                .iter()
                .map(|w| {
                    (
                        w.to_string(),
                        format!("<{w}:{{name}}:{{millis}}:{{readings}}:{{share}}:{{left}}:{{checks}}>"),
                    )
                })
                .collect(),
        )
    }

    // A host that gives no time says so, and a work whose runs reported no test says that; neither reads as no new test.
    #[test]
    fn a_host_that_gives_no_time_and_a_work_with_no_reading_are_said_apart() {
        let mut work = Work::default();
        assert_eq!(facts(&work, &[], 19), Facts::NoneRead);
        work.timed.push(reading(Source::Command, &[]));
        assert_eq!(facts(&work, &[], 19), Facts::NoTimes);
    }

    // A test no merged check has is new, with the median of the work's readings and their number, and where it stands
    // among the project's tests; one a merged check has and that took longer than its own line is slowed; the history
    // short of the samples makes no comparison and says so.
    #[test]
    fn new_tests_are_listed_with_their_median_and_where_they_stand_and_a_short_history_compares_nothing() {
        let history: Vec<Merge> = (1..=3)
            .map(|w| merge(w, &[("old", 100), ("quick", 10), ("quicker", 5), ("slow", 900)], &[]))
            .collect();
        let mut work = Work::default();
        work.timed.push(reading(Source::Fast, &[("fresh", 500), ("old", 100)]));
        work.timed.push(reading(Source::Fast, &[("fresh", 700), ("old", 100)]));
        let Facts::Read { new, slowed, suite, .. } = facts(&work, &history, 3) else {
            panic!("the work's runs reported tests");
        };
        assert_eq!(slowed, Vec::new());
        assert_eq!(new.len(), 1);
        assert_eq!(
            (new[0].name.as_str(), new[0].millis, new[0].readings),
            ("fresh", 600, 2)
        );
        assert_eq!(new[0].share, Some(75), "slower than 3 of the 4 tests");
        assert_eq!(suite, Some(1015));
        let short = facts(&work, &history[..2], 3);
        let Facts::Read { new, suite, .. } = &short else {
            panic!("the work's runs reported tests");
        };
        assert_eq!((new.len(), *suite), (2, None), "every test is listed, none compared");
        let said = render(&short, &words());
        assert!(said.contains("<not_compared:"), "it says no comparison is made: {said}");
        assert!(!said.contains("<sum:"), "no share of the suite is told: {said}");
    }

    // The calls of `run_tests` are written in the order their results were recorded, not the order they were asked in,
    // each result as the tool's bound gives it (cut at a whole character, with the datum's own words, and whole where
    // there is no bound) and with no place in it filled again; a call of another tool, and a call with no result yet,
    // are none of them.
    #[test]
    fn the_calls_of_the_tests_are_given_in_the_order_they_ended_cut_at_the_bound_and_no_other_call_is() {
        use decide::{CallId, Done};
        let tool = |name: &str, input: &str| {
            Call::Asked(Ask::Tool {
                name: name.to_string(),
                input: input.to_string(),
            })
        };
        let mut work = Work::default();
        work.calls.insert(CallId(4), tool(RUN_TESTS, "first asked"));
        work.calls.insert(CallId(5), tool(RUN_TESTS, "second asked"));
        work.calls.insert(CallId(6), tool("query", "other"));
        work.calls.insert(CallId(7), tool(RUN_TESTS, "not ended"));
        // Nine bytes, then characters of two bytes each: the bound of ten bytes falls inside the first of them.
        let long = format!("{}{}", "x".repeat(9), "é".repeat(5));
        for (n, (call, result)) in [(5, "ended first {n}\n"), (6, "q"), (4, long.as_str())]
            .into_iter()
            .enumerate()
        {
            work.records.done.push(Done {
                call: CallId(call),
                at: Seq(n as u64 + 1),
            });
            work.results.insert(CallId(call), result.to_string());
        }
        let texts = Texts(vec![
            ("call".to_string(), "#{n} {request}: {result}".to_string()),
            ("cut".to_string(), "(cut: {given} of {size})".to_string()),
        ]);
        let given = ResultGiven {
            bound: 10,
            cut: "(cut: {given} of {size}, in {file})".to_string(),
        };
        assert_eq!(
            runs(&work, &texts, Some(&given)),
            [
                "#1 second asked: ended first {n}".to_string(),
                "#2 first asked: xxxxxxxxx\n\n(cut: 9 of 19)".to_string(),
            ],
            "a result longer than the bound is cut before the character the bound falls in, with no file named, and a \
             shorter one is whole"
        );
        assert_eq!(
            runs(&work, &texts, None),
            [
                "#1 second asked: ended first {n}".to_string(),
                format!("#2 first asked: {long}"),
            ],
            "with no bound a result is given whole"
        );
    }
}
