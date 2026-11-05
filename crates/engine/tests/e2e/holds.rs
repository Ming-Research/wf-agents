//! The architect's stops and starts over, its works on free slots, and the holds on a project's new starts.

use super::*;

// The architect pauses a work in its effects: the engine pauses it, with a reason that says the architect stopped it
// and why. Every work let go on, it stays stopped; the owner's Start of that work lets it go on, which is the owner's
// release of it: the architect is told in the owner's words.
#[test]
fn a_work_the_architect_stops_stays_stopped_through_start_all() {
    let s = setup("architect-stop");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                let pause = json!({ "kind": "pause_work", "item": 1, "reason": "It waits for the parser." });
                return (Vec::new(), vec![("outcome.effects", json!({ "actions": [pause] }))]);
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let work = e.step(None).unwrap().work.unwrap();
    say(&mut e, "Stop the feature until the parser is in.");
    // A work of the architect starts for what was said, its first step beside the work's own.
    let pass = e.pass(&Go::default()).unwrap();
    let log = said.lock().unwrap().join("\n");
    let started = pass.jobs.iter().find(|s| s.work != Some(work)).expect(&log);
    assert_eq!(started.cell.as_deref(), Some("Q0"), "{log}");
    let arch = started.work.unwrap();
    let stopped = |e: &mut Engine| state(e, item).is_some_and(|x| x.2.is_some());
    assert!(steps_until(&mut e, 40, stopped), "{}", said.lock().unwrap().join("\n"));
    let why = format!("{}: It waits for the parser.", db::STOPPED_BY_ARCHITECT);
    assert_eq!(state(&e, item).unwrap().2, Some(why.clone()));
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"));
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let n = rounds_count(&e);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(e.step(Some(work)).unwrap().work, None, "{log}");
    assert_eq!((rounds_count(&e), state(&e, item).unwrap().2), (n, Some(why)), "{log}");
    assert_eq!(
        api::resume(&mut e.db, work).unwrap(),
        format!("work {work} goes on once its machine has a slot free; the architect, which had stopped it, is told")
    );
    // The engine's next turn lifts the pause, as its machine has a slot free, and tells the architect.
    e.dispatch(&Go::default()).unwrap();
    e.drain().unwrap();
    let told: String =
        e.db.conn()
            .query_row(
                "SELECT body ->> 'text' FROM architect_matters WHERE kind = 'owner_words' ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(
        told,
        format!("I started work {work}, which you had stopped (It waits for the parser): it goes on.")
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architect starts a work over in its effects: the engine takes it over as it takes over a work the owner started
// over, saying the architect started it over and why, in the work's end and in the group's notice, and a new work on
// the item takes it over. An item with no work running cannot be started over, and the return naming one is sent back;
// nor can a work of the architect.
#[test]
fn the_architect_starts_a_work_over() {
    let s = setup("architect-start-over");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            let item = if n == 1 { 2 } else { 1 };
            let over = json!({ "kind": "start_work_over", "item": item, "reason": "Its design went the wrong way." });
            (Vec::new(), vec![("outcome.effects", json!({ "actions": [over] }))])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let gone =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level, cancelled)
                 VALUES ('p', 'Gone', 'x', 'y', 'change', 'l', 'not needed')",
                [],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    assert_eq!(gone, 2);
    let work = e.step(None).unwrap().work.unwrap();
    say(&mut e, "Begin the feature again, and keep what it built.");
    // A work of the architect starts for what was said, its first step beside the work's own.
    let pass = e.pass(&Go::default()).unwrap();
    let log = said.lock().unwrap().join("\n");
    let started = pass.jobs.iter().find(|s| s.work != Some(work)).expect(&log);
    assert_eq!(started.cell.as_deref(), Some("Q0"), "{log}");
    let arch = started.work.unwrap();
    let over = |e: &mut Engine| works_of(e, item).len() == 2;
    assert!(steps_until(&mut e, 40, over), "{}", said.lock().unwrap().join("\n"));
    let why = "stopped: the architect started it over, and a new work on it takes the item over: Its design went the \
               wrong way";
    let works = works_of(&e, item);
    assert_eq!((works[0].0, works[0].5.as_deref()), (work, Some(why)));
    let notice: String =
        e.db.conn()
            .query_row(
                "SELECT text FROM engine_notices WHERE work = ?1 ORDER BY id DESC LIMIT 1",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(notice, format!("Work {work} stopped: {}.", &why["stopped: ".len()..]));
    let sent_back: String =
        e.db.conn()
            .query_row(
                "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'H8' AND rejected IS NOT NULL",
                [arch],
                |r| r.get(0),
            )
            .unwrap();
    assert!(
        sent_back.contains("action 1: item 2 has no work running"),
        "{sent_back}"
    );
    assert_eq!(
        api::start_over(&mut e.db, arch),
        Err(format!(
            "work {arch} is the architect's: only a work on an item can be started over"
        ))
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// While slots are free and fewer of a project's items are ready to start than slots are free, here none, the engine
// queues a matter of slots free for its architect, with the slots free and the items ready, which starts a work of it;
// once one is handled, no other comes for an hour, nor after it while the slots free and the items ready are those the
// last held, since the architect would only come to the same end.
#[test]
fn slots_free_start_a_work_of_the_architect_once_an_hour_at_most_and_only_on_new_facts() {
    let s = setup("slots-free");
    let (mut e, said) = engine(&s, scripts());
    let started = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(started.cell.as_deref(), Some("Q0"), "{log}");
    let arch = started.work.unwrap();
    let body: String =
        e.db.conn()
            .query_row(
                "SELECT body FROM architect_matters WHERE kind = 'slots_free'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(body, r#"{"free":1,"ready":[]}"#);
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    assert!(path_of(&e, arch).contains(&"N1".to_string()));
    assert_eq!(e.step(None).unwrap().work, None);
    let count = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM architect_matters WHERE kind = 'slots_free'",
                [],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(count(&e), 1);
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE rounds SET ended_at = ended_at - 7200000
             WHERE (work, n) IN (SELECT handled_work, handled_round FROM architect_matters WHERE kind = 'slots_free')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    // Past the wait, with the same slot free and still nothing ready, none comes.
    assert_eq!(e.step(None).unwrap().work, None);
    assert_eq!(count(&e), 1);
    // Once the facts differ from those the last held, here as if an item had been ready then, one comes.
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE architect_matters SET body = '{\"free\":1,\"ready\":[7]}' WHERE kind = 'slots_free'",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let again = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(again.cell.as_deref(), Some("Q0"), "{log}");
    assert_eq!(count(&e), 2);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A hold of a project's new starts stops new works of it from starting from the queue, and the works running go on;
// once it is released, new works start again.
#[test]
fn a_dispatch_hold_stops_new_works_and_the_running_ones_go_on() {
    let s = setup("dispatch-hold");
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(&s.data, "install.toml", &install.replace("slots = 1", "slots = 2"));
    git(&s.data, &["commit", "-q", "-am", "two slots"]);
    let (mut e, said, first) = start(&s, scripts());
    let hold = |e: &mut Engine, on: bool| {
        e.db.write(Actor::Architect, |tx| {
            if on {
                tx.execute(
                    "INSERT INTO dispatch_holds (project, reason, since) VALUES ('p', 'architect', 1)",
                    [],
                )?;
            } else {
                tx.execute("DELETE FROM dispatch_holds", [])?;
            }
            Ok(())
        })
        .unwrap();
    };
    hold(&mut e, true);
    assert_eq!(e.step(None).unwrap().work, None);
    assert!(state(&e, first).is_none());
    hold(&mut e, false);
    let started = e.step(None).unwrap();
    assert_eq!(started.started_item, Some(first), "{}", said.lock().unwrap().join("\n"));
    let work = started.work;
    let second =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level)
                 VALUES ('p', 'Second', 'x', 'y', 'change', 'l')",
                [],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 2.0)", [id])?;
            Ok(id)
        })
        .unwrap();
    hold(&mut e, true);
    let went_on = e.step(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!((went_on.work, went_on.started_item), (work, None), "{log}");
    assert!(state(&e, second).is_none(), "{log}");
    hold(&mut e, false);
    assert_eq!(e.step(work).unwrap().started_item, Some(second));
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The matters of free slots queued for project p's architect, handled or not.
fn slots_free_matters(e: &Engine) -> i64 {
    e.db.conn()
        .query_row(
            "SELECT count(*) FROM architect_matters WHERE project = 'p' AND kind = 'slots_free'",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

/// The reasons of the holds on project p's new starts, in order.
fn hold_reasons(e: &Engine) -> Vec<String> {
    let mut st =
        e.db.conn()
            .prepare("SELECT reason FROM dispatch_holds WHERE project = 'p' ORDER BY reason")
            .unwrap();
    st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
}

// Item #239 (O63): while the owner has paused every work no new work can start, so no matter of free slots is queued for
// any architect, whether the owner started the project's architect or steps it by hand, which before woke the
// architect for a round that could only end in starting nothing. It uses nothing of the owner's hold, so that it runs,
// and fails, on the code before that hold existed. Once every work goes on, the matter is queued as it always was.
#[test]
fn no_matter_of_free_slots_is_queued_while_every_work_is_paused() {
    let s = setup("slots-free-paused");
    let (mut e, said) = engine(&s, scripts());
    let works = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row("SELECT count(*) FROM works WHERE graph = 'architect'", [], |r| r.get(0))
            .unwrap()
    };
    // The owner started the project's architect and paused every other work.
    let started = Go {
        pause: held_but(&[], &["p"]),
        ..Go::default()
    };
    e.pass(&started).unwrap();
    assert_eq!(
        (slots_free_matters(&e), works(&e)),
        (0, 0),
        "{}",
        said.lock().unwrap().join("\n")
    );
    // The owner steps the architect by hand while every work is paused.
    e.step_architect("p").unwrap();
    assert_eq!(
        (slots_free_matters(&e), works(&e)),
        (0, 0),
        "{}",
        said.lock().unwrap().join("\n")
    );
    // Every work paused, no architect started.
    e.pass(&Go {
        pause: held_but(&[], &[]),
        ..Go::default()
    })
    .unwrap();
    assert_eq!(slots_free_matters(&e), 0);
    // With every work going on again, it is queued as before and starts a work of the architect.
    let again = e.step(None).unwrap();
    assert_eq!(again.cell.as_deref(), Some("Q0"), "{}", said.lock().unwrap().join("\n"));
    assert_eq!(slots_free_matters(&e), 1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #239: a hold of the owner's on the project stops the matter of free slots as well, for the new starts it stops
// are the only thing that matter could lead to. Released, the matter is queued as it always was.
#[test]
fn no_matter_of_free_slots_is_queued_while_the_owner_holds_the_project() {
    let s = setup("slots-free-owner-hold");
    let (mut e, said) = engine(&s, scripts());
    let works = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row("SELECT count(*) FROM works WHERE graph = 'architect'", [], |r| r.get(0))
            .unwrap()
    };
    // The owner holds the project: every work goes on, and none is queued.
    api::hold_starts(&mut e.db, &e.config.config, Some("p"), "Leaving for a week.").unwrap();
    e.step(None).unwrap();
    assert_eq!(
        (slots_free_matters(&e), works(&e)),
        (0, 0),
        "{}",
        said.lock().unwrap().join("\n")
    );
    // Released, it is queued as before and starts a work of the architect.
    api::release_hold(&mut e.db, &e.config.config, Some("p"), "owner").unwrap();
    let again = e.step(None).unwrap();
    assert_eq!(again.cell.as_deref(), Some("Q0"), "{}", said.lock().unwrap().join("\n"));
    assert_eq!(slots_free_matters(&e), 1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #239: while the owner holds a project's new starts, a work already running goes on to its merge, and no other
// item of the project starts, though a slot is free; once the owner releases the hold, the waiting item starts. The
// hold is the owner's, written as the owner, and stands whatever the architect does.
#[test]
fn an_owner_hold_lets_the_running_work_merge_and_starts_the_next_item_only_once_released() {
    let s = setup("owner-hold-merge");
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(&s.data, "install.toml", &install.replace("slots = 1", "slots = 2"));
    git(&s.data, &["commit", "-q", "-am", "two slots"]);
    let (mut e, said, first) = start(&s, scripts());
    let started = e.step(None).unwrap();
    assert_eq!(started.started_item, Some(first), "{}", said.lock().unwrap().join("\n"));
    let second =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level)
                 VALUES ('p', 'Second', 'x', 'y', 'change', 'l')",
                [],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 2.0)", [id])?;
            Ok(id)
        })
        .unwrap();
    api::hold_starts(&mut e.db, &e.config.config, Some("p"), "Installing the new machine.").unwrap();
    let actor: String =
        e.db.conn()
            .query_row(
                "SELECT actor FROM changes WHERE table_name = 'dispatch_holds' ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(actor, "owner");
    // The first item's work goes through every cell, the waits for CI and the merge included, with a slot free all the
    // while; the second item does not start.
    let work = state(&e, first).unwrap().0;
    let mut done = false;
    for _ in 0..200 {
        e.tick().unwrap();
        assert!(state(&e, second).is_none(), "{}", said.lock().unwrap().join("\n"));
        if e.advance(work).unwrap() == Advance::Stopped {
            done = true;
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert!(done, "the work did not end:\n{log}");
    let main = git(
        &s.root,
        &["--git-dir", &s.origin.to_string_lossy(), "show", "main:src/lib.txt"],
    );
    assert_eq!(main, "old\nfeature", "{log}");
    // Still held after the merge, the next item waits; released, it starts.
    e.step(None).unwrap();
    assert!(state(&e, second).is_none(), "{log}");
    api::release_hold(&mut e.db, &e.config.config, Some("p"), "owner").unwrap();
    let next = e.step(None).unwrap();
    assert_eq!(next.started_item, Some(second), "{}", said.lock().unwrap().join("\n"));
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #239: the architect's hold and its release leave the owner's hold alone. With the owner's hold in place, the
// architect holds new starts in its effects: its own row is written beside the owner's, as the architect; it releases
// its own, and only its own: the owner's stays, and the board of its later cells still lists it.
#[test]
fn the_architect_s_release_leaves_the_owner_s_hold_and_its_own_hold_stands_beside_it() {
    let s = setup("architect-and-owner-hold");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, _: usize| {
            let hold = json!({ "kind": "hold_starts", "reason": "The plan is wrong." });
            let release = json!({ "kind": "release_starts", "reason": "The plan is right again." });
            (
                vec![("outcome.answer", "done")],
                vec![
                    ("outcome.answer.reply", json!("I hold, then let go.")),
                    ("outcome.effects", json!({ "actions": [hold, release] })),
                ],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    api::hold_starts(&mut e.db, &e.config.config, Some("p"), "Leaving for a week.").unwrap();
    let held_at: i64 =
        e.db.conn()
            .query_row("SELECT since FROM dispatch_holds WHERE reason = 'owner'", [], |r| {
                r.get(0)
            })
            .unwrap();
    say(&mut e, "Hold and release.");
    let started = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(started.cell.as_deref(), Some("Q0"), "{log}");
    let arch = started.work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    // The owner's row is as the owner wrote it; the architect's own came and went.
    assert_eq!(hold_reasons(&e), ["owner"]);
    let owner_row: (i64, String) =
        e.db.conn()
            .query_row(
                "SELECT since, why FROM dispatch_holds WHERE reason = 'owner'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!(owner_row, (held_at, "Leaving for a week.".to_string()));
    let changes: Vec<(String, String, bool)> = {
        let mut st =
            e.db.conn()
                .prepare(
                    "SELECT actor, json_extract(coalesce(new_row, old_row), '$.reason'), new_row IS NULL FROM changes
                     WHERE table_name = 'dispatch_holds' ORDER BY id",
                )
                .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    let row = |a: &str, r: &str, gone: bool| (a.to_string(), r.to_string(), gone);
    assert_eq!(
        changes,
        [
            row("owner", "owner", false),
            row("architect", "architect", false),
            row("architect", "architect", true)
        ]
    );
    // The board the architect was given at its first cell lists the owner's hold. A later work of the architect, after
    // its hold and release, is given no other: its session already holds that board, which the message refers to by
    // its address because nothing in it changed, the architect's own hold gone and the owner's there.
    let m = message_at(&e, arch, "N1", false);
    let board = given_value(&m, "·engine·board").expect(&m);
    let held = &board["starts_held"];
    assert_eq!(held.as_array().map(Vec::len), Some(1), "{board}");
    assert_eq!(
        (&held[0]["reason"], &held[0]["why"]),
        (&json!("owner"), &json!("Leaving for a week.")),
        "{board}"
    );
    let address = m
        .lines()
        .find(|l| l.starts_with("### ") && l.ends_with("·engine·board"))
        .map(|l| l.trim_start_matches("### ").to_string())
        .expect(&m);
    let later = architect_work_at(&mut e, "N1");
    let m = engine::next_message(&mut e, later).unwrap().text;
    assert!(given_value(&m, "·engine·board").is_none(), "{m}");
    assert!(work_from(&m).iter().any(|l| l.ends_with(&address)), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #239: the board of a cell that can hold or release new starts lists each hold on the project with its reason,
// since when and why, whoever holds, and has no such key while nothing holds; the owner's release of any reason takes
// its row, and one of a reason not there is refused saying so.
#[test]
fn the_board_lists_each_hold_and_the_owner_releases_any_of_them() {
    let s = setup("board-holds");
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "N1");
    let board = |e: &mut Engine| -> Value {
        let m = engine::next_message(e, arch).unwrap().text;
        given_value(&m, "·engine·board").expect(&m)
    };
    assert_eq!(board(&mut e).get("starts_held"), None);
    api::hold_starts(&mut e.db, &e.config.config, Some("p"), "Leaving.").unwrap();
    e.db.write(Actor::Architect, |tx| {
        db::hold_starts(tx, "p", db::HoldReason::Architect, Some("Wrong order."))?;
        db::hold_starts(tx, "p", db::HoldReason::SetStraight, None)
    })
    .unwrap();
    let held = board(&mut e)["starts_held"].clone();
    let reasons: Vec<&str> = held
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["reason"].as_str().unwrap())
        .collect();
    let mut sorted = reasons.clone();
    sorted.sort();
    assert_eq!(sorted, ["architect", "owner", "set_straight"], "{held}");
    let of = |reason: &str| {
        held.as_array()
            .unwrap()
            .iter()
            .find(|h| h["reason"] == reason)
            .unwrap()
            .clone()
    };
    assert_eq!(of("owner")["why"], "Leaving.");
    assert_eq!(of("architect")["why"], "Wrong order.");
    assert_eq!(of("set_straight").get("why"), None);
    assert!(of("owner")["since"].is_string(), "{held}");
    for reason in ["set_straight", "architect", "owner"] {
        api::release_hold(&mut e.db, &e.config.config, Some("p"), reason).unwrap();
        assert!(!hold_reasons(&e).contains(&reason.to_string()), "{reason}");
        let again = api::release_hold(&mut e.db, &e.config.config, Some("p"), reason).unwrap_err();
        assert!(again.contains("has no hold of the reason"), "{again}");
    }
    assert_eq!(board(&mut e).get("starts_held"), None);
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #239: an architect work the engine stops, here because the flow version it ran on had a graph for it that the
// version the engine runs lacks (the setting of a policy, a graph of its own before; item #230 ends such a work),
// leaves behind no hold to set the project straight, and takes nothing else: the owner's hold, the architect's own
// hold and a hold of another project all stay.
#[test]
fn an_architect_work_taken_over_releases_only_its_projects_hold_to_set_straight() {
    let s = setup("takeover-holds");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let work = architect_work_at(&mut e, "N1");
    // The work ran on a flow version where it was a setting of a policy, a graph the version the engine runs lacks.
    let now = e.flow_now;
    let mut f = flow::Compiled::clone(&e.flow(now).unwrap());
    let mut policy = f.graph(engine::ARCHITECT).unwrap().clone();
    policy.name = engine::POLICY.into();
    f.graphs.push(policy);
    let text = f.json();
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO flow_versions (digest, compiled) VALUES ('older', ?1)",
            [&text],
        )?;
        let v = tx.last_insert_rowid();
        tx.execute(
            "UPDATE works SET flow_version = ?2, graph = ?3 WHERE id = ?1",
            params![work, v, engine::POLICY],
        )?;
        Ok(())
    })
    .unwrap();
    api::hold_starts(&mut e.db, &e.config.config, Some("p"), "Leaving for a week.").unwrap();
    e.db.write(Actor::Architect, |tx| {
        db::hold_starts(tx, "p", db::HoldReason::Architect, Some("Wrong order."))?;
        db::hold_starts(tx, "p", db::HoldReason::SetStraight, None)?;
        db::hold_starts(tx, "q", db::HoldReason::SetStraight, None)
    })
    .unwrap();
    e.pass(&Go::default()).unwrap();
    let log = said.lock().unwrap().join("\n");
    let ended: Option<String> =
        e.db.conn()
            .query_row("SELECT ended FROM works WHERE id = ?1", [work], |r| r.get(0))
            .unwrap();
    assert!(ended.is_some_and(|w| w.starts_with("stopped")), "{log}");
    let rows: Vec<(String, String)> = {
        let mut st =
            e.db.conn()
                .prepare("SELECT project, reason FROM dispatch_holds ORDER BY project, reason")
                .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    let row = |p: &str, r: &str| (p.to_string(), r.to_string());
    assert_eq!(
        rows,
        [row("p", "architect"), row("p", "owner"), row("q", "set_straight")],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #239: the owner may release their hold while the architect's round is going on, and the architect's actions of
// that round, a release of its own hold that is not there and a hold, still go through: the owner's row is gone as the
// owner released it, the architect's own row stands, and the round is not sent back for an action that found nothing.
#[test]
fn the_owner_releasing_in_the_middle_of_an_architect_round_leaves_its_actions_working() {
    let s = setup("owner-releases-mid-round");
    let path = config::database(&s.data);
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(move |_: &Request, _: usize| {
            // The owner releases their hold while the architect is answering.
            let mut db = db::Db::open(&path).unwrap();
            db.write(Actor::Owner, |tx| {
                db::release_starts(tx, "p", db::HoldReason::Owner).map(|_| ())
            })
            .unwrap();
            let release = json!({ "kind": "release_starts", "reason": "Nothing to hold any more." });
            let hold = json!({ "kind": "hold_starts", "reason": "The plan is wrong." });
            (
                vec![("outcome.answer", "done")],
                vec![
                    ("outcome.answer.reply", json!("Let go, then hold.")),
                    ("outcome.effects", json!({ "actions": [release, hold] })),
                ],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    api::hold_starts(&mut e.db, &e.config.config, Some("p"), "Leaving for a week.").unwrap();
    say(&mut e, "Release and hold.");
    let started = e.step(None).unwrap();
    let arch = started.work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(hold_reasons(&e), ["architect"], "{log}");
    let changes: Vec<(String, String, bool)> = {
        let mut st =
            e.db.conn()
                .prepare(
                    "SELECT actor, json_extract(coalesce(new_row, old_row), '$.reason'), new_row IS NULL FROM changes
                     WHERE table_name = 'dispatch_holds' ORDER BY id",
                )
                .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    let row = |a: &str, r: &str, gone: bool| (a.to_string(), r.to_string(), gone);
    assert_eq!(
        changes,
        [
            row("owner", "owner", false),
            row("owner", "owner", true),
            row("architect", "architect", false)
        ],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}
