//! What a round's message gives: each input once in a session, what changed since, and its cached prefix.

use super::*;

// Building a work's next message is a dry run beside an engine that may be running: with the built-in flow newer than
// the work's, it records no flow version, stops no work, starts none, and writes nothing at all.
#[test]
fn building_a_message_writes_nothing() {
    let s = setup("dry");
    let (mut e, _, item) = start(&s, scripts());
    step_to(&mut e, item, "I1");
    let (work, _, _) = state(&e, item).unwrap();
    // The work runs on a flow version the engine no longer has built in.
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO flow_versions (digest, compiled) SELECT 'older', compiled FROM flow_versions WHERE id = 1",
            [],
        )?;
        let v = tx.last_insert_rowid();
        tx.execute("UPDATE works SET flow_version = ?2 WHERE id = ?1", params![work, v])?;
        Ok(())
    })
    .unwrap();
    drop(e);
    let path = config::database(&s.data);
    let state_of = || -> Vec<i64> {
        let c = db::open_read(&path).unwrap();
        let n = |q: &str| -> i64 { c.query_row(q, [], |r| r.get(0)).unwrap() };
        vec![
            n("SELECT count(*) FROM flow_versions"),
            n("SELECT count(*) FROM works"),
            n("SELECT count(*) FROM works WHERE ended IS NOT NULL"),
            n("SELECT count(*) FROM rounds"),
            n("SELECT count(*) FROM changes"),
            n("SELECT count(*) FROM engine_notices"),
            n("SELECT count(*) FROM outbox"),
        ]
    };
    let before = state_of();
    let agents = Scripted {
        scripts: scripts(),
        rounds: Mutex::new(BTreeMap::new()),
    };
    let hub = FakeHub {
        origin: s.origin.clone(),
        scratch: s.root.clone(),
        prs: s.prs.clone(),
        ci: s.ci.clone(),
        merging: s.merging.clone(),
        closing: s.closing.clone(),
        by_hand: s.by_hand.clone(),
        watch_main: s.watch_main.clone(),
    };
    let committed = config::committed(&s.data).unwrap();
    let mut r = Engine::reading(&s.data, committed, Box::new(agents), Box::new(hub)).unwrap();
    let m = engine::next_message(&mut r, work).unwrap();
    assert!(m.text.contains("cell I1"), "{}", m.text);
    drop(r);
    assert_eq!(state_of(), before);
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The check of the test's records, standing in for the reasoning system's: it says what it was given, and is red until
/// the records hold a log.
const CHECK_PY: &str = "import os, sys\nprint('given: ' + ' '.join(sys.argv[1:]))\nsys.exit(0 if os.path.exists('design/log.md') else 1)\n";

/// A setup whose project keeps records, in `design/`, kept by a copy of the reasoning system in the test's directory
/// that the installation names: its rules, each saying which it is, and its check ([`CHECK_PY`]).
fn setup_records(name: &str) -> Setup {
    let s = setup(name);
    let system = s.root.join("reasoning-system");
    for rule in ["reading", "writing", "dcr", "extension-work", "extension-cards"] {
        write(&system, &format!("rules/{rule}.md"), &format!("The rule {rule}.\n"));
    }
    write(&system, "check.py", CHECK_PY);
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(
        &s.data,
        "install.toml",
        &format!("{install}\n[reasoning]\nsystem = \"{}\"\n", system.display()),
    );
    let p = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    write(
        &s.data,
        "projects/p.toml",
        &p.replace(
            "rules = [\"ground\"]",
            "rules = [\"ground\"]\nrecords = { root = \"design\", ascii = true }",
        ),
    );
    git(
        &s.data,
        &["commit", "-q", "-am", "records kept by the reasoning system"],
    );
    s
}

/// The hash of the conditional input at an address in a round's message.
fn hash_of(input: &Value, address: &str) -> Option<String> {
    input["conditional"]
        .as_array()?
        .iter()
        .find(|m| m["address"] == address)
        .and_then(|m| m["hash"].as_str().map(str::to_string))
}

// Every message starts with its round's address; within a session a conditional input is given once and referred to
// after; the database is an index; and the research's plan, with its acceptance, is a conditional input held by the
// engineer's session and given to the review's.
#[test]
fn a_message_gives_each_conditional_input_once_in_a_session() {
    let s = setup("conditional");
    let (mut e, said, item) = start(&s, scripts());
    for _ in 0..100 {
        e.tick().unwrap();
        if agent_rounds(&e, item).iter().any(|r| r.1 == "T3") {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    let rounds = agent_rounds(&e, item);
    let at = |cell: &str| -> Value {
        rounds
            .iter()
            .find(|r| r.1 == cell)
            .map(|r| r.2.clone())
            .unwrap_or_else(|| panic!("no round at {cell}:\n{log}"))
    };
    for (id, cell, input) in &rounds {
        let m = input["message"].as_str().unwrap();
        assert!(m.starts_with(&format!("# {id} · cell {cell} · ")), "{m}");
    }
    let (first, second) = (&at("R1"), &at("I1"));
    let (work, _, _) = state(&e, item).unwrap();
    let workspace = format!("work:{work}");
    for a in ["format:R1", "inputs:R1", workspace.as_str()] {
        assert_eq!(given(first, a), Some(true), "{a}");
    }
    // What every session of the line is given alike is in the session's system prompt, which its messages refer to.
    let system = system_text(first).expect("the system prompt of the engineer's first session");
    for a in ["format:common", "rule:ground", "rule:general", "rule:engineering"] {
        assert_eq!(given(first, a), Some(false), "{a}");
        assert!(in_system(first, a), "{a}: {}", first["system"]);
        assert!(system.contains(&format!("\n### {a}\n\n")), "{a}: {system}");
    }
    assert!(!in_system(first, "format:R1") && !in_system(first, &workspace));
    for a in ["format:common", "rule:ground", workspace.as_str()] {
        assert_eq!(given(second, a), Some(false), "{a}");
    }
    // A session going on carries the system prompt it started with, recorded once; every session of the line starts
    // with the same, byte for byte, and another line's with its own.
    assert_eq!(system_text(second), Some(system.clone()));
    assert!(second.get("system").is_none(), "{second}");
    let review = system_text(&at("T2")).unwrap();
    assert_ne!(review, system);
    assert!(review.contains("\n### format:common\n\n") && !review.contains("\n### rule:engineering\n\n"));
    // The inputs to work from are listed by their numbers, which `inputs:<cell>` explains once in a session.
    let m = first["message"].as_str().unwrap();
    assert!(m.contains("\n1. item:"), "{m}");
    assert!(m.contains(&format!("Worktree: {workspace}")), "{m}");
    assert_eq!(given(second, "format:I1"), Some(true));
    // The project guide is an input every agent's cell has: given in a session's first round, before anything else,
    // referred to after, and given again to the review's session.
    let guide = |input: &Value| -> Option<(String, bool)> {
        input["conditional"].as_array()?.iter().find_map(|m| {
            let a = m["address"].as_str()?;
            a.ends_with("·engine·project_guide")
                .then(|| (a.to_string(), m["given"].as_bool().unwrap_or(false)))
        })
    };
    let (address, sent) = guide(first).expect("the project guide in cell R1's round");
    assert!(sent, "{address}");
    let m = first["message"].as_str().unwrap();
    let at_guide = m.find(&format!("### {address}")).unwrap();
    assert!(at_guide < m.find("### format:R1").unwrap(), "{m}");
    assert_eq!(guide(second).map(|g| g.1), Some(false));
    assert_eq!(guide(&at("T2")).map(|g| g.1), Some(true));
    let m = second["message"].as_str().unwrap();
    assert!(!m.contains("### rule:ground") && m.contains("rule:ground"), "{m}");
    let m = first["message"].as_str().unwrap();
    // The index of the database is in the query tool's description, which the return formats give.
    assert!(!m.contains("db:tables"), "{m}");
    assert!(given(first, "db:tables").is_none());
    // The formats, in the system prompt and the message, name the owner's language; the engine writes the configured
    // one beside it.
    let formats = format!("{system}\n{m}");
    assert!(formats.contains("`owner.language` (English)"), "{formats}");
    assert!(
        formats.contains("SELECT sql FROM sqlite_master WHERE name = '<name>'"),
        "{formats}"
    );
    assert!(formats.contains("- items (id, project, title, "), "{formats}");
    assert!(!formats.contains("CREATE TABLE"), "{formats}");
    let plan = "·R1·can_start[yes]";
    // Implementing goes on in the research's session, which holds the plan; the review's session is given it whole.
    assert_eq!(given_ending(&at("I1"), plan), Some(false), "{}", at("I1"));
    assert_eq!(given_ending(&at("T2"), plan), Some(true), "{}", at("T2"));
    assert_eq!(given_ending(&at("T3"), plan), Some(false), "{}", at("T3"));
    assert!(
        !rounds
            .iter()
            .any(|r| r.2["conditional"].to_string().contains("item_acceptance"))
    );
    let call = &second["call"];
    assert_eq!(call["cli"], "scripted");
    assert!(call["level"].is_string(), "{call}");
    assert_eq!(call["engine"], engine::VERSION);
    assert!(call["session"]["resumed"].is_string(), "{call}");
    assert!(first["call"]["session"]["started"].is_string());
    let command: Vec<&str> = call["command"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        command.contains(&"--json-schema") && command.contains(&"--resume"),
        "{call}"
    );
    assert!(
        call["env"]["CLAUDE_CONFIG_DIR"]
            .as_str()
            .is_some_and(|d| d.ends_with("agents/claude")),
        "{call}"
    );
    // The record keeps each large text once: the shared formats and the structured-output schema of each line (the
    // engineer's, the reader of the history's and the review's) are in contents once, however many rounds and sessions
    // were given them, and a round's record refers to them.
    let kept = |like: &str| -> i64 {
        e.db.conn()
            .query_row("SELECT count(*) FROM contents WHERE text LIKE ?1", [like], |r| r.get(0))
            .unwrap()
    };
    assert_eq!(kept("The forms every agent's return shares.%"), 3);
    assert_eq!(kept("{\"type\":\"object\"%"), 3);
    let raw: String =
        e.db.conn()
            .query_row(
                "SELECT input FROM rounds WHERE 'R' || work || '-' || n = ?1",
                [&rounds[0].0],
                |r| r.get(0),
            )
            .unwrap();
    assert!(
        !raw.contains("The forms every agent's return shares.") && raw.contains("{\"content\":"),
        "{raw}"
    );
    // The agents' configuration directory is kept out of the data directory's git.
    let ignore = std::fs::read_to_string(s.data.join("agents/claude/.gitignore")).unwrap();
    assert_eq!(ignore, "*\n");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A rule that changed since the session got it in its system prompt is given as what changed in it, under its address;
// after a compaction the session holds nothing but its system prompt, and the next round is given what it needs again
// whole, the session's own earlier returns included, and what changed since its system prompt again as what changed.
#[test]
fn a_changed_rule_gives_what_changed_and_a_compaction_gives_everything_again() {
    let s = setup("again");
    let mut scripts = scripts();
    scripts.insert(
        "I1",
        adding("I1", |n| {
            if n == 1 {
                vec![("@compactions", json!(1))]
            } else {
                Vec::new()
            }
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 1);
    write(&s.data, "rules/ground.md", "Facts first, then the rest.\n");
    git(&s.data, &["add", "-A"]);
    git(&s.data, &["commit", "-q", "-m", "a changed rule"]);
    agent_rounds_to(&mut e, item, 3);
    let log = said.lock().unwrap().join("\n");
    let rounds = agent_rounds(&e, item);
    let cells: Vec<&str> = rounds.iter().map(|r| r.1.as_str()).collect();
    assert_eq!(&cells[..3], ["R1", "I1", "I1"], "{log}");
    let (a, b, c) = (&rounds[0].2, &rounds[1].2, &rounds[2].2);
    assert_eq!(given(b, "rule:ground"), Some(true));
    assert_ne!(hash_of(a, "rule:ground"), hash_of(b, "rule:ground"));
    let changes = |input: &Value| {
        input["conditional"]
            .as_array()
            .and_then(|xs| xs.iter().find(|m| m["address"] == "rule:ground"))
            .map(|m| m["changes"] == true)
    };
    assert_eq!((changes(b), changes(c)), (Some(true), Some(true)));
    assert!(in_system(a, "rule:ground"), "{a}");
    assert_eq!(
        given_text(b, "rule:ground").unwrap().trim_end(),
        format!(
            "What changed since rule:ground, given in {}, as a unified diff of its lines, a long line cut before each \
             `,\"`:\n@@ -1,1 +1,1 @@\n\
             -Facts first.\n+Facts first, then the rest.",
            rounds[0].0
        )
    );
    assert_eq!(given(b, "format:common"), Some(false));
    assert_eq!(given(c, "format:common"), Some(false));
    assert_eq!(given(c, "rule:ground"), Some(true));
    assert_eq!(given_text(c, "rule:ground"), given_text(b, "rule:ground"));
    assert_eq!(hash_of(b, "rule:ground"), hash_of(c, "rule:ground"));
    assert_eq!(given(c, "format:I1"), Some(true));
    let understanding = format!("{}·R1·understanding", rounds[0].0);
    assert_eq!(given(c, &understanding), Some(true), "{c}");
    let m = c["message"].as_str().unwrap();
    assert!(m.contains(&format!("compacted in {}", rounds[1].0)), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// What a round's message gives under an address.
fn given_text(input: &Value, address: &str) -> Option<String> {
    let m = input["message"].as_str()?;
    let heading = format!("### {address}\n\n");
    let rest = &m[m.find(&heading)? + heading.len()..];
    let end = [rest.find("\n### "), rest.find("\n## ")]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

// The project guide is read from main as the work's branch merged it: the work's own edits to its records and to
// AGENTS.md, committed or not, never make it a new one, so its session is not given it again.
#[test]
fn a_works_own_edits_to_its_records_do_not_give_the_guide_again() {
    let s = setup_records("own-guide");
    let (mut e, said, item) = start(&s, scripts());
    agent_rounds_to(&mut e, item, 1);
    let (work, _, _) = state(&e, item).unwrap();
    let wt = worktree_of(&e, work);
    write(&wt, "design/added.md", "# Added\nThe work's own node.\n");
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-q", "-m", "Add a node"]);
    write(&wt, "AGENTS.md", "Edited, not committed.\n");
    agent_rounds_to(&mut e, item, 3);
    let log = said.lock().unwrap().join("\n");
    let rounds = agent_rounds(&e, item);
    let cells: Vec<&str> = rounds.iter().map(|r| r.1.as_str()).collect();
    assert_eq!(&cells[..3], ["R1", "I1", "I1"], "{log}");
    let (address, sent) = guide_in(&rounds[0].2).unwrap();
    assert!(sent, "{address}");
    for (id, _, input) in &rounds[1..3] {
        assert_eq!(guide_in(input), Some((address.clone(), false)), "{id}");
        let m = input["message"].as_str().unwrap();
        assert!(
            !m.contains("design/added.md") && !m.contains("Edited, not committed."),
            "{m}"
        );
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// When main changed the project guide since the version a session holds, and the branch merged it, the session is
// given only the entries that differ, under the new version's address, and holds the new version after.
#[test]
fn a_changed_main_gives_only_what_changed_in_the_guide() {
    let s = setup_records("main-guide");
    let (mut e, said, item) = start(&s, scripts());
    agent_rounds_to(&mut e, item, 1);
    let (work, _, _) = state(&e, item).unwrap();
    main_moves_on(
        &s.root,
        &[("design/later.md", "# Later\nMain's later node.\n")],
        "Main's later node",
    );
    merge_main(&worktree_of(&e, work));
    agent_rounds_to(&mut e, item, 3);
    let log = said.lock().unwrap().join("\n");
    let rounds = agent_rounds(&e, item);
    let cells: Vec<&str> = rounds.iter().map(|r| r.1.as_str()).collect();
    assert_eq!(&cells[..3], ["R1", "I1", "I1"], "{log}");
    let (first, _) = guide_in(&rounds[0].2).unwrap();
    let (changed, sent) = guide_in(&rounds[1].2).unwrap();
    assert!(sent, "{changed}");
    assert_eq!(changed, format!("{}·engine·project_guide", rounds[1].0));
    let text = given_text(&rounds[1].2, &changed).unwrap();
    assert!(
        text.starts_with(&format!(
            "What changed since {first}, given in {}: each entry here takes the place of the one there, and null \
             removes it. ",
            rounds[0].0
        )),
        "{text}"
    );
    assert!(
        text.trim_end()
            .ends_with(r##"{"records":{"design/later.md":"# Later"}}"##),
        "{text}"
    );
    assert!(
        !text.contains("How to work here.") && !text.contains("design/overview.md"),
        "{text}"
    );
    assert_eq!(guide_in(&rounds[2].2), Some((changed, false)));
    let _ = std::fs::remove_dir_all(&s.root);
}

// Every agent's cell of a line, in every graph of the flow, gives the command line what it caches before the session
// the same way, so that the line's session is read from the prompt cache at every change of cell: the program and its
// arguments, the model, the structured-output schema, the tools and commands refused and the directories added, with
// the configuration directory and where it runs. The effort may differ, as far as the flow's check lets it, and the
// round's own session and hooks do; a cell on no line has a session of its own every round.
#[test]
fn every_cell_of_a_line_gives_the_command_line_the_same_cached_prefix() {
    let s = setup("prefix");
    let (mut e, _) = engine(&s, scripts());
    let item = a_ready_item(&mut e, "Parse it");
    let v = e.flow_now;
    let work =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'R1')",
                params![item, v],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    let arch = architect_work_at(&mut e, "B1");
    let mut lines = 0;
    for id in [work, arch] {
        let w = e.work(id).unwrap();
        let g = e.graph_of(&w).unwrap();
        let item = e.subject(&w).unwrap();
        let mut by_line: BTreeMap<String, Vec<(String, Vec<String>)>> = BTreeMap::new();
        for cell in g.cells.iter().filter(|c| c.kind == "agent") {
            let Some(line) = &cell.line else { continue };
            let r = engine::cell_request(&e, &w, &g, cell, item.as_ref()).unwrap();
            let mut prefix = vec![r.worktree.display().to_string(), format!("{:?}", r.config_dir)];
            let mut words = engine::agent::command_line(&r).into_iter();
            while let Some(x) = words.next() {
                if matches!(x.as_str(), "--effort" | "--settings" | "--resume" | "--session-id") {
                    words.next();
                    continue;
                }
                prefix.push(x);
            }
            by_line.entry(line.clone()).or_default().push((cell.id.clone(), prefix));
        }
        for (line, cells) in &by_line {
            lines += 1;
            let (first, want) = &cells[0];
            for (cell, got) in cells {
                assert_eq!(
                    got, want,
                    "line {line}: cell {cell} gives the command line another prefix than cell {first}"
                );
            }
        }
    }
    // The engineer's graph has six lines, the architect's three.
    assert_eq!(lines, 9);
    let _ = std::fs::remove_dir_all(&s.root);
}
