//! The architect's item changes: what they change, the second eye on those that can lose something, the dependencies
//! they make or remove, and what waits for them.

use super::*;

// A cancel loses what the item held, so a second eye forked from the same base looks before it is made: it finds a
// problem, and the change goes back to the matter's session, at the cell that handed it in, with what it found; none of
// it took effect. The session revises it there, with all it holds; the second eye looks again in its own session and
// finds it sound, and only then is the item cancelled, the first hand-in kept as revised and the second as made.
#[test]
fn a_cancel_waits_for_the_second_eye_and_goes_back_to_its_session_with_what_it_found() {
    let mut agents = scripts();
    agents.insert(
        "H1",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                return (Vec::new(), Vec::new());
            }
            let reason = if n == 2 {
                "Not needed."
            } else {
                "Its acceptance is in #2 now."
            };
            let cancel = json!({ "kind": "cancel", "item": "1", "goes": "Done by #2.", "reason": reason });
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "item_changes": [cancel] }))],
            )
        }),
    );
    agents.insert(
        "C2",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let problem = json!({ "change": 1, "kind": "loses", "what": "Its acceptance has no other home.",
                                  "fact": "Item 1's acceptance is in no other item.", "fix": "Move it first." });
            (
                vec![("outcome.check", "problems")],
                vec![("outcome.check.problems", json!([problem]))],
            )
        }),
    );
    let seen = keep_requests(&mut agents, &["C2"]);
    let (s, e, said, works) = architect_works_in(
        "second-eye",
        agents,
        &[(0, "Look at the queue."), (60_000, "Drop item 1.")],
        false,
    );
    let (first, second) = (works[0], works[1]);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        path_of(&e, second),
        ["N0", "N2", "H1", "C1", "C2", "N2", "H1", "C1", "C2", "N0", "R1"],
        "{log}"
    );
    assert_eq!(rows_at(&e, second, "C2"), ["problems", "sound"], "{log}");
    let cancelled: Option<String> =
        e.db.conn()
            .query_row("SELECT cancelled FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    assert!(cancelled.is_some(), "{log}");
    let states: Vec<String> = {
        let mut st =
            e.db.conn()
                .prepare("SELECT state FROM architect_item_changes ORDER BY id")
                .unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    assert_eq!(states, ["revised", "made"], "{log}");
    // The session that handed the cancel in is told what the second eye found, and revises it in that session.
    let again = message_at(&e, second, "H1", true);
    assert!(again.contains("Its acceptance has no other home."), "{again}");
    let base = base_session(&e, first);
    assert_eq!(
        sessions_of(&e, second),
        [
            (
                "matter".to_string(),
                "fork".to_string(),
                Some(base.clone()),
                false,
                true
            ),
            (
                "second_eye".to_string(),
                "fork".to_string(),
                Some(base.clone()),
                true,
                true
            ),
        ],
        "{log}"
    );
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2, "{log}");
    assert_eq!((seen[0].fork, seen[0].resume.as_deref()), (true, Some(base.as_str())));
    assert!(!seen[1].fork && seen[1].resume.is_some() && seen[1].resume.as_deref() != Some(base.as_str()));
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A matter of project p's architect's queue, as the owner's words.
fn a_matter(tx: &db::rusqlite::Transaction<'_>, what: &str, item: Option<i64>, body: Value) -> Result<i64, db::Error> {
    db::queue_matter(
        tx,
        &db::Matter {
            project: "p".into(),
            kind: "owner_words".into(),
            detail: None,
            what: what.into(),
            body,
            item,
            raised: None,
        },
    )
}

// While an architect work's batch is open, the engine starts no work on an item a change the batch collected names;
// once the batch's changes are made, the item starts as ready_items says.
#[test]
fn an_item_a_batch_changes_does_not_start_until_the_batch_ends() {
    let s = setup("batch-hold");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let item = a_ready_item(&mut e, "Lex it");
    let arch = architect_work_at(&mut e, "N0");
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE works SET paused = 'later', paused_by = 'owner' WHERE id = ?1",
            [arch],
        )?;
        record_round(tx, arch, 1, "H1", &json!({}), "done")?;
        let m = a_matter(tx, "Order the items.", None, json!({}))?;
        let change = json!({ "kind": "add_dependency", "item": item.to_string(), "on": "99", "dependency": "blocking",
                             "reason": "r" });
        tx.execute(
            "INSERT INTO architect_item_changes (work, round, matter, change, state)
             VALUES (?1, 1, ?2, ?3, 'collected')",
            params![arch, m, change.to_string()],
        )?;
        Ok(())
    })
    .unwrap();
    let held = e.step(None).unwrap();
    assert_eq!(held.started_item, None, "{}", said.lock().unwrap().join("\n"));
    e.db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE architect_item_changes SET state = 'made'", [])?;
        Ok(())
    })
    .unwrap();
    let started = e.step(None).unwrap();
    assert_eq!(started.started_item, Some(item), "{}", said.lock().unwrap().join("\n"));
    let _ = std::fs::remove_dir_all(&s.root);
}

// An item that depends on another by a blocking dependency does not start, though it is first in the start order,
// while the other is not merged: the other starts instead.
#[test]
fn an_item_waits_to_start_for_what_it_depends_on_by_a_blocking_dependency() {
    let s = setup("blocking");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let library = a_ready_item(&mut e, "Make the lexer");
    let parser = a_ready_item(&mut e, "Parse with it");
    e.db.write(Actor::Owner, |tx| {
        db::add_dependency(tx, parser, library, "its lexer", "blocking")?;
        tx.execute("INSERT INTO start_order (item, place) VALUES (?1, 1)", [parser])?;
        Ok(())
    })
    .unwrap();
    let started = e.step(None).unwrap();
    assert_eq!(
        started.started_item,
        Some(library),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work that waits for the items its item depends on (`A6`) goes on at once to choose the way on
// (`A4`) when one of them is cancelled, told which, why and what it needed of it; its item no longer depends on
// the cancelled one.
#[test]
fn a_work_waiting_for_a_cancelled_item_goes_on_to_choose_the_way_on() {
    let s = setup("cancelled-dependency");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let lexer = a_ready_item(&mut e, "Make the lexer");
    let parser = a_ready_item(&mut e, "Parse with it");
    let v = e.flow_now;
    let work =
        e.db.write(Actor::Engine, |tx| {
            db::add_dependency(tx, parser, lexer, "its tokens", "parallel")?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'A6')",
                params![parser, v],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    let at = |e: &Engine| -> String {
        e.db.conn()
            .query_row("SELECT at FROM works WHERE id = ?1", [work], |r| r.get(0))
            .unwrap()
    };
    e.step_work(work).unwrap();
    assert_eq!(at(&e), "A6", "{}", said.lock().unwrap().join("\n"));
    let projects = e.config.config.projects.clone();
    e.db.write(Actor::Owner, |tx| {
        engine::ending::cancel_item(tx, lexer, "no longer needed", "owner", &projects).map_err(db::Error::Abort)
    })
    .unwrap();
    e.step_work(work).unwrap();
    assert_eq!(at(&e), "A4", "{}", said.lock().unwrap().join("\n"));
    let told: String =
        e.db.conn()
            .query_row("SELECT text FROM interjections WHERE work = ?1", [work], |r| r.get(0))
            .unwrap();
    assert!(
        told.contains("its tokens") && told.contains("no longer needed"),
        "{told}"
    );
    let links: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM item_links", [], |r| r.get(0))
            .unwrap();
    assert_eq!(links, 0);
    let _ = std::fs::remove_dir_all(&s.root);
}

// `B2` hands in its steps, most first, and the engine queues each as one matter, in that order, ahead of a report; they
// take the place of the steps of the plan before no session took, while a measure the policy needs stays.
#[test]
fn the_plan_s_steps_are_queued_in_its_order_and_replace_those_of_the_plan_before() {
    let s = setup("steps");
    let mut agents = scripts();
    agents.insert(
        "B2",
        Box::new(|_: &Request, _: usize| {
            let steps = json!([
                { "what": "Make the item lex", "why": "The direction needs it." },
                { "what": "Have #5 wait for lex", "why": "It builds on it." }
            ]);
            (Vec::new(), vec![("outcome.steps", steps)])
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "B1");
    e.db.write(Actor::Engine, |tx| {
        for (what, body) in [
            (
                "An old step",
                json!({ "fix": { "where": "An old step", "steps": ["An old step"] } }),
            ),
            (
                "A measure",
                json!({ "fix": { "where": "A measure" }, "measure": { "measure": "m" } }),
            ),
        ] {
            db::queue_matter(
                tx,
                &db::Matter {
                    project: "p".into(),
                    kind: "set_straight".into(),
                    detail: None,
                    what: what.into(),
                    body,
                    item: None,
                    raised: None,
                },
            )?;
        }
        Ok(())
    })
    .unwrap();
    let open = |e: &Engine| -> Vec<String> {
        engine::query_json(
            e.db.conn(),
            "SELECT what FROM architect_matters WHERE kind = 'set_straight' AND withdrawn IS NULL ORDER BY id",
            &[],
            10,
        )
        .unwrap()
        .iter()
        .filter_map(|r| r["what"].as_str().map(str::to_string))
        .collect()
    };
    for _ in 0..12 {
        if open(&e).len() > 2 {
            break;
        }
        e.step_work(arch).unwrap();
    }
    assert_eq!(
        open(&e),
        ["A measure", "Make the item lex", "Have #5 wait for lex"],
        "{}",
        said.lock().unwrap().join("\n")
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// An item change may name an item an earlier change of the batch creates by its key: a new item is placed before
// another in the order works start in, and the engine gives it its number as it makes the batch.
#[test]
fn a_change_names_an_item_the_batch_creates_by_its_key() {
    let s = setup("keys");
    let mut agents = scripts();
    let changes = json!([
        { "kind": "create_item", "key": "spend", "project": "p", "title": "Measure the spend", "problem": "Unknown.",
          "why": "Cost.", "acceptance": ["It is measured."], "level": "l", "deliver": "change", "reason": "r" },
        { "kind": "move", "item": "spend", "to": { "place": "before", "item": "1" }, "reason": "r" },
    ]);
    agents.insert(
        "H1",
        Box::new(move |_: &Request, _: usize| {
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "item_changes": changes.clone() }))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let first = an_item(&mut e);
    say(&mut e, "Measure the spend before parsing.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let order: Vec<(i64, String)> = engine::query_json(
        e.db.conn(),
        "SELECT i.id, i.title FROM items i LEFT JOIN start_order s ON s.item = i.id WHERE i.project = 'p'
         ORDER BY s.place NULLS LAST, i.id",
        &[],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| (r["id"].as_i64().unwrap(), r["title"].as_str().unwrap().to_string()))
    .collect();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(order.len(), 2, "{log}");
    assert_eq!(order[0].1, "Measure the spend", "{log}");
    assert_eq!(order[1].0, first, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Item #217: an item's title is as wide as the installation's `items.title_max` at most (here 24), spaces not counted,
// a Chinese character counting 2 and an English letter 1: an answer that creates an item, or retitles one, with 13
// Chinese characters (26 wide) or 25 letters is sent back saying how wide each is, and handed in again with 12 Chinese
// characters or 24 letters, its changes are made. A title already there, as wide as the limit lets none now be, is not
// looked at again by a change that does not give it a title.
#[test]
fn an_item_title_wider_than_the_installations_limit_is_sent_back() {
    let s = setup_titled("title-too-long");
    let path = config::database(&s.data);
    let mut agents = scripts();
    agents.insert(
        "H1",
        Box::new(move |_: &Request, n: usize| {
            if n > 2 {
                return (Vec::new(), Vec::new());
            }
            // 26 wide and 25; then 24 and 24. The retitle starts with a Chinese character, as a title in the owner's
            // language does.
            let (title, retitle) = if n == 1 {
                (chinese(13), format!("{}Abcdefghijklmnopqrstuvw", chinese(1)))
            } else {
                (chinese(12), format!("{}Abcdefghijklmnopqrstuv", chinese(1)))
            };
            let changes = versioned(&path, json!([
                { "kind": "create_item", "key": "match", "project": "p", "title": title, "problem": "No match.",
                  "why": "Needed.", "acceptance": ["It matches."], "level": "l", "deliver": "change", "reason": "r" },
                { "kind": "edit_item", "item": "1", "part": "title", "text": retitle, "reason": "r" },
                // Item 2 has a title wider than 24, which no change here gives it.
                { "kind": "edit_acceptance", "item": "2", "acceptance": ["It holds."], "reason": "r" },
            ]));
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "item_changes": changes }))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    // Item 2, whose title is 33 wide, as an installation that allowed more once made it.
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level)
             VALUES ('p', 'Abcdefghijklmnopqrstuvwxyzabcdefg', 'x', 'y', 'change', 'l')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Let match take integers.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let rejected = why_sent_back(&e, arch, "H1");
    assert_eq!(rejected.len(), 1, "{rejected:?}");
    assert!(
        rejected[0].contains("item change 1 (create_item): the title is 26 wide and a title is 24 wide at most")
            && rejected[0].contains("item change 2 (edit_item): the title is 25 wide and a title is 24 wide at most"),
        "{rejected:?}"
    );
    // The change that gives item 2 no title is not sent back for the title it has.
    assert!(!rejected[0].contains("item change 3"), "{rejected:?}");
    let titles: Vec<String> = engine::query_json(e.db.conn(), "SELECT title FROM items ORDER BY id", &[], 10)
        .unwrap()
        .iter()
        .map(|r| r["title"].as_str().unwrap_or_default().to_string())
        .collect();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        titles,
        [
            format!("{}Abcdefghijklmnopqrstuv", chinese(1)),
            "Abcdefghijklmnopqrstuvwxyzabcdefg".to_string(),
            chinese(12)
        ],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The dependencies between items, each as the item and the item it depends on, in order.
fn links_of(e: &Engine) -> Vec<(i64, i64)> {
    let mut st =
        e.db.conn()
            .prepare("SELECT item, depends_on FROM item_links ORDER BY item, depends_on")
            .unwrap();
    st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

// An architect's change removes a dependency, and the review of the batch lets it be made: the item no longer depends
// on the other, the dependency it keeps stands, and the changes are made. A dependency the batch makes and removes again
// is not there once the batch is made: each change is checked against what the earlier ones leave.
#[test]
fn a_batch_removes_a_dependency_and_the_review_makes_it() {
    let s = setup("remove-dependency");
    let mut agents = scripts();
    // The items are made below, numbered 1 to 4 in order: the parser, the lexer, the tokens and one to do later. The
    // parser depends on the lexer and on the tokens; the batch removes the first, and makes and removes one on item 4.
    let changes = json!([
        { "kind": "remove_dependency", "item": "1", "on": "2", "reason": "The parser needs nothing the lexer hands in." },
        { "kind": "add_dependency", "item": "1", "on": "4", "dependency": "blocking", "reason": "It seemed to need it." },
        { "kind": "remove_dependency", "item": "1", "on": "4", "reason": "It does not, on a second look." },
    ]);
    agents.insert(
        "H1",
        Box::new(move |_: &Request, _: usize| {
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "item_changes": changes.clone() }))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let parser = an_item(&mut e);
    let lexer = a_ready_item(&mut e, "Lex it");
    let tokens = a_ready_item(&mut e, "Name tokens");
    let later = a_ready_item(&mut e, "Do it later");
    assert_eq!((parser, lexer, tokens, later), (1, 2, 3, 4));
    e.db.write(Actor::Owner, |tx| {
        db::add_dependency(tx, parser, lexer, "its lexer", "blocking")?;
        db::add_dependency(tx, parser, tokens, "its tokens", "parallel")?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "The parser needs nothing the lexer hands in.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(links_of(&e), [(parser, tokens)], "{log}");
    let states: Vec<String> = {
        let mut st =
            e.db.conn()
                .prepare("SELECT state FROM architect_item_changes ORDER BY id")
                .unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    assert_eq!(states, ["made", "made", "made"], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A change that removes a dependency the items do not have then is sent back, with every other reason, and nothing is
// made: one removed twice, one never made, one a cancel of the batch took away, and one of an item another project's
// architect changes, which is the project's own business to remove. A dependency made after a cancel still closes a
// cycle with a dependency the cancel took away, since the write may leave it.
#[test]
fn a_removal_of_a_dependency_there_is_not_is_sent_back_and_nothing_is_made() {
    let s = setup("remove-dependency-refused");
    let mut agents = scripts();
    // The items are numbered 1 to 5 in order: the parser, the lexer, the tokens, one to do later and one of project q.
    // The parser depends on the lexer and the tokens, and the item of q on the lexer.
    agents.insert(
        "H1",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let changes = json!([
                { "kind": "remove_dependency", "item": "1", "on": "2", "reason": "r" },
                { "kind": "remove_dependency", "item": "1", "on": "2", "reason": "r" },
                { "kind": "remove_dependency", "item": "1", "on": "4", "reason": "r" },
                { "kind": "cancel", "item": "3", "goes": "Not needed.", "reason": "r" },
                { "kind": "remove_dependency", "item": "1", "on": "3", "reason": "r" },
                { "kind": "remove_dependency", "item": "5", "on": "2", "reason": "r" },
                // The tokens depend on item 4, and the cancel of the tokens takes that away only when no work goes on
                // them: the write may leave it, so this closes a cycle all the same.
                { "kind": "add_dependency", "item": "4", "on": "3", "dependency": "blocking", "reason": "r" },
            ]);
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "item_changes": changes }))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let parser = an_item(&mut e);
    let lexer = a_ready_item(&mut e, "Lex it");
    let tokens = a_ready_item(&mut e, "Name tokens");
    let later = a_ready_item(&mut e, "Do it later");
    let other =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level)
                 VALUES ('q', 'Lex too', 'Missing.', 'Needed.', 'change', 'l')",
                [],
            )?;
            let other = tx.last_insert_rowid();
            db::add_dependency(tx, parser, lexer, "its lexer", "blocking")?;
            db::add_dependency(tx, parser, tokens, "its tokens", "parallel")?;
            db::add_dependency(tx, other, lexer, "its lexer", "parallel")?;
            db::add_dependency(tx, tokens, later, "its later part", "parallel")?;
            Ok(other)
        })
        .unwrap();
    assert_eq!((parser, lexer, tokens, other), (1, 2, 3, 5));
    say(&mut e, "Take away what the parser does not need.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let rejected = why_sent_back(&e, arch, "H1");
    assert_eq!(rejected.len(), 1, "{rejected:?}\n{log}");
    for reason in [
        "item change 2 (remove_dependency): item 1 does not depend on item 2",
        "item change 3 (remove_dependency): item 1 does not depend on item 4",
        "item change 5 (remove_dependency): item 1 does not depend on item 3",
        "item change 6 (remove_dependency): item 5 is not of p",
    ] {
        assert!(rejected[0].contains(reason), "{reason}\n{rejected:?}");
    }
    assert!(
        rejected[0].contains("the items' dependencies close a cycle"),
        "{rejected:?}"
    );
    // The first change is sound, and the cancel is: neither is named.
    assert!(!rejected[0].contains("item change 1 "), "{rejected:?}");
    assert!(!rejected[0].contains("item change 4 "), "{rejected:?}");
    // Nothing was made: the dependencies stand and the item is not cancelled.
    assert_eq!(links_of(&e), [(1, 2), (1, 3), (3, 4), (5, 2)], "{log}");
    let cancelled: Option<String> =
        e.db.conn()
            .query_row("SELECT cancelled FROM items WHERE id = 3", [], |r| r.get(0))
            .unwrap();
    assert_eq!(cancelled, None, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A move under another item changes only where the item stands: the batch moves the lexer under the checker, whose
// dependency on it is made and becomes the main one, and the parser and the docs still depend on the lexer, so a later
// change of the batch can remove the docs' dependency: the check of the changes finds it there, and so does the write.
#[test]
fn a_move_under_another_item_keeps_the_items_that_depend_on_it() {
    let s = setup("move-under");
    let mut agents = scripts();
    // The items are made below, numbered 1 to 4 in order: the parser, the lexer, the docs and the checker. The parser
    // and the docs depend on the lexer, the parser's dependency made first and so the main one.
    agents.insert(
        "H1",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let changes = json!([
                { "kind": "move", "item": "2", "to": { "place": "under", "item": "4", "dependency": "parallel" },
                  "reason": "The checker reads the lexer's tokens first." },
                { "kind": "remove_dependency", "item": "3", "on": "2",
                  "reason": "The docs need nothing the lexer hands in." },
            ]);
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "item_changes": changes }))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let parser = an_item(&mut e);
    let lexer = a_ready_item(&mut e, "Lex it");
    let docs = a_ready_item(&mut e, "Write the docs");
    let checker = a_ready_item(&mut e, "Check it");
    assert_eq!((parser, lexer, docs, checker), (1, 2, 3, 4));
    e.db.write(Actor::Owner, |tx| {
        db::add_dependency(tx, parser, lexer, "its lexer", "blocking")?;
        db::add_dependency(tx, docs, lexer, "the names of its tokens", "parallel")?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Put the lexer under the checker; the docs need nothing of it.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(why_sent_back(&e, arch, "H1"), Vec::<String>::new(), "{log}");
    let links: Vec<(i64, i64, String, i64)> = {
        let mut st =
            e.db.conn()
                .prepare("SELECT item, depends_on, kind, main FROM item_links ORDER BY item, depends_on")
                .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    assert_eq!(
        links,
        [
            (parser, lexer, "blocking".to_string(), 0),
            (checker, lexer, "parallel".to_string(), 1)
        ],
        "{log}"
    );
    let states: Vec<String> = {
        let mut st =
            e.db.conn()
                .prepare("SELECT state FROM architect_item_changes ORDER BY id")
                .unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    assert_eq!(states, ["made", "made"], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A move under an item that reaches the moved one through other items too, by the dependencies the batch's earlier
// changes leave, is sent back before anything is written, naming the items between them that depend on it directly;
// a move in the same batch under an item that does not reach it is sound.
#[test]
fn a_move_under_an_item_that_reaches_the_moved_one_through_others_too_is_sent_back() {
    let s = setup("move-under-reached");
    let mut agents = scripts();
    // The items are made below, numbered 1 to 5 in order: the parser, the syntax, the lexer, the tokens and the
    // checker. The parser depends on the syntax, the syntax on the lexer and the lexer on the tokens; the batch has the
    // syntax depend on the tokens too, then moves the tokens under the parser, and under the checker.
    agents.insert(
        "H1",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let changes = json!([
                { "kind": "add_dependency", "item": "2", "on": "4", "dependency": "blocking", "reason": "r" },
                { "kind": "move", "item": "4", "to": { "place": "under", "item": "1", "dependency": "blocking" },
                  "reason": "r" },
                { "kind": "move", "item": "4", "to": { "place": "under", "item": "5", "dependency": "blocking" },
                  "reason": "r" },
            ]);
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "item_changes": changes }))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let parser = an_item(&mut e);
    let syntax = a_ready_item(&mut e, "Read the syntax");
    let lexer = a_ready_item(&mut e, "Lex it");
    let tokens = a_ready_item(&mut e, "Name tokens");
    let checker = a_ready_item(&mut e, "Check it");
    assert_eq!((parser, syntax, lexer, tokens, checker), (1, 2, 3, 4, 5));
    e.db.write(Actor::Owner, |tx| {
        db::add_dependency(tx, parser, syntax, "its syntax", "blocking")?;
        db::add_dependency(tx, syntax, lexer, "its lexer", "blocking")?;
        db::add_dependency(tx, lexer, tokens, "its tokens", "blocking")?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Put the tokens where they belong.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let rejected = why_sent_back(&e, arch, "H1");
    assert_eq!(rejected.len(), 1, "{rejected:?}\n{log}");
    assert!(
        rejected[0].contains(
            "item change 2 (move): item 1 reaches item 4 through items 2, 3 too: item 4 stands under an item that \
             needs it directly, so move it under one of them"
        ),
        "{rejected:?}"
    );
    assert!(!rejected[0].contains("item change 1 "), "{rejected:?}");
    assert!(!rejected[0].contains("item change 3 "), "{rejected:?}");
    // Nothing was made.
    assert_eq!(links_of(&e), [(1, 2), (2, 3), (3, 4)], "{log}");
    let made: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM architect_item_changes WHERE state = 'made'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(made, 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work that waits for an item at the merge's judge (`M1`) goes on when the dependency on that item is removed, with
// nobody to say so to it: while the dependency stands it stays, though a slot is free; once it is removed it stays,
// its blocker open, for as long as the machine's one slot is held by another work, and when that slot is free it takes
// the row that clears, towards the merge (`M2`), its blocker resolved.
#[test]
fn a_work_waiting_for_an_item_goes_on_when_the_dependency_is_removed_and_a_slot_is_free() {
    let s = setup("remove-dependency-slot");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let lexer = a_ready_item(&mut e, "Make the lexer");
    let parser = a_ready_item(&mut e, "Parse with it");
    let other = a_ready_item(&mut e, "Keep the slot");
    let v = e.flow_now;
    let work =
        e.db.write(Actor::Engine, |tx| {
            db::add_dependency(tx, parser, lexer, "its tokens", "blocking")?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'M1')",
                params![parser, v],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    // Only this work goes on, so that no other starts or takes the slot.
    let go = Go {
        pause: held_but(&[work], &[]),
        ..Go::default()
    };
    let blockers = |e: &Engine| -> Vec<(String, Option<String>)> {
        let mut st =
            e.db.conn()
                .prepare("SELECT holder, ended FROM blockers WHERE work = ?1 ORDER BY id")
                .unwrap();
        st.query_map([work], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    let log = || said.lock().unwrap().join("\n");
    // It waits for the item, and its blocker is held by the items it waits for.
    e.pass(&go).unwrap();
    assert_eq!(transitions(&e, parser, "M1"), ["wait"], "{}", log());
    assert_eq!(blockers(&e), [("items".to_string(), None)], "{}", log());
    // A slot is free, and the dependency stands: it stays as it is.
    assert_eq!(e.free_slots().unwrap(), 1);
    e.pass(&go).unwrap();
    assert_eq!(transitions(&e, parser, "M1"), ["wait"], "{}", log());
    assert_eq!(blockers(&e), [("items".to_string(), None)], "{}", log());
    // Another work takes the machine's one slot, and the dependency is removed.
    let holder =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'I1')",
                params![other, v],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    assert_eq!(e.free_slots().unwrap(), 0);
    api::remove_dependency(&mut e.db, parser, lexer).unwrap();
    // With no slot free it does not go on, though nothing holds it back: it waits for a slot, its blocker open.
    for _ in 0..2 {
        e.pass(&go).unwrap();
        assert_eq!(transitions(&e, parser, "M1"), ["wait"], "{}", log());
        assert_eq!(blockers(&e), [("items".to_string(), None)], "{}", log());
        assert_eq!(state(&e, parser).unwrap().1, "M1", "{}", log());
        assert_eq!(e.waiting_for_slot().unwrap(), [work], "{}", log());
    }
    // The slot is free: it goes on, the row that clears, and its blocker is resolved. No one said anything to it.
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE works SET ended = 'stopped by hand' WHERE id = ?1", [holder])?;
        Ok(())
    })
    .unwrap();
    e.pass(&go).unwrap();
    assert_eq!(transitions(&e, parser, "M1"), ["wait", "clear"], "{}", log());
    assert_eq!(
        blockers(&e),
        [("items".to_string(), Some("resolved".to_string()))],
        "{}",
        log()
    );
    assert_eq!(state(&e, parser).unwrap().1, "M2", "{}", log());
    let told: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM interjections WHERE work = ?1", [work], |r| {
                r.get(0)
            })
            .unwrap();
    assert_eq!(told, 0, "{}", log());
    let _ = std::fs::remove_dir_all(&s.root);
}

// An answer that cancels an item it creates, named by its key, is sent back: an item is never made only to be
// cancelled, and in the sixth trial a batch held a create, a move and a cancel of one item, which the review could only
// say in words should go. Handed in again without them, nothing of them is made.
#[test]
fn an_answer_that_cancels_an_item_it_creates_is_sent_back_and_never_makes_it() {
    let s = setup("cancel-own");
    let mut agents = scripts();
    let changes = json!([
        { "kind": "create_item", "key": "spend", "project": "p", "title": "Measure the spend", "problem": "Unknown.",
          "why": "Cost.", "acceptance": ["It is measured."], "level": "l", "deliver": "change", "reason": "r" },
        { "kind": "move", "item": "spend", "to": { "place": "first" }, "reason": "r" },
        { "kind": "cancel", "item": "spend", "goes": "Not needed: the owner measures it by hand.", "reason": "r" },
    ]);
    agents.insert(
        "H1",
        Box::new(move |_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "item_changes": changes.clone() }))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Measure the spend, or not.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let back = why_sent_back(&e, arch, "H1");
    assert_eq!(back.len(), 1, "{log}");
    assert!(back[0].contains("cancels spend, which this answer creates"), "{back:?}");
    assert!(!path_of(&e, arch).contains(&"C2".to_string()), "{log}");
    let made: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM items WHERE title = 'Measure the spend'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(made, 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The plan, which judges what runs and what starts, is told, with the slots, whether the engine runs works or the owner
// paused every work, and which run despite it: in the sixth trial the plan was told two slots were free while three
// works stood mid-flow, and could not tell whether the engine ran.
#[test]
fn the_slots_say_whether_works_run() {
    let s = setup("engine-state");
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "B2");
    e.set_global_pause(Some(engine::GlobalPause {
        warm: true,
        runs: [7].into(),
        architects: ["p".to_string()].into(),
    }));
    let m = engine::next_message(&mut e, arch).unwrap().text;
    let slots = given_value(&m, "·engine·slots").expect(&m);
    assert_eq!(slots["engine"]["runs"], "paused", "{slots}");
    assert_eq!(slots["engine"]["started"], json!([7]), "{slots}");
    e.set_global_pause(None);
    let m = engine::next_message(&mut e, arch).unwrap().text;
    let slots = given_value(&m, "·engine·slots").expect(&m);
    assert_eq!(slots["engine"], json!({ "runs": "running" }), "{slots}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// `open_items` says a paused work asked to go on waits for a free slot, and a work asked to start over is stopping, so
// that a cell reading it does not ask for either again.
#[test]
fn open_items_says_a_resume_waits_and_a_work_is_stopping() {
    let s = setup("material-requests");
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    let resuming = a_ready_item(&mut e, "Lex it");
    let stopped = a_ready_item(&mut e, "Parse it");
    let left = a_ready_item(&mut e, "Check it");
    let v = e.flow_now;
    let works: Vec<i64> =
        e.db.write(Actor::Engine, |tx| {
            let mut works = Vec::new();
            for item in [resuming, stopped, left] {
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'R1')",
                    params![item, v],
                )?;
                works.push(tx.last_insert_rowid());
            }
            tx.execute(
                "UPDATE works SET paused = 'wait for the parser', paused_by = 'owner' WHERE id IN (?1, ?2)",
                params![works[0], works[2]],
            )?;
            Ok(works)
        })
        .unwrap();
    api::resume(&mut e.db, works[0]).unwrap();
    api::start_over(&mut e.db, works[1]).unwrap();
    let state = |item: i64| -> String {
        e.db.conn()
            .query_row("SELECT work_state FROM open_items WHERE item = ?1", [item], |r| {
                r.get(0)
            })
            .unwrap()
    };
    assert_eq!(state(resuming), "waiting_for_slot");
    assert_eq!(state(stopped), "stopping");
    assert_eq!(state(left), "paused");
    let _ = std::fs::remove_dir_all(&s.root);
}
