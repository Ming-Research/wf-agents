//! Whether the main branch of a project is red on a test, worked out from the works' records whenever it is read and
//! stored nowhere (principle `state-from-records`).
//!
//! The check before a merge runs the tests that failed the full workflows again, on the commit and, where they still
//! fail there, on the head of main. Its runs are kept as readings of the work (module [`work::timed`]), all under the
//! one asking of the round of the check: the full workflows on the commit checked, the fast workflow on that commit,
//! and the fast workflow on the head of main. So a reading is a run on the head of main where its asking is a round of
//! the check (`run/...`), its source is the fast workflow, and its commit is not one the full workflows of the same
//! asking ran on. A call of `run_tests` (`call/...`) and the check's own runs on the commit checked are not.
//!
//! A test is red on main of a project while the latest reading of it on the current head of main, in any work of the
//! project, failed. What the readings say of the test is its spans of failure, each from a failure to the pass that
//! ended it. Main has been red since the start of the run of such spans, of all the tests, that overlap or touch, up to
//! the spans still open: one test passing while another goes on failing leaves main red, and a new red is one that
//! begins after every span had ended. A reading is on the current head where its commit, the whole id, is in the latest
//! version of the block of main of a work that has not ended: the engine writes that block into each of them each time
//! the head changes ([`crate::Driver::outside`]) with the head in its words, and every version of the words holds it.
//! The comparison is of heads and not of times, so a reading kept after main moved on, of a run on the head it left, is
//! not a reading of the head main has, and a move of main is a move whatever else happened to the work as it moved.
//! Where the latest reading of a test failed on a head that main has left, what the project's main is is not known
//! until a check runs the test on the new head, and it is not said to be red or green; a red that comes back so is the
//! same one, since the same time. The same holds where no work of the project has not ended, and so nothing tells which
//! head main has.
//!
//! The works of a project that wait for main to pass are given beside it, as what the planner may look at first; main
//! is red whether any work waits or not.

use std::collections::{BTreeMap, BTreeSet};

use decide::{BlockId, Decision, Seq, Waited, decide};
use store::WorkNumber;
use work::timed::Source;
use work::{Time, Work};

use crate::{Driver, MAIN, Standing};

/// A test that fails on the head of main: the unit it is in, its name, and the head of main it failed on, as the latest
/// reading of it has it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RedTest {
    pub unit: String,
    pub name: String,
    pub head: String,
}

/// The main branch of a project is red.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Red {
    pub project: String,
    /// Since when main has been red without every failing test having passed between: the start of the run of spans in
    /// which a test failed, overlapping or touching, that the spans still open end, so that a test recovering while
    /// another goes on failing is no new red.
    pub since: Time,
    /// The tests that are red now, in the order of their units and names.
    pub tests: Vec<RedTest>,
    /// The works of the project that have not ended and wait for main to pass, in the order of their numbers.
    pub waiting: Vec<WorkNumber>,
}

/// A work of a project, as far as whether main is red is read from it.
pub struct Member<'a> {
    pub number: WorkNumber,
    pub work: &'a Work,
    /// The block of main of the flow the work is on, where it has one.
    pub main: Option<&'a BlockId>,
    /// Whether the work has ended, so that the engine writes it nothing more.
    pub ended: bool,
}

/// What one reading of a test on the head of main said.
struct Seen {
    time: Time,
    /// The place of the entry that kept it, which orders the readings kept in one moment.
    place: Seq,
    work: WorkNumber,
    passed: bool,
    /// Whether the commit is the head of main that the works have been told last.
    current: bool,
    head: String,
}

/// A reading of a run on the head of main, with the time and the place of the entry that kept it.
type OnMain<'a> = (Time, Seq, &'a work::timed::Reading);

/// Each reading of the work that ran on the head of main.
fn on_main(work: &Work) -> Vec<OnMain<'_>> {
    let mut asked: BTreeMap<&str, Vec<&work::timed::Timed>> = BTreeMap::new();
    for timed in &work.timed {
        if timed.reading.asking.starts_with("run/") {
            asked.entry(timed.reading.asking.as_str()).or_default().push(timed);
        }
    }
    let mut found = Vec::new();
    for readings in asked.values() {
        let checked: BTreeSet<&str> = (readings.iter())
            .filter(|t| t.reading.source == Source::Full)
            .map(|t| t.reading.commit.as_str())
            .collect();
        if checked.is_empty() {
            continue;
        }
        for timed in readings {
            let reading = &timed.reading;
            if reading.source == Source::Fast && !checked.contains(reading.commit.as_str()) {
                found.push((work.time_of(timed.at).unwrap_or_default(), timed.at, reading));
            }
        }
    }
    found
}

/// The latest version of the block of main of a work, which is where the head of main it was last told stands.
fn latest_main<'a>(member: &Member<'a>) -> Option<&'a str> {
    let block = member.main?;
    let (_, text) = (member.work.contents)
        .range((block.clone(), Seq(0))..=(block.clone(), Seq(u64::MAX)))
        .next_back()?;
    Some(text.as_str())
}

/// When the red that goes on now began, from the spans of time in which a test failed, each from its first failure to
/// the pass that ended it, or open where it has not: the start of the run of spans that overlap or touch and that the
/// open ones end. A test that recovers while another goes on failing so leaves the start where it was, and main having
/// been green between two spans, because every span before it had ended, makes the later one a start of its own.
fn began(mut spans: Vec<(Time, Option<Time>)>) -> Time {
    spans.sort_by_key(|(from, _)| *from);
    let mut chain: Option<(Time, Option<Time>)> = None;
    for (from, until) in spans {
        chain = Some(match chain {
            Some((start, end)) if end.is_none_or(|end| from <= end) => {
                let ends = match (end, until) {
                    (Some(end), Some(until)) => Some(end.max(until)),
                    _ => None,
                };
                (start, ends)
            }
            _ => (from, until),
        });
    }
    chain.map(|(start, _)| start).unwrap_or_default()
}

/// Where main is red in the project, if it is: of its works, with how each waits given by `waiting`. Where no work of
/// it has not ended, nothing tells what the head of main is: no test is red then, and one that last failed is not known
/// to be red or green.
pub fn of_project(project: &str, members: &[Member<'_>], waiting: impl Fn(WorkNumber) -> bool) -> Option<Red> {
    let going: Vec<&Member<'_>> = members.iter().filter(|m| !m.ended && m.main.is_some()).collect();
    // The works of a project are mostly told one head, so the readings are matched against the distinct words.
    let heads: BTreeSet<&str> = going.iter().filter_map(|m| latest_main(m)).collect();
    let current = |commit: &str| !commit.is_empty() && heads.iter().any(|head| head.contains(commit));
    let mut seen: BTreeMap<(&str, &str), Vec<Seen>> = BTreeMap::new();
    let readings: Vec<(WorkNumber, Vec<OnMain<'_>>)> = members.iter().map(|m| (m.number, on_main(m.work))).collect();
    for (work, readings) in &readings {
        for (time, place, reading) in readings {
            let on_head = current(&reading.commit);
            for test in &reading.tests {
                seen.entry((test.unit.as_str(), test.name.as_str()))
                    .or_default()
                    .push(Seen {
                        time: *time,
                        place: *place,
                        work: *work,
                        passed: test.passed,
                        current: on_head,
                        head: reading.commit.clone(),
                    });
            }
        }
    }
    let mut tests = Vec::new();
    let mut spans: Vec<(Time, Option<Time>)> = Vec::new();
    for ((unit, name), mut list) in seen {
        list.sort_by_key(|s| (s.time, s.work, s.place));
        // Of the readings on the head main has, the latest decides: a test that passed there is not red. Where none is
        // on it, the test last failed on a head main has left, which is not known to be red, or last passed.
        let Some(at) = list.iter().rposition(|s| s.current) else {
            continue;
        };
        // The spans the test failed in up to that reading: each ended by the pass after it, the last open where the
        // reading failed, which is the one that makes the test red.
        let mut failing: Option<Time> = None;
        for reading in &list[..=at] {
            match (reading.passed, failing) {
                (false, None) => failing = Some(reading.time),
                (true, Some(from)) => {
                    spans.push((from, Some(reading.time)));
                    failing = None;
                }
                _ => {}
            }
        }
        if let Some(from) = failing {
            spans.push((from, None));
            tests.push(RedTest {
                unit: unit.to_string(),
                name: name.to_string(),
                head: list[at].head.clone(),
            });
        }
    }
    if tests.is_empty() {
        return None;
    }
    let waiting = (going.iter())
        .map(|m| m.number)
        .filter(|number| waiting(*number))
        .collect();
    Some(Red {
        project: project.to_string(),
        since: began(spans),
        tests,
        waiting,
    })
}

impl Driver {
    /// Each project whose main is red, in the order of the projects' names.
    pub fn red_main(&self) -> Vec<Red> {
        self.red_in(None)
    }

    /// The tests that are red on the main of the project the work's item belongs to; none where it belongs to none, or
    /// where its main is not known to be red.
    pub fn red_tests(&self, work: WorkNumber) -> Vec<RedTest> {
        let Some(project) = (self.works.get(&work)).and_then(|held| held.item.project.as_deref()) else {
            return Vec::new();
        };
        (self.red_in(Some(project)).into_iter())
            .flat_map(|red| red.tests)
            .collect()
    }

    /// The projects whose main is red, only the project named where one is.
    fn red_in(&self, only: Option<&str>) -> Vec<Red> {
        let mut by_project: BTreeMap<&str, Vec<Member<'_>>> = BTreeMap::new();
        for held in self.shown() {
            let Some(project) = held
                .item
                .project
                .as_deref()
                .filter(|p| only.is_none_or(|only| only == *p))
            else {
                continue;
            };
            let main = (held.on.outline()).and_then(|outline| {
                (outline.from.iter())
                    .find(|(_, source)| source.as_str() == MAIN)
                    .map(|(block, _)| block)
            });
            by_project.entry(project).or_default().push(Member {
                number: held.head.number,
                work: &held.work,
                main,
                ended: held.standing != Standing::Going,
            });
        }
        (by_project.into_iter())
            .filter_map(|(project, members)| of_project(project, &members, |number| self.waits_for_main(number)))
            .collect()
    }

    /// Whether the work waits for the head of main to change, as the check before the merge found a test it failed to
    /// fail on main as well.
    fn waits_for_main(&self, number: WorkNumber) -> bool {
        let Some(held) = self.works.get(&number) else {
            return false;
        };
        match decide(&held.flow.flow, &held.work.records) {
            Decision::Wait { on, .. } => {
                (on.iter()).any(|waited| matches!(waited, Waited::Outside { source, .. } if source == MAIN))
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use work::timed::{Reading, Test, Timed};
    use work::{Moment, OnFlow};

    use super::*;

    const MAIN_BLOCK: &str = "main";

    fn reading(asking: &str, source: Source, commit: &str, tests: &[(&str, bool)]) -> Reading {
        Reading {
            asking: asking.to_string(),
            source,
            commit: commit.to_string(),
            workflow: "ci.yml".to_string(),
            jobs: Vec::new(),
            tests: (tests.iter())
                .map(|(name, passed)| Test {
                    unit: "unit".to_string(),
                    name: name.to_string(),
                    passed: *passed,
                    millis: 10,
                })
                .collect(),
        }
    }

    /// A work whose entries are at the places `1..`, each a moment of its own, the place n at the time 100 * n.
    fn work_of(places: u64) -> Work {
        Work {
            moments: (1..=places)
                .map(|n| Moment {
                    from: Seq(n),
                    at: Time(100 * n),
                })
                .collect(),
            ..Work::default()
        }
    }

    /// A check at the places `at`: the full workflows on `commit`, the fast workflow on it, and on main's head.
    fn check(
        work: &mut Work,
        at: u64,
        ask: &str,
        commit: &str,
        main: &str,
        on_commit: &[(&str, bool)],
        on_main: &[(&str, bool)],
    ) {
        let asking = format!("run/{ask}/round/1");
        for (n, reading) in [
            reading(&asking, Source::Full, commit, on_commit),
            reading(&asking, Source::Fast, commit, on_commit),
            reading(&asking, Source::Fast, main, on_main),
        ]
        .into_iter()
        .enumerate()
        {
            work.timed.push(Timed {
                at: Seq(at + n as u64),
                reading,
            });
        }
    }

    fn says(work: &mut Work, at: u64, text: &str) {
        work.contents
            .insert((BlockId(MAIN_BLOCK.to_string()), Seq(at)), text.to_string());
    }

    fn red_of(members: &[Member<'_>]) -> Option<Red> {
        of_project("project", members, |_| false)
    }

    fn member<'a>(number: u64, work: &'a Work, block: &'a BlockId, ended: bool) -> Member<'a> {
        Member {
            number: WorkNumber(number),
            work,
            main: Some(block),
            ended,
        }
    }

    // A check whose run on the head of main failed makes the test red; the run on the commit is no run on main.
    #[test]
    fn a_test_that_failed_on_the_head_of_main_is_red() {
        let block = BlockId(MAIN_BLOCK.to_string());
        let mut work = work_of(20);
        says(&mut work, 1, "main at A");
        check(
            &mut work,
            5,
            "1",
            "C",
            "A",
            &[("t", false), ("u", false)],
            &[("t", false), ("u", true)],
        );
        let red = red_of(&[member(1, &work, &block, false)]).expect("main is red on `t`");
        assert_eq!(
            red.tests,
            [RedTest {
                unit: "unit".to_string(),
                name: "t".to_string(),
                head: "A".to_string()
            }]
        );
        assert_eq!(red.since, Time(700), "the reading on main is the third of the check");
    }

    // What failed on the commit and was not run on main says nothing of main.
    #[test]
    fn a_failure_on_the_commit_alone_is_no_red_main() {
        let block = BlockId(MAIN_BLOCK.to_string());
        let mut work = work_of(20);
        says(&mut work, 1, "main at A");
        check(&mut work, 5, "1", "C", "A", &[("t", false)], &[("t", true)]);
        assert_eq!(red_of(&[member(1, &work, &block, false)]), None);
    }

    // A call of `run_tests` is asked for by a call, and what it ran on is no head of main, whatever the commit is.
    #[test]
    fn a_call_of_run_tests_is_no_run_on_main() {
        let block = BlockId(MAIN_BLOCK.to_string());
        let mut work = work_of(20);
        says(&mut work, 1, "main at A");
        work.timed.push(Timed {
            at: Seq(5),
            reading: reading("call/3", Source::Fast, "A", &[("t", false)]),
        });
        assert_eq!(red_of(&[member(1, &work, &block, false)]), None);
    }

    // A red main stands though the work that found it was cancelled, and the work that waits for it was paused.
    #[test]
    fn the_fact_does_not_depend_on_the_work_that_found_it() {
        let block = BlockId(MAIN_BLOCK.to_string());
        let mut found = work_of(20);
        says(&mut found, 1, "main at A");
        check(&mut found, 5, "1", "C", "A", &[("t", false)], &[("t", false)]);
        let mut other = work_of(20);
        says(&mut other, 2, "main at A");
        let red = red_of(&[member(1, &found, &block, true), member(2, &other, &block, false)]);
        assert_eq!(red.map(|r| r.tests.len()), Some(1));
    }

    // Where every work of the project has ended, nothing tells whether main moved, and none is said red.
    #[test]
    fn a_project_with_no_work_going_has_no_red() {
        let block = BlockId(MAIN_BLOCK.to_string());
        let mut work = work_of(20);
        says(&mut work, 1, "main at A");
        check(&mut work, 5, "1", "C", "A", &[("t", false)], &[("t", false)]);
        assert_eq!(red_of(&[member(1, &work, &block, true)]), None);
    }

    // A later run of the test on main that passed ends the red, and one that failed again goes on with the same streak.
    #[test]
    fn a_pass_on_main_ends_it_and_a_failure_goes_on_with_the_streak() {
        let block = BlockId(MAIN_BLOCK.to_string());
        let mut work = work_of(40);
        says(&mut work, 1, "main at A");
        check(&mut work, 5, "1", "C", "A", &[("t", false)], &[("t", false)]);
        says(&mut work, 10, "main at B");
        check(&mut work, 15, "2", "D", "B", &[("t", false)], &[("t", false)]);
        let red = red_of(&[member(1, &work, &block, false)]).expect("still red");
        assert_eq!(red.since, Time(700), "the streak began with the first failure");
        assert_eq!(red.tests[0].head, "B");
        says(&mut work, 20, "main at E");
        check(&mut work, 25, "3", "F", "E", &[("t", false)], &[("t", true)]);
        assert_eq!(red_of(&[member(1, &work, &block, false)]), None);
    }

    // Once main moved and no run on it came after, it is not known red: it is not said to be.
    #[test]
    fn once_main_moved_it_is_not_known_to_be_red() {
        let block = BlockId(MAIN_BLOCK.to_string());
        let mut work = work_of(40);
        says(&mut work, 1, "main at A");
        check(&mut work, 5, "1", "C", "A", &[("t", false)], &[("t", false)]);
        says(&mut work, 10, "main at B");
        assert_eq!(red_of(&[member(1, &work, &block, false)]), None);
        // A run on the new head that passes says where main stands: it is not red.
        check(&mut work, 15, "2", "D", "B", &[("t", false)], &[("t", true)]);
        assert_eq!(red_of(&[member(1, &work, &block, false)]), None);
    }

    // A work added after main went red is told the head of main as its first version, which is the head the readings ran on.
    #[test]
    fn a_work_added_later_is_told_the_same_head_and_main_stays_red() {
        let block = BlockId(MAIN_BLOCK.to_string());
        let mut found = work_of(40);
        says(&mut found, 1, "main at A");
        check(&mut found, 5, "1", "C", "A", &[("t", false)], &[("t", false)]);
        let mut added = work_of(40);
        says(&mut added, 9, "main at A");
        let both = [member(1, &found, &block, false), member(2, &added, &block, false)];
        assert!(red_of(&both).is_some());
    }

    // Words of the block of main that another version of the flow gives hold the same head, so main stands.
    #[test]
    fn a_change_of_the_flows_words_is_no_move() {
        let block = BlockId(MAIN_BLOCK.to_string());
        let mut work = work_of(40);
        says(&mut work, 1, "main at A");
        check(&mut work, 5, "1", "C", "A", &[("t", false)], &[("t", false)]);
        work.flows.push(OnFlow {
            at: Seq(10),
            version: Some("v2".to_string()),
        });
        says(&mut work, 11, "The head of main is A.");
        assert!(red_of(&[member(1, &work, &block, false)]).is_some());
        // A real move after it is one.
        says(&mut work, 20, "The head of main is B.");
        assert_eq!(red_of(&[member(1, &work, &block, false)]), None);
    }

    // Main moving on while the work is put on another version of its flow is a move: the head the readings ran on is not
    // the head the work was told, whatever words say it.
    #[test]
    fn a_move_together_with_a_change_of_the_flow_is_a_move() {
        let block = BlockId(MAIN_BLOCK.to_string());
        let mut work = work_of(40);
        says(&mut work, 1, "main at A");
        check(&mut work, 5, "1", "C", "A", &[("t", false)], &[("t", false)]);
        work.flows.push(OnFlow {
            at: Seq(10),
            version: Some("v2".to_string()),
        });
        says(&mut work, 11, "The head of main is B.");
        assert_eq!(
            red_of(&[member(1, &work, &block, false)]),
            None,
            "the red was read on a head main has left"
        );
    }

    // A reading kept after main moved on, of a run on the head main left, is not a reading of the head main has.
    #[test]
    fn a_failure_kept_after_main_moved_on_the_head_it_ran_on_is_not_red() {
        let block = BlockId(MAIN_BLOCK.to_string());
        let mut work = work_of(40);
        says(&mut work, 1, "main at A");
        says(&mut work, 5, "main at B");
        check(&mut work, 10, "1", "C", "A", &[("t", false)], &[("t", false)]);
        assert_eq!(red_of(&[member(1, &work, &block, false)]), None);
        // A run on the head main has says where it stands, and a late reading of the head it left does not hide that.
        check(&mut work, 15, "2", "D", "B", &[("t", false)], &[("t", false)]);
        check(&mut work, 20, "3", "E", "A", &[("t", false)], &[("t", true)]);
        let red = red_of(&[member(1, &work, &block, false)]).expect("red on the head main has");
        assert_eq!(red.tests[0].head, "B");
        assert_eq!(
            red.since,
            Time(1200),
            "the streak begins with the first reading on main, the one on the head main left, and ends with the one on the current head"
        );
    }

    // A test recovering while another goes on failing leaves main red since the start: not every failing test had passed.
    #[test]
    fn a_test_recovering_while_another_goes_on_failing_leaves_main_red_since_the_start() {
        let block = BlockId(MAIN_BLOCK.to_string());
        let mut work = work_of(60);
        says(&mut work, 1, "main at A");
        check(&mut work, 5, "1", "C", "A", &[("a", false)], &[("a", false)]);
        says(&mut work, 10, "main at B");
        check(
            &mut work,
            15,
            "2",
            "D",
            "B",
            &[("a", false), ("b", false)],
            &[("a", false), ("b", false)],
        );
        says(&mut work, 20, "main at E");
        check(
            &mut work,
            25,
            "3",
            "F",
            "E",
            &[("a", false), ("b", false)],
            &[("a", true), ("b", false)],
        );
        let red = red_of(&[member(1, &work, &block, false)]).expect("b is red");
        let names: Vec<&str> = red.tests.iter().map(|test| test.name.as_str()).collect();
        assert_eq!(names, ["b"], "a passed");
        assert_eq!(red.since, Time(700), "main was red from the first failure of a");
    }

    // Main green between two failures, every test that failed having passed, makes the later failure a red of its own.
    #[test]
    fn main_green_between_two_failures_makes_the_later_a_red_of_its_own() {
        let block = BlockId(MAIN_BLOCK.to_string());
        let mut work = work_of(60);
        says(&mut work, 1, "main at A");
        check(&mut work, 5, "1", "C", "A", &[("a", false)], &[("a", false)]);
        says(&mut work, 10, "main at B");
        check(&mut work, 15, "2", "D", "B", &[("a", false)], &[("a", true)]);
        says(&mut work, 20, "main at E");
        check(&mut work, 25, "3", "F", "E", &[("b", false)], &[("b", false)]);
        let red = red_of(&[member(1, &work, &block, false)]).expect("b is red");
        assert_eq!(red.since, Time(2700), "the red of b began after a had passed");
    }

    // Readings kept in one moment are in the order they were kept, not the order of the words of their askings.
    #[test]
    fn readings_kept_in_one_moment_are_in_the_order_they_were_kept() {
        let block = BlockId(MAIN_BLOCK.to_string());
        let mut work = Work {
            moments: vec![Moment {
                from: Seq(1),
                at: Time(100),
            }],
            ..Work::default()
        };
        says(&mut work, 1, "main at A");
        check(&mut work, 5, "9", "C", "A", &[("t", true)], &[("t", true)]);
        check(&mut work, 15, "10", "D", "A", &[("t", false)], &[("t", false)]);
        let red = red_of(&[member(1, &work, &block, false)]).expect("the later check failed");
        assert_eq!(red.since, Time(100));
    }

    // The works that wait are given beside it.
    #[test]
    fn the_works_that_wait_for_main_are_given() {
        let block = BlockId(MAIN_BLOCK.to_string());
        let mut work = work_of(20);
        says(&mut work, 1, "main at A");
        check(&mut work, 5, "1", "C", "A", &[("t", false)], &[("t", false)]);
        let red = of_project("project", &[member(7, &work, &block, false)], |n| n == WorkNumber(7))
            .red
            .unwrap();
        assert_eq!(red.waiting, [WorkNumber(7)]);
    }
}
