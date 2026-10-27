//! The interface over a real socket: what it serves, what it refuses, and the event stream.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use api::{Bus, Control, Event, Mode};

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
    db::Db::open(&path).unwrap();
    let bus = Arc::new(Bus::new(5));
    let control = Arc::new(Control::new(Mode::Hold));
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
fn the_owner_switches_the_mode_and_asks_for_a_step() {
    let s = serve("mode");
    assert_eq!(post(s.port, "/api/v1/engine/step", "{}", "application/json").0, 409);
    assert_eq!(
        post(
            s.port,
            "/api/v1/engine/mode",
            "mode=step",
            "application/x-www-form-urlencoded"
        )
        .0,
        415
    );
    assert_eq!(s.control.asked().mode, Mode::Hold);
    assert_eq!(
        post(s.port, "/api/v1/engine/mode", r#"{"mode":"step"}"#, "application/json").0,
        204
    );
    assert_eq!(s.control.asked().mode, Mode::Step);
    assert_eq!(post(s.port, "/api/v1/engine/step", "{}", "application/json").0, 202);
    assert!(s.control.asked().step);
    assert_eq!(post(s.port, "/api/v1/engine/step", "{}", "application/json").0, 409);
    let _ = std::fs::remove_dir_all(&s.dir);
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

// The page reads what agents told the owner, and the owner marks it read: one message, then the rest.
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
            "INSERT INTO rounds (work, cell, input, config_commit, started_at) VALUES (1, '5a', '{}', 'c', 1)",
            [],
        )?;
        tx.execute("INSERT INTO owner_messages (round, text) VALUES (1, 'first'), (1, 'second')", [])?;
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
    let (status, marked) = post(s.port, "/api/v1/messages/read", r#"{"ids":[1]}"#, "application/json");
    assert_eq!(status, 200, "{marked}");
    assert!(marked.ends_with(r#"{"marked":1}"#), "{marked}");
    let (_, unread) = get(s.port, "/api/v1/messages", &here, "");
    assert!(
        !unread.contains(r#""text":"first""#) && unread.contains(r#""text":"second""#),
        "{unread}"
    );
    let (_, marked) = post(s.port, "/api/v1/messages/read", r#"{"all":true}"#, "application/json");
    assert!(marked.ends_with(r#"{"marked":1}"#), "{marked}");
    let (_, every) = get(s.port, "/api/v1/messages?all=true", &here, "");
    assert!(
        every.contains(r#""text":"first""#) && every.contains(r#""text":"second""#),
        "{every}"
    );
    assert!(!every.contains(r#""read_at":null"#), "{every}");
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
