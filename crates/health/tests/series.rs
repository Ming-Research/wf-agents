//! Each health number over records seeded for it, against values worked out by hand: at a step of a minute over ten
//! minutes, and over one period of another length as a whole. Each test seeds what its numbers count and some of what
//! they must leave out: another project's, another kind's, an engine's step for an agent's round.

use std::path::Path;
use std::process::Command;

use db::rusqlite::{Connection, params};
use db::{Actor, Db};
use health::{MainLog, Scope, measure, series, series_by_subject};
use serde_json::{Value, json};

/// A time on the minute, long past, from which every record of these tests is placed.
const T0: i64 = 1_700_000_040_000;
/// A minute.
const M: i64 = 60_000;
/// The period of another length read as a whole: from a minute and a half after `T0` up to seven and a half minutes
/// after it.
const FROM: i64 = T0 + 90_000;
const TO: i64 = T0 + 450_000;

struct Seed {
    db: Db,
}

/// A round to seed: its work, its cell, when it started and ended (not yet when none), whether an agent ran it, in its
/// work's session, its usage, the row it took or why it was rejected, and its output.
#[derive(Default)]
struct R {
    work: i64,
    cell: &'static str,
    started: i64,
    ended: Option<i64>,
    agent: bool,
    usage: Option<Value>,
    transition: Option<&'static str>,
    rejected: Option<Value>,
    output: Option<Value>,
}

impl Seed {
    fn new() -> Seed {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO flow_versions (id, digest, compiled) VALUES (1, 'v', '{}')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        Seed { db }
    }

    fn conn(&self) -> &Connection {
        self.db.conn()
    }

    /// Runs a statement in a write of its own, and gives the row it inserted last.
    fn sql(&mut self, sql: &str, args: impl db::rusqlite::Params) -> i64 {
        self.db
            .write(Actor::Engine, |tx| {
                tx.execute(sql, args)?;
                Ok(tx.last_insert_rowid())
            })
            .unwrap()
    }

    /// Places the latest change to a row of a table at a time.
    fn restamp(&self, table: &str, id: i64, at: i64) {
        self.conn()
            .execute(
                "UPDATE changes SET at = ?3 WHERE id = (SELECT max(id) FROM changes
                   WHERE table_name = ?1 AND row_key = json_object('id', ?2))",
                params![table, id, at],
            )
            .unwrap();
    }

    /// An item of a project made at a time, imported from an earlier board when `imported`.
    fn item(&mut self, project: &str, imported: bool, at: i64) -> i64 {
        let id = self.sql(
            "INSERT INTO items (project, title, problem, why, deliver, level, imported_from)
             VALUES (?1, 't', 'p', 'w', 'change', 'l', ?2)",
            params![project, imported.then_some("an earlier board")],
        );
        self.restamp("items", id, at);
        id
    }

    /// Cancels an item at a time.
    fn cancel(&mut self, item: i64, at: i64) {
        self.sql("UPDATE items SET cancelled = 'not needed' WHERE id = ?1", [item]);
        self.restamp("items", item, at);
    }

    /// A work of a graph, on an item or of a project's architect, with a session for its agents' rounds.
    fn work(&mut self, item: Option<i64>, graph: &str, project: Option<&str>) -> i64 {
        let w = self.sql(
            "INSERT INTO works (item, flow_version, graph, at, project) VALUES (?1, 1, ?2, 'I1', ?3)",
            params![item, graph, project],
        );
        self.sql(
            "INSERT INTO sessions (work, line, cli, cli_session) VALUES (?1, ?2, 'claude', 's')",
            params![w, graph],
        );
        w
    }

    /// Pauses a work at a time, or lets it go on.
    fn pause(&mut self, work: i64, paused: bool, at: i64) {
        self.sql(
            "UPDATE works SET paused = ?2, paused_by = ?3 WHERE id = ?1",
            params![work, paused.then_some("held"), paused.then_some("owner")],
        );
        self.restamp("works", work, at);
    }

    /// A round, and its number in its work.
    fn round(&mut self, r: R) -> i64 {
        let text = |v: Option<Value>| v.map(|v| v.to_string());
        self.sql(
            "INSERT INTO rounds (work, n, cell, session, input, output, transition, rejected, config_commit, usage,
                                 started_at, ended_at)
             VALUES (?1, (SELECT coalesce(max(n), 0) + 1 FROM rounds WHERE work = ?1), ?2,
                     CASE WHEN ?3 THEN (SELECT id FROM sessions WHERE work = ?1) END, '{}', ?4, ?5, ?6, 'c', ?7, ?8, ?9)",
            params![
                r.work,
                r.cell,
                r.agent,
                text(r.output),
                r.transition,
                text(r.rejected),
                text(r.usage),
                r.started,
                r.ended
            ],
        );
        self.conn()
            .query_row("SELECT max(n) FROM rounds WHERE work = ?1", [r.work], |x| x.get(0))
            .unwrap()
    }

    /// A run of a tool in a round, queued at a time and ended at another, or not yet.
    fn tool(&mut self, work: i64, round: i64, tool: &str, queued: i64, ended: Option<i64>) {
        self.tool_run((work, round), tool, &json!({}), None, queued, ended);
    }

    /// A run of a tool in a round, by its work and its number, with what it was given and what it gave.
    fn tool_run(
        &mut self,
        (work, round): (i64, i64),
        tool: &str,
        args: &Value,
        result: Option<&Value>,
        queued: i64,
        ended: Option<i64>,
    ) {
        self.sql(
            "INSERT INTO tool_runs (work, round, tool, args, result, attempt, queued_at, started_at, ended_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6, ?7)",
            params![
                work,
                round,
                tool,
                args.to_string(),
                result.map(Value::to_string),
                queued,
                ended
            ],
        );
    }

    /// A merge the engine reverted at a time, as the architect's work asked; `revert` is the commit on main.
    fn revert(&mut self, work: i64, merge: &str, revert: &str, at: i64) {
        self.sql(
            "INSERT INTO reverts (work, merge_commit, revert_commit, at) VALUES (?1, ?2, ?3, ?4)",
            params![work, merge, revert, at],
        );
    }

    /// A blocker of a work, raised at a time, ended at another or still open.
    fn blocker(&mut self, work: i64, kind: &str, at: i64, ended: Option<i64>) {
        let holder = match kind {
            "owner_ask" => "owner",
            "items" => "items",
            _ => "advisor",
        };
        self.sql(
            "INSERT INTO blockers (work, cell, kind, what, holder, items, at, since, ended, outcome, ended_at)
             VALUES (?1, 'I1', ?2, 'w', ?3, CASE WHEN ?3 = 'items' THEN '[]' END, ?4, ?4,
                     CASE WHEN ?5 IS NOT NULL THEN 'resolved' END, CASE WHEN ?5 IS NOT NULL THEN '{}' END, ?5)",
            params![work, kind, holder, at, ended],
        );
    }

    /// A matter for a project's architect that came at a time, handled by a round or not yet.
    fn matter(&mut self, project: &str, kind: &str, at: i64, handled: Option<(i64, i64)>) {
        self.sql(
            "INSERT INTO architect_matters (project, kind, what, body, at, handled_work, handled_round)
             VALUES (?1, ?2, 'w', '{}', ?3, ?4, ?5)",
            params![project, kind, at, handled.map(|h| h.0), handled.map(|h| h.1)],
        );
    }

    /// A sample the engine took.
    fn sample(&mut self, name: &str, project: &str, at: i64, value: f64) {
        self.conn()
            .execute(
                "INSERT INTO samples (measure, project, at, value) VALUES (?1, ?2, ?3, ?4)",
                params![name, project, at, value],
            )
            .unwrap();
    }

    /// A measure's values from `from` up to `to` at a step, for a project or all, for a subject or all together.
    fn values(&self, name: &str, scope: Scope<'_>, from: i64, to: i64, step: i64) -> Vec<Option<f64>> {
        let m = measure(name).unwrap_or_else(|| panic!("no measure {name}"));
        let s = series(self.conn(), m, &scope, from, to, step).unwrap();
        s.points.iter().map(|p| p.value).collect()
    }

    /// A measure's values over the ten minutes from `T0`, a minute each.
    fn minutes(&self, name: &str, scope: Scope<'_>) -> Vec<Option<f64>> {
        self.values(name, scope, T0, T0 + 10 * M, M)
    }

    /// A measure's value over the period of another length, as a whole.
    fn whole(&self, name: &str, scope: Scope<'_>) -> Option<f64> {
        let v = self.values(name, scope, FROM, TO, TO - FROM);
        assert_eq!(v.len(), 1);
        v[0]
    }
}

fn p() -> Scope<'static> {
    Scope {
        project: Some("p"),
        ..Scope::default()
    }
}

fn all() -> Scope<'static> {
    Scope::default()
}

fn of(subject: &'static str) -> Scope<'static> {
    Scope {
        project: Some("p"),
        subject: Some(subject),
        ..Scope::default()
    }
}

/// Ten minutes' values: those given by minute, the others `rest`.
fn ten(given: &[(usize, f64)], rest: Option<f64>) -> Vec<Option<f64>> {
    let mut v = vec![rest; 10];
    for &(k, x) in given {
        v[k] = Some(x);
    }
    v
}

/// The usage of an agent's round whose calls' prompts were these: input, read from the cache, written to it.
fn usage(calls: &[(i64, i64, i64)]) -> Value {
    let calls: Vec<Value> = calls
        .iter()
        .map(|&(i, r, w)| json!({ "input_tokens": i, "cache_read_input_tokens": r, "cache_creation_input_tokens": w }))
        .collect();
    json!({ "usage": { "iterations": calls }, "cost_usd": 0.01 })
}

// The architect's check of main's red CI that found a merge made main red counts in the minute it ended, the first
// millisecond of a minute being that minute's; a check that found something else, a round of another cell, another
// project's check, and the old watch of main on an engineer's graph do not count for the project.
#[test]
fn a_merge_that_broke_main_counts_where_its_check_ended() {
    let mut s = Seed::new();
    let i = s.item("p", false, T0 - 60 * M);
    let (w, v, old) = (
        s.work(None, "architect", Some("p")),
        s.work(None, "architect", Some("q")),
        s.work(Some(i), "engineer", None),
    );
    let broken = || Some(json!({ "main": { "kind": "broken_by_merge" } }));
    let check = |work: i64, cell: &'static str, ended: i64, output: Option<Value>| R {
        work,
        cell,
        started: ended - 5_000,
        ended: Some(ended),
        transition: Some("next"),
        output,
        ..R::default()
    };
    s.round(check(w, "L1", T0 + 3 * M, broken()));
    s.round(check(w, "L1", T0 + 3 * M - 1, broken()));
    s.round(check(
        w,
        "L1",
        T0 + 4 * M + 10,
        Some(json!({ "main": { "kind": "environment_changed" } })),
    ));
    s.round(check(w, "L2", T0 + 6 * M, broken()));
    s.round(check(v, "L1", T0 + 5 * M, broken()));
    s.round(check(old, "L1", T0 + 7 * M, broken()));
    assert_eq!(s.minutes("merge.broken", p()), ten(&[(2, 1.0), (3, 1.0)], Some(0.0)));
    assert_eq!(
        s.minutes("merge.broken", all()),
        ten(&[(2, 1.0), (3, 1.0), (5, 1.0)], Some(0.0))
    );
    assert_eq!(s.whole("merge.broken", p()), Some(2.0));
    assert_eq!(s.whole("merge.broken", all()), Some(3.0));
}

// A merge the engine reverted counts in the minute the revert merged, for the project of the architect's work that
// asked for it.
#[test]
fn a_merge_reverted_counts_where_its_revert_merged() {
    let mut s = Seed::new();
    let (a, b) = (
        s.work(None, "architect", Some("p")),
        s.work(None, "architect", Some("q")),
    );
    s.revert(a, "m1", "r1", T0 + 2 * M);
    s.revert(a, "m2", "r2", T0 + 3 * M - 1);
    s.revert(a, "m3", "r3", T0 - 1);
    s.revert(b, "m4", "r4", T0 + 3 * M);
    assert_eq!(s.minutes("merge.reverted", p()), ten(&[(2, 2.0)], Some(0.0)));
    assert_eq!(
        s.minutes("merge.reverted", all()),
        ten(&[(2, 2.0), (3, 1.0)], Some(0.0))
    );
    assert_eq!(s.whole("merge.reverted", p()), Some(2.0));
    assert_eq!(s.whole("merge.reverted", all()), Some(3.0));
}

// A measure nothing records yet has no values, and says why; so does a subject of a measure not recorded yet.
#[test]
fn a_number_nothing_records_says_why_and_has_no_values() {
    let s = Seed::new();
    let m = measure("merge.defects").unwrap();
    let got = series(s.conn(), m, &p(), T0, T0 + 10 * M, M).unwrap();
    assert!(got.not_measured.is_some_and(|why| !why.is_empty()));
    assert!(got.points.iter().all(|x| x.value.is_none() && x.n == 0));
    let stops = measure("work.stops").unwrap();
    let slot = series(s.conn(), stops, &of("slot"), T0, T0 + 10 * M, M).unwrap();
    assert!(slot.not_measured.is_some(), "{slot:?}");
    assert!(slot.points.iter().all(|x| x.value.is_none()));
    // A count the records hold is a zero where nothing happened.
    let made = series(s.conn(), measure("items.made").unwrap(), &p(), T0, T0 + 10 * M, M).unwrap();
    assert_eq!(made.not_measured, None);
    assert!(made.points.iter().all(|x| x.value == Some(0.0)));
}

// The context an agent's round ended with is its last call's prompt; the median and the largest of the rounds that
// ended in each minute, of every cell together and of one; an engine's step, and a round whose usage holds no call, are
// none.
#[test]
fn the_context_is_the_median_and_the_largest_of_the_rounds_that_ended() {
    let mut s = Seed::new();
    let i = s.item("p", false, T0 - 60 * M);
    let w = s.work(Some(i), "engineer", None);
    let a = s.work(None, "architect", Some("p"));
    let agent = |work: i64, cell: &'static str, ended: i64, u: Value| R {
        work,
        cell,
        started: ended - 1_000,
        ended: Some(ended),
        agent: true,
        usage: Some(u),
        transition: Some("next"),
        ..R::default()
    };
    s.round(agent(w, "I1", T0 + M, usage(&[(5, 50, 500), (10, 100, 1000)])));
    s.round(agent(w, "I1", T0 + M + 30_000, usage(&[(0, 2000, 0)])));
    s.round(agent(w, "T3", T0 + M + 40_000, usage(&[(3000, 0, 0)])));
    s.round(agent(w, "I1", T0 + 2 * M - 1, usage(&[(0, 0, 4000)])));
    s.round(agent(w, "I1", T0 + 5 * M, json!({ "usage": { "input_tokens": 9 } })));
    s.round(R {
        agent: false,
        ..agent(w, "I1", T0 + 5 * M + 1, usage(&[(0, 9000, 0)]))
    });
    s.round(agent(a, "U1", T0 + 5 * M + 2, usage(&[(700, 0, 0)])));
    assert_eq!(s.minutes("round.context", p()), ten(&[(1, 2500.0), (5, 700.0)], None));
    assert_eq!(
        s.minutes("round.context_max", p()),
        ten(&[(1, 4000.0), (5, 700.0)], None)
    );
    assert_eq!(s.minutes("round.context", of("engineer/I1")), ten(&[(1, 2000.0)], None));
    assert_eq!(
        s.minutes("round.context_max", of("engineer/I1")),
        ten(&[(1, 4000.0)], None)
    );
    assert_eq!(s.minutes("round.context", of("engineer/T3")), ten(&[(1, 3000.0)], None));
    assert_eq!(s.minutes("round.context", of("architect/U1")), ten(&[(5, 700.0)], None));
    // From 90 s: 2000 (at 90 s itself), 3000, 4000 and 700.
    assert_eq!(s.whole("round.context", p()), Some(2500.0));
    assert_eq!(s.whole("round.context_max", p()), Some(4000.0));
    let by = series_by_subject(
        s.conn(),
        measure("round.context").unwrap(),
        &p(),
        T0,
        T0 + 10 * M,
        10 * M,
    )
    .unwrap();
    let names: Vec<&str> = by.iter().map(|(x, _)| x.as_str()).collect();
    assert_eq!(names, ["architect/U1", "engineer/I1", "engineer/T3"]);
    assert_eq!(by[1].1.points[0].value, Some(2000.0));
}

// Of the agents' rounds that ended in a minute, the part sent back, and the part whose run failed or was interrupted,
// which is no sending back; an engine's step and a round still running are none of them.
#[test]
fn sent_back_and_failed_are_parts_of_the_agents_rounds() {
    let mut s = Seed::new();
    let i = s.item("p", false, T0 - 60 * M);
    let w = s.work(Some(i), "engineer", None);
    let ended = |at: i64, transition: Option<&'static str>, rejected: Option<Value>| R {
        work: w,
        cell: "I1",
        started: at - 1_000,
        ended: Some(at),
        agent: true,
        transition,
        rejected,
        ..R::default()
    };
    s.round(ended(T0 + M + 1, Some("done"), None));
    s.round(ended(T0 + M + 2, None, Some(json!({ "check": "wrong" }))));
    s.round(ended(
        T0 + M + 3,
        None,
        Some(json!({ "failed": { "kind": "usage_limit" } })),
    ));
    s.round(ended(T0 + M + 4, None, Some(json!({ "error": "it stopped" }))));
    s.round(ended(
        T0 + 5 * M,
        None,
        Some(json!({ "interrupted": "the engine stopped" })),
    ));
    s.round(ended(T0 + 5 * M + 1, Some("next"), None));
    s.round(R {
        agent: false,
        ..ended(T0 + 5 * M + 2, None, Some(json!({ "check": "wrong" })))
    });
    s.round(R {
        ended: None,
        ..ended(T0 + 5 * M + 3, None, None)
    });
    assert_eq!(s.minutes("round.sent_back", p()), ten(&[(1, 0.25), (5, 0.0)], None));
    assert_eq!(s.minutes("round.failed", p()), ten(&[(1, 0.5), (5, 0.5)], None));
    assert_eq!(s.whole("round.sent_back", p()), Some(0.0));
    assert_eq!(s.whole("round.failed", p()), Some(0.5));
}

// The owner's words wait from when they came to the end of the architect's round that handled them: the median of
// those handled in each minute, another kind of matter and words not handled yet left out.
#[test]
fn the_owner_s_words_wait_until_the_round_that_handled_them_ended() {
    let mut s = Seed::new();
    let (a, b) = (
        s.work(None, "architect", Some("p")),
        s.work(None, "architect", Some("q")),
    );
    let at = |work: i64, ended: i64| R {
        work,
        cell: "H12",
        started: ended - 1_000,
        ended: Some(ended),
        agent: true,
        transition: Some("next"),
        ..R::default()
    };
    let a1 = s.round(at(a, T0 + 3 * M));
    let a2 = s.round(at(a, T0 + 3 * M + 10_000));
    let b1 = s.round(at(b, T0 + 4 * M));
    s.matter("p", "owner_words", T0, Some((a, a1)));
    s.matter("p", "owner_words", T0 + M, Some((a, a2)));
    s.matter("p", "owner_words", T0 + 2 * M, None);
    s.matter("p", "report", T0, Some((a, a1)));
    s.matter("q", "owner_words", T0, Some((b, b1)));
    assert_eq!(s.minutes("owner.reply", p()), ten(&[(3, 155_000.0)], None));
    assert_eq!(
        s.minutes("owner.reply", all()),
        ten(&[(3, 155_000.0), (4, 240_000.0)], None)
    );
    assert_eq!(s.whole("owner.reply", p()), Some(155_000.0));
    assert_eq!(s.whole("owner.reply", all()), Some(180_000.0));
}

// A stop counts once, in the minute it started, and lasts in each minute for the part of it there: a blocker until it
// ended or, still open, until now; a pause until the work went on; a pause that began before the period, still going
// on, all along.
#[test]
fn stops_count_where_they_start_and_last_where_they_are() {
    let mut s = Seed::new();
    let (i, j) = (s.item("p", false, T0 - 60 * M), s.item("q", false, T0 - 60 * M));
    let w = s.work(Some(i), "engineer", None);
    let u = s.work(Some(i), "engineer", None);
    let e = s.work(Some(i), "engineer", None);
    let q = s.work(Some(j), "engineer", None);
    s.blocker(w, "items", T0 - 5 * M, Some(T0 + M));
    s.blocker(w, "question", T0 + 2 * M + 30_000, Some(T0 + 4 * M + 30_000));
    s.blocker(w, "owner_ask", T0 + 6 * M, None);
    s.blocker(q, "question", T0 + 2 * M, Some(T0 + 3 * M));
    s.pause(u, true, T0 + 7 * M);
    s.pause(u, false, T0 + 8 * M + 20_000);
    s.pause(e, true, T0 - 10 * M);
    assert_eq!(
        s.minutes("work.stops", p()),
        ten(&[(2, 1.0), (6, 1.0), (7, 1.0)], Some(0.0))
    );
    assert_eq!(
        s.minutes("work.stops", all()),
        ten(&[(2, 2.0), (6, 1.0), (7, 1.0)], Some(0.0))
    );
    let stopped: Vec<Option<f64>> = [
        120_000.0, 60_000.0, 90_000.0, 120_000.0, 90_000.0, 60_000.0, 120_000.0, 180_000.0, 140_000.0, 120_000.0,
    ]
    .into_iter()
    .map(Some)
    .collect();
    assert_eq!(s.minutes("work.stopped", p()), stopped);
    assert_eq!(s.minutes("work.stops", of("question")), ten(&[(2, 1.0)], Some(0.0)));
    assert_eq!(
        s.minutes("work.stopped", of("question")),
        ten(&[(2, 30_000.0), (3, 60_000.0), (4, 30_000.0)], Some(0.0))
    );
    assert_eq!(s.minutes("work.stops", of("paused")), ten(&[(7, 1.0)], Some(0.0)));
    assert_eq!(
        s.minutes("work.stopped", of("paused")),
        ten(&[(7, 120_000.0), (8, 80_000.0)], Some(60_000.0))
    );
    // From 90 s to 450 s: the question (150 s to 270 s), the owner's ask from 360 s, the pause from 420 s, and the
    // pause all along.
    assert_eq!(s.whole("work.stops", p()), Some(3.0));
    assert_eq!(
        s.whole("work.stopped", p()),
        Some(120_000.0 + 90_000.0 + 30_000.0 + 360_000.0)
    );
}

// What the engine sampled: the median of the minutes of its CPU, a minute it took none having no value; the longest
// wait of the queue, of a project or of all of them.
#[test]
fn the_engine_s_samples_fold_by_their_measure() {
    let mut s = Seed::new();
    s.sample("engine.cpu", "", T0 + M, 0.2);
    s.sample("engine.cpu", "", T0 + 2 * M, 0.4);
    s.sample("engine.cpu", "", T0 + 3 * M, 0.1);
    s.sample("queue.oldest", "p", T0 + M, 5_000.0);
    s.sample("queue.oldest", "q", T0 + M, 9_000.0);
    assert_eq!(s.minutes("engine.cpu", p()), ten(&[(1, 0.2), (2, 0.4), (3, 0.1)], None));
    assert_eq!(s.whole("engine.cpu", p()), Some(0.25));
    assert_eq!(s.minutes("queue.oldest", p()), ten(&[(1, 5_000.0)], None));
    assert_eq!(s.minutes("queue.oldest", all()), ten(&[(1, 9_000.0)], None));
}

// An item's wait for CI is the time its CI runs cover before its merge. The first item's work has a ci and a ci_before
// that overlap, whose overlap counts once; a full_ci; a wait of the merge tool for the full CI that gave its checks and
// one that failed, both counted; and what is no wait for CI: the merge tool's call to merge, one that gave the commit
// and one that failed, a tool of another name, a ci still going on, and a ci that ended after the merge. The time
// covered is 170 s, where the runs of ci, ci_before and full_ci added up are 105 s. The second item's runs lie in two of
// its works, and overlap, so they are one span of 120 s. The third has none, which is a wait of 0.
#[test]
fn the_wait_for_ci_is_the_time_the_runs_of_an_item_cover_before_its_merge() {
    let mut s = Seed::new();
    let [i, j, k] = [(); 3].map(|_| s.item("p", false, T0 - 60 * M));
    let merged = |work: i64, ended: i64| R {
        work,
        cell: "M2",
        started: ended - 1_000,
        ended: Some(ended),
        transition: Some("merged"),
        ..R::default()
    };
    let w = s.work(Some(i), "engineer", None);
    let r = s.round(R {
        work: w,
        cell: "I1",
        started: T0,
        ended: Some(T0 + 1),
        transition: Some("tools"),
        ..R::default()
    });
    s.tool(w, r, "ci", T0 - 2 * M, Some(T0 - 2 * M + 30_000));
    s.tool(w, r, "ci_before", T0 - 2 * M + 10_000, Some(T0 - 2 * M + 40_000));
    s.tool(w, r, "full_ci", T0, Some(T0 + 45_000));
    let (waited, called) = (
        json!({ "step": health::FULL_CI_STEP, "head": "h", "pr": 1 }),
        json!({ "head": "h", "pr": 1 }),
    );
    let failed = json!({ "failed": { "kind": "other", "what": "no" } });
    let checks = json!([{ "name": "check", "result": "green" }]);
    let run = (w, r);
    s.tool_run(run, "merge", &waited, Some(&failed), T0 + M, Some(T0 + M + 5_000));
    s.tool_run(
        run,
        "merge",
        &waited,
        Some(&checks),
        T0 + 2 * M,
        Some(T0 + 2 * M + 80_000),
    );
    s.tool_run(
        run,
        "merge",
        &called,
        Some(&failed),
        T0 + 4 * M,
        Some(T0 + 4 * M + 7_000),
    );
    s.tool_run(
        run,
        "merge",
        &called,
        Some(&json!("abc")),
        T0 + 8 * M,
        Some(T0 + 8 * M + 9_000),
    );
    s.tool(w, r, "gh", T0, Some(T0 + 100_000));
    s.tool(w, r, "ci", T0 + 6 * M, None);
    s.tool(w, r, "ci", T0 + 9 * M - 500, Some(T0 + 9 * M + 5_000));
    s.round(merged(w, T0 + 9 * M));
    // Two works of the second item: the first's ci, and after it was taken over the second's ci_before, which overlaps.
    let (a, b) = (s.work(Some(j), "engineer", None), s.work(Some(j), "engineer", None));
    let round_of = |work: i64| R {
        work,
        cell: "I1",
        started: T0 - 10 * M,
        ended: Some(T0 - 10 * M + 1),
        transition: Some("tools"),
        ..R::default()
    };
    let (ra, rb) = (s.round(round_of(a)), s.round(round_of(b)));
    s.tool(a, ra, "ci", T0 - 5 * M, Some(T0 - 4 * M));
    s.tool(b, rb, "ci_before", T0 - 4 * M - 30_000, Some(T0 - 3 * M));
    s.round(merged(b, T0 + 7 * M));
    let c = s.work(Some(k), "engineer", None);
    s.round(merged(c, T0 + 5 * M));
    assert_eq!(
        s.minutes("ci.wait", p()),
        ten(&[(5, 0.0), (7, 120_000.0), (9, 170_000.0)], None)
    );
    // Over the whole ten minutes, the middle of the three; from 90 s up to 450 s, the merges at 300 s and 420 s.
    assert_eq!(s.values("ci.wait", p(), T0, T0 + 10 * M, 10 * M), [Some(120_000.0)]);
    assert_eq!(s.whole("ci.wait", p()), Some(60_000.0));
}

// An item's time to merge runs from the first round of any of its works, a work taken over included, to the merge: the
// median of the items merged in the minute.
#[test]
fn time_to_merge_is_the_median_of_the_items_merged() {
    let mut s = Seed::new();
    let (i, k, j) = (
        s.item("p", false, T0 - 60 * M),
        s.item("p", false, T0 - 60 * M),
        s.item("q", false, T0 - 60 * M),
    );
    let step = |work: i64, cell: &'static str, started: i64, ended: i64, transition: &'static str| R {
        work,
        cell,
        started,
        ended: Some(ended),
        transition: Some(transition),
        ..R::default()
    };
    // Item i: an earlier work, taken over, and the work that merged it.
    let b = s.work(Some(i), "engineer", None);
    s.round(step(b, "S1", T0 - 20 * M, T0 - 20 * M + 1, "next"));
    let a = s.work(Some(i), "engineer", None);
    s.round(step(a, "I1", T0, T0 + 1, "tools"));
    s.round(step(a, "M2", T0 + 9 * M - 1_000, T0 + 9 * M, "merged"));
    // Item k: one work, no CI; a merge that did not go through first.
    let f = s.work(Some(k), "engineer", None);
    s.round(step(f, "I1", T0 + 9 * M, T0 + 9 * M + 1, "next"));
    s.round(step(f, "M2", T0 + 9 * M + 2, T0 + 9 * M + 3, "conflict"));
    s.round(step(f, "M2", T0 + 9 * M + 4, T0 + 9 * M + 30_000, "merged"));
    // Item j, of another project.
    let c = s.work(Some(j), "engineer", None);
    s.round(step(c, "I1", T0 + 8 * M, T0 + 8 * M + 1, "next"));
    s.round(step(c, "M2", T0 + 9 * M, T0 + 9 * M + 1, "merged"));
    assert_eq!(
        s.minutes("work.to_merge", p()),
        ten(&[(9, (29.0 * 60_000.0 + 30_000.0) / 2.0)], None)
    );
    assert_eq!(s.minutes("items.merged", p()), ten(&[(9, 2.0)], Some(0.0)));
    assert_eq!(s.minutes("work.to_merge", all()), ten(&[(9, 60_001.0)], None));
    assert_eq!(s.minutes("items.merged", all()), ten(&[(9, 3.0)], Some(0.0)));
    assert_eq!(s.whole("work.to_merge", p()), None);
    assert_eq!(s.whole("items.merged", p()), Some(0.0));
}

// An item made here counts when it was made; one imported from an earlier board does not, though its cancellation
// here counts as one.
#[test]
fn items_made_leave_out_those_imported() {
    let mut s = Seed::new();
    let i1 = s.item("p", false, T0 + M + 5_000);
    s.item("p", true, T0 + M + 10_000);
    s.item("q", false, T0 + 2 * M);
    let i4 = s.item("p", false, T0 + 3 * M);
    let i5 = s.item("p", false, T0 - 30 * M);
    s.cancel(i5, T0 + 4 * M + 1);
    let i6 = s.item("p", true, T0 - 30 * M);
    s.cancel(i6, T0 + 6 * M);
    assert_eq!(s.minutes("items.made", p()), ten(&[(1, 1.0), (3, 1.0)], Some(0.0)));
    assert_eq!(
        s.minutes("items.made", all()),
        ten(&[(1, 1.0), (2, 1.0), (3, 1.0)], Some(0.0))
    );
    assert_eq!(s.minutes("items.cancelled", p()), ten(&[(4, 1.0), (6, 1.0)], Some(0.0)));
    assert_eq!(s.whole("items.made", p()), Some(1.0));
    assert_eq!(s.whole("items.cancelled", p()), Some(2.0));
    assert_eq!(
        health::items_made(s.conn(), Some("p"), T0, T0 + 10 * M).unwrap(),
        [(i1, T0 + M + 5_000), (i4, T0 + 3 * M)]
    );
}

// What the rounds cost counts when each started, by the graph of its work: the items' works and the architect's.
#[test]
fn what_was_spent_counts_by_the_graph_when_each_round_started() {
    let mut s = Seed::new();
    let i = s.item("p", false, T0 - 60 * M);
    let w = s.work(Some(i), "engineer", None);
    let a = s.work(None, "architect", Some("p"));
    let cost = |work: i64, started: i64, usd: Option<f64>| R {
        work,
        cell: "I1",
        started,
        ended: Some(T0 + 20 * M),
        agent: true,
        usage: Some(usd.map_or(json!({ "turns": 1 }), |c| json!({ "cost_usd": c }))),
        transition: Some("next"),
        ..R::default()
    };
    s.round(cost(w, T0, Some(0.5)));
    s.round(cost(w, T0 + M + 10_000, Some(0.25)));
    s.round(cost(w, T0 + M + 20_000, None));
    s.round(cost(a, T0 + 2 * M, Some(1.0)));
    assert_eq!(
        s.minutes("spend.usd", p()),
        ten(&[(0, 0.5), (1, 0.25), (2, 1.0)], Some(0.0))
    );
    assert_eq!(
        s.minutes("spend.usd", of("engineer")),
        ten(&[(0, 0.5), (1, 0.25)], Some(0.0))
    );
    assert_eq!(s.minutes("spend.usd", of("architect")), ten(&[(2, 1.0)], Some(0.0)));
    assert_eq!(s.whole("spend.usd", p()), Some(1.0));
}

/// Runs git in a directory, a commit it makes made at `at` (ms), by its author's and its committer's time; gives what
/// it printed.
fn git(dir: &Path, args: &[&str], at: i64) -> String {
    let when = format!("@{} +0000", at / 1000);
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@localhost",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env("GIT_AUTHOR_DATE", &when)
        .env("GIT_COMMITTER_DATE", &when)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

// Item 231, A7: main holds two merges by the flow, the second only as the second parent of a merge made by hand; a commit
// pushed straight to it; that merge by hand with the two commits it brought, one made in the period before; and the
// engine's revert of a merge, which its record names. The flow also recorded two merges main does not hold, one ended
// before main's tip was made and one after. The flow's merges count all four it recorded. The commits without the flow
// count the one pushed, the two brought and the merge itself, each when it was made, and neither the flow's merges nor
// the revert. The merge main does not hold counts where its round ended; the one that ended after main's tip was made
// is not judged, as main as read could not hold it. Without main's commits read, those two say why they have no values.
#[test]
fn commits_come_onto_main_by_the_flow_by_hand_or_by_the_engine_s_revert() {
    let mut s = Seed::new();
    let dir = std::env::temp_dir().join(format!("health-main-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"], T0);
    let commit = |message: &str, at: i64| {
        git(&dir, &["commit", "-q", "--allow-empty", "-m", message], at);
        git(&dir, &["rev-parse", "HEAD"], at)
    };
    commit("start", T0 - 60 * M);
    let a = commit("Merge PR 1", T0 + M + 5_000);
    let pushed = commit("Pushed straight to main", T0 + 2 * M + 5_000);
    let trunk = git(&dir, &["rev-parse", "--abbrev-ref", "HEAD"], T0);
    git(&dir, &["checkout", "-q", "-b", "side"], T0);
    let older = commit("On the side, made earlier", T0 - 5 * M);
    let later = commit("On the side", T0 + 3 * M + 5_000);
    git(&dir, &["checkout", "-q", &trunk], T0);
    let b = commit("Merge PR 2", T0 + 3 * M + 30_000);
    git(&dir, &["checkout", "-q", "side"], T0);
    // A merge by hand made from the side line: what the flow merged is its second parent.
    git(
        &dir,
        &["merge", "-q", "--no-ff", "-m", "By hand", &trunk],
        T0 + 4 * M + 5_000,
    );
    let by_hand = git(&dir, &["rev-parse", "HEAD"], T0);
    let revert = commit("Revert PR 2", T0 + 5 * M + 5_000);
    let first_parents = git(&dir, &["log", "--first-parent", "--format=%H", "HEAD"], T0);
    assert!(!first_parents.contains(&b), "{first_parents}");
    let mut log = MainLog::GIT_LOG.to_vec();
    log.push("HEAD");
    let main = MainLog::parse(&git(&dir, &log, T0)).unwrap();
    for held in [&a, &pushed, &older, &later, &b, &by_hand, &revert] {
        assert!(main.holds(held), "{held}");
    }

    let i = s.item("p", false, T0 - 60 * M);
    let w = s.work(Some(i), "engineer", None);
    let merged = |commit: &str, ended: i64| R {
        work: w,
        cell: "M2",
        started: ended - 1_000,
        ended: Some(ended),
        transition: Some("merged"),
        output: Some(json!({ "result": { "kind": "merged", "merge_commit": commit } })),
        ..R::default()
    };
    s.round(merged(&a, T0 + M + 10_000));
    s.round(merged(&b, T0 + 3 * M + 40_000));
    s.round(merged(&"f".repeat(40), T0 + 4 * M));
    s.round(merged(&"e".repeat(40), T0 + 6 * M));
    let architect = s.work(None, "architect", Some("p"));
    s.revert(architect, &b, &revert, T0 + 5 * M + 5_000);
    let read = Scope {
        main: Some(&main),
        ..p()
    };
    assert_eq!(
        s.minutes("items.merged", p()),
        ten(&[(1, 1.0), (3, 1.0), (4, 1.0), (6, 1.0)], Some(0.0))
    );
    assert_eq!(s.values("items.merged", p(), T0, T0 + 10 * M, 10 * M), [Some(4.0)]);
    assert_eq!(
        s.minutes("main.outside_flow", read),
        ten(&[(2, 1.0), (3, 1.0), (4, 1.0)], Some(0.0))
    );
    // The commit brought that was made earlier falls in the period before; the first commit, an hour before, in neither.
    assert_eq!(
        s.values("main.outside_flow", read, T0 - 10 * M, T0, 10 * M),
        [Some(1.0)]
    );
    assert_eq!(
        s.values("main.outside_flow", read, T0 - 10 * M, T0 + 10 * M, 20 * M),
        [Some(4.0)]
    );
    assert_eq!(s.minutes("merge.not_on_main", read), ten(&[(4, 1.0)], Some(0.0)));
    assert_eq!(
        s.values("merge.not_on_main", read, T0, T0 + 10 * M, 10 * M),
        [Some(1.0)]
    );
    assert_eq!(s.minutes("merge.reverted", p()), ten(&[(5, 1.0)], Some(0.0)));
    // With no commits of main given, or with no project to judge by them.
    let no_project = Scope {
        main: Some(&main),
        ..all()
    };
    for name in ["main.outside_flow", "merge.not_on_main"] {
        for scope in [p(), no_project] {
            let unread = series(s.conn(), measure(name).unwrap(), &scope, T0, T0 + 10 * M, M).unwrap();
            assert!(unread.not_measured.is_some(), "{name}: {unread:?}");
            assert!(unread.points.iter().all(|x| x.value.is_none()), "{name}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// A commit on main counts by its committer time, whatever the order of the times along main: main's newest commit
// was made before the ten minutes, and the two commits under it that came without the flow were made within them, so
// a reading that stopped at the first commit made before the period, as git's `--since` does, would find neither. The
// flow's merge between them is not one of them.
#[test]
fn a_commit_on_main_counts_by_its_committer_time_whatever_its_order_on_main() {
    let mut s = Seed::new();
    let dir = std::env::temp_dir().join(format!("health-order-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"], T0);
    let commit = |message: &str, at: i64| {
        git(&dir, &["commit", "-q", "--allow-empty", "-m", message], at);
        git(&dir, &["rev-parse", "HEAD"], at)
    };
    // Along main, the oldest first: made an hour before, at 125 s, at 300 s by the flow, at 420 s, and the newest
    // made 5 minutes before T0, as a commit made earlier and pushed by hand last would be.
    commit("start", T0 - 60 * M);
    commit("By hand", T0 + 125_000);
    let merge = commit("Merge PR 1", T0 + 300_000);
    commit("By hand again", T0 + 420_000);
    commit("Made earlier, pushed last", T0 - 5 * M);
    let mut log = MainLog::GIT_LOG.to_vec();
    log.push("HEAD");
    let main = MainLog::parse(&git(&dir, &log, T0)).unwrap();
    let i = s.item("p", false, T0 - 60 * M);
    let w = s.work(Some(i), "engineer", None);
    s.round(R {
        work: w,
        cell: "M2",
        started: T0 + 300_000,
        ended: Some(T0 + 305_000),
        transition: Some("merged"),
        output: Some(json!({ "result": { "kind": "merged", "merge_commit": merge } })),
        ..R::default()
    });
    let read = Scope {
        main: Some(&main),
        ..p()
    };
    assert_eq!(
        s.minutes("main.outside_flow", read),
        ten(&[(2, 1.0), (7, 1.0)], Some(0.0))
    );
    // From 90 s up to 450 s: the commits at 125 s and 420 s.
    assert_eq!(s.whole("main.outside_flow", read), Some(2.0));
    let _ = std::fs::remove_dir_all(&dir);
}

// A merge the flow recorded whose commit main does not hold is judged when its round ended no later than main's newest
// commit was made, the end at that very moment included, and not 1 ms later; a merge whose commit main holds is not one,
// nor is another project's. Main's commits are given as git gives them, its newest made at 300 s.
#[test]
fn a_merge_ending_as_main_s_newest_commit_was_made_is_judged() {
    let mut s = Seed::new();
    let (on, tip) = ("a".repeat(40), "b".repeat(40));
    let log = format!("{tip} {}\n{on} {}\n", (T0 + 300_000) / 1000, (T0 + 60_000) / 1000);
    let main = MainLog::parse(&log).unwrap();
    let (i, j) = (s.item("p", false, T0 - 60 * M), s.item("q", false, T0 - 60 * M));
    let (w, v) = (s.work(Some(i), "engineer", None), s.work(Some(j), "engineer", None));
    let merged = |work: i64, commit: &str, ended: i64| R {
        work,
        cell: "M2",
        started: ended - 1_000,
        ended: Some(ended),
        transition: Some("merged"),
        output: Some(json!({ "result": { "kind": "merged", "merge_commit": commit } })),
        ..R::default()
    };
    s.round(merged(w, &on, T0 + 120_000));
    s.round(merged(w, &"c".repeat(40), T0 + 300_000));
    s.round(merged(w, &"d".repeat(40), T0 + 300_001));
    s.round(merged(v, &"e".repeat(40), T0 + 200_000));
    let read = Scope {
        main: Some(&main),
        ..p()
    };
    assert_eq!(s.minutes("merge.not_on_main", read), ten(&[(5, 1.0)], Some(0.0)));
    assert_eq!(s.whole("merge.not_on_main", read), Some(1.0));
    // The newest commit came without the flow; the other is the merge the flow recorded.
    assert_eq!(s.minutes("main.outside_flow", read), ten(&[(5, 1.0)], Some(0.0)));
    assert_eq!(s.minutes("items.merged", p()), ten(&[(2, 1.0), (5, 2.0)], Some(0.0)));
}

// Every measure of the registry gives a series over seeded records, of its unit and with a point at each step.
#[test]
fn every_measure_gives_a_series() {
    let s = Seed::new();
    for m in health::measures() {
        let got = series(s.conn(), m, &all(), T0, T0 + 10 * M, M).unwrap();
        assert_eq!(got.points.len(), 10, "{}", m.name);
        assert_eq!(got.points[0].from, T0, "{}", m.name);
        assert_eq!(got.points[9].to, T0 + 10 * M, "{}", m.name);
    }
}

// A period that is no whole number of steps and does not start on a minute: its points start at the period's start,
// a step apart, and its last point ends at the period's end, holding only what fell before it. From 30 s up to 190 s
// at a step of a minute, the points are [30 s, 90 s), [90 s, 150 s) and [150 s, 190 s).
#[test]
fn a_period_of_no_whole_number_of_steps_ends_its_last_point_at_its_end() {
    let mut s = Seed::new();
    let (from, to) = (T0 + 30_000, T0 + 190_000);
    // Made 1 ms before the period, at its first millisecond, within the first point, at the start of the last point,
    // and at the period's end, which is no longer in it.
    for at in [from - 1, from, T0 + 89_999, T0 + 150_000, to] {
        s.item("p", false, at);
    }
    let i = s.item("p", false, T0 - 60 * M);
    let (w, v) = (s.work(Some(i), "engineer", None), s.work(Some(i), "engineer", None));
    // A question from 60 s to 170 s, and an ask of the owner from 100 s still open.
    s.blocker(w, "question", T0 + 60_000, Some(T0 + 170_000));
    s.blocker(v, "owner_ask", T0 + 100_000, None);
    // The engine's CPU sampled at the start of each minute; the one at 0 s is before the period.
    for (k, cpu) in [0.1, 0.2, 0.3, 0.4].into_iter().enumerate() {
        s.sample("engine.cpu", "", T0 + k as i64 * M, cpu);
    }
    let m = measure("items.made").unwrap();
    let made = series(s.conn(), m, &p(), from, to, M).unwrap();
    let bounds: Vec<(i64, i64)> = made.points.iter().map(|x| (x.from, x.to)).collect();
    assert_eq!(
        bounds,
        [(from, T0 + 90_000), (T0 + 90_000, T0 + 150_000), (T0 + 150_000, to)]
    );
    let ns: Vec<i64> = made.points.iter().map(|x| x.n).collect();
    assert_eq!(ns, [2, 0, 1]);
    assert_eq!(
        s.values("items.made", p(), from, to, M),
        [Some(2.0), Some(0.0), Some(1.0)]
    );
    assert_eq!(
        s.values("work.stopped", p(), from, to, M),
        [Some(30_000.0), Some(50_000.0 + 60_000.0), Some(20_000.0 + 40_000.0)]
    );
    assert_eq!(
        s.values("work.stops", p(), from, to, M),
        [Some(1.0), Some(1.0), Some(0.0)]
    );
    assert_eq!(
        s.values("engine.cpu", p(), from, to, M),
        [Some(0.2), Some(0.3), Some(0.4)]
    );
}

// A pause lasts only for the part of it within the period, wherever it began and ended: one that began before the
// period, ended within it and began again, still going on; one that began within it and ended after it; one of a work
// that ended while paused; and neither one that began and ended before the period nor one that began after it. A
// blocker open since before the period lasts all of it and counts in none of its points.
#[test]
fn a_pause_that_turns_outside_the_period_lasts_only_within_it() {
    let mut s = Seed::new();
    let i = s.item("p", false, T0 - 60 * M);
    let [u1, u2, u3, u4, u5] = [(); 5].map(|_| s.work(Some(i), "engineer", None));
    // u1: paused from 10 minutes before, on again at 150 s, paused again at 360 s and still paused.
    s.pause(u1, true, T0 - 10 * M);
    s.pause(u1, false, T0 + 150_000);
    s.pause(u1, true, T0 + 6 * M);
    // u2: paused at 300 s, on again 15 minutes after T0.
    s.pause(u2, true, T0 + 5 * M);
    s.pause(u2, false, T0 + 15 * M);
    // u3: paused from 3 minutes before, ended at 240 s while paused.
    s.pause(u3, true, T0 - 3 * M);
    s.sql("UPDATE works SET ended = 'done' WHERE id = ?1", [u3]);
    s.restamp("works", u3, T0 + 4 * M);
    // u4: paused and on again before the period; a ruling it waits for since 2 minutes before, still open.
    s.pause(u4, true, T0 - 8 * M);
    s.pause(u4, false, T0 - M);
    s.blocker(u4, "ruling", T0 - 2 * M, None);
    // u5: paused 12 minutes after T0, after the ten minutes, and still paused.
    s.pause(u5, true, T0 + 12 * M);
    let paused = [
        120_000.0, 120_000.0, 90_000.0, 60_000.0, 0.0, 60_000.0, 120_000.0, 120_000.0, 120_000.0, 120_000.0,
    ];
    assert_eq!(s.minutes("work.stopped", of("paused")), paused.map(Some).to_vec());
    assert_eq!(
        s.minutes("work.stops", of("paused")),
        ten(&[(5, 1.0), (6, 1.0)], Some(0.0))
    );
    assert_eq!(s.minutes("work.stopped", of("ruling")), ten(&[], Some(60_000.0)));
    assert_eq!(s.minutes("work.stops", of("ruling")), ten(&[], Some(0.0)));
    assert_eq!(
        s.minutes("work.stopped", p()),
        paused.map(|x| Some(x + 60_000.0)).to_vec()
    );
    // From 90 s to 450 s: u1 from 90 s to 150 s and from 360 s, u2 from 300 s, u3 from 90 s to 240 s.
    assert_eq!(
        s.whole("work.stopped", of("paused")),
        Some(60_000.0 + 90_000.0 + 150_000.0 + 150_000.0)
    );
    assert_eq!(s.whole("work.stops", of("paused")), Some(2.0));
    assert_eq!(s.whole("work.stopped", of("ruling")), Some(360_000.0));
    assert_eq!(s.whole("work.stops", p()), Some(2.0));
}
