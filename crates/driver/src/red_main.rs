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
//! A test is red on main of a project while the latest reading of it on the head of main, in any work of the project,
//! failed, and the head of main has not moved since. What the readings say of the test is its streak: the readings
//! since the last that passed. That main moved is read from the block of main of the works that have not ended, which
//! the engine writes into each of them each time the head changes ([`crate::Driver::outside`]): a version of it that
//! says other than the version before it in the same work, with the work not put on another version of its flow
//! between the two, as the words of the block may differ from one version of a flow to another. Once main moved, what
//! the project's main is is not known until a check runs the test on it again, and it is not said to be red or green.
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
    /// Since when: the earliest start, of the streaks of failures of the tests that are red now.
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
    work: WorkNumber,
    passed: bool,
    head: String,
}

/// Each reading of the work that ran on the head of main, with the time it was kept at.
fn on_main(work: &Work) -> Vec<(Time, &work::timed::Reading)> {
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
                found.push((work.time_of(timed.at).unwrap_or_default(), reading));
            }
        }
    }
    found
}

/// The times main moved, as the block of main of a work that has not ended says it.
fn moves(member: &Member<'_>) -> Vec<Time> {
    let Some(block) = member.main else {
        return Vec::new();
    };
    let work = member.work;
    let mut says = work
        .contents
        .range((block.clone(), Seq(0))..=(block.clone(), Seq(u64::MAX)));
    let Some(((_, mut before), mut was)) = says.next().map(|((b, at), text)| ((b, *at), text)) else {
        return Vec::new();
    };
    let mut moved = Vec::new();
    for ((_, at), text) in says {
        let put_on_another = work.flows.iter().any(|on| on.at > before && on.at <= *at);
        if text != was && !put_on_another {
            moved.push(work.time_of(*at).unwrap_or_default());
        }
        (before, was) = (*at, text);
    }
    moved
}

/// Whether main is red in the project, and where: of its works, with how each waits given by `waiting`. None where no
/// work of it has not ended, since then nothing tells whether main moved, and where no test is red now.
pub fn of_project(project: &str, members: &[Member<'_>], waiting: impl Fn(WorkNumber) -> bool) -> Option<Red> {
    let going: Vec<&Member<'_>> = members.iter().filter(|m| !m.ended && m.main.is_some()).collect();
    if going.is_empty() {
        return None;
    }
    let moved = going.iter().flat_map(|m| moves(m)).max();
    let mut seen: BTreeMap<(&str, &str), Vec<Seen>> = BTreeMap::new();
    let readings: Vec<(WorkNumber, Vec<(Time, &work::timed::Reading)>)> =
        members.iter().map(|m| (m.number, on_main(m.work))).collect();
    for (work, readings) in &readings {
        for (time, reading) in readings {
            for test in &reading.tests {
                seen.entry((test.unit.as_str(), test.name.as_str()))
                    .or_default()
                    .push(Seen {
                        time: *time,
                        work: *work,
                        passed: test.passed,
                        head: reading.commit.clone(),
                    });
            }
        }
    }
    let mut tests = Vec::new();
    let mut since: Option<Time> = None;
    for ((unit, name), mut list) in seen {
        list.sort_by_key(|s| (s.time, s.work));
        let Some(last) = list.last() else {
            continue;
        };
        // A test that passed on the head of main last is not red; one whose last failure came before main moved is not
        // known to be, since main is not what it was.
        if last.passed || moved.is_some_and(|moved| last.time <= moved) {
            continue;
        }
        let from = list.iter().rposition(|s| s.passed).map_or(0, |passed| passed + 1);
        since = Some(since.map_or(list[from].time, |earliest| earliest.min(list[from].time)));
        tests.push(RedTest {
            unit: unit.to_string(),
            name: name.to_string(),
            head: last.head.clone(),
        });
    }
    let since = since?;
    let waiting = (going.iter())
        .map(|m| m.number)
        .filter(|number| waiting(*number))
        .collect();
    Some(Red {
        project: project.to_string(),
        since,
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
    fn a_project_with_no_work_going_says_nothing() {
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
    }

    // A work added after main went red is told the head of main as its first version, which is no move.
    #[test]
    fn the_first_version_of_a_work_is_no_move() {
        let block = BlockId(MAIN_BLOCK.to_string());
        let mut found = work_of(40);
        says(&mut found, 1, "main at A");
        check(&mut found, 5, "1", "C", "A", &[("t", false)], &[("t", false)]);
        let mut added = work_of(40);
        says(&mut added, 9, "main at A");
        let both = [member(1, &found, &block, false), member(2, &added, &block, false)];
        assert!(red_of(&both).is_some());
    }

    // Words of the block of main that another version of the flow gives are no move of main.
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

    // The works that wait are given beside it.
    #[test]
    fn the_works_that_wait_for_main_are_given() {
        let block = BlockId(MAIN_BLOCK.to_string());
        let mut work = work_of(20);
        says(&mut work, 1, "main at A");
        check(&mut work, 5, "1", "C", "A", &[("t", false)], &[("t", false)]);
        let red = of_project("project", &[member(7, &work, &block, false)], |n| n == WorkNumber(7)).unwrap();
        assert_eq!(red.waiting, [WorkNumber(7)]);
    }
}
