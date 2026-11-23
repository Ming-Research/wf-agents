//! The one-off repair of recorded costs (`carry::repair`), over a data directory of kept streams and, for the whole
//! repair, a store file. The oracle of what a round cost is the program's own account in the kept result: what it spent
//! since the result before it (`model_usage_since_last_result`) and the session's total (`total_cost_usd`), the second
//! being the sum of the first over the session's results; a repair is right where the rounds of a session add up to
//! the last total, which no part of the repair computes.

use std::path::Path;

use carry::repair::{costs, repair};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use store::Store;
use store::repair::{ForkCost, RoundCost, Rows};

/// The real stream of a session's first round, its result saying it spent $0.002614.
const STREAM: &str = include_str!("fixtures/stream.jsonl");

/// The session the real stream names.
const SESSION: &str = "00000000-0000-5000-8000-000000000001";

/// The fixture's lines, with the result's account of what was spent and the session's total as given, and the session
/// it names as given.
fn stream_of(session: &str, total: f64, since: f64) -> String {
    let lines: Vec<String> = STREAM
        .lines()
        .map(|line| match serde_json::from_str::<Value>(line) {
            Ok(mut result) if result["type"] == "result" => {
                result["total_cost_usd"] = json!(total);
                result["model_usage_since_last_result"]["claude-haiku-5-5"]["costUSD"] = json!(since);
                result.to_string().replace(SESSION, session)
            }
            _ => line.replace(SESSION, session),
        })
        .collect();
    lines.join("\n") + "\n"
}

fn stream(total: f64, since: f64) -> String {
    stream_of(SESSION, total, since)
}

/// The fixture's lines up to its result: a round the engine's restart cut off.
fn cut_off() -> String {
    let lines: Vec<&str> = STREAM
        .lines()
        .take_while(|line| !serde_json::from_str::<Value>(line).is_ok_and(|v| v["type"] == "result"))
        .collect();
    lines.join("\n") + "\n"
}

fn keep(data: &Path, run: u64, text: &str) {
    let dir = data.join("rounds").join("1");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{run}-1.stream.jsonl")), text).unwrap();
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

/// A work's rounds in one session: the first told $1.00 spent and $1.00 in all, the second cut off by a restart, the
/// third told $0.50 spent and $3.00 in all, so that the second spent $1.50.
fn cut_between_two(data: &Path) -> Vec<RoundCost> {
    keep(data, 9, &stream(1.0, 1.0));
    keep(data, 10, &cut_off());
    keep(data, 11, &stream(3.0, 0.5));
    vec![round(9, 10, 1_000_000), round(10, 20, 2_099), round(11, 30, 500_000)]
}

// Two rounds in a row with no result between two that have one: the totals around them say what the two cost together
// and not what each cost, so neither is set and each is listed. A few milliseconds, in a temporary directory; the
// engineer's tests have one round cut off between two results.
#[test]
fn two_rounds_cut_off_in_a_row_are_both_listed_and_left_as_they_were() {
    let data = tempfile::tempdir().unwrap();
    keep(data.path(), 9, &stream(1.0, 1.0));
    keep(data.path(), 10, &cut_off());
    keep(data.path(), 11, &cut_off());
    keep(data.path(), 12, &stream(3.0, 0.5));
    let rows = Rows {
        rounds: vec![
            round(9, 10, 1_000_000),
            round(10, 20, 2_099),
            round(11, 30, 2_099),
            round(12, 40, 500_000),
        ],
        ..Rows::default()
    };
    let report = costs(data.path(), &rows);
    assert!(report.rounds.is_empty(), "{:?}", report.rounds);
    assert_eq!(report.listed.len(), 2, "{:?}", report.listed);
}

// A round of another session between the results around a cut-off round spent nothing in the cut-off round's session,
// and does not stop its cost being told; a round of the same work is no reason to list one, only a ping, a fork or a
// round whose session is not known. A few milliseconds, in a temporary directory; no other test has a second session.
#[test]
fn a_round_of_another_session_between_the_results_around_a_cut_off_round_does_not_stop_it_being_told() {
    let data = tempfile::tempdir().unwrap();
    let mut rounds = cut_between_two(data.path());
    keep(data.path(), 14, &stream_of("another-session", 7.0, 7.0));
    rounds.insert(2, round(14, 25, 7_000_000));
    let rows = Rows {
        rounds,
        ..Rows::default()
    };
    let report = costs(data.path(), &rows);
    assert_eq!(report.rounds, [(1, 10, 1, 1_500_000)], "{:?}", report.listed);
    assert!(report.listed.is_empty(), "{:?}", report.listed);
}

// A fork of the work made between the results around a cut-off round may have spent in the session, and the round is
// listed and left, as it is for a ping. The fork's own cost is left where its stream is not kept. A few milliseconds, in
// a temporary directory; the engineer's tests do not put a fork between the results.
#[test]
fn a_fork_between_the_results_around_a_cut_off_round_stops_it_being_told() {
    let data = tempfile::tempdir().unwrap();
    let rounds = cut_between_two(data.path());
    let fork = ForkCost {
        work: 1,
        at: 25,
        run: 9,
        round: 1,
        usd_micros: 40_000,
    };
    let rows = Rows {
        rounds,
        forks: vec![fork],
        ..Rows::default()
    };
    let report = costs(data.path(), &rows);
    assert!(report.rounds.is_empty(), "{:?}", report.rounds);
    assert!(report.forks.is_empty(), "{:?}", report.forks);
    assert!(
        report
            .listed
            .iter()
            .any(|l| l.contains("run 10") && l.contains("between")),
        "{:?}",
        report.listed
    );
}

// Totals that hold less than the result after spent itself, which no cost a round could have makes right, are listed and
// not turned into a number: here the session's total is $0.50 after a result that spent $0.50, and $1.00 before it. A
// few milliseconds, in a temporary directory.
#[test]
fn totals_that_leave_less_than_nothing_for_a_cut_off_round_are_listed() {
    let data = tempfile::tempdir().unwrap();
    keep(data.path(), 9, &stream(1.0, 1.0));
    keep(data.path(), 10, &cut_off());
    keep(data.path(), 11, &stream(0.5, 0.5));
    let rows = Rows {
        rounds: vec![round(9, 10, 1_000_000), round(10, 20, 2_099), round(11, 30, 500_000)],
        ..Rows::default()
    };
    let report = costs(data.path(), &rows);
    assert!(report.rounds.is_empty(), "{:?}", report.rounds);
    assert_eq!(report.listed.len(), 1, "{:?}", report.listed);
}

// A fork whose stream says nothing of what it spent is listed and its recorded cost is left, not set to what its calls
// would be priced at. A few milliseconds, in a temporary directory.
#[test]
fn a_fork_whose_results_say_nothing_of_what_it_spent_is_listed_and_left() {
    let data = tempfile::tempdir().unwrap();
    let text: String = STREAM
        .lines()
        .map(|line| match serde_json::from_str::<Value>(line) {
            Ok(mut result) if result["type"] == "result" => {
                result.as_object_mut().unwrap().remove("model_usage_since_last_result");
                result.to_string() + "\n"
            }
            _ => line.to_string() + "\n",
        })
        .collect();
    keep(data.path(), 7, &stream(1.0, 1.0));
    std::fs::write(data.path().join("rounds/1/7-1.fork.1.stream.jsonl"), text).unwrap();
    let rows = Rows {
        rounds: vec![round(7, 10, 1_000_000)],
        forks: vec![ForkCost {
            work: 1,
            at: 12,
            run: 7,
            round: 1,
            usd_micros: 12_644_000,
        }],
        ..Rows::default()
    };
    let report = costs(data.path(), &rows);
    assert!(report.forks.is_empty(), "{:?}", report.forks);
    assert_eq!(report.listed.len(), 1, "{:?}", report.listed);
}

/// A store file with the costs of the rounds the way the engine before this work recorded them: the first round's is
/// right, the cut-off round's is its calls priced, and the round after it was charged the session's total less what the
/// session had cost before the result (here $3.00 less $1.00).
fn store_with_rounds(data: &Path) -> Store {
    drop(Store::open(data.join(store::FILE)).unwrap());
    let db = Connection::open(data.join(store::FILE)).unwrap();
    // The rows stand alone: the repair reads and rewrites costs, and no work is made for them.
    db.execute_batch("PRAGMA foreign_keys = OFF").unwrap();
    for (run, at, usd) in [(9u64, 10u64, 1_000_000u64), (10, 20, 2_099), (11, 30, 2_000_000)] {
        db.execute(
            "INSERT INTO ran (work, at, run, round, attempt, level, model) VALUES (1, ?1, ?2, 1, 1, 'l', 'm')",
            params![at, run],
        )
        .unwrap();
        db.execute(
            "INSERT INTO spent (work, run, round, usd_micros, input_tokens, cached_tokens, output_tokens) \
             VALUES (1, ?1, 1, ?2, 0, 0, 0)",
            params![run, usd],
        )
        .unwrap();
    }
    drop(db);
    Store::open(data.join(store::FILE)).unwrap()
}

fn recorded(data: &Path) -> Vec<(u64, u64)> {
    let db = Connection::open(data.join(store::FILE)).unwrap();
    let mut rows = db.prepare("SELECT run, usd_micros FROM spent ORDER BY run").unwrap();
    rows.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

// The whole repair over a store and a data directory: a copy of the database is taken, the rounds' costs are set to what
// their results and the totals around them say, the report's last line says no round that kept its results records
// another cost, and a second run has nothing to set. The oracle is the program's own total: the three rounds of the
// session cost $3.00 together, the total its last result says, where recorded they cost $3.002099. A few tens of
// milliseconds, a store file and a copy of it in a temporary directory; no other test runs the repair to its end, and
// the last line is what the owner reads to know it is done.
#[test]
fn the_repair_sets_the_costs_takes_a_copy_and_ends_by_saying_none_differs() {
    let data = tempfile::tempdir().unwrap();
    cut_between_two(data.path());
    let mut store = store_with_rounds(data.path());
    let before = costs(data.path(), &store.cost_rows().unwrap());
    assert_eq!(before.differing, 1, "the round charged the session's total differs");

    let said = repair(data.path(), &mut store, 10).expect("the repair runs");
    assert_eq!(
        recorded(data.path()),
        [(9, 1_000_000), (10, 1_500_000), (11, 500_000)],
        "{said:?}"
    );
    assert_eq!(
        recorded(data.path()).iter().map(|(_, usd)| usd).sum::<u64>(),
        3_000_000,
        "the rounds add up to the session's total"
    );
    let last = said.last().expect("a last line");
    assert!(last.starts_with("0 rounds that kept their results"), "{said:?}");
    let copies: Vec<_> = std::fs::read_dir(data.path().join("backups"))
        .expect("the copy is kept beside the store")
        .collect();
    assert_eq!(copies.len(), 1);

    let again = repair(data.path(), &mut store, 10).expect("the repair runs again");
    assert!(
        again[0].starts_with("0 rounds and 0 forks are to be corrected"),
        "{again:?}"
    );
    assert_eq!(
        recorded(data.path()),
        [(9, 1_000_000), (10, 1_500_000), (11, 500_000)],
        "nothing changed by the second run"
    );
}
