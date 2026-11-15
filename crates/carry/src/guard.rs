//! The run guard: while an agent's round runs, what its calls to the model come to, and whether that is past what the
//! history of its cell explains (crate `driver`, module `history`). Each call costs a few comparisons. A round is
//! stopped on either of two signals, each of two conditions, so that a round that is long, and a message that is large,
//! both of them usual, are told from growth nothing explains:
//!
//! - growth: its context grew over the round by more than the line, and for each call after the first by more than the
//!   line;
//! - jump: its first prompt rose over its session's last by more than the line, and for each character of the message
//!   it was given by more than the line.
//!
//! A round the owner let run after the guard stopped its run is judged all the same, so that its record says it went
//! past the lines and it never joins the history, but it is not stopped: the owner looked, and let it run. What a
//! round's cost and time are past their lines is read from the records, and stops nothing: neither tells a long round
//! that is usual from one running away, which a rise of context for each call does.

use driver::history::{Line, Lines, Scope};
use flow::agents::GuardWords;
use flow::agents::fill;

use crate::cli::Calls;

/// How many of a round's last uses of tools the reason it was stopped names: enough to show what it was doing as its
/// context grew, few enough that the reason stays a line the owner reads.
pub const LAST_TOOLS: usize = 8;

/// A running round as the guard sees it.
#[derive(Clone, Debug)]
pub struct Guard {
    lines: Lines,
    /// The prompt its session last had before it; none for a round that begins its session.
    before: Option<u64>,
    /// The characters of its message.
    given: u64,
    /// Whether the owner let it run past the guard.
    waived: bool,
    /// How many calls it has judged.
    judged: u32,
}

/// A signal that stops a round, with its measures and lines.
#[derive(Clone, Debug, PartialEq)]
pub struct Crossing {
    /// `growth` or `jump`: the name of its words.
    pub signal: &'static str,
    pub value: f64,
    pub line: Line,
    pub per: f64,
    pub per_line: Line,
}

impl Guard {
    /// The guard of a round judged by these lines, whose session last had this prompt, given a message of this many
    /// characters, and let run past the guard where `waived`.
    pub fn new(lines: Lines, before: Option<u64>, given: u64, waived: bool) -> Guard {
        Guard {
            lines,
            before,
            given,
            waived,
            judged: 0,
        }
    }

    /// Whether a signal past its lines stops the round: not where the owner let it run.
    pub fn stops(&self) -> bool {
        !self.waived
    }

    /// Judges the calls the round has made so far, each once: the signal past its lines, the first time one is.
    pub fn judge(&mut self, calls: &Calls) -> Option<Crossing> {
        if calls.count == self.judged {
            return None;
        }
        let first = self.judged == 0;
        self.judged = calls.count;
        if first
            && let (Some((_, whole)), Some(before)) = (calls.first, self.before)
            && let (Some(line), Some(per_line)) = (self.lines.jump, self.lines.ratio)
            && self.given > 0
        {
            let value = whole.saturating_sub(before) as f64;
            let per = value / self.given as f64;
            if value > line.value && per > per_line.value {
                return Some(Crossing {
                    signal: "jump",
                    value,
                    line,
                    per,
                    per_line,
                });
            }
        }
        if calls.count >= 2
            && let (Some(line), Some(per_line)) = (self.lines.added, self.lines.rate)
        {
            let value = calls.added as f64;
            let per = value / f64::from(calls.count - 1);
            if value > line.value && per > per_line.value {
                return Some(Crossing {
                    signal: "growth",
                    value,
                    line,
                    per,
                    per_line,
                });
            }
        }
        None
    }
}

impl Crossing {
    /// Why the round was stopped, in the words beside the flows, with the last tools it used, each with what it was
    /// used on; `again` where the guard stopped the run's round before it too, in the new session its line was given.
    pub fn words(&self, words: &GuardWords, tools: &[(String, String, String)], again: bool) -> String {
        let said = match self.signal {
            "jump" => &words.jump,
            _ => &words.growth,
        };
        let whole = |v: f64| format!("{v:.0}");
        let per = |v: f64| match self.signal {
            "jump" => format!("{v:.1}"),
            _ => format!("{v:.0}"),
        };
        let scope = match self.line.scope {
            Scope::Cell => &words.scope.cell,
            Scope::Flow => &words.scope.flow,
            Scope::Works => &words.scope.works,
            Scope::Line => &words.scope.line,
        };
        let from = fill(
            &words.from,
            &[("samples", &self.line.samples.to_string()), ("scope", scope)],
        );
        let last = &tools[tools.len().saturating_sub(LAST_TOOLS)..];
        let named: Vec<String> = (last.iter())
            .map(|(_, tool, on)| fill(&words.tool, &[("tool", tool), ("on", on)]))
            .collect();
        let tools = match named.is_empty() {
            true => words.no_tools.clone(),
            false => named.join(&words.between),
        };
        let stopped = fill(
            said,
            &[
                ("value", &whole(self.value)),
                ("line", &whole(self.line.value)),
                ("per", &per(self.per)),
                ("per_line", &per(self.per_line.value)),
                ("from", &from),
                ("tools", &tools),
            ],
        );
        match again {
            true => fill(&words.again, &[("stopped", &stopped)]),
            false => stopped,
        }
    }
}

#[cfg(test)]
mod tests {
    use flow::agents::ScopeWords;

    use super::*;

    fn line(value: f64) -> Option<Line> {
        Some(Line {
            value,
            samples: 19,
            scope: Scope::Cell,
        })
    }

    fn lines() -> Lines {
        Lines {
            added: line(1000.0),
            rate: line(100.0),
            jump: line(5000.0),
            ratio: line(2.0),
            ..Lines::default()
        }
    }

    fn calls(prompts: &[u64]) -> Calls {
        let mut calls = Calls::default();
        for p in prompts {
            calls.count += 1;
            calls.fresh += p;
            calls.first.get_or_insert((0, *p));
            if let Some(last) = calls.last {
                calls.added += p.saturating_sub(last);
            }
            calls.last = Some(*p);
        }
        calls
    }

    #[test]
    fn a_round_is_stopped_where_its_context_grows_past_both_its_lines_and_not_where_past_one() {
        let mut long = Guard::new(lines(), None, 100, false);
        // Long and slow: past the line in all, not for each call.
        let slow: Vec<u64> = (0..30).map(|i| 10_000 + i * 50).collect();
        assert_eq!(long.judge(&calls(&slow)), None);
        let mut fast = Guard::new(lines(), None, 100, false);
        let stopped = fast.judge(&calls(&[10_000, 10_800, 11_600])).expect("stopped");
        assert_eq!((stopped.signal, stopped.value, stopped.per), ("growth", 1600.0, 800.0));
    }

    #[test]
    fn a_round_is_stopped_where_its_first_prompt_jumps_past_what_its_message_explains() {
        let mut guard = Guard::new(lines(), Some(10_000), 100, false);
        let stopped = guard.judge(&calls(&[20_000])).expect("stopped");
        assert_eq!(stopped.signal, "jump");
        let mut large = Guard::new(lines(), Some(10_000), 50_000, false);
        assert_eq!(large.judge(&calls(&[20_000])), None, "a large message explains it");
        let mut begins = Guard::new(lines(), None, 100, false);
        assert_eq!(begins.judge(&calls(&[20_000])), None, "a round that begins its session");
    }

    #[test]
    fn a_round_the_owner_let_run_is_judged_and_not_stopped_and_one_with_too_short_a_history_is_neither() {
        let mut waived = Guard::new(lines(), Some(10_000), 100, true);
        assert!(
            waived.judge(&calls(&[20_000, 40_000, 60_000])).is_some(),
            "it is judged"
        );
        assert!(!waived.stops());
        let mut short = Guard::new(Lines::default(), Some(10_000), 100, false);
        assert_eq!(short.judge(&calls(&[20_000, 40_000, 60_000])), None);
    }

    #[test]
    fn the_reason_names_the_signal_its_lines_where_they_come_from_and_the_last_tools() {
        let words = GuardWords {
            growth: "grew {value} past {line}, {per} past {per_line}, from {from}; tools: {tools}".to_string(),
            jump: "jumped".to_string(),
            again: "again: {stopped}".to_string(),
            from: "{samples} rounds {scope}".to_string(),
            scope: ScopeWords {
                cell: "of its cell".to_string(),
                flow: "of its flow".to_string(),
                works: "of the other works".to_string(),
                line: "of its line".to_string(),
            },
            tool: "{tool} {on}".to_string(),
            between: "; ".to_string(),
            no_tools: "none".to_string(),
        };
        let mut guard = Guard::new(lines(), None, 100, false);
        let stopped = guard.judge(&calls(&[10_000, 10_800, 11_600])).unwrap();
        let tools: Vec<(String, String, String)> = (0..10)
            .map(|i| (format!("t{i}"), "Read".to_string(), format!("{{\"n\":{i}}}")))
            .collect();
        let said = stopped.words(&words, &tools, false);
        assert!(said.starts_with("grew 1600 past 1000, 800 past 100, from 19 rounds of its cell; tools: "));
        assert!(
            said.contains("Read {\"n\":2}") && !said.contains("Read {\"n\":1}"),
            "the last eight: {said}"
        );
        assert!(stopped.words(&words, &[], false).ends_with("tools: none"));
        assert!(stopped.words(&words, &[], true).starts_with("again: grew 1600"));
    }
}
