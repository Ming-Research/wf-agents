//! The architect's works: understanding a matter and the project, its sessions, its level, and what it is told.

use super::*;

// The owner's words start a work of project p's architect, which goes once through the whole graph: it starts, finds
// its new session has not understood the project, understands it, which queues the check of the policy and the plan of
// what to do next at the head of the queue, and takes the owner's words first, in one round: it replies, in the owner's
// one-to-one, and hands in an item change, which is collected; then it checks the policy and plans what to do next;
// the batch is done, its review finds the change sound, and the engine makes it as the architect; the rethink goes on,
// nothing is left, and the work ends. A field the path cell may leave out and wrote as null is taken as left out.
#[test]
fn an_architect_work_goes_through_understanding_a_matter_and_its_review() {
    let s = setup("architect-work");
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
                        json!({ "item_changes": [given.clone()], "observation_changes": null }),
                    ),
                ],
            )
        }),
    );
    agents.insert(
        "B2",
        Box::new(|_: &Request, _: usize| {
            (
                Vec::new(),
                vec![(
                    "outcome.review",
                    json!([{ "change": 1, "verdict": "ok", "why": "It reads better." }]),
                )],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    db::say_to_architect(
        &mut e.db,
        "p",
        None,
        None,
        "Make the parser's item clearer.",
        &[],
        db::Speaker::Owner,
    )
    .unwrap();
    let started = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(started.cell.as_deref(), Some("S1"), "{log}");
    let arch = started.work.unwrap();
    let project: Option<String> =
        e.db.conn()
            .query_row("SELECT project FROM works WHERE id = ?1", [arch], |r| r.get(0))
            .unwrap();
    assert_eq!(project.as_deref(), Some("p"));
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        path_of(&e, arch),
        [
            "S1", "U1", "Q0", "Q2", "H8", "B1", "Q0", "Q2", "P1", "B1", "Q0", "Q2", "N1", "B1", "B5", "B2", "B4", "Q0",
            "E2"
        ],
        "{log}"
    );
    let rows: Vec<Option<String>> = cells_of(&e, arch).into_iter().map(|c| c.1).collect();
    assert_eq!(rows[0].as_deref(), Some("not_understood"));
    assert_eq!(rows[2].as_deref(), Some("owner_words"));
    assert_eq!(rows[4].as_deref(), Some("done"));
    assert_eq!(
        rows[6..9].iter().map(|r| r.as_deref()).collect::<Vec<_>>(),
        [Some("head"), Some("to_policy_check"), Some("holds")]
    );
    assert_eq!(
        rows[10..13].iter().map(|r| r.as_deref()).collect::<Vec<_>>(),
        [Some("head"), Some("to_plan"), Some("fits")]
    );
    // The batch's end goes to the recording of the owner's decisions, which has none and sends the change collected to
    // its review.
    assert_eq!(rows[13].as_deref(), Some("decisions"));
    assert_eq!(rows[14].as_deref(), Some("changes"));
    assert_eq!(rows[15].as_deref(), Some("ok"));
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
    assert_eq!(serde_json::from_str::<Value>(&logged).unwrap(), change);
    assert_eq!(matters(&e), [("owner_words".to_string(), false, true)], "{log}");
    // The review ran on its own line, whose session the batch ended.
    let review: (String, Option<i64>) =
        e.db.conn()
            .query_row(
                "SELECT line, ended FROM sessions WHERE work = ?1 AND line = 'architect_review'",
                [arch],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert!(review.1.is_some(), "{review:?}");
    // The review is given the owner's words the change came from, whole, which in the sixth trial it had only the
    // first line of and could not check a change's reason against.
    let looked = message_at(&e, arch, "B2", false);
    let words = given_value(&looked, "·engine·words_behind_changes").expect(&looked);
    let first = words
        .as_object()
        .and_then(|o| o.values().next())
        .cloned()
        .expect(&looked);
    assert_eq!(first["words"], "Make the parser's item clearer.", "{words}");
    // It is given each item a change touches in full, as its creator wrote it, so that it checks a change against the
    // facts it rests on, and the levels an item may be at, each with its model and the work it is for.
    let touched = given_value(&looked, "·engine·changes_touch").expect(&looked);
    assert_eq!(touched["#1"]["item"]["problem"], "No parser.", "{touched}");
    let levels = given_value(&looked, "·engine·levels").expect(&looked);
    assert_eq!(levels, json!([{ "level": "l", "model": "m", "for": "all work" }]));
    // Every round of the architect's line gives the CLI the same schema, the rethink's included, which keeps the
    // session's prompt cache.
    let schemas: Vec<(String, Value)> = engine::query_json(
        e.db.conn(),
        "SELECT r.cell, r.input FROM rounds r JOIN sessions s ON s.id = r.session
         WHERE r.work = ?1 AND s.line = 'architect' ORDER BY r.n",
        &[&arch],
        100,
    )
    .unwrap()
    .iter()
    .map(|r| {
        let args = r["input"]["call"]["command"].as_array().cloned().unwrap_or_default();
        let at = args.iter().position(|a| a == "--json-schema").unwrap();
        (r["cell"].as_str().unwrap().to_string(), args[at + 1].clone())
    })
    .collect();
    assert!(schemas.iter().any(|x| x.0 == "B4"), "{schemas:?}");
    assert!(schemas.iter().all(|x| x.1 == schemas[0].1), "{schemas:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A batch that handled no matter and changed no item, as when its one matter was parked for the owner's reply, ends with
// no round of the rethink: in the sixth trial the rethink ran an agent's round on such a batch, which changed nothing.
#[test]
fn a_batch_that_changed_nothing_ends_with_no_rethink() {
    let s = setup("nothing-to-rethink");
    no_reply_wait(&s);
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            if n == 2 {
                return asks_the_owner("Which parser goes first?");
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look at the queue.");
    let first = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, first)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    say(&mut e, "Ask me which parser goes first.");
    let second = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| architect_work_done(e, second)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        path_of(&e, second),
        ["S1", "Q0", "Q2", "H8", "A3", "B1", "Q0", "E2"],
        "{log}"
    );
    assert_eq!(rows_at(&e, second, "B1"), ["nothing"], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work of the architect goes on in the session the project's last architect work left: one that understood the
// project in it goes from starting straight to the queue, and does not understand it again.
#[test]
fn a_resumed_session_goes_from_starting_to_the_queue() {
    let s = setup("resumed");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look at the queue.");
    let first = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| architect_work_done(e, first)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    say(&mut e, "Look again.");
    let second = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| architect_work_done(e, second)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        path_of(&e, second),
        ["S1", "Q0", "Q2", "H8", "B1", "B5", "B4", "Q0", "E2"],
        "{log}"
    );
    let sessions: Vec<(String, String)> = {
        let mut st = e
            .db
            .conn()
            .prepare(
                "SELECT started, cli_session FROM sessions WHERE line = 'architect' AND work IN (?1, ?2) ORDER BY id",
            )
            .unwrap();
        st.query_map([first, second], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    assert_eq!(sessions.len(), 2, "{sessions:?}");
    assert_eq!(sessions[1].0, "carried");
    assert_eq!(sessions[0].1, sessions[1].1);
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Two works of project p's architect, the owner's `first` words starting the first and `second` words the second,
/// said `age` milliseconds after the first work ended (the first work and what it was told made that much older).
fn two_architect_works(
    name: &str,
    age: i64,
    first: &str,
    second: &str,
) -> (Setup, Engine, Arc<Mutex<Vec<String>>>, i64, i64) {
    let (s, e, said, works) = architect_works_in(name, scripts(), &[(0, first), (age, second)]);
    (s, e, said, works[0], works[1])
}

// A work of the architect whose project's previous architect work ended 90 minutes ago, a session whose last call the
// CLI's cache has let go, understands the project in a new session, started as expired, and is told what was said
// since the session before began and not what was said before; the same work 30 minutes after the other goes on in
// the session it had, as before.
#[test]
fn a_work_after_the_cache_expired_understands_the_project_in_a_new_session() {
    // The end of each of the words is its own, which the first sentence a matter is named by does not carry.
    let (s, e, said, first, second) = two_architect_works(
        "expired-between",
        90 * 60_000,
        "Alpha talk. ALPHAEND",
        "Beta talk. BETAEND",
    );
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, second);
    assert_eq!(&path[..2], ["S1", "U1"], "{path:?}\n{log}");
    let sessions = architect_sessions(&e, first, second);
    assert_eq!(sessions.len(), 2, "{sessions:?}\n{log}");
    assert_eq!(sessions[1].0, "expired", "{sessions:?}");
    assert_ne!(sessions[0].1, sessions[1].1);
    // The new session is told the talk since the session before began: the second words, not the first, which were
    // said before it began.
    let u1 = input_at(&e, second, "U1", false);
    assert_eq!(given_ending(&u1, "·engine·owner_talk"), Some(true), "{u1}");
    let m = u1["message"].as_str().unwrap();
    assert!(m.contains("BETAEND"), "{m}");
    assert!(!m.contains("ALPHAEND"), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);

    let (s, e, said, first, second) = two_architect_works(
        "fresh-between",
        30 * 60_000,
        "Alpha talk. ALPHAEND",
        "Beta talk. BETAEND",
    );
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, second);
    assert_eq!(&path[..2], ["S1", "Q0"], "{path:?}\n{log}");
    let sessions = architect_sessions(&e, first, second);
    assert_eq!(sessions[1].0, "carried", "{sessions:?}");
    assert_eq!(sessions[0].1, sessions[1].1);
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

// The cache of the architect's session expiring in the middle of a work, with no keep-warm ping to hold it, has the
// next round take the row for a new session, naming the expired cache: the matter in hand goes back to the queue and
// the work understands the project in a new session, started as expired, before it takes the matter again.
#[test]
fn the_cache_expiring_in_the_middle_of_an_architect_work_starts_a_new_session() {
    let s = setup("expired-midway");
    understanding_each_session(&s);
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "One.");
    say(&mut e, "Two.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 20, |e| e.work(arch).unwrap().pos.at == "H8"),
        "{}",
        said.lock().unwrap().join("\n")
    );
    age_architect(&mut e, 90 * 60_000);
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, arch);
    let back = path.iter().rposition(|c| c == "S1").expect(&log);
    assert_eq!(&path[back - 1..back + 3], ["H8", "S1", "U1", "Q0"], "{path:?}\n{log}");
    let why: String =
        e.db.conn()
            .query_row(
                "SELECT json_extract(input, '$.engine.new_session') FROM rounds
                 WHERE work = ?1 AND transition = 'session_anew'",
                [arch],
                |r| r.get(0),
            )
            .unwrap();
    assert!(why.contains("prompt cache expired"), "{why}");
    let lines = line_sessions(&e, arch, "architect");
    assert_eq!(lines.len(), 2, "{lines:?}\n{log}");
    assert_eq!(lines[1].0, "expired", "{lines:?}");
    assert!(lines[1].2.starts_with("U1,"), "{lines:?}");
    assert_eq!(
        matters(&e),
        [
            ("owner_words".to_string(), false, true),
            ("owner_words".to_string(), false, true)
        ],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The cache of the architect's session expiring in the middle of a work while the project's latest understanding holds
// opens a new session at the next round, started as expired, which goes on from the notebook: the work does not go back
// to understanding the project, and the matter in hand stays in hand.
#[test]
fn a_new_session_goes_on_from_the_notebook_while_the_understanding_holds() {
    let s = setup("expired-notebook");
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "One.");
    say(&mut e, "Two.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 20, |e| e.work(arch).unwrap().pos.at == "H8"),
        "{}",
        said.lock().unwrap().join("\n")
    );
    age_architect(&mut e, 90 * 60_000);
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, arch);
    assert_eq!(path.iter().filter(|c| *c == "S1").count(), 1, "{path:?}\n{log}");
    assert_eq!(path.iter().filter(|c| *c == "U1").count(), 1, "{path:?}\n{log}");
    assert!(
        rows_at(&e, arch, "H8").iter().all(|r| r != "session_anew"),
        "{path:?}\n{log}"
    );
    let lines = line_sessions(&e, arch, "architect");
    assert_eq!(lines.len(), 2, "{lines:?}\n{log}");
    assert_eq!(lines[1].0, "expired", "{lines:?}");
    assert!(lines[1].2.starts_with("H8"), "{lines:?}");
    assert_eq!(
        matters(&e),
        [
            ("owner_words".to_string(), false, true),
            ("owner_words".to_string(), false, true)
        ],
        "{log}"
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
    no_reply_wait(&s);
    let mut agents = scripts();
    let seen = keep_what_they_saw(&mut agents, &["U1", "P1"]);
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "U1");
    let worktrees = s.data.join("worktrees");
    let architect = worktrees.join("architect").join("p");
    main_moves_on(&s.root, &[("src/lib.txt", "one\n")], "One");
    for _ in 0..5 {
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
        // project, and the check of the policy, in the same session, is pointed to it.
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
// which starts a work of q's architect of its own; q's answer comes back to p's queue as a message.
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
    let done_all = |e: &mut Engine| {
        let ws = projects(e);
        ws.len() >= 3 && ws.iter().all(|(w, _)| architect_work_done(e, *w))
    };
    assert!(
        steps_until(&mut e, 120, done_all),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let ws = projects(&e);
    let names: Vec<&str> = ws.iter().map(|w| w.1.as_str()).collect();
    assert_eq!(names, ["p", "q", "p"], "{log}");
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

// The architect runs at the level the installation names for it, and so does the review of its item changes: their
// rounds run with the named level's model, not the top level's, at the effort their cell declares whatever the model,
// and record the level in their call.
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

// At a project's first architect work its architect is told what happened in the week before it, as at a later work
// since the last; its board holds every item not done: one whose last work stopped, queued again, with why; one whose
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
    let since = given_value(&m, "·engine·since_last_work").expect(&m);
    assert_eq!(since["first_work"], true, "{since}");
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

// A new session of the architect's line that would start in the middle of an architect work, here at a matter's path,
// remembers nothing but the notebook: the work goes back to understanding the project first, in the new session, and the matter
// picked goes back to the queue, to be picked again and handled.
#[test]
fn a_new_session_in_the_middle_of_an_architect_work_understands_the_project_first() {
    let s = setup("new-session-midway");
    understanding_each_session(&s);
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "One.");
    say(&mut e, "Two.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 20, |e| e.work(arch).unwrap().pos.at == "H8"),
        "{}",
        said.lock().unwrap().join("\n")
    );
    e.sessions().ask_new(api::Fresh::Architect("p".into()));
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, arch);
    // The work was at `H8`, the matter picked, when the new session was asked for: it took the row for that there.
    let back = path.iter().rposition(|c| c == "S1").expect(&log);
    assert_eq!(&path[back - 1..back + 3], ["H8", "S1", "U1", "Q0"], "{path:?}\n{log}");
    assert!(path[back..].contains(&"H8".to_string()), "{path:?}");
    let lines = line_sessions(&e, arch, "architect");
    assert_eq!(lines.len(), 2, "{lines:?}\n{log}");
    assert!(lines[1].2.starts_with("U1,"), "{lines:?}");
    assert_eq!(
        matters(&e),
        [
            ("owner_words".to_string(), false, true),
            ("owner_words".to_string(), false, true)
        ],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}
