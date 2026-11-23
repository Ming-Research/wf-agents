//! The one-off repair of the costs recorded before a round's cost was what its results say it spent (`wf-agents
//! repair-costs`), which rewrites the recorded cost of rounds and forks in place from the streams kept of them, after
//! a copy of the database (decision `a-wrongly-recorded-cost-is-corrected-once-in-place`). It is removed with the
//! command.
//!
//! A round that kept a stream with its results costs what they say they spent. A round with no result, such as one
//! its engine's restart cut off, was priced from its calls and never told what it cost; it is told only where the
//! session's own total says it: the result after it gives the session's total less what it spent itself, which less the
//! total the result before it gave is what the cut-off round cost, provided nothing else was done in the session
//! between them, a keep-warm ping or a fork of the work among it. Anything else is listed and left as it was.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use decide::RunId;
use store::repair::{ForkCorrection, RoundCorrection, Rows};
use store::{Store, WorkNumber};

use crate::cli::{Stream, Told};
use crate::kept;

/// What the repair found and would change.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub rounds: Vec<RoundCorrection>,
    pub forks: Vec<ForkCorrection>,
    /// Each change, in words: the round or fork, the cost it was recorded at and the cost it is to be.
    pub changes: Vec<String>,
    /// What could not be told, each with why.
    pub listed: Vec<String>,
    /// How many rounds that kept their results record a cost other than what those say they spent.
    pub differing: usize,
}

impl Report {
    /// The report in words, each of the changes and of what is listed at most `limit`, with how many more there are.
    pub fn said(&self, limit: usize) -> Vec<String> {
        let mut said = vec![format!(
            "{} rounds and {} forks are to be corrected; {} could not be told.",
            self.rounds.len(),
            self.forks.len(),
            self.listed.len()
        )];
        for (heading, lines) in [("changed", &self.changes), ("listed", &self.listed)] {
            said.extend(lines.iter().take(limit).cloned());
            if lines.len() > limit {
                said.push(format!("{} more {heading} are left out.", lines.len() - limit));
            }
        }
        said
    }
}

/// A round's stream as the repair reads it.
struct Seen<'a> {
    row: &'a store::repair::RoundCost,
    /// Whether the round kept its stream.
    kept: bool,
    told: Vec<Told>,
    session: Option<String>,
    /// What the stream cost, and whether any of it is priced from its calls.
    cost: (u64, bool),
}

fn read(path: &Path) -> Option<Stream> {
    let bytes = std::fs::read(path).ok()?;
    let mut stream = Stream::default();
    for line in bytes.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        stream.line(line);
    }
    Some(stream)
}

/// Lists a round that could not be told, with why.
fn list(report: &mut Report, name: &str, why: &str) {
    report.listed.push(format!("{name}: {why}"));
}

fn dollars(micros: u64) -> String {
    format!("${:.4}", micros as f64 / 1_000_000.0)
}

/// What the repair would change in the costs the store holds, from the streams kept in this data directory.
pub fn costs(data: &Path, rows: &Rows) -> Report {
    let mut report = Report::default();
    let mut works: BTreeMap<u64, Vec<&store::repair::RoundCost>> = BTreeMap::new();
    for row in &rows.rounds {
        works.entry(row.work).or_default().push(row);
    }
    for (work, rounds) in &works {
        let seen: Vec<Seen> = rounds
            .iter()
            .copied()
            .map(|row| {
                let base = kept::base(data, WorkNumber(*work), RunId(row.run), row.round);
                match read(&kept::stream_at(&base, row.attempt)) {
                    Some(stream) => Seen {
                        row,
                        kept: true,
                        told: stream.told(),
                        session: stream.session().map(str::to_string),
                        cost: stream.cost(),
                    },
                    None => Seen {
                        row,
                        kept: false,
                        told: Vec::new(),
                        session: None,
                        cost: (0, false),
                    },
                }
            })
            .collect();
        // Whether a keep-warm ping or a fork of the work was made between these places.
        let between = |from: u64, to: u64| {
            (rows.pings.iter()).any(|(w, at)| w == work && *at > from && *at < to)
                || (rows.forks.iter()).any(|f| f.work == *work && f.at > from && f.at < to)
        };
        for now in &seen {
            let row = now.row;
            let name = format!("W{work} run {} round {}", row.run, row.round);
            if !now.kept {
                list(&mut report, &name, "no stream is kept of it");
                continue;
            }
            let new = if !now.told.is_empty() && !now.cost.1 {
                if now.cost.0 != row.usd_micros {
                    report.differing += 1;
                }
                now.cost.0
            } else if !now.told.is_empty() {
                list(&mut report, &name, "a result of it says nothing of what it spent");
                continue;
            } else {
                let Some(session) = &now.session else {
                    list(
                        &mut report,
                        &name,
                        "it printed no result, and its stream names no session",
                    );
                    continue;
                };
                let same: Vec<&Seen> = seen.iter().filter(|s| s.session.as_ref() == Some(session)).collect();
                let at = same.iter().position(|s| std::ptr::eq(s.row, row)).unwrap_or(0);
                let (Some(prev), Some(next)) = (at.checked_sub(1).map(|p| same[p]), same.get(at + 1)) else {
                    list(
                        &mut report,
                        &name,
                        "it printed no result, and no result of its session comes before and after it",
                    );
                    continue;
                };
                let (Some(before), Some(first)) = (prev.told.last().and_then(|t| t.total), next.told.first()) else {
                    list(
                        &mut report,
                        &name,
                        "it printed no result, and the results around it say no total",
                    );
                    continue;
                };
                let (Some(total), Some(since)) = (first.total, first.since) else {
                    list(
                        &mut report,
                        &name,
                        "it printed no result, and the result after it says no total or what it spent",
                    );
                    continue;
                };
                if between(prev.row.at, next.row.at) {
                    list(
                        &mut report,
                        &name,
                        "it printed no result, and a keep-warm ping or a fork was made between the results around it",
                    );
                    continue;
                }
                let Some(gap) = total.checked_sub(since).and_then(|total| total.checked_sub(before)) else {
                    list(
                        &mut report,
                        &name,
                        "it printed no result, and the totals around it hold less than the result after it spent",
                    );
                    continue;
                };
                gap
            };
            if new != row.usd_micros {
                report.rounds.push((*work, row.run, row.round, new));
                report
                    .changes
                    .push(format!("{name}: {} -> {}", dollars(row.usd_micros), dollars(new)));
            }
        }
    }
    forks(data, rows, &mut report);
    report
}

/// The forks of each round, whose streams are kept as `<run>-<round>.fork.<n>.stream.jsonl`, the first being one, in
/// the order the forks were recorded: kept as many as recorded, each costs what its results say it spent.
fn forks(data: &Path, rows: &Rows, report: &mut Report) {
    let mut of_round: BTreeMap<(u64, u64, usize), Vec<&store::repair::ForkCost>> = BTreeMap::new();
    for fork in &rows.forks {
        of_round
            .entry((fork.work, fork.run, fork.round))
            .or_default()
            .push(fork);
    }
    for ((work, run, round), recorded) in &of_round {
        let name = format!("W{work} run {run} round {round}");
        let base = kept::base(data, WorkNumber(*work), RunId(*run), *round);
        let stem = base
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let streams: Vec<PathBuf> = (1..)
            .map(|n| base.with_file_name(format!("{stem}.fork.{n}.stream.jsonl")))
            .take_while(|path| path.is_file())
            .collect();
        if streams.len() != recorded.len() {
            report.listed.push(format!(
                "{name}: {} forks are recorded and {} streams kept",
                recorded.len(),
                streams.len()
            ));
            continue;
        }
        for (fork, path) in recorded.iter().zip(&streams) {
            let told = read(path).map(|stream| (stream.told(), stream.cost()));
            let Some((told, (usd, estimated))) = told else {
                report.listed.push(format!("{name}: a fork's stream cannot be read"));
                continue;
            };
            if told.is_empty() || estimated {
                report
                    .listed
                    .push(format!("{name}: a fork's results say nothing of what it spent"));
                continue;
            }
            if usd != fork.usd_micros {
                report.forks.push((*work, fork.at, usd));
                report.changes.push(format!(
                    "{name}, a fork: {} -> {}",
                    dollars(fork.usd_micros),
                    dollars(usd)
                ));
            }
        }
    }
}

/// Repairs the costs the store holds from the streams kept in this data directory: takes a copy of the database,
/// rewrites what the streams say, and says what it did, with how many rounds record a cost other than what their
/// results say once it has. Writes nothing where the copy cannot be taken.
pub fn repair(data: &Path, store: &mut Store, limit: usize) -> Result<Vec<String>, String> {
    store
        .copy_before_repair()
        .map_err(|e| format!("the copy of the database before the repair could not be taken: {e:?}"))?;
    let rows = store.cost_rows().map_err(|e| format!("{e:?}"))?;
    let report = costs(data, &rows);
    store
        .correct_costs(&report.rounds, &report.forks)
        .map_err(|e| format!("{e:?}"))?;
    let after = costs(data, &store.cost_rows().map_err(|e| format!("{e:?}"))?);
    let mut said = report.said(limit);
    said.push(format!(
        "{} rounds that kept their results record a cost other than what those say.",
        after.differing
    ));
    Ok(said)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use store::repair::{ForkCost, RoundCost};

    use super::*;

    /// A stream the command line printed for a session's first round: its program's first line, two calls and their
    /// ends, and a result that says it spent $0.002614, the session $0.00261438 in all.
    const STREAM: &str = include_str!("../tests/fixtures/stream.jsonl");

    /// The fixture's lines, with the result's account of what was spent and the session's total as given.
    fn stream(total: f64, since: f64) -> String {
        let lines: Vec<String> = STREAM
            .lines()
            .map(|line| match serde_json::from_str::<Value>(line) {
                Ok(mut result) if result["type"] == "result" => {
                    result["total_cost_usd"] = json!(total);
                    result["model_usage_since_last_result"]["claude-haiku-5-5"]["costUSD"] = json!(since);
                    result.to_string()
                }
                _ => line.to_string(),
            })
            .collect();
        lines.join("\n") + "\n"
    }

    /// The fixture's lines up to its result: a round the engine's restart cut off.
    fn cut_off() -> String {
        let lines: Vec<&str> = STREAM
            .lines()
            .take_while(|line| !line.contains(r#""type":"result""#))
            .collect();
        lines.join("\n") + "\n"
    }

    fn keep(data: &Path, round: &RoundCost, text: &str) {
        let base = kept::base(data, WorkNumber(round.work), RunId(round.run), round.round);
        std::fs::create_dir_all(base.parent().unwrap()).unwrap();
        std::fs::write(kept::stream_at(&base, round.attempt), text).unwrap();
    }

    fn round(run: u64, at: u64, usd_micros: u64) -> RoundCost {
        RoundCost {
            work: 1,
            run,
            round: 1,
            at,
            attempt: 1,
            usd_micros,
        }
    }

    // A round that kept its result costs what the result says it spent: here a round recorded at the whole of the
    // session's total, $72.8774, which its result says it spent $0.1832 of. In memory and a temporary directory: a few
    // milliseconds; no other test holds the repair of a round whose result was kept.
    #[test]
    fn a_round_that_kept_its_result_costs_what_the_result_says_it_spent() {
        let data = tempfile::tempdir().unwrap();
        let wrong = round(11, 10, 71_409_000);
        keep(data.path(), &wrong, &stream(72.8774, 0.1832));
        let right = round(12, 20, 2_614);
        keep(data.path(), &right, &stream(0.00261438, 0.00261438));
        let rows = Rows {
            rounds: vec![wrong, right],
            ..Rows::default()
        };
        let report = costs(data.path(), &rows);
        assert_eq!(report.rounds, [(1, 11, 1, 183_200)]);
        assert_eq!(report.differing, 1, "only the wrong one differs");
        assert!(report.listed.is_empty(), "{:?}", report.listed);
        // What is set is what the results say: nothing differs once it is.
        let corrected = Rows {
            rounds: vec![round(11, 10, 183_200), round(12, 20, 2_614)],
            ..Rows::default()
        };
        assert_eq!(costs(data.path(), &corrected).differing, 0);
    }

    // The round a restart cut off was priced at its calls, and the round after it was charged the rest of the session's
    // total. Run 10 printed no result; the session had cost $1.00 at the result before it and $3.00 at the one after,
    // which spent $0.50 itself, so run 10 cost $1.50. A few milliseconds, in a temporary directory.
    #[test]
    fn a_round_with_no_result_costs_what_the_totals_around_it_leave() {
        let data = tempfile::tempdir().unwrap();
        let (before, cut, after) = (round(9, 10, 1_000_000), round(10, 20, 2_099), round(11, 30, 500_000));
        keep(data.path(), &before, &stream(1.0, 1.0));
        keep(data.path(), &cut, &cut_off());
        keep(data.path(), &after, &stream(3.0, 0.5));
        let rows = Rows {
            rounds: vec![before, cut, after],
            ..Rows::default()
        };
        let report = costs(data.path(), &rows);
        assert_eq!(report.rounds, [(1, 10, 1, 1_500_000)]);
        assert!(report.listed.is_empty(), "{:?}", report.listed);
    }

    // The same, with a keep-warm ping of the work made between the results around it: its cost is in the session's total
    // too, and the round is listed and left as it was, as it is where no result comes before or after it, where its
    // stream names no session, and where no stream is kept of it.
    #[test]
    fn a_round_with_no_result_is_listed_where_the_totals_cannot_tell_what_it_cost() {
        let data = tempfile::tempdir().unwrap();
        let (before, cut, after) = (round(9, 10, 1_000_000), round(10, 20, 2_099), round(11, 30, 500_000));
        keep(data.path(), &before, &stream(1.0, 1.0));
        keep(data.path(), &cut, &cut_off());
        keep(data.path(), &after, &stream(3.0, 0.5));
        let mut rows = Rows {
            rounds: vec![before, cut, after],
            pings: vec![(1, 25)],
            ..Rows::default()
        };
        let report = costs(data.path(), &rows);
        assert!(report.rounds.is_empty(), "{:?}", report.rounds);
        assert_eq!(report.listed.len(), 1);
        assert!(report.listed[0].contains("ping"), "{:?}", report.listed);
        // A ping of another work does not count.
        rows.pings = vec![(2, 25)];
        assert_eq!(costs(data.path(), &rows).rounds, [(1, 10, 1, 1_500_000)]);
        // The first round of its session has no result before it.
        rows.pings.clear();
        rows.rounds.remove(0);
        let report = costs(data.path(), &rows);
        assert_eq!((report.rounds.len(), report.listed.len()), (0, 1));
        // No stream is kept of it.
        let lone = Rows {
            rounds: vec![round(12, 40, 5)],
            ..Rows::default()
        };
        assert!(costs(data.path(), &lone).listed[0].contains("no stream is kept"));
    }

    // A fork costs what the results of its two messages say they spent, found by the order the forks of a round were
    // recorded in and kept; where the forks recorded and the streams kept are not as many, they are listed.
    #[test]
    fn a_fork_costs_what_its_results_say_and_forks_that_cannot_be_matched_are_listed() {
        let data = tempfile::tempdir().unwrap();
        let round = round(7, 10, 0);
        keep(data.path(), &round, &stream(1.0, 1.0));
        let base = kept::base(data.path(), WorkNumber(1), RunId(7), 1);
        let second: String = STREAM
            .lines()
            .skip(1)
            .map(|line| line.replace("msg_011", "msg_022").replace("toolu_01", "toolu_02") + "\n")
            .collect();
        let both = format!("{STREAM}{second}");
        std::fs::write(base.with_file_name("7-1.fork.1.stream.jsonl"), both).unwrap();
        let fork = ForkCost {
            work: 1,
            at: 12,
            run: 7,
            round: 1,
            usd_micros: 12_644_000,
        };
        let rows = Rows {
            rounds: vec![round],
            forks: vec![fork],
            ..Rows::default()
        };
        // The fixture's result says $0.002614, twice.
        let report = costs(data.path(), &rows);
        assert_eq!(report.forks, [(1, 12, 5_228)]);
        let more = Rows {
            forks: vec![fork, ForkCost { at: 13, ..fork }],
            ..rows
        };
        let report = costs(data.path(), &more);
        assert!(report.forks.is_empty());
        assert!(
            report.listed[0].contains("2 forks are recorded and 1 streams kept"),
            "{:?}",
            report.listed
        );
    }

    // What the report says is bounded, as every command's output is, and says how much it left out.
    #[test]
    fn the_report_lists_at_most_the_limit_and_says_how_many_more() {
        let report = Report {
            changes: (0..50).map(|n| format!("change {n}")).collect(),
            listed: (0..2).map(|n| format!("listed {n}")).collect(),
            ..Report::default()
        };
        let said = report.said(3);
        assert_eq!(said.len(), 1 + 3 + 1 + 2, "{said:?}");
        assert!(said.contains(&"47 more changed are left out.".to_string()), "{said:?}");
    }
}
