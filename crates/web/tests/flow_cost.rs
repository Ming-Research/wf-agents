//! The owner's page asking what installing a flow would do with the works that go on (`GET /api/v1/flows/cost`),
//! attacked where it takes input from outside the program (the git ref in the address) and where it reads beside the
//! engine (the database while a write is under way).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

const INSTALL: &str = r#"
[owner]
language = "English"
[items]
title_max = 40
[machines]
workstation = "here"
heavy = "builds"
poll_interval = 60
[machines.hosts.here]
slots = 1
[agents.clis.claude]
kind = "claude"
command = "claude"
[agents.levels.l]
desc = "all work"
cli = "claude"
model = "m"
[gh]
allow = ["pr view"]
[query]
max_rows = 10
timeout_ms = 1000
"#;

fn project(checkout: &Path) -> String {
    format!(
        "repo = \"o/p\"\ncheckout = \"{}\"\ndesign_goal = \"simple\"\ntest_paths = [\"tests/\"]\nrules = [\"ground\"]\n\
         [ci]\nfast = \"ci\"\nfull = [\"ci\"]\nmerge_checks = [\"check\"]\n[limits]\nfixes = 5\n",
        checkout.display()
    )
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wf-agents-web-cost-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env("GIT_CEILING_DIRECTORIES", std::env::temp_dir())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Commits everything in the directory and gives the commit.
fn commit_all(dir: &Path, message: &str) -> String {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    git(dir, &["rev-parse", "HEAD"])
}

/// A data directory of project p, whose checkout is `checkout`, with its database, and the interface over it.
fn serve(name: &str, checkout: &Path) -> (PathBuf, u16) {
    let dir = temp(name);
    std::fs::create_dir_all(dir.join("projects")).unwrap();
    std::fs::create_dir_all(dir.join("rules")).unwrap();
    std::fs::write(dir.join("install.toml"), INSTALL).unwrap();
    std::fs::write(dir.join("projects/p.toml"), project(checkout)).unwrap();
    std::fs::write(dir.join("rules/ground.md"), "Facts first.").unwrap();
    git(&dir, &["init", "-q"]);
    commit_all(&dir, "configuration");
    // The database, made as the program makes it (the web crate's tests do not reach the engine crate, which the page
    // before this change did not need).
    std::fs::create_dir_all(dir.join("db")).unwrap();
    drop(db::Db::create(&config::database(&dir)).unwrap());
    let port = web::start(web::Options {
        data: dir.clone(),
        db: config::database(&dir),
        port: 0,
        bus: Arc::new(api::Bus::new(5)),
        control: Arc::new(api::Control::default()),
    })
    .unwrap();
    (dir, port)
}

/// The query's value as an address carries it: everything but letters, digits and `._-/` as %XX.
fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b'-' | b'/' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The answer to `GET /api/v1/flows/cost?query`: its status and its body.
fn cost(port: u16, query: &str) -> (u16, String) {
    let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(std::time::Duration::from_secs(30))).unwrap();
    let request =
        format!("GET /api/v1/flows/cost?{query} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    s.write_all(request.as_bytes()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    let status = out.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    let body = out.split_once("\r\n\r\n").map_or("", |(_, b)| b).to_string();
    (status, body)
}

fn at_ref(port: u16, rev: &str) -> (u16, String) {
    cost(port, &format!("project=p&ref={}", enc(rev)))
}

// The ref in the address goes to git, so it is checked: whatever names a commit reads the flow at that commit, whatever
// way it is written, and no more than one; what is not a single ref (an option, a path, a range, a control character,
// a too long text) is refused before git sees it, and a commit that holds no flow is not found. Every flow here is none
// (git's text of the file is refused by the compiler at once), so the test does not wait for an engine to be made: the
// refusal's words tell which commit's file was read, and the three commits' files differ.
#[test]
fn the_cost_of_a_flow_is_read_at_the_ref_that_names_one_commit_and_other_refs_are_refused() {
    let checkout = temp("refs-checkout");
    git(&checkout, &["init", "-q", "-b", "main"]);
    std::fs::create_dir_all(checkout.join("flow")).unwrap();
    // c1: a flow that is an empty object, tagged and on a branch whose name has a slash.
    std::fs::write(checkout.join("flow/flow.json"), "{}").unwrap();
    let c1 = commit_all(&checkout, "one");
    git(&checkout, &["tag", "v1"]);
    git(&checkout, &["branch", "feature/x"]);
    // c2: no flow at all.
    std::fs::remove_file(checkout.join("flow/flow.json")).unwrap();
    std::fs::write(checkout.join("note.txt"), "x").unwrap();
    let c2 = commit_all(&checkout, "two");
    // c3: a flow that is no JSON.
    std::fs::write(checkout.join("flow/flow.json"), "not json").unwrap();
    let c3 = commit_all(&checkout, "three");
    let (dir, port) = serve("refs", &checkout);

    // Whatever names c1 gets c1's words, and the same words.
    let (status, words1) = at_ref(port, &c1);
    assert_eq!(status, 422, "{words1}");
    for rev in ["v1", "feature/x", "HEAD~2", "HEAD^^", "v1^{commit}", &c1[..12]] {
        assert_eq!(at_ref(port, rev), (422, words1.clone()), "{rev}");
    }
    // Whatever names c3 gets c3's, which differ from c1's.
    let (status, words3) = at_ref(port, &c3);
    assert_eq!(status, 422, "{words3}");
    assert_ne!(words1, words3);
    for rev in ["HEAD", "@", "HEAD^0", "main", "HEAD~0"] {
        assert_eq!(at_ref(port, rev), (422, words3.clone()), "{rev}");
    }
    // c2 holds no flow: not found, saying what git was asked for.
    for rev in [c2.as_str(), "HEAD~1", "HEAD^"] {
        let (status, body) = at_ref(port, rev);
        assert_eq!(status, 404, "{rev}: {body}");
        assert!(body.contains("flow/flow.json"), "{rev}: {body}");
    }
    // Not a ref, or not one commit: refused as invalid, never handed to git as an option or a path.
    let long = "a".repeat(201);
    for rev in [
        "",
        "-h",
        "--output=x",
        "-",
        "HEAD:flow/flow.json",
        "HEAD\n",
        "HEAD\0",
        "a b",
        "HEAD;ls",
        "$(ls)",
        "HEAD\\",
        "h\u{e9}ad",
        long.as_str(),
    ] {
        let (status, body) = at_ref(port, rev);
        assert_eq!(status, 422, "{rev:?}: {body}");
        assert!(body.contains("not a git ref"), "{rev:?}: {body}");
    }
    // Looks like a ref, names no single commit: not found.
    for rev in [
        "nothing",
        "HEAD~1000",
        "HEAD..HEAD~1",
        "HEAD^{tree}",
        "HEAD^{blob}",
        "main..",
    ] {
        let (status, body) = at_ref(port, rev);
        assert_eq!(status, 404, "{rev}: {body}");
    }
    // A longest ref that is allowed is a ref, not too long.
    assert_eq!(at_ref(port, &"a".repeat(200)).0, 404);
    // Without a ref, or a project, the question is not one; a project not in the configuration is not found.
    assert_eq!(cost(port, "project=p").0, 400);
    assert_eq!(cost(port, "ref=HEAD").0, 400);
    assert_eq!(cost(port, "project=nope&ref=HEAD").0, 404);
    assert_eq!(cost(port, "project=..%2Fp&ref=HEAD").0, 404);
    for d in [&dir, &checkout] {
        let _ = std::fs::remove_dir_all(d);
    }
}

// The engine writes while the owner looks at the page: the cost is read beside a write under way, without waiting for
// it, and as the database was when the write began, nothing of what it has not committed; once it is committed the next
// look sees it. The works are on a version that cannot be read, so the answer lists them by id with no engine step. The
// flow is the one built into the program, which an engine is made to run, as the command does.
#[test]
fn the_cost_of_a_flow_is_read_beside_a_write_under_way_and_sees_only_what_is_committed() {
    let checkout = temp("writer-checkout");
    git(&checkout, &["init", "-q", "-b", "main"]);
    std::fs::create_dir_all(checkout.join("flow")).unwrap();
    std::fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../flow/flow.json"),
        checkout.join("flow/flow.json"),
    )
    .unwrap();
    commit_all(&checkout, "flow");
    let (dir, port) = serve("writer", &checkout);
    let path = config::database(&dir);
    let mut base = db::Db::open(&path).unwrap();
    base.write(db::Actor::Engine, |tx| {
        tx.execute_batch(
            "INSERT INTO flow_versions (id, digest, compiled) VALUES (3, 'broken', '{}');
             INSERT INTO items (id, project, title, problem, why, deliver, level)
             VALUES (5, 'p', 'Broken', 'x', 'y', 'change', 'l');
             INSERT INTO works (id, item, flow_version, graph, at) VALUES (8, 5, 3, 'engineer', 'R1');",
        )?;
        Ok(())
    })
    .unwrap();
    let works = |answer: (u16, String)| -> (i64, bool, Vec<i64>) {
        assert_eq!(answer.0, 200, "{}", answer.1);
        let page: serde_json::Value = serde_json::from_str(&answer.1).unwrap();
        let ids = page["works"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w["work"].as_i64().unwrap());
        for w in page["works"].as_array().unwrap() {
            assert_eq!(w["effect"]["kind"], "unreadable", "{page}");
        }
        (
            page["version"].as_i64().unwrap(),
            page["stored"].as_bool().unwrap(),
            ids.collect(),
        )
    };
    // A write under way holds its lock while the page asks: the work it adds is not yet there.
    let beside = base
        .write(db::Actor::Engine, |tx| {
            tx.execute_batch(
                "INSERT INTO items (id, project, title, problem, why, deliver, level)
                 VALUES (6, 'p', 'Later', 'x', 'y', 'change', 'l');
                 INSERT INTO works (id, item, flow_version, graph, at) VALUES (9, 6, 3, 'engineer', 'R1');",
            )?;
            Ok(works(at_ref(port, "HEAD")))
        })
        .unwrap();
    // The flow is not in the database (versions 1 and 2 are not there either): it would be the one after the greatest.
    assert_eq!(beside, (4, false, vec![8]));
    // The write is committed now: the next look sees its work.
    assert_eq!(works(at_ref(port, "HEAD")), (4, false, vec![8, 9]));
    for d in [&dir, &checkout] {
        let _ = std::fs::remove_dir_all(d);
    }
}
