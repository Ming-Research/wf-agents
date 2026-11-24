//! Where a round or a call the engine carries out stands as it goes, as what carries it out says: the act of a check
//! waits for the project's turn, which another work holds; a tool or an act runs as a CI run, at its link. No record
//! holds it: it is what the outside says at a moment, kept only in memory while the round or the call is under way,
//! and said again by what carries it out once it is given again after a stop.

use decide::{CallId, Records, RunId};
use store::WorkNumber;

use crate::history::{Line, Lines};
use crate::judged::Against;

/// Where a round or a call stands as it goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Along {
    /// It waits for the project's turn to check and merge, which this work holds: the works of a project check and
    /// merge one at a time.
    Turn { holder: WorkNumber },
    /// It runs as the CI run at this link.
    Runs { at: String },
    /// An agent's round: what its calls to the model have come to so far.
    Going(Going),
}

/// A measure of a round under way, with the line the round is judged by where its history gives one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Figure {
    pub value: f64,
    pub line: Option<Line>,
}

impl Figure {
    /// Whether the measure is past its line: none where there is no line. Judged as [`Against::past`] judges it.
    pub fn past(&self) -> bool {
        self.line.is_some_and(|line| {
            Against {
                value: self.value,
                line,
            }
            .past()
        })
    }
}

/// What the calls of an agent's round to the model have come to so far, each measure none before a call tells it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Going {
    /// The whole prompt of the latest call, in tokens.
    pub context: Option<u64>,
    /// The tokens its context has grown by over the round, against the line of the run guard.
    pub added: Option<Figure>,
    /// The tokens its context has grown by for each call after the first, against the line of the run guard. Told from
    /// the second call.
    pub per_call: Option<Figure>,
    /// What the calls cost, in millionths of a dollar, priced from the calls, against the line of rounds on its model.
    pub cost: Option<Figure>,
}

impl Going {
    /// What `calls` calls to the model came to: the latest's whole prompt, the rises of the prompts over them and what
    /// they cost, against the lines the round is judged by. None where no call was made.
    pub fn of(lines: &Lines, calls: u32, context: Option<u64>, added: u64, usd_micros: u64) -> Going {
        if calls == 0 {
            return Going::default();
        }
        let figure = |value: f64, line: Option<Line>| Some(Figure { value, line });
        Going {
            context,
            added: figure(added as f64, lines.added),
            per_call: (calls >= 2).then(|| Figure {
                value: added as f64 / f64::from(calls - 1),
                line: lines.rate,
            }),
            cost: figure(usd_micros as f64, lines.cost),
        }
    }
}

/// What a carrier says where it stands of: a round of a run, by its number in the run, or a call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Under {
    Round { run: RunId, round: usize },
    Call(CallId),
}

impl Under {
    /// Whether it is under way by the work's records: the round began and has not ended, or the call was made and has
    /// no result.
    pub fn under_way(&self, records: &Records) -> bool {
        match *self {
            Under::Round { run, round } => {
                let began = records.rounds.iter().filter(|r| r.run == run).count();
                let ended = records.returned.iter().filter(|r| r.run == run).count();
                began >= round && ended < round
            }
            Under::Call(call) => {
                records.called.iter().any(|c| c.call == call) && !records.done.iter().any(|d| d.call == call)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::Scope;

    fn line(value: f64) -> Line {
        Line {
            value,
            samples: 12,
            scope: Scope::Cell,
        }
    }

    fn lines() -> Lines {
        Lines {
            added: Some(line(5_000.0)),
            rate: Some(line(1_500.0)),
            cost: Some(line(400_000.0)),
            ..Lines::default()
        }
    }

    // Three calls whose prompts were 10k, 14k and 12k: the context is the last prompt, the rises are 4k, and for each
    // call after the first 2k, which is past a line of 1.5k and not past one of 5k.
    #[test]
    fn three_calls_tell_the_last_prompt_the_rises_and_what_each_is_judged_against() {
        let going = Going::of(&lines(), 3, Some(12_000), 4_000, 250_000);
        assert_eq!(going.context, Some(12_000));
        let added = going.added.expect("added");
        assert_eq!((added.value, added.line.map(|l| l.value)), (4_000.0, Some(5_000.0)));
        assert!(!added.past(), "4k is not past a line of 5k");
        let per_call = going.per_call.expect("per call");
        assert_eq!(
            (per_call.value, per_call.line.map(|l| l.value)),
            (2_000.0, Some(1_500.0))
        );
        assert!(per_call.past(), "2k a call is past a line of 1.5k");
        let cost = going.cost.expect("cost");
        assert_eq!(cost.value, 250_000.0);
        assert!(!cost.past());
    }

    // The same growth against a line below it is past it; the run guard stops a round only where growth and growth for
    // each call are both past (`crates/carry/src/guard.rs`), so the two figures say that as the guard does.
    #[test]
    fn a_figure_is_past_exactly_where_it_is_above_its_own_line() {
        let low = Lines {
            added: Some(line(3_000.0)),
            rate: Some(line(1_500.0)),
            ..Lines::default()
        };
        let going = Going::of(&low, 3, Some(12_000), 4_000, 0);
        assert!(going.added.expect("added").past());
        assert!(going.per_call.expect("per call").past());
        let equal = Going::of(&low, 3, Some(12_000), 3_000, 0);
        assert!(!equal.added.expect("added").past(), "at its line is not past it");
    }

    // One call has no call after it, so there is no growth for each call; no call tells nothing at all.
    #[test]
    fn one_call_tells_no_growth_for_each_call_and_none_tells_nothing() {
        let one = Going::of(&lines(), 1, Some(9_000), 0, 10_000);
        assert_eq!(one.context, Some(9_000));
        assert!(one.per_call.is_none());
        assert_eq!(Going::of(&lines(), 0, None, 0, 0), Going::default());
    }

    // A line the history is too short to give is none, and its figure is never past.
    #[test]
    fn a_figure_with_no_line_is_not_past() {
        let going = Going::of(&Lines::default(), 3, Some(12_000), 4_000, 250_000);
        for figure in [going.added, going.per_call, going.cost] {
            let figure = figure.expect("a value");
            assert!(figure.line.is_none());
            assert!(!figure.past());
        }
    }
}
