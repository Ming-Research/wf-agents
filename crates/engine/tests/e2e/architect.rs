//! The architect's works: its base, the matters handled in sessions forked from it, its level, and what it is told.

use super::*;

// The owner's words start a work of project p's architect, which goes once through its graph: the look at what comes
// next takes the owner's words first, before anything else, and they are handled in one round, with no base yet in a
// new session of their own: it replies, in the owner's one-to-one, and hands in an item change, made at once as the
// architect's, since a new title loses nothing a second eye would look for, and its line of the decision log, which the
// engine keeps with the matter; the matter closes and its session ends. With no base, a planning is due: the project
// is read in a new session, which plans in the same session, and that session is the base; the check of the policy
// reading the project queued is a matter the plan queued, handled in a session forked from the base; nothing is left,
// and the work ends.
#[test]
fn an_architect_work_handles_the_owner_s_words_then_builds_its_base_and_forks_from_it() {
    let s = setup("architect-work");
    let path = config::database(&s.data);
    let mut agents = scripts();
    let change = json!({ "kind": "edit_item", "item": "1", "part": "title", "text": "Parse the input first",
                         "reason": "Clearer." });
    let given = change.clone();
    agents.insert(
        "H8",
        Box::new(move |_: &Request, _: usize| {
            (
                vec![("outcome.answer", "done")],
                vec![
                    ("outcome.answer.reply", json!("I retitle it as an action.")),
                    (
                        "outcome.effects",
                        json!({ "item_changes": versioned(&path, json!([given.clone()])) }),
                    ),
                    (
                        "outcome.closing.log",
                        json!("Retitled #1 as an action, as the owner asked."),
                    ),
                ],
            )
        }),
    );
    let seen = keep_requests(&mut agents, &["H8", "U1", "N1", "P1"]);
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Make the parser's item clearer.");
    let started = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(started.cell.as_deref(), Some("Q0"), "{log}");
    let arch = started.work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        path_of(&e, arch),
        [
            "Q0", "Q2", "H8", "G1", "Q0", "U1", "N1", "Q0", "Q2", "P1", "G1", "Q0", "E2"
        ],
        "{log}"
    );
    let rows: Vec<Option<String>> = cells_of(&e, arch).into_iter().map(|c| c.1).collect();
    assert_eq!(
        rows.iter().map(|r| r.as_deref()).collect::<Vec<_>>(),
        [
            Some("owner"),
            Some("to_owner_words"),
            Some("done"),
            Some("none"),
            Some("plan"),
            Some("to_plan"),
            Some("fits"),
            Some("planned"),
            Some("to_policy_check"),
            Some("holds"),
            Some("none"),
            Some("none"),
            None
        ],
        "{log}"
    );
    // Its reply is its message in the owner's one-to-one.
    let replied: String =
        e.db.conn()
            .query_row(
                "SELECT text FROM conversation_entries WHERE conversation = 'architect:p' AND source = 'message'
                 AND sender = 'architect' ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(replied, "I retitle it as an action.");
    // The change is made, as the architect's, and logged as made with the matter it came from.
    let title: String =
        e.db.conn()
            .query_row("SELECT title FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    assert_eq!(title, "Parse the input first", "{log}");
    let actor: String =
        e.db.conn()
            .query_row(
                "SELECT actor FROM changes WHERE table_name = 'items' AND json_extract(new_row, '$.title') = ?1",
                [&title],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(actor, "architect");
    let (state, logged): (String, String) =
        e.db.conn()
            .query_row("SELECT state, change FROM architect_item_changes", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!(state, "made");
    let mut logged: Value = serde_json::from_str(&logged).unwrap();
    assert!(logged["version"].is_i64(), "{logged}");
    logged.as_object_mut().unwrap().remove("version");
    assert_eq!(logged, change);
    assert_eq!(matters(&e), [("owner_words".to_string(), false, true)], "{log}");
    // The matter keeps its line of the decision log and its two answers.
    let line: String =
        e.db.conn()
            .query_row(
                "SELECT json_extract(logged, '$.log') FROM architect_matters WHERE kind = 'owner_words'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(line, "Retitled #1 as an action, as the owner asked.");
    // The owner's words had a session of their own, with no base to fork; the base read the project and planned in one
    // session, which goes on; the check of the policy forked it, and its session ended with it.
    let base = base_session(&e, arch);
    let sessions = sessions_of(&e, arch);
    assert_eq!(
        sessions,
        [
            ("matter".to_string(), "line".to_string(), None, true, true),
            ("architect".to_string(), "line".to_string(), None, false, false),
            ("matter".to_string(), "fork".to_string(), Some(base.clone()), true, true),
        ],
        "{log}"
    );
    let seen = seen.lock().unwrap();
    let by = |cell: &str| {
        seen.iter()
            .find(|r| r.cell == cell)
            .unwrap_or_else(|| panic!("{cell}\n{log}"))
    };
    assert!(!by("H8").fork && by("H8").resume.is_none());
    assert!(!by("U1").fork && by("U1").resume.is_none());
    assert_eq!(
        (by("N1").fork, by("N1").resume.as_deref()),
        (false, Some(base.as_str()))
    );
    assert_eq!((by("P1").fork, by("P1").resume.as_deref()), (true, Some(base.as_str())));
    // The fork reads the base from the prompt cache: all that comes before the base's messages in the prompt (the model,
    // the tools with the schema of the return among them, the system prompt and the command line's version) is the same
    // in the base's last round and in the fork's first, where only the messages and the effort differ.
    let before = |cell: &str| {
        let r = by(cell);
        sessions::miss::before_messages_of(&engine::agent::call(
            r,
            &engine::agent::command_line(r),
            "2.1.318",
            "e2e",
        ))
    };
    assert!(
        before("N1").iter().any(|(part, _)| part == "--append-system-prompt"),
        "{:?}",
        before("N1")
    );
    assert_eq!(before("P1"), before("N1"));
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The setup and engine of a work of an architect, its log, the work, and the requests of its fork's rounds.
type WarmBase = (Setup, Engine, Arc<Mutex<Vec<String>>>, i64, Arc<Mutex<Vec<Request>>>);

/// A work of project p's architect on a model with prices, which handles the owner's words, builds its base and forks it
/// for the check of the policy the plan queued, the base's rounds and the fork's first telling what they read from the
/// cache as the command line does: the fork's first call reads `fork_read` tokens of the 90,503 the base held.
fn warm_base_work(name: &str, fork_read: i64) -> WarmBase {
    let s = setup_priced(name);
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, _: usize| {
            (
                Vec::new(),
                vec![("outcome.closing.log", json!("Answered the owner on the queue."))],
            )
        }),
    );
    for cell in ["U1", "N1"] {
        agents.insert(cell, adding(cell, |_| vec![("@usage", used(1.0, 90_000))]));
    }
    let first = json!({ "session_cost_usd": 2.0,
        "first_call": { "input_tokens": 3, "cache_read_input_tokens": fork_read, "cache_creation_input_tokens": 2_000 },
        "usage": { "input_tokens": 3, "cache_read_input_tokens": fork_read, "cache_creation_input_tokens": 2_000,
            "output_tokens": 50,
            "iterations": [{ "input_tokens": 3, "cache_read_input_tokens": fork_read, "cache_creation_input_tokens": 2_000 }] } });
    agents.insert("P1", adding("P1", move |_| vec![("@usage", first.clone())]));
    let seen = keep_requests(&mut agents, &["P1"]);
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look at the queue.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        path_of(&e, arch),
        [
            "Q0", "Q2", "H8", "G1", "Q0", "U1", "N1", "Q0", "Q2", "P1", "G1", "Q0", "E2"
        ],
        "{log}"
    );
    (s, e, said, arch, seen)
}

/// A JSON text the database keeps, as the first column of the one row a query gives.
fn kept_json(e: &Engine, sql: &str, arg: &dyn db::rusqlite::ToSql) -> Value {
    let text: String = e.db.conn().query_row(sql, [arg], |r| r.get(0)).unwrap();
    serde_json::from_str(&text).unwrap()
}

// The base's last round, which built it, ended its call before the return it handed in, and a fork reads the base from
// the prompt cache only up to a call the cache keeps under what the fork puts before its messages: before the first fork
// of a base just built, the session manager pings the base once with the fork's command line, resuming the base, so that
// the fork reads all the base holds. The ping and the fork's first round put the same before their messages (the model,
// the schema with the tools, the base's system prompt, the command line's version); the fork forks the base after the
// ping, and having read it, missed nothing.
#[test]
fn the_first_fork_of_a_base_is_made_after_a_ping_that_warms_the_base_for_it() {
    let (s, e, said, arch, seen) = warm_base_work("warm-base", 90_503);
    let log = said.lock().unwrap().join("\n");
    let base = base_session(&e, arch);
    let count = |sql: &str| -> i64 { e.db.conn().query_row(sql, [&base], |r| r.get(0)).unwrap() };
    assert_eq!(
        count("SELECT count(*) FROM pings p JOIN sessions s ON s.id = p.session WHERE s.cli_session = ?1"),
        1,
        "{log}"
    );
    let on_base = "FROM pings p JOIN sessions s ON s.id = p.session WHERE s.cli_session = ?1";
    let waited = kept_json(&e, &format!("SELECT p.waited {on_base}"), &base);
    assert_eq!(
        (waited["cell"].as_str(), waited["on"].as_str()),
        (Some("P1"), Some("fork"))
    );
    let ping = kept_json(&e, &format!("SELECT p.call {on_base}"), &base);
    assert_eq!(ping["session"], json!({ "resumed": base }), "{ping}");
    let fork = kept_json(
        &e,
        "SELECT json_extract(input, '$.call') FROM rounds WHERE work = ?1 AND cell = 'P1' ORDER BY n LIMIT 1",
        &arch,
    );
    let before = sessions::miss::before_messages_of(&ping);
    assert!(
        before.iter().any(|(part, _)| part == "--append-system-prompt"),
        "{before:?}"
    );
    assert_eq!(before, sessions::miss::before_messages_of(&fork));
    let pinged: i64 = count(&format!("SELECT p.started_at {on_base}"));
    let forked: i64 =
        e.db.conn()
            .query_row(
                "SELECT min(started_at) FROM rounds WHERE work = ?1 AND cell = 'P1'",
                [arch],
                |r| r.get(0),
            )
            .unwrap();
    assert!(pinged <= forked, "{pinged} {forked}");
    let seen = seen.lock().unwrap();
    assert_eq!((seen[0].fork, seen[0].resume.as_deref()), (true, Some(base.as_str())));
    assert_eq!(
        count("SELECT count(*) FROM cache_misses WHERE work = (SELECT work FROM sessions WHERE cli_session = ?1)"),
        0,
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A fork that still reads little of its base after the ping that warmed it under the fork's command line missed the
// cache through a fault of wf-agents: it is compared with the ping's call, whose part before the messages is the fork's,
// and the owner is told, naming the ping.
#[test]
fn a_fork_that_misses_its_warmed_base_is_a_fault_named_by_the_ping() {
    let (s, e, said, arch, _) = warm_base_work("warm-base-miss", 30_000);
    let log = said.lock().unwrap().join("\n");
    let (notice, text): (Option<i64>, Option<String>) =
        e.db.conn()
            .query_row(
                "SELECT c.notice, n.text FROM cache_misses c JOIN rounds r ON r.work = c.work AND r.n = c.round
                 LEFT JOIN engine_notices n ON n.id = c.notice WHERE c.work = ?1 AND r.cell = 'P1'",
                [arch],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or_else(|x| panic!("{x}\n{log}"));
    assert!(notice.is_some(), "{log}");
    let text = text.unwrap_or_default();
    assert!(text.contains("from keep-warm ping"), "{text}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work of the architect after the one that built the base takes its matter in a session forked from that base, though
// the base's prompt cache has long been let go: neither time nor a cold cache reads the project again, and the
// project's next work goes from the look at what comes next straight to the matter, told what changed since the base.
#[test]
fn the_next_work_forks_its_matter_from_the_base_however_long_after() {
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, _: usize| {
            (
                Vec::new(),
                vec![("outcome.closing.log", json!("Answered the owner on the queue."))],
            )
        }),
    );
    let seen = keep_requests(&mut agents, &["H8"]);
    let (s, e, said, works) = architect_works_in(
        "fork-later",
        agents,
        &[(0, "Look at the queue."), (3 * 60 * 60_000, "Look again.")],
        false,
    );
    let (first, second) = (works[0], works[1]);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(path_of(&e, second), ["Q0", "Q2", "H8", "G1", "Q0", "E2"], "{log}");
    let base = base_session(&e, first);
    assert_eq!(
        sessions_of(&e, second),
        [("matter".to_string(), "fork".to_string(), Some(base.clone()), true, true)],
        "{log}"
    );
    let seen = seen.lock().unwrap();
    let last = seen.last().unwrap();
    assert_eq!(
        (last.fork, last.resume.as_deref()),
        (true, Some(base.as_str())),
        "{log}"
    );
    let h8 = message_at(&e, second, "H8", false);
    assert!(given_value(&h8, "·engine·since_base").is_some(), "{h8}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The first understanding of a project, with no earlier session to cut the talk at, is given the latest
// entries of the one-to-one; over the size limit it is given the latest whole entries and one line saying how many
// earlier ones there are.
#[test]
fn the_first_understanding_is_given_the_latest_whole_entries_of_the_talk() {
    let s = setup("owner-talk-limit");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    // The end of each long entry is its own, which the first sentence a matter is named by does not carry.
    say(&mut e, "Alpha talk.");
    say(&mut e, &format!("{} GAMMAEND", "g".repeat(35_000)));
    say(&mut e, &format!("{} DELTAEND", "d".repeat(35_000)));
    say(&mut e, "Epsilon talk. EPSILONEND");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| path_of(e, arch).iter().any(|c| c == "U1")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let u1 = input_at(&e, arch, "U1", false);
    assert_eq!(given_ending(&u1, "·engine·owner_talk"), Some(true), "{u1}");
    let m = u1["message"].as_str().unwrap();
    assert!(
        m.contains("DELTAEND") && m.contains("EPSILONEND"),
        "the latest entries, whole"
    );
    assert!(!m.contains("GAMMAEND"), "an entry that does not fit is left out whole");
    assert!(
        m.contains("and 2 earlier entries before these"),
        "{}",
        &m[m.len().saturating_sub(3_000)..]
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// What an agent's round of an architect saw as it ran: its request, and in its working directory the text of
/// `src/lib.txt`, the commit it stood at and whether a file left there earlier was still there.
struct Saw {
    r: Request,
    lib: String,
    head: String,
    left: bool,
}

/// Scripts that keep what the rounds of the cells named saw; the study of a policy asks gh at its first round.
fn keep_what_they_saw(agents: &mut BTreeMap<&'static str, Script>, cells: &[&'static str]) -> Arc<Mutex<Vec<Saw>>> {
    let seen = Arc::new(Mutex::new(Vec::<Saw>::new()));
    for &cell in cells {
        let kept = seen.clone();
        let then = agents.remove(cell);
        agents.insert(
            cell,
            Box::new(move |r: &Request, n: usize| {
                let wt = &r.worktree;
                kept.lock().unwrap().push(Saw {
                    r: r.clone(),
                    lib: std::fs::read_to_string(wt.join("src/lib.txt")).unwrap_or_default(),
                    head: if wt.join(".git").exists() {
                        git(wt, &["rev-parse", "HEAD"])
                    } else {
                        String::new()
                    },
                    left: wt.join("left.txt").exists(),
                });
                if r.cell == "P1" && n == 1 {
                    let tools = json!([{ "tool": "gh", "command": "pr view 1" }]);
                    return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
                }
                then.as_ref().map(|f| f(r, n)).unwrap_or_default()
            }),
        );
    }
    seen
}

// Every line of a project's architect reads the project in the one worktree of its own, detached at main and brought to
// the latest main before each round, whatever was changed there dropped: the check of the policy too, which edits
// nothing and changes no repository; gh for a work on no item runs in that worktree, where gh knows the repository.
#[test]
fn an_architect_s_lines_read_the_latest_main_in_a_worktree_of_its_own() {
    let s = setup("architect-worktrees");
    let mut agents = scripts();
    let seen = keep_what_they_saw(&mut agents, &["U1", "P1"]);
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "Q0");
    let worktrees = s.data.join("worktrees");
    let architect = worktrees.join("architect").join("p");
    main_moves_on(&s.root, &[("src/lib.txt", "one\n")], "One");
    for _ in 0..10 {
        if seen.lock().unwrap().len() >= 2 {
            break;
        }
        e.advance(arch).unwrap();
    }
    let main = git(&s.origin, &["rev-parse", "main"]);
    {
        let seen = seen.lock().unwrap();
        let log = said.lock().unwrap().join("\n");
        assert_eq!(seen.len(), 2, "{log}");
        let (u1, p1) = (&seen[0], &seen[1]);
        assert_eq!((u1.r.cell.as_str(), p1.r.cell.as_str()), ("U1", "P1"), "{log}");
        for saw in [u1, p1] {
            assert_eq!(saw.r.worktree, architect);
            assert_eq!((saw.lib.as_str(), saw.head.as_str()), ("one\n", main.as_str()));
            // The project's checkout is no longer a directory added beside where it runs.
            assert_eq!(saw.r.readable.len(), 2, "{:?}", saw.r.readable);
        }
        for t in config::EDIT_TOOLS {
            assert!(p1.r.refused.iter().any(|x| x == t), "{t}: {:?}", p1.r.refused);
        }
        for g in config::GIT_CHANGES {
            assert!(p1.r.refused.contains(&format!("Bash(git {g} *)")), "git {g}");
        }
        // The session is told where it works in the system prompt it starts with, the same in every work of the
        // project, and the check of the policy, in a session forked from it, is pointed to it.
        let system = u1.r.system.clone().unwrap_or_default();
        assert!(
            system.contains(&format!(
                "\n### work:architect\n\nYou are the architect of the project p. Your working directory is {},",
                u1.r.worktree.display()
            )),
            "{system}"
        );
        assert_eq!(p1.r.system.as_deref(), Some(system.as_str()));
        assert!(p1.r.prompt.contains("Worktree: work:architect"), "{}", p1.r.prompt);
    }
    assert!(!worktrees.join("policy").exists());
    e.advance(arch).unwrap();
    let ran: Value =
        e.db.conn()
            .query_row(
                "SELECT output FROM rounds WHERE work = ?1 AND cell = 'X1' AND transition IS NOT NULL",
                [arch],
                |r| r.get::<_, String>(0),
            )
            .map(|o| serde_json::from_str(&o).unwrap())
            .unwrap();
    assert_eq!(
        ran["results"][0]["ran"]["output"],
        format!("pr view 1 in {}", architect.display()),
        "{ran}"
    );
    // What is changed in the worktree is dropped as it is brought to the latest main for the next round.
    write(&architect, "src/lib.txt", "changed\n");
    write(&architect, "left.txt", "left\n");
    main_moves_on(&s.root, &[("src/lib.txt", "two\n")], "Two");
    for _ in 0..5 {
        if seen.lock().unwrap().len() > 2 {
            break;
        }
        e.advance(arch).unwrap();
    }
    let seen = seen.lock().unwrap();
    let log = said.lock().unwrap().join("\n");
    let next = seen.get(2).expect(&log);
    assert_eq!(next.r.worktree, architect, "{log}");
    assert_eq!(
        (next.lib.as_str(), next.head.as_str(), next.left),
        ("two\n", git(&s.origin, &["rev-parse", "main"]).as_str(), false),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// Each project has its architect: a message from project p's architect is a matter of project q's architect's queue,
// which starts a work of q's architect of its own, since q has no base yet; q's answer comes back to p's queue as a
// message, where it waits for p's next planning, since no work waits on it and p has its base.
#[test]
fn two_projects_architects_send_each_other_messages() {
    let s = setup("two-architects");
    let q = s.root.join("q-checkout");
    git(
        &s.root,
        &["clone", "-q", &s.origin.to_string_lossy(), &q.to_string_lossy()],
    );
    let p_file = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    let checkout = s.root.join("checkout");
    write(
        &s.data,
        "projects/q.toml",
        &p_file
            .replace("o/p", "o/q")
            .replace(&checkout.display().to_string(), &q.display().to_string()),
    );
    git(&s.data, &["add", "-A"]);
    git(&s.data, &["commit", "-q", "-m", "a second project"]);
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, _: usize| {
            let message = json!({ "project": "q", "text": "Does q still need the old format?" });
            (Vec::new(), vec![("outcome.effects", json!({ "messages": [message] }))])
        }),
    );
    agents.insert(
        "H9",
        Box::new(|r: &Request, _: usize| {
            if project_of(r) == "q" {
                return (Vec::new(), vec![("outcome.answer.reply", json!("No, drop it."))]);
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "INSERT INTO dispatch_holds (project, reason, since) VALUES ('q', 'architect', 1)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Ask q about the old format.");
    let projects = |e: &Engine| -> Vec<(i64, String)> {
        let mut st =
            e.db.conn()
                .prepare("SELECT id, project FROM works WHERE graph = 'architect' AND ended IS NULL ORDER BY id")
                .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    let replied = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM architect_queue WHERE project = 'p' AND kind = 'other_architect'",
                [],
                |r| r.get(0),
            )
            .unwrap()
    };
    let done_all = |e: &mut Engine| {
        let ws = projects(e);
        ws.len() >= 2 && replied(e) == 1 && ws.iter().all(|(w, _)| architect_work_done(e, *w))
    };
    assert!(
        steps_until(&mut e, 120, done_all),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let ws = projects(&e);
    let names: Vec<&str> = ws.iter().map(|w| w.1.as_str()).collect();
    assert_eq!(names, ["p", "q"], "{log}");
    assert!(path_of(&e, ws[1].0).contains(&"H9".to_string()), "{log}");
    let messages: Vec<(String, String, String)> = {
        let mut st =
            e.db.conn()
                .prepare(
                    "SELECT project, detail, body ->> 'from' FROM architect_matters WHERE kind = 'other_architect'
                     ORDER BY id",
                )
                .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    assert_eq!(
        messages,
        [
            ("q".to_string(), "message".to_string(), "p".to_string()),
            ("p".to_string(), "reply".to_string(), "q".to_string())
        ],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architect runs at the level the installation names for it, its base, its matters' sessions and the second eye:
// their rounds run with the named level's model, not the top level's, at the effort their cell declares whatever the
// model, and record the level in their call.
#[test]
fn the_architect_runs_at_the_level_the_installation_names() {
    let s = setup("architect-level");
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    let levels = "[agents.levels.2]\ndesc = \"testing\"\ncli = \"claude\"\nmodel = \"m-two\"\n\
                  [agents.levels.4]\ndesc = \"running\"\ncli = \"claude\"\nmodel = \"m-four\"\n\
                  [agents]\narchitect = \"2\"\n";
    write(&s.data, "install.toml", &format!("{install}\n{levels}"));
    git(&s.data, &["commit", "-q", "-am", "the architect's level"]);
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let ran = |e: &Engine, work: i64, cell: &str| -> Option<(String, Value)> {
        let (model, input): (String, String) =
            e.db.conn()
                .query_row(
                    "SELECT model, input FROM rounds WHERE work = ?1 AND cell = ?2 ORDER BY n LIMIT 1",
                    params![work, cell],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok()?;
        let input = db::put_back(e.db.conn(), &serde_json::from_str(&input).unwrap());
        Some((model, input["call"].clone()))
    };
    let at_level = |model: &str, call: &Value| -> (String, Value, Option<String>, Option<String>) {
        let command: Vec<String> = call["command"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|a| a.as_str().map(str::to_string))
            .collect();
        let after = |flag: &str| {
            command
                .iter()
                .position(|a| a == flag)
                .and_then(|i| command.get(i + 1).cloned())
        };
        (
            model.to_string(),
            call["level"].clone(),
            after("--model"),
            after("--effort"),
        )
    };
    say(&mut e, "Look at the queue.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 10, |e| ran(e, arch, "U1").is_some()),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let (model, call) = ran(&e, arch, "U1").unwrap();
    let want = (
        "m-two".to_string(),
        json!("2"),
        Some("m-two".to_string()),
        Some("high".to_string()),
    );
    assert_eq!(at_level(&model, &call), want);
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work that delivered a report ends with its what to do next, which reaches its project's architect as a report of
// its own: the text as written, the report's paths under research/ and the commit the work merged; a work that handed
// in no what to do next queues none.
#[test]
fn a_report_s_what_to_do_next_reaches_the_architect_once_its_work_ends() {
    let s = setup("report-followup");
    let checkout = s.root.join("checkout");
    write(&checkout, "research/report.md", "# Report\nWhat was found.\n");
    git(&checkout, &["add", "-A"]);
    git(&checkout, &["commit", "-q", "-m", "the report"]);
    let merged = git(&checkout, &["rev-parse", "HEAD"]).trim().to_string();
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let at_end = |e: &mut Engine, followup: Option<&str>| -> i64 {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level)
                 VALUES ('p', 'Study the parser', 'x', 'y', 'report', 'l')",
                [],
            )?;
            let item = tx.last_insert_rowid();
            let version: i64 = tx.query_row("SELECT max(id) FROM flow_versions", [], |r| r.get(0))?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'E1')",
                params![item, version],
            )?;
            let w = tx.last_insert_rowid();
            let implemented = match followup {
                Some(f) => json!({ "outcome": { "followup": f } }),
                None => json!({ "outcome": {} }),
            };
            let merged_round = json!({ "result": { "verdict": "merged", "merge_commit": merged } });
            for (n, cell, output, row) in [(1, "I1", implemented, "done"), (2, "M2", merged_round, "merged")] {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                     VALUES (?1, ?2, ?3, '{}', ?4, ?5, 'c', ?2, ?2)",
                    params![w, n, cell, output.to_string(), row],
                )?;
            }
            Ok(w)
        })
        .unwrap()
    };
    type Queued = (String, Option<i64>, Option<i64>, Option<i64>, Value);
    let followups = |e: &Engine| -> Vec<Queued> {
        let mut st =
            e.db.conn()
                .prepare(
                    "SELECT project, item, work, round, body FROM architect_matters
                     WHERE kind = 'report' AND detail = 'followup'",
                )
                .unwrap();
        st.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                serde_json::from_str(&r.get::<_, String>(4)?).unwrap(),
            ))
        })
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
    };
    let with = at_end(&mut e, Some("Open an item to measure the parser."));
    e.step_work(with).unwrap();
    let log = said.lock().unwrap().join("\n");
    let item: i64 =
        e.db.conn()
            .query_row("SELECT item FROM works WHERE id = ?1", [with], |r| r.get(0))
            .unwrap();
    assert_eq!(
        followups(&e),
        [(
            "p".to_string(),
            Some(item),
            Some(with),
            Some(1),
            json!({
                "followup": "Open an item to measure the parser.",
                "reports": ["research/report.md"],
                "commit": merged,
            })
        )],
        "{log}"
    );
    let without = at_end(&mut e, None);
    e.step_work(without).unwrap();
    assert_eq!(followups(&e).len(), 1, "{}", said.lock().unwrap().join("\n"));
    let _ = std::fs::remove_dir_all(&s.root);
}

// At a project's first base its architect is told what happened in the week before it, as at a later base since the
// one before; its board holds every item not done: one whose last work stopped, queued again, with why; one whose
// work asks the owner, at the stage of the cell it stopped at, its pause said once, with who paused it; and one whose
// work runs tools, at the stage of the cell that waits for them; and the owner's start queue, the stopped item first in
// it, which starts ahead of the ranks: in the sixth trial a plan put an item first while the owner's queue started
// another, which no cell was given.
#[test]
fn the_architect_s_first_work_is_told_the_week_before_it_and_every_item_not_done() {
    let s = setup("first-work");
    let (mut e, said) = engine(&s, scripts());
    let merged = an_item(&mut e);
    let v = e.flow_now;
    let ago = db::now() - 24 * 3_600_000;
    let (stopped, asking, tools) =
        e.db.write(Actor::Owner, |tx| {
            let item = |title: &str| -> Result<i64, db::Error> {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level)
                     VALUES ('p', ?1, 'Missing.', 'Needed.', 'change', 'l')",
                    [title],
                )?;
                Ok(tx.last_insert_rowid())
            };
            let (stopped, asking, tools) = (item("Lex it")?, item("Print it")?, item("Test it")?);
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (?1, ?2, 'engineer', 'I1', 'stopped: by hand')",
                params![stopped, v],
            )?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, resume_at, paused, paused_by)
                 VALUES (?1, ?2, 'engineer', 'A2', 'I1', 'later', 'owner')",
                params![asking, v],
            )?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, waiting, paused, paused_by)
                 VALUES (?1, ?2, 'engineer', 'X1', '[\"I1\"]', 'later', 'owner')",
                params![tools, v],
            )?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'E1')",
                params![merged, v],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (?1, 1, 'M2', '{}', '{\"result\":{\"merge_commit\":\"abc\"}}', 'merged', 'c', ?2, ?2)",
                params![tx.last_insert_rowid(), ago],
            )?;
            // The owner put the stopped item first to start, by hand.
            tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 1.0)", [stopped])?;
            Ok((stopped, asking, tools))
        })
        .unwrap();
    say(&mut e, "Look at the items.");
    // A work of the architect starts for what was said, beside the step of the work that merged.
    e.tick().unwrap();
    let arch: i64 =
        e.db.conn()
            .query_row("SELECT id FROM works WHERE graph = 'architect'", [], |r| r.get(0))
            .unwrap();
    assert!(
        steps_until(&mut e, 10, |e| path_of(e, arch).iter().any(|c| c == "U1")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let m = message_at(&e, arch, "U1", false);
    let since = given_value(&m, "·engine·since_last_base").expect(&m);
    assert_eq!(since["first_base"], true, "{since}");
    assert_eq!(since["merged"][0]["item"], merged, "{since}");
    let board = given_value(&m, "·engine·board").expect(&m);
    let row = |id: i64| board["items"][format!("#{id}")].clone();
    assert_eq!(row(merged), Value::Null, "{board}");
    assert_eq!(row(stopped)["last_work_ended"], "stopped: by hand", "{board}");
    assert_eq!(row(stopped)["ready"], true, "{board}");
    assert_eq!(row(stopped).get("work"), None, "{board}");
    assert_eq!(row(asking)["stage"], "implement", "{board}");
    assert_eq!(
        row(asking)["paused"],
        json!({ "by": "owner", "why": "later" }),
        "{board}"
    );
    assert_eq!(row(asking)["waits"], "the architect's reply to its question", "{board}");
    assert_eq!(row(tools)["stage"], "implement", "{board}");
    assert_eq!(board["owner_start_queue"], json!([format!("#{stopped}")]), "{board}");
    assert_eq!(row(stopped)["in_owner_queue"], 1, "{board}");
    assert_eq!(row(asking).get("in_owner_queue"), None, "{board}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The numbers the architect is given are measured up to the start of its work, its own rounds counting for nothing, and
// what each week spent is split between the items' works and the architect's works.
#[test]
fn the_numbers_hold_through_an_architect_work_and_split_what_was_spent() {
    let s = setup("numbers");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let item = an_item(&mut e);
    let v = e.flow_now;
    let ago = db::now() - 3_600_000;
    e.db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE rounds SET usage = '{\"cost_usd\":0.25}' WHERE cell = 'H10'", [])?;
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (?1, ?2, 'engineer', 'I1', 'stopped: by hand')",
            params![item, v],
        )?;
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, transition, config_commit, usage, started_at, ended_at)
             VALUES (?1, 1, 'I1', '{}', 'tools', 'c', '{\"cost_usd\":0.5}', ?2, ?2)",
            params![tx.last_insert_rowid(), ago],
        )?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Look at the numbers.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 20, |e| path_of(e, arch).iter().any(|c| c == "N1")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let understanding = message_at(&e, arch, "U1", false);
    let numbers = given_value(&understanding, "·engine·numbers").expect(&understanding);
    assert_eq!(
        numbers["weeks"][0]["spent_usd"],
        json!({ "items": 0.5, "architect": 0.25 }),
        "{numbers}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The engine watches each project's main: once the full CI of the commit main is at has finished red, a matter of main
// red is queued for that commit, once, which starts a work of the architect and stops the merge queue while it is
// open. The look at what comes next takes it first after the owner's words; the engine checks it mechanically, running
// the red check again on the same commit, reading the known red checks and running the check on main's commit before
// the merge, which is green: the merge made main red, so the matter goes to the architect, which says which merge and
// what is done, and closes it.
#[test]
fn main_red_is_checked_by_the_engine_and_then_judged_by_the_architect() {
    let s = setup("main-red");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    main_moves_on(
        &s.root,
        &[(&format!("tests/{REGRESSION}.txt"), "It fails.\n")],
        "Add a test that fails",
    );
    let head = git(&s.origin, &["rev-parse", "main"]);
    s.watch_main.store(true, Ordering::SeqCst);
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, arch);
    assert_eq!(&path[..5], ["Q0", "Q2", "L1", "L2", "G1"], "{log}");
    assert_eq!(rows_at(&e, arch, "Q0")[0], "main_red", "{log}");
    assert_eq!(rows_at(&e, arch, "L1"), ["red"], "{log}");
    let checked: Value =
        e.db.conn()
            .query_row(
                "SELECT output FROM rounds WHERE work = ?1 AND cell = 'L1' AND transition IS NOT NULL",
                [arch],
                |r| r.get::<_, String>(0),
            )
            .map(|o| serde_json::from_str(&o).unwrap())
            .unwrap();
    assert_eq!(checked["main"]["kind"], "broken_by_merge", "{checked}");
    let (commit, handled): (String, bool) =
        e.db.conn()
            .query_row(
                "SELECT json_extract(body, '$.commit'), handled_round IS NOT NULL FROM architect_matters
                 WHERE kind = 'main_red'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!((commit.as_str(), handled), (head.as_str(), true), "{log}");
    // Main is still red on that commit, and the matter is not queued again for it.
    for _ in 0..3 {
        e.step(None).unwrap();
    }
    let queued: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM architect_matters WHERE kind = 'main_red'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(queued, 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// What waits for the planning and the planning has read is taken after it. Counted again, it called for another planning
// each time: W320 planned nine times in an afternoon, and took nothing the plan queued, nor anything else that waited.
#[test]
fn what_a_planning_has_read_does_not_call_for_another() {
    let s = setup("plan-once");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    for i in 0..12 {
        let m = db::Matter {
            project: "p".into(),
            kind: "other_architect".into(),
            detail: Some("message".into()),
            what: format!("The architect of q says: {i}"),
            body: json!({ "text": format!("{i}") }),
            ..db::Matter::default()
        };
        e.db.write(Actor::Engine, |tx| db::queue_matter(tx, &m)).unwrap();
    }
    let arch = e.step(None).unwrap().work.unwrap();
    let handled = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM architect_matters WHERE kind = 'other_architect' AND handled_round IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert!(
        steps_until(&mut e, 200, |e| handled(e) > 0),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let plans: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'Q0' AND transition = 'plan'",
                [arch],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(plans, 1, "{}", said.lock().unwrap().join("\n"));
    let _ = std::fs::remove_dir_all(&s.root);
}
