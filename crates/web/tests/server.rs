//! The interface over a real socket: what it serves, what it refuses, and the event stream.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use api::{Bus, Control, Event};

struct Server {
    port: u16,
    bus: Arc<Bus>,
    control: Arc<Control>,
    dir: PathBuf,
}

fn serve(name: &str) -> Server {
    let dir = std::env::temp_dir().join(format!("wf-agents-web-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("db")).unwrap();
    let path = dir.join("db/wf-agents.sqlite");
    db::Db::create(&path).unwrap();
    let bus = Arc::new(Bus::new(5));
    let control = Arc::new(Control::default());
    let port = web::start(web::Options {
        data: dir.clone(),
        db: path,
        port: 0,
        bus: Arc::clone(&bus),
        control: Arc::clone(&control),
    })
    .unwrap();
    Server {
        port,
        bus,
        control,
        dir,
    }
}

/// Sends a request and reads the whole answer: its status and its text.
fn send(port: u16, request: &str) -> (u16, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    let status = out.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    (status, out)
}

fn get(port: u16, path: &str, host: &str, extra: &str) -> (u16, String) {
    send(
        port,
        &format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n{extra}\r\n"),
    )
}

fn post(port: u16, path: &str, body: &str, kind: &str) -> (u16, String) {
    let host = format!("127.0.0.1:{port}");
    send(
        port,
        &format!(
            "POST {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    )
}

#[test]
fn it_serves_its_own_address_and_refuses_other_sites() {
    let s = serve("guard");
    let here = format!("127.0.0.1:{}", s.port);
    let (status, text) = get(s.port, "/api/v1/works", &here, "");
    assert_eq!(status, 200, "{text}");
    assert!(text.ends_with("[]"), "{text}");
    assert_eq!(
        get(s.port, "/api/v1/works", &format!("localhost:{}", s.port), "").0,
        200
    );
    assert_eq!(get(s.port, "/api/v1/works", "evil.example:80", "").0, 403);
    assert_eq!(
        get(s.port, "/api/v1/works", &format!("evil.example:{}", s.port), "").0,
        403
    );
    assert_eq!(
        get(s.port, "/api/v1/works", &here, "Origin: https://evil.example\r\n").0,
        403
    );
    let ours = format!("Origin: http://{here}\r\n");
    assert_eq!(get(s.port, "/api/v1/works", &here, &ours).0, 200);
    assert_eq!(
        get(s.port, "/api/v1/works", &here, "Sec-Fetch-Site: cross-site\r\n").0,
        403
    );
    assert_eq!(
        get(s.port, "/api/v1/works", &here, "Sec-Fetch-Site: same-origin\r\n").0,
        200
    );
    assert_eq!(get(s.port, "/api/v1/works/7", &here, "").0, 404);
    assert_eq!(get(s.port, "/api/v1/nothing", &here, "").0, 404);
    assert_eq!(get(s.port, "/api/v1/files/logs/..", &here, "").0, 404);
    // The page answers every other path, so the client routes it.
    let (status, page) = get(s.port, "/works/3", &here, "");
    assert_eq!(status, 200);
    assert!(page.contains("text/html"), "{page}");
    assert_eq!(get(s.port, "/assets/missing.js", &here, "").0, 404);
    let _ = std::fs::remove_dir_all(&s.dir);
}

#[test]
fn the_owner_starts_and_pauses_every_work_and_runs_one_work_s_round() {
    let s = serve("mode");
    let mut db = db::Db::open(&s.dir.join("db/wf-agents.sqlite")).unwrap();
    db.write(db::Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', 'change', '1')",
            [],
        )?;
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        tx.execute("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', '1')", [])?;
        tx.execute("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', '1')", [])?;
        tx.execute(
            "UPDATE works SET paused = 'cell 1: no', paused_by = 'engine' WHERE id = 2",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let paused_by = |db: &db::Db, id: i64| -> Option<String> {
        db.conn()
            .query_row("SELECT paused_by FROM works WHERE id = ?1", [id], |r| r.get(0))
            .unwrap()
    };
    s.bus.publish(Event::Status {
        status: Box::new(status()),
    });
    assert_eq!(
        post(
            s.port,
            "/api/v1/engine/start",
            "start",
            "application/x-www-form-urlencoded"
        )
        .0,
        415
    );
    // Every work starts paused, waiting sessions kept warm; the owner runs one work's round, once at a time, in a new
    // session when asked.
    assert!(s.control.asked().paused && s.control.asked().keep_warm);
    assert_eq!(post(s.port, "/api/v1/works/3/step", "{}", "application/json").0, 202);
    assert_eq!(s.control.asked().step, Some(3));
    assert_eq!(
        post(
            s.port,
            "/api/v1/works/4/step",
            r#"{"new_session":true}"#,
            "application/json"
        )
        .0,
        409
    );
    s.control.stepped(Some(3));
    assert!(s.control.take_new_sessions().is_empty());
    assert_eq!(
        post(
            s.port,
            "/api/v1/works/4/step",
            r#"{"new_session":true}"#,
            "application/json"
        )
        .0,
        202
    );
    assert_eq!(s.control.take_new_sessions(), vec![Some(4)]);
    s.control.stepped(Some(4));
    // The switch that keeps waiting sessions warm is the owner's, written so.
    let (status, done) = post(
        s.port,
        "/api/v1/engine/keep-warm",
        r#"{"on":false}"#,
        "application/json",
    );
    assert_eq!(status, 200, "{done}");
    assert!(!s.control.asked().keep_warm);
    assert!(!api::keep_warm_paused(db.conn()).unwrap());
    // The owner starts a work while every work is paused: it goes on, its pause to be lifted by the engine; a work that
    // is not there cannot be started.
    let (status, done) = post(
        s.port,
        "/api/v1/works/2/start",
        r#"{"new_session":true}"#,
        "application/json",
    );
    assert_eq!(status, 200, "{done}");
    assert!(s.control.asked().started.contains(&2) && s.control.asked().releasing.contains(&2));
    assert_eq!(s.control.take_new_sessions(), vec![Some(2)]);
    assert_eq!(post(s.port, "/api/v1/works/9/start", "{}", "application/json").0, 404);
    // Its Pause ends its Start; the engine's pause stays.
    let (status, done) = post(s.port, "/api/v1/works/2/pause", "{}", "application/json");
    assert_eq!(status, 200, "{done}");
    assert!(done.ends_with(r#"{"done":"work 2 stays paused"}"#), "{done}");
    assert!(s.control.asked().started.is_empty() && s.control.asked().releasing.is_empty());
    // The owner pauses one work; Start all does not lift it, nor the engine's pause.
    let (status, done) = post(s.port, "/api/v1/works/1/pause", "{}", "application/json");
    assert_eq!(status, 200, "{done}");
    assert_eq!(paused_by(&db, 1).as_deref(), Some("owner"));
    let (status, done) = post(s.port, "/api/v1/engine/start", "{}", "application/json");
    assert_eq!(status, 200, "{done}");
    assert!(!s.control.asked().paused);
    assert_eq!(paused_by(&db, 1).as_deref(), Some("owner"));
    assert_eq!(paused_by(&db, 2).as_deref(), Some("engine"));
    // The architect is started and paused the same way.
    let (status, done) = post(
        s.port,
        "/api/v1/architect/start",
        r#"{"new_session":true}"#,
        "application/json",
    );
    assert_eq!(status, 200, "{done}");
    assert!(s.control.asked().architect_started);
    assert_eq!(s.control.take_new_sessions(), vec![None]);
    let (status, done) = post(s.port, "/api/v1/architect/pause", "{}", "application/json");
    assert_eq!(status, 200, "{done}");
    assert!(!s.control.asked().architect_started);
    // A work that does not go on by itself is stepped while the others go on; the status says what the owner asked.
    assert_eq!(post(s.port, "/api/v1/works/2/step", "{}", "application/json").0, 202);
    let here = format!("127.0.0.1:{}", s.port);
    let (_, now) = get(s.port, "/api/v1/status", &here, "");
    assert!(
        now.contains(r#""paused":false"#) && now.contains(r#""stepping":2"#),
        "{now}"
    );
    s.control.stepped(Some(2));
    assert_eq!(post(s.port, "/api/v1/engine/pause", "{}", "application/json").0, 204);
    assert!(s.control.asked().paused);
    assert!(s.bus.status().is_some_and(|x| x.paused));
    // What a work's next round would be is answered by the engine's loop, which this test plays.
    let control = std::sync::Arc::clone(&s.control);
    let loop_ = std::thread::spawn(move || {
        for _ in 0..100 {
            if let Some((work, _, answer)) = control.take_previews().pop() {
                let _ = answer.send(Err(format!("work {work} has no next round")));
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
    let here = format!("127.0.0.1:{}", s.port);
    let (status, body) = get(s.port, "/api/v1/works/9/next", &here, "");
    loop_.join().unwrap();
    assert_eq!(status, 422, "{body}");
    assert!(body.contains("work 9 has no next round"), "{body}");
    let _ = std::fs::remove_dir_all(&s.dir);
}

/// The engine's status as its loop tells it before the owner asks anything.
fn status() -> api::Status {
    api::Status {
        paused: true,
        keep_warm: true,
        started: Vec::new(),
        releasing: Vec::new(),
        architect_started: false,
        running: None,
        stepping: None,
        last_step: None,
        slots: 1,
        slots_used: 0,
        flow_version: 1,
        config_commit: "c".into(),
        repos: std::collections::BTreeMap::new(),
        started_at: 1,
        version: "v".into(),
        halt: None,
    }
}

/// A flow of two engine steps, as the database keeps a compiled flow.
const FLOW: &str = r#"{"format":"wf-agents compiled 1","engine":{"steps":{},"actions":{},"checks":{},"inputs":{},
"values":{},"needs":{}},"tools":[],"settings":[],"graphs":[{"name":"g","start":"a","end":"b","within":"outcome",
"asking":[],"receive":null,"counters":[],"rules":[],"interject":null,"cells":[
{"id":"a","kind":"engine","gate":false,"title":"Start","inputs":[],"prompt_by":null,"schema":{"type":"object",
"properties":{"head":{"type":"string","description":"The head"}},"required":["head"]},"rows":[{"name":"go",
"if":[["has","head"]],"to":"b","waits":false,"interject":false,"main":true,"common":false}]},
{"id":"b","kind":"engine","gate":false,"title":"End","inputs":[{"what":"the head","from":[["a","head"]],
"maybe":false}],"prompt_by":null,"schema":null,"rows":[]}]}]}"#;

#[test]
fn the_flow_page_reads_a_version_explained_and_compared() {
    let s = serve("flow");
    let mut db = db::Db::open(&s.dir.join("db/wf-agents.sqlite")).unwrap();
    db.write(db::Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO flow_versions (digest, compiled) VALUES ('d', ?1)",
            [FLOW.replace('\n', "")],
        )?;
        Ok(())
    })
    .unwrap();
    let here = format!("127.0.0.1:{}", s.port);
    let (status, view) = get(s.port, "/api/v1/flows/1/view", &here, "");
    assert_eq!(status, 200, "{view}");
    // The end is entered from the start's row, and the start's field is read by the end's input and the row.
    assert!(view.contains(r#""entered_from":[{"cell":"a","row":0}]"#), "{view}");
    assert!(
        view.contains(r#""used_by":[{"kind":"condition","cell":"a","row":0},{"kind":"input","cell":"b","input":0}]"#),
        "{view}"
    );
    let (status, diff) = get(s.port, "/api/v1/flows/diff?from=1&to=1", &here, "");
    assert_eq!(status, 200, "{diff}");
    assert!(diff.ends_with(r#"{"graphs":[]}"#), "{diff}");
    assert_eq!(get(s.port, "/api/v1/flows/diff?from=1", &here, "").0, 422);
    assert_eq!(get(s.port, "/api/v1/flows/2/view", &here, "").0, 404);
    // The data directory here holds no configuration, so no project's checkout is known.
    assert_ne!(get(s.port, "/api/v1/flows/at?project=p&ref=main", &here, "").0, 200);
    let _ = std::fs::remove_dir_all(&s.dir);
}

// The page reads what agents and the engine told the owner, and the owner marks it read: one message, then the rest.
#[test]
fn the_owner_reads_messages_and_marks_them_read() {
    let s = serve("messages");
    let mut db = db::Db::open(&s.dir.join("db/wf-agents.sqlite")).unwrap();
    db.write(db::Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', 'change', '1')",
            [],
        )?;
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        tx.execute("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', '1')", [])?;
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, '5a', '{}', 'c', 1)",
            [],
        )?;
        tx.execute(
            "INSERT INTO owner_messages (work, round, text) VALUES (1, 1, 'first'), (1, 1, 'second')",
            [],
        )?;
        tx.execute(
            "INSERT INTO engine_notices (work, text, at) VALUES (1, 'Work 1 stopped.', 5)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let here = format!("127.0.0.1:{}", s.port);
    let (status, unread) = get(s.port, "/api/v1/messages", &here, "");
    assert_eq!(status, 200, "{unread}");
    assert!(
        unread.contains(r#""text":"first""#) && unread.contains(r#""cell":"5a""#),
        "{unread}"
    );
    assert!(
        unread.contains(r#""engine":true,"round":null"#) && unread.contains(r#""text":"Work 1 stopped.""#),
        "{unread}"
    );
    let (status, marked) = post(s.port, "/api/v1/messages/read", r#"{"ids":[1]}"#, "application/json");
    assert_eq!(status, 200, "{marked}");
    assert!(marked.ends_with(r#"{"marked":1}"#), "{marked}");
    let (_, unread) = get(s.port, "/api/v1/messages", &here, "");
    assert!(
        !unread.contains(r#""text":"first""#) && unread.contains(r#""text":"second""#),
        "{unread}"
    );
    let (_, marked) = post(s.port, "/api/v1/messages/read", r#"{"all":true}"#, "application/json");
    assert!(marked.ends_with(r#"{"marked":2}"#), "{marked}");
    let (_, every) = get(s.port, "/api/v1/messages?all=true", &here, "");
    assert!(
        every.contains(r#""text":"first""#) && every.contains(r#""text":"second""#),
        "{every}"
    );
    assert!(!every.contains(r#""read_at":null"#), "{every}");
    let _ = std::fs::remove_dir_all(&s.dir);
}

// The owner answers an ask of the owner and rules on a design card on the page, as the command line would write them;
// what cannot be done is refused with the reason.
#[test]
fn the_owner_answers_and_rules_on_the_page() {
    let s = serve("decide");
    let mut db = db::Db::open(&s.dir.join("db/wf-agents.sqlite")).unwrap();
    db.write(db::Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', 'change', '1')",
            [],
        )?;
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        tx.execute("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', '16')", [])?;
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, '4', '{}', 'c', 1)",
            [],
        )?;
        tx.execute(
            "INSERT INTO owner_asks (work, round, body) VALUES (1, 1, '{\"to\":\"owner\"}')",
            [],
        )?;
        tx.execute(
            "INSERT INTO design_cards (work, round, body, status) VALUES (1, 1, '[]', 'open')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let answer = r#"{"text":"Chosen option: B."}"#;
    let (status, done) = post(s.port, "/api/v1/asks/1/answer", answer, "application/json");
    assert_eq!(status, 200, "{done}");
    assert!(done.ends_with(r#"{"done":"ask 1 answered"}"#), "{done}");
    let (status, again) = post(s.port, "/api/v1/asks/1/answer", answer, "application/json");
    assert_eq!(status, 422, "{again}");
    assert!(again.contains("answered already"), "{again}");
    let ruling = r#"{"decisions":[{"decision":1,"verdict":"disagree","text":"Keep the old name."}]}"#;
    let (status, done) = post(s.port, "/api/v1/design-cards/1/rule", ruling, "application/json");
    assert_eq!(status, 200, "{done}");
    let said: String = db
        .conn()
        .query_row("SELECT text FROM interjections WHERE work = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        said,
        "Ruling on design card 1:\n- decision 1: disagree: Keep the old name."
    );
    // A work is started; one that ended cannot be.
    let (status, done) = post(s.port, "/api/v1/works/1/start", "{}", "application/json");
    assert_eq!(status, 200, "{done}");
    assert!(done.ends_with(r#"{"done":"work 1 goes on"}"#), "{done}");
    db.write(db::Actor::Engine, |tx| {
        tx.execute("UPDATE works SET ended = 'cancelled' WHERE id = 1", [])?;
        Ok(())
    })
    .unwrap();
    let (status, again) = post(s.port, "/api/v1/works/1/start", "{}", "application/json");
    assert_eq!(status, 422, "{again}");
    let _ = std::fs::remove_dir_all(&s.dir);
}

/// Reads the stream until `want` appears in it.
fn read_until(s: &mut TcpStream, seen: &mut String, want: &str) {
    let started = Instant::now();
    let mut buf = [0u8; 4096];
    while !seen.contains(want) {
        assert!(started.elapsed() < Duration::from_secs(10), "no {want} in:\n{seen}");
        match s.read(&mut buf) {
            Ok(0) => panic!("the stream ended before {want}:\n{seen}"),
            Ok(n) => seen.push_str(&String::from_utf8_lossy(&buf[..n])),
            Err(_) => {}
        }
    }
}

#[test]
fn the_stream_gives_each_event_and_what_a_client_missed() {
    let s = serve("events");
    let open = |extra: &str| {
        let mut t = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
        t.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
        let req = format!(
            "GET /api/v1/events HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAccept: text/event-stream\r\n{extra}\r\n",
            s.port
        );
        t.write_all(req.as_bytes()).unwrap();
        t
    };
    let mut t = open("");
    let mut seen = String::new();
    read_until(&mut t, &mut seen, r#"{"kind":"ready"}"#);
    s.bus.publish(Event::Work { work: 3 });
    read_until(&mut t, &mut seen, r#"{"kind":"work","work":3}"#);
    assert!(seen.contains("id: 5-1"), "{seen}");
    s.bus.publish(Event::Item { item: Some(2) });
    // A client that saw the first event is given the second when it reconnects.
    let mut again = open("Last-Event-ID: 5-1\r\n");
    let mut seen = String::new();
    read_until(&mut again, &mut seen, r#"{"kind":"item","item":2}"#);
    assert!(!seen.contains(r#""work":3"#), "{seen}");
    let mut other = open("Last-Event-ID: 4-1\r\n");
    let mut seen = String::new();
    read_until(&mut other, &mut seen, r#"{"kind":"resync"}"#);
    let _ = std::fs::remove_dir_all(&s.dir);
}
