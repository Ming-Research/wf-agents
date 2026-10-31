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

/// A configuration of one project, p, committed in the data directory, which the architect's routes read.
fn configure(dir: &std::path::Path) {
    let install = "[owner]\nlanguage = \"English\"\n[items]\ntitle_max = 40\n[machines]\nworkstation = \"here\"\n\
                   heavy = \"builds\"\npoll_interval = 60\n[machines.hosts.here]\nslots = 1\n[agents.clis.claude]\n\
                   kind = \"claude\"\ncommand = \"claude\"\n[agents.levels.l]\ndesc = \"all\"\ncli = \"claude\"\n\
                   model = \"m\"\n[gh]\nallow = []\n[query]\nmax_rows = 10\ntimeout_ms = 1000\n";
    let project = "repo = \"o/p\"\ncheckout = \"/p\"\ndesign_goal = \"simple\"\ndesign_paths = [\"design/\"]\n\
                   test_paths = [\"tests/\"]\nobservation_tags = [{ name = \"perf\", desc = \"How fast\" }]\n\
                   [ci]\nfast = \"ci\"\nfull = [\"ci\"]\nmerge_checks = []\n";
    std::fs::create_dir_all(dir.join("projects")).unwrap();
    std::fs::write(dir.join("install.toml"), install).unwrap();
    std::fs::write(dir.join("projects/p.toml"), project).unwrap();
    std::fs::write(dir.join(".gitignore"), "db/\n").unwrap();
    for args in [
        &["init", "-q"][..],
        &["add", "-A"],
        &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "c"],
    ] {
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .unwrap();
        assert!(ok.success(), "git {args:?}");
    }
}

fn serve(name: &str) -> Server {
    let dir = std::env::temp_dir().join(format!("wf-agents-web-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("db")).unwrap();
    configure(&dir);
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
    // The System view reads an empty database: the configuration's project has an architect, no work of it running.
    let (status, system) = get(s.port, "/api/v1/system?weeks=4&tz=480", &here, "");
    assert_eq!(status, 200, "{system}");
    assert!(
        system.contains(r#""architects":[{"project":"p","work":null"#),
        "{system}"
    );
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
        tx.execute("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'R1')", [])?;
        tx.execute("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'R1')", [])?;
        tx.execute(
            "UPDATE works SET paused = 'cell R1: no', paused_by = 'engine' WHERE id = 2",
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
    assert_eq!(s.control.take_new_sessions(), vec![api::Fresh::Work(4)]);
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
    assert_eq!(s.control.take_new_sessions(), vec![api::Fresh::Work(2)]);
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
    // The owner starts a work over: the engine is asked to, once, and wakes.
    let woken = s.control.asked().woken;
    let (status, done) = post(s.port, "/api/v1/works/1/start-over", "{}", "application/json");
    assert_eq!(status, 200, "{done}");
    assert!(done.ends_with(r#"{"done":"work 1 is started over"}"#), "{done}");
    assert!(db::start_over_asked(db.conn(), 1).unwrap());
    assert_eq!(s.control.asked().woken, woken + 1);
    let (status, done) = post(s.port, "/api/v1/works/1/start-over", "{}", "application/json");
    assert_eq!(status, 422, "{done}");
    assert!(done.contains("work 1 is being started over already"), "{done}");
    // A project's architect is started and paused the same way; the configuration's one project is the default, and a
    // project there is not is refused.
    let (status, done) = post(
        s.port,
        "/api/v1/architect/start",
        r#"{"new_session":true}"#,
        "application/json",
    );
    assert_eq!(status, 200, "{done}");
    assert!(s.control.asked().architects_started.contains("p"));
    assert_eq!(s.control.take_new_sessions(), vec![api::Fresh::Architect("p".into())]);
    let (status, done) = post(s.port, "/api/v1/architect/pause?project=p", "{}", "application/json");
    assert_eq!(status, 200, "{done}");
    assert!(s.control.asked().architects_started.is_empty());
    assert_eq!(
        post(s.port, "/api/v1/architect/start?project=q", "{}", "application/json").0,
        404
    );
    // The architect's tab names its project and every project, and no constitution for a project that names none.
    let here = format!("127.0.0.1:{}", s.port);
    let (status, tab) = get(s.port, "/api/v1/architect", &here, "");
    assert_eq!(status, 200, "{tab}");
    assert!(
        tab.contains(r#""project":"p""#) && tab.contains(r#""projects":["p"]"#),
        "{tab}"
    );
    assert!(tab.contains(r#""constitution":null"#), "{tab}");
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
    let _ = std::fs::remove_dir_all(&s.dir);
}

/// The engine's status as its loop tells it before the owner asks anything.
fn status() -> api::Status {
    api::Status {
        paused: true,
        keep_warm: true,
        started: Vec::new(),
        releasing: Vec::new(),
        waiting_for_slot: Vec::new(),
        architects_started: Vec::new(),
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

// Every part of the page that shows a role reads the lines of the flow built into the program, each with the colour its
// role is drawn in, from one place that needs no flow version stored.
#[test]
fn the_page_reads_the_lines_and_their_colours() {
    let s = serve("lines");
    let here = format!("127.0.0.1:{}", s.port);
    let (status, out) = get(s.port, "/api/v1/flows/lines", &here, "");
    assert_eq!(status, 200, "{out}");
    let body = out.split_once("\r\n\r\n").map_or("", |x| x.1);
    let graphs: serde_json::Value = serde_json::from_str(body).unwrap();
    let text = |v: &serde_json::Value| v.as_str().unwrap_or_default().to_string();
    let mut lines = Vec::new();
    for g in graphs.as_array().unwrap() {
        for l in g["lines"].as_array().unwrap() {
            lines.push(format!(
                "{}/{}: {}",
                text(&g["graph"]),
                text(&l["name"]),
                text(&l["colour"])
            ));
        }
    }
    assert!(lines.iter().any(|l| l == "engineer/engineer: blue"), "{lines:?}");
    assert!(lines.iter().any(|l| l == "architect/architect: purple"), "{lines:?}");
    let _ = std::fs::remove_dir_all(&s.dir);
}

// The page reads the owner's conversations in their fixed order, the architect's one-to-one and wf-agents' first, what
// agents and the engine told the owner in a work's group, and marks how far the owner read it, which marks its messages
// read; the owner says something in the group, to its engineer, to its architect and to both, and a line that is no
// member of the group is refused.
#[test]
fn the_owner_reads_conversations_marks_them_read_and_says_something() {
    let s = serve("messages");
    let mut db = db::Db::open(&s.dir.join("db/wf-agents.sqlite")).unwrap();
    db.write(db::Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', 'change', '1')",
            [],
        )?;
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        tx.execute("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'R1')", [])?;
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'T4', '{}', 'c', 1)",
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
    let (status, list) = get(s.port, "/api/v1/conversations", &here, "");
    assert_eq!(status, 200, "{list}");
    let order: Vec<usize> = [r#""key":"architect:p""#, r#""key":"wf-agents""#, r#""key":"work:1""#]
        .iter()
        .map(|k| list.find(k).unwrap_or_else(|| panic!("{k} in {list}")))
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{list}");
    // The engine's notice is no entry of a work's group: the two messages to the owner are unread.
    assert!(list.contains(r#""unread":2"#), "{list}");
    let (status, talk) = get(s.port, "/api/v1/conversation?key=work%3A1", &here, "");
    assert_eq!(status, 200, "{talk}");
    assert!(
        talk.contains(r#""text":"first""#) && !talk.contains(r#""source":"notice""#),
        "{talk}"
    );
    let (status, marked) = post(
        s.port,
        "/api/v1/conversations/read",
        r#"{"key":"work:1","at":5}"#,
        "application/json",
    );
    assert_eq!(status, 200, "{marked}");
    assert!(marked.ends_with(r#"{"marked":3}"#), "{marked}");
    let (_, list) = get(s.port, "/api/v1/conversations", &here, "");
    assert!(!list.contains(r#""unread":2"#), "{list}");
    let (_, marked) = post(s.port, "/api/v1/messages/read", r#"{"all":true}"#, "application/json");
    assert!(marked.ends_with(r#"{"marked":0}"#), "{marked}");
    let (status, said) = post(
        s.port,
        "/api/v1/works/1/say",
        r#"{"text":"Look at the log."}"#,
        "application/json",
    );
    assert_eq!(status, 200, "{said}");
    assert!(said.contains("goes to the engineer of work 1"), "{said}");
    let (status, said) = post(
        s.port,
        "/api/v1/works/1/say",
        r#"{"text":"Hold the merge.","to":"architect"}"#,
        "application/json",
    );
    assert_eq!(status, 200, "{said}");
    assert!(said.contains("the engineer of work 1 is told"), "{said}");
    let (status, said) = post(
        s.port,
        "/api/v1/works/1/say",
        r#"{"text":"Both of you.","to":"both"}"#,
        "application/json",
    );
    assert_eq!(status, 200, "{said}");
    let (status, refused) = post(
        s.port,
        "/api/v1/works/1/say",
        r#"{"text":"Not yours.","to":"review"}"#,
        "application/json",
    );
    assert_eq!(status, 422, "{refused}");
    let (_, talk) = get(s.port, "/api/v1/conversation?key=work%3A1", &here, "");
    assert!(
        talk.contains(r#""source":"interjection""#) && talk.contains("Look at the log."),
        "{talk}"
    );
    // The words to the architect show once, to the architect, with the words the engineer was given left out.
    assert_eq!(talk.matches("Hold the merge.").count(), 1, "{talk}");
    assert!(
        talk.contains(r#""recipient":"architect""#) && talk.contains(r#""recipient":"both""#),
        "{talk}"
    );
    let (_, list) = get(s.port, "/api/v1/conversations", &here, "");
    assert!(list.contains(r#""key":"architect:p""#), "{list}");
    let _ = std::fs::remove_dir_all(&s.dir);
}

/// Reads what the interface serves at a path as bytes: its status, its headers and its body.
fn get_bytes(port: u16, path: &str) -> (u16, String, Vec<u8>) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    s.write_all(request.as_bytes()).unwrap();
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    let at = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&out[..at]).into_owned();
    let status = head.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    (status, head, out[at + 4..].to_vec())
}

// The owner sends images with words, or alone: they are kept with the words, shown with them in the conversation and
// served by their id as they were sent, with their media type. What the owner may not send is refused, saying why: a
// kind that is no PNG, JPEG, GIF or WebP image, bytes that are not the kind they say, an image over 5 MB, more than 10
// to a message, and data that is not base64.
#[test]
fn the_owner_sends_images_with_words_and_the_page_reads_them_by_id() {
    let s = serve("images");
    let mut db = db::Db::open(&s.dir.join("db/wf-agents.sqlite")).unwrap();
    db.write(db::Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', 'change', '1')",
            [],
        )?;
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        tx.execute("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'R1')", [])?;
        Ok(())
    })
    .unwrap();
    let png = b"\x89PNG\r\n\x1a\n a screenshot".to_vec();
    let image = |kind: &str, bytes: &[u8]| {
        format!(
            r#"{{"media_type":"{kind}","data":"{}"}}"#,
            db::attachments::base64(bytes)
        )
    };
    let say = |images: Vec<String>| {
        let body = format!(r#"{{"text":"","images":[{}]}}"#, images.join(","));
        post(s.port, "/api/v1/works/1/say", &body, "application/json")
    };
    let (status, said) = say(vec![image("image/png", &png)]);
    assert_eq!(status, 200, "{said}");
    let id: i64 = db
        .conn()
        .query_row("SELECT id FROM attachments WHERE interjection = 1", [], |r| r.get(0))
        .unwrap();
    let (status, head, body) = get_bytes(s.port, &format!("/api/v1/attachments/{id}"));
    assert_eq!(status, 200, "{head}");
    assert!(head.to_lowercase().contains("content-type: image/png"), "{head}");
    assert_eq!(body, png);
    assert_eq!(get_bytes(s.port, "/api/v1/attachments/99").0, 404);
    let here = format!("127.0.0.1:{}", s.port);
    let (_, talk) = get(s.port, "/api/v1/conversation?key=work%3A1", &here, "");
    assert!(talk.contains(&format!(r#""images":[{id}]"#)), "{talk}");
    let refused = |images: Vec<String>, why: &str| {
        let (status, text) = say(images);
        assert_eq!(status, 422, "{text}");
        assert!(text.contains(why), "{why}: {text}");
    };
    refused(
        vec![image("application/pdf", b"%PDF-1.7")],
        "only PNG, JPEG, GIF and WebP",
    );
    refused(vec![image("image/jpeg", &png)], "not the image/jpeg image");
    let mut big = png.clone();
    big.resize(5 * 1024 * 1024 + 1, 0);
    refused(vec![image("image/png", &big)], "at most 5 MB");
    refused(vec![image("image/png", &png); 11], "at most 10 images");
    refused(
        vec![r#"{"media_type":"image/png","data":"not base64!"}"#.to_string()],
        "not given as base64",
    );
    let kept: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM attachments", [], |r| r.get(0))
        .unwrap();
    assert_eq!(kept, 1);
    // Ten images to a message are taken.
    let (status, said) = say(vec![image("image/png", &png); 10]);
    assert_eq!(status, 200, "{said}");
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
        tx.execute("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'A3')", [])?;
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'I1', '{}', 'c', 1)",
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

// The Observations view reads every project's observations that has any, by name, each project's with the tags the
// configuration gives them and none for a project it no longer names; the owner closes one there, saying why.
#[test]
fn the_owner_reads_each_project_s_observations_and_closes_one() {
    let s = serve("observations");
    let mut db = db::Db::open(&s.dir.join("db/wf-agents.sqlite")).unwrap();
    db.write(db::Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO observations (project, title, what, tags, source)
             VALUES ('p', 'Slow parse', 'Slow.', '[\"perf\"]', 's'), ('gone', NULL, 'Old.', '[]', 's')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let here = format!("127.0.0.1:{}", s.port);
    let (status, all) = get(s.port, "/api/v1/observations", &here, "");
    assert_eq!(status, 200, "{all}");
    assert!(
        all.contains(r#"[{"project":"gone","tags":[],"observations":[{"id":2,"title":null,"what":"Old.""#),
        "{all}"
    );
    assert!(
        all.contains(
            r#"{"project":"p","tags":[{"name":"perf","desc":"How fast"}],"observations":[{"id":1,"title":"Slow parse","what":"Slow.""#
        ),
        "{all}"
    );
    let (status, done) = post(
        s.port,
        "/api/v1/observations/1/close",
        r#"{"why":"Not worth it."}"#,
        "application/json",
    );
    assert_eq!(status, 200, "{done}");
    let (_, again) = get(s.port, "/api/v1/observations", &here, "");
    assert!(
        again.contains(r#""closed":"owner","closed_why":"Not worth it.""#),
        "{again}"
    );
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
