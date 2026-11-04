//! Works that take over an item's branch and pull request, and the pull request they merge.

use super::*;

// A new work that takes over from one the owner started over takes a slot only when one is free; until then its item
// waits at the head of the queue, ahead of the items queued before, and starts first once a slot frees.
#[test]
fn a_takeover_waits_for_a_free_slot_at_the_head_of_the_queue() {
    let s = setup("takeover-slots");
    let (mut e, said, queued) = start(&s, scripts());
    // Two items have works running, as many as the machine's one slot and one past it.
    let v = e.flow_now;
    let items: Vec<i64> =
        e.db.write(Actor::Owner, |tx| {
            let mut items = Vec::new();
            for title in ["First", "Second"] {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level)
                     VALUES ('p', ?1, 'It is missing.', 'People need it.', 'change', 'l')",
                    [title],
                )?;
                let item = tx.last_insert_rowid();
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'R1')",
                    params![item, v],
                )?;
                items.push(item);
            }
            Ok(items)
        })
        .unwrap();
    let ready = |e: &Engine| -> Vec<i64> {
        let mut st = e.db.conn().prepare("SELECT item FROM ready_items").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    let first = works_of(&e, items[0])[0].0;
    api::start_over(&mut e.db, first).unwrap();
    // The first stops; the second still holds the slot, so its item waits ahead of the item queued before.
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, items[0]);
    assert!(works.len() == 1 && works[0].5.is_some(), "{works:?}\n{log}");
    assert_eq!(ready(&e), vec![items[0], queued], "{log}");
    assert_eq!(e.free_slots().unwrap(), 0);
    // Once the slot is free, the first starts, before the item queued before.
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE works SET ended = 'stopped by hand' WHERE item = ?1", [items[1]])?;
        Ok(())
    })
    .unwrap();
    let stepped = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(stepped.started_item, Some(items[0]), "{log}");
    // The second, whose work was stopped, waits again in the architect's order, after the item the owner queued.
    assert_eq!(ready(&e), vec![queued, items[1]], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// An item imported with a branch and its pull request takes them over in its first work, and its cell
// `R1` is told where it came from.
#[test]
fn an_imported_item_takes_over_its_branch_and_pull_request() {
    let s = setup("imported");
    let checkout = s.root.join("checkout");
    git(&checkout, &["checkout", "-q", "-b", "outside/feature"]);
    write(&checkout, "src/lib.txt", "half done\n");
    git(&checkout, &["commit", "-q", "-am", "Half of the feature"]);
    git(&checkout, &["push", "-q", "origin", "outside/feature"]);
    let tip = git(&checkout, &["rev-parse", "HEAD"]).trim().to_string();
    git(&checkout, &["checkout", "-q", "main"]);
    git(&checkout, &["branch", "-q", "-D", "outside/feature"]);
    let (mut e, said, item) = start(&s, scripts());
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "UPDATE items SET imported_from = 'item 41 of the earlier board', imported_branch = 'outside/feature',
             imported_pr = 9 WHERE id = ?1",
            [item],
        )?;
        Ok(())
    })
    .unwrap();
    e.step(None).unwrap();
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(
        (works[0].3.as_deref(), works[0].4),
        (Some("outside/feature"), Some(9)),
        "{log}"
    );
    let head = git(Path::new(works[0].2.as_deref().unwrap()), &["rev-parse", "HEAD"]);
    assert_eq!(head.trim(), tip);
    let rounds = agent_rounds(&e, item);
    let m = rounds[0].2["message"].as_str().unwrap();
    assert!(
        m.contains("imported from outside this flow: item 41 of the earlier board"),
        "{m}"
    );
    assert!(m.contains("pr view 9 --comments"), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A pull request keeps the base it was opened with, and a work merges it into the project's main and nowhere else: the
// pull request an imported item takes over, opened against another branch, is set to merge into main as the work
// first pushes to it, and, its base another branch again by the time it merges, is set again before the merge; the
// branch it was opened against gets nothing.
#[test]
fn a_pull_request_opened_against_another_branch_merges_into_main_only() {
    let s = setup("base");
    let checkout = s.root.join("checkout");
    let origin = s.origin.to_string_lossy().to_string();
    git(&checkout, &["push", "-q", "origin", "HEAD:first-version"]);
    let first_version = git(&s.root, &["--git-dir", &origin, "rev-parse", "first-version"]);
    git(&checkout, &["checkout", "-q", "-b", "outside/feature"]);
    write(&checkout, "src/lib.txt", "half done\n");
    git(&checkout, &["commit", "-q", "-am", "Half of the feature"]);
    git(&checkout, &["push", "-q", "origin", "outside/feature"]);
    git(&checkout, &["checkout", "-q", "main"]);
    git(&checkout, &["branch", "-q", "-D", "outside/feature"]);
    let elsewhere = Pr {
        branch: "outside/feature".into(),
        base: "first-version".into(),
        state: PrState::Open,
        merged_as: None,
    };
    s.prs.lock().unwrap().insert(9, elsewhere.clone());
    let (mut e, said, item) = start(&s, scripts());
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "UPDATE items SET imported_from = 'item 41 of the earlier board', imported_branch = 'outside/feature',
             imported_pr = 9 WHERE id = ?1",
            [item],
        )?;
        Ok(())
    })
    .unwrap();
    let base_of = |n: i64| s.prs.lock().unwrap()[&n].base.clone();
    let mut pushed = false;
    for _ in 0..100 {
        if state(&e, item).is_some_and(|(_, at, _)| at == "M2") {
            break;
        }
        e.step(None).unwrap();
        pushed |= !ended_at(&e, item, "X1").is_empty();
        if !pushed {
            assert_eq!(base_of(9), "first-version");
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(state(&e, item).map(|(_, at, _)| at).as_deref(), Some("M2"), "{log}");
    assert!(pushed, "{log}");
    assert_eq!(base_of(9), "main", "{log}");
    // Its base is set again before it merges, whatever changed it since.
    s.prs.lock().unwrap().insert(9, elsewhere);
    for _ in 0..20 {
        if !ended_at(&e, item, "M2").is_empty() {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_at(&e, item, "M2"), [(Some("merged".to_string()), None)], "{log}");
    assert_eq!(base_of(9), "main", "{log}");
    assert_eq!(
        git(&s.root, &["--git-dir", &origin, "show", "main:src/lib.txt"]),
        "old\nfeature"
    );
    assert_eq!(
        git(&s.root, &["--git-dir", &origin, "rev-parse", "first-version"]),
        first_version
    );
    let _ = std::fs::remove_dir_all(&s.root);
}
