//! What the engine samples once a minute, since no record holds it.

use super::*;

/// Reads a path of the owner's page served on a port, as the page does, and gives the whole answer.
fn page_read(port: u16, path: &str) -> String {
    use std::io::{Read as _, Write as _};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let ask = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    s.write_all(ask.as_bytes()).unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    out
}

/// Asks the owner's page's server something as the page does, on a connection that stays open, and gives the connection
/// with the start of the answer.
fn page_opened(port: u16, method: &str, path: &str) -> (std::net::TcpStream, String) {
    use std::io::{Read as _, Write as _};
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let ask = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: 0\r\n\r\n");
    s.write_all(ask.as_bytes()).unwrap();
    let mut start = [0; 256];
    let n = s.read(&mut start).unwrap();
    let start = String::from_utf8_lossy(&start[..n]).to_string();
    (s, start)
}

/// What the samples hold under a minute, by each one's measure and project.
fn samples_at(e: &Engine, at: i64) -> BTreeMap<String, f64> {
    let mut st =
        e.db.conn()
            .prepare("SELECT measure || ':' || project, value FROM samples WHERE at = ?1")
            .unwrap();
    st.query_map([at], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

// An engine that starts again keeps what the minutes it finds hold, and measures only from its own first sample. The
// engine before took samples at 5 s and at 65 s, the second keeping the first's amounts under minute 0 and its states
// under minute 1. The next, started at 70 s, takes its first sample in minute 1: what that minute holds stays, it keeps
// no amounts, and the reads served until then wait for its next sample, which keeps them under minute 1 with those
// after. Minute 0 stays as it was, and the reads add up over the period. Worked out by hand from the design's section 4
// and acceptance A3.
#[test]
fn an_engine_started_again_within_a_minute_keeps_what_that_minute_holds() {
    let s = setup("beat-again");
    let (mut first, _) = engine(&s, scripts());
    let t = health::minute(db::now()) + 5_000;
    first
        .db
        .write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO architect_matters (project, kind, what, body, at) VALUES ('p', 'owner_words', 'Look.', '{}', ?1)",
                [t - 90_000],
            )?;
            Ok(())
        })
        .unwrap();
    let ten_ms = Duration::from_millis(10);
    let reads = health::PageReads::default();
    assert!(first.beat(t, &reads).unwrap());
    reads.add(ten_ms);
    reads.add(ten_ms);
    assert!(first.beat(t + 60_000, &reads).unwrap());
    let (m0, m1, m2) = (
        health::minute(t),
        health::minute(t) + health::MINUTE,
        health::minute(t) + 2 * health::MINUTE,
    );
    let (zero, one) = (samples_at(&first, m0), samples_at(&first, m1));
    assert_eq!(zero["page.reads:"], 2.0, "{zero:?}");
    assert_eq!(zero["page.read_ms:"], 20.0, "{zero:?}");
    assert!(zero.contains_key("engine.cpu:"), "{zero:?}");
    assert_eq!(one["queue.oldest:p"], 150_000.0, "{one:?}");
    assert_eq!(one.len(), 3, "{one:?}");
    drop(first);
    let (mut again, _) = engine(&s, scripts());
    let reads = health::PageReads::default();
    reads.add(ten_ms);
    assert!(again.beat(t + 65_000, &reads).unwrap());
    assert_eq!(samples_at(&again, m0), zero);
    assert_eq!(samples_at(&again, m1), one);
    reads.add(ten_ms);
    assert!(again.beat(t + 120_000, &reads).unwrap());
    assert_eq!(samples_at(&again, m0), zero);
    let then = samples_at(&again, m1);
    assert_eq!(then["page.reads:"], 2.0, "{then:?}");
    assert_eq!(then["page.read_ms:"], 20.0, "{then:?}");
    assert!(then.contains_key("engine.cpu:"), "{then:?}");
    assert_eq!(then["queue.oldest:p"], 150_000.0, "{then:?}");
    assert_eq!(samples_at(&again, m2)["queue.oldest:p"], 210_000.0);
    let m = health::measure("page.reads").unwrap();
    let scope = health::Scope::default();
    let read = health::series(again.db.conn(), m, &scope, m0, m2, m2 - m0).unwrap();
    assert_eq!(read.points[0].value, Some(4.0));
    let _ = std::fs::remove_dir_all(&s.root);
}

// One work goes the whole flow to its merge, on the records the engine really writes. While its merge call is held, a
// sample finds the one work holding a slot and with a step running. Once it merged, the numbers read those records: the
// commit the merge cell recorded is on main and is not among the commits outside the flow, which are all the others; the
// merge counts once and is on main; its time to merge runs from the first round of the work to the merge cell's end;
// its wait for CI is the time the runs recorded cover, among them the merge tool's marked wait for the full CI.
#[test]
fn a_work_in_its_merge_is_sampled_and_its_merge_counted_on_the_records_it_wrote() {
    let s = setup("beat-merge");
    let (mut e, said, item) = start(&s, scripts());
    let log = |said: &Mutex<Vec<String>>| said.lock().unwrap().join("\n");
    let at = turns_until(&mut e, &Go::default(), |e| state(e, item).is_some_and(|w| w.1 == "M2"));
    assert!(at, "{}", log(&said));
    let work = state(&e, item).unwrap().0;
    s.merging.shut();
    let held = turns_until(&mut e, &Go::default(), |_| s.merging.came() > 0);
    assert!(held, "{}", log(&said));
    let t = db::now();
    assert!(e.beat(t, &health::PageReads::default()).unwrap());
    let sampled = samples_at(&e, health::minute(t));
    assert_eq!(sampled["slots.used:"], 1.0, "{sampled:?}");
    assert_eq!(sampled["works.running:"], 1.0, "{sampled:?}");
    s.merging.open();
    let ended = turns_until(&mut e, &Go::default(), |e| {
        transitions_at(e, work, "M2").iter().any(Option::is_some)
    });
    assert!(ended, "{}", log(&said));
    e.drain().unwrap();
    assert_eq!(
        transitions_at(&e, work, "M2"),
        [Some("merged".to_string())],
        "{}",
        log(&said)
    );
    let conn = e.db.conn();
    let (commit, merged_at): (String, i64) = conn
        .query_row(
            "SELECT json_extract(output, '$.result.merge_commit'), ended_at FROM rounds
             WHERE work = ?1 AND cell = 'M2' AND transition = 'merged'",
            [work],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let first_round: i64 = conn
        .query_row("SELECT min(started_at) FROM rounds WHERE work = ?1", [work], |r| {
            r.get(0)
        })
        .unwrap();
    let marked: i64 = conn
        .query_row(
            "SELECT count(*) FROM tool_runs WHERE work = ?1 AND tool = 'merge' AND json_extract(args, '$.step') = ?2",
            params![work, health::FULL_CI_STEP],
            |r| r.get(0),
        )
        .unwrap();
    assert!(marked >= 1, "{}", log(&said));
    let mut st = conn
        .prepare(
            "SELECT queued_at, ended_at FROM tool_runs
             WHERE work = ?1 AND ended_at IS NOT NULL AND ended_at <= ?2
               AND (tool IN ('ci', 'ci_before', 'full_ci')
                    OR (tool = 'merge' AND json_extract(args, '$.step') = ?3))
             ORDER BY queued_at, ended_at",
        )
        .unwrap();
    let runs: Vec<(i64, i64)> = st
        .query_map(params![work, merged_at, health::FULL_CI_STEP], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut covered = 0;
    let mut reached = i64::MIN;
    for (from, to) in runs {
        covered += (to - from.max(reached)).max(0);
        reached = reached.max(to);
    }
    let mut read = health::MainLog::GIT_LOG.to_vec();
    read.push("main");
    let main = health::MainLog::parse(&git(&s.origin, &read)).unwrap();
    assert!(main.holds(&commit), "{commit}");
    let scope = health::Scope {
        project: Some("p"),
        subject: None,
        main: Some(&main),
    };
    let (from, to) = (db::now() - 3_600_000, db::now() + 60_000);
    let whole = |name: &str| {
        let m = health::measure(name).unwrap();
        health::series(conn, m, &scope, from, to, to - from).unwrap().points[0].value
    };
    assert_eq!(whole("items.merged"), Some(1.0));
    assert_eq!(whole("merge.not_on_main"), Some(0.0));
    let commits: f64 = git(&s.origin, &["rev-list", "--count", "main"]).parse().unwrap();
    assert_eq!(whole("main.outside_flow"), Some(commits - 1.0));
    assert_eq!(whole("work.to_merge"), Some((merged_at - first_round) as f64));
    assert_eq!(whole("ci.wait"), Some(covered as f64));
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item 231, A3: once a minute the engine samples what no record holds: the slots held, the works running and each
// project's oldest wait in its queue under the sample's minute; and the reads the page served and its CPU since the
// sample before, under the minute of that one, from its second sample on, which has one before it to be measured from.
// A second sample in the same minute writes nothing. Over any period, the samples fold as each measure says.
#[test]
fn the_engine_samples_what_no_record_holds_once_a_minute() {
    let s = setup("beat");
    let (mut e, _) = engine(&s, scripts());
    let reads = Arc::new(health::PageReads::default());
    let port = web::start(web::Options {
        data: s.data.clone(),
        db: config::database(&s.data),
        port: 0,
        bus: Arc::new(api::Bus::new(db::now())),
        control: Arc::new(api::Control::default()),
        reads: Arc::clone(&reads),
    })
    .unwrap();
    // Five seconds into a minute, with a matter that came 90 s before waiting in project p's queue.
    let t = health::minute(db::now()) + 5_000;
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO architect_matters (project, kind, what, body, at) VALUES ('p', 'owner_words', 'Look.', '{}', ?1)",
            [t - 90_000],
        )?;
        Ok(())
    })
    .unwrap();
    let sampled = |e: &Engine, at: i64| -> BTreeMap<String, f64> {
        let mut st =
            e.db.conn()
                .prepare("SELECT measure || ':' || project, value FROM samples WHERE at = ?1")
                .unwrap();
        st.query_map([at], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    let first = BTreeMap::from([
        ("slots.used:".to_string(), 0.0),
        ("works.running:".to_string(), 0.0),
        ("queue.oldest:p".to_string(), 90_000.0),
    ]);
    assert!(e.beat(t, &reads).unwrap());
    assert_eq!(sampled(&e, health::minute(t)), first);
    // The event stream, which stays open, and a POST are no reads of the page.
    let (stream, answer) = page_opened(port, "GET", "/api/v1/events");
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    let (_posted, answer) = page_opened(port, "POST", "/api/v1/engine/pause");
    assert!(answer.starts_with("HTTP/1.1 415"), "{answer}");
    for path in ["/api/v1/works", "/api/v1/items", "/"] {
        assert!(page_read(port, path).starts_with("HTTP/1.1 200"), "{path}");
    }
    assert!(!e.beat(t + 20_000, &reads).unwrap());
    assert_eq!(sampled(&e, health::minute(t)), first);
    assert!(e.beat(t + 60_000, &reads).unwrap());
    // The reads and the CPU from the first sample on fall in its minute, in which the reads were made.
    let then = sampled(&e, health::minute(t));
    assert_eq!(then["page.reads:"], 3.0, "{then:?}");
    assert!(then["page.read_ms:"] > 0.0, "{then:?}");
    assert!(then["engine.cpu:"] >= 0.0, "{then:?}");
    assert_eq!(then.len(), 6, "{then:?}");
    let next = sampled(&e, health::minute(t) + 60_000);
    assert_eq!(next["queue.oldest:p"], 150_000.0, "{next:?}");
    assert_eq!(next.len(), 3, "{next:?}");
    // Over the two minutes as one point: the reads add up, and the queue's wait is the longer of the two.
    let (from, to) = (health::minute(t), health::minute(t) + 2 * health::MINUTE);
    let whole = |name: &str| {
        let scope = health::Scope {
            project: Some("p"),
            ..health::Scope::default()
        };
        let m = health::measure(name).unwrap();
        health::series(e.db.conn(), m, &scope, from, to, to - from)
            .unwrap()
            .points[0]
            .value
    };
    assert_eq!(whole("page.reads"), Some(3.0));
    assert_eq!(whole("queue.oldest"), Some(150_000.0));
    assert_eq!(whole("slots.used"), Some(0.0));
    assert_eq!(whole("works.running"), Some(0.0));
    assert_eq!(whole("engine.cpu"), Some(then["engine.cpu:"]));
    drop(stream);
    let _ = std::fs::remove_dir_all(&s.root);
}
