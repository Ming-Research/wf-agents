//! What a run of the project's workflows said of how long each of its tests and jobs took, as the work's records keep
//! it: each run the host read, at a place of its own in the order of what happened to the work. Later tests and later
//! checks are judged by what earlier ones took (principle `judged-by-own-history`), so the times are kept as the host
//! read them, never as the words an agent was told.

use decide::Seq;
use serde::{Deserialize, Serialize};

/// Which run of the project's CI a record is of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    /// A run of the fast workflow, which a call of `run_tests` asked for: it runs only the tests and checks it names.
    Fast,
    /// A run of the full workflows, which a round of the check before the merge asked for.
    Full,
    /// A host that runs the project's check command and reads no JUnit: it gives no time of any test, and says so.
    NoTimes,
}

impl Source {
    /// The name the store keeps it by.
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Fast => "fast",
            Source::Full => "full",
            Source::NoTimes => "no_times",
        }
    }

    /// The source a name the store keeps names; none for any other.
    pub fn named(name: &str) -> Option<Source> {
        [Source::Fast, Source::Full, Source::NoTimes]
            .into_iter()
            .find(|s| s.as_str() == name)
    }
}

/// How long a job of a run took, from its start to its end, as the host records it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub name: String,
    pub millis: u64,
}

/// How long a test took in a run, in the run's own results: the unit it is in (a test binary, a test file), its name,
/// whether it passed, and its time in milliseconds. A test the run did not run has no reading and is not here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Test {
    pub unit: String,
    pub name: String,
    pub passed: bool,
    pub millis: u64,
}

/// What one run of the project's CI said of time, before it has a place among the work's records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reading {
    /// What asked for the run, as the host names it (`call/<call>` or `run/<run>/round/<round>`): the same asking
    /// given again after the engine started again reads the same run, and is kept once.
    pub asking: String,
    pub source: Source,
    /// The commit the run ran on.
    pub commit: String,
    pub jobs: Vec<Job>,
    pub tests: Vec<Test>,
}

/// A [`Reading`] at its place in the order of what happened to the work.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timed {
    /// The place of the entry in the order of what happened to the work.
    pub at: Seq,
    pub reading: Reading,
}

impl Reading {
    /// The reading with its jobs in the order of their names and its tests in the order of their units and names, each
    /// once, the first reading of a name kept: the order the store gives them back in, so that what is kept is what was
    /// held.
    pub fn normalised(mut self) -> Reading {
        self.jobs.sort_by(|a, b| a.name.cmp(&b.name));
        self.jobs.dedup_by(|later, first| later.name == first.name);
        self.tests.sort_by(|a, b| (&a.unit, &a.name).cmp(&(&b.unit, &b.name)));
        self.tests
            .dedup_by(|later, first| later.unit == first.unit && later.name == first.name);
        self
    }

    /// Whether this is the same run as that other: the same asking of the same source on the same commit.
    pub fn same_run(&self, other: &Reading) -> bool {
        self.asking == other.asking && self.source == other.source && self.commit == other.commit
    }
}

/// How long a test took, from the `time` its results give, in seconds with a fraction: in milliseconds, to the nearest.
/// An empty time is none; a time that is no number of seconds, or is negative, is why it is none.
pub fn millis_of(seconds: &str) -> Result<Option<u64>, String> {
    let seconds = seconds.trim();
    if seconds.is_empty() {
        return Ok(None);
    }
    match seconds.parse::<f64>() {
        Ok(s) if s.is_finite() && s >= 0.0 => Ok(Some((s * 1000.0).round() as u64)),
        _ => Err(format!("{seconds:?} is no time in seconds")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A reading is kept in the order the store gives it back in, each name once, its first reading kept.
    #[test]
    fn a_reading_is_normalised_to_the_order_the_store_gives_back_with_each_name_once() {
        let test = |unit: &str, name: &str, millis| Test {
            unit: unit.to_string(),
            name: name.to_string(),
            passed: true,
            millis,
        };
        let reading = Reading {
            asking: "call/1".to_string(),
            source: Source::Fast,
            commit: "c".to_string(),
            jobs: vec![
                Job {
                    name: "b".to_string(),
                    millis: 2,
                },
                Job {
                    name: "a".to_string(),
                    millis: 1,
                },
            ],
            tests: vec![
                test("u2", "x", 5),
                test("u1", "y", 7),
                test("u2", "x", 9),
                test("u1", "a", 3),
            ],
        }
        .normalised();
        assert_eq!(
            reading.jobs.iter().map(|j| j.name.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(
            reading.tests,
            [test("u1", "a", 3), test("u1", "y", 7), test("u2", "x", 5)],
            "sorted by unit and name, the first reading of a name kept"
        );
    }

    // A time is read to the millisecond, and one that is no time is refused rather than read as nothing.
    #[test]
    fn a_time_in_seconds_is_read_to_the_millisecond_and_a_non_time_is_refused() {
        assert_eq!(millis_of("4.887"), Ok(Some(4887)));
        assert_eq!(millis_of("0"), Ok(Some(0)));
        assert_eq!(millis_of("0.0216"), Ok(Some(22)));
        assert_eq!(millis_of(" "), Ok(None));
        for bad in ["abc", "-1", "NaN", "inf"] {
            assert!(millis_of(bad).is_err(), "{bad} is no time");
        }
    }
}
