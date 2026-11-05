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
    let _ = std::fs::remove_dir_all(&s.root);
}
