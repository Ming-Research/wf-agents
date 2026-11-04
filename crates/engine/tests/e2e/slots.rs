//! Works that wait for a free slot, and which of them takes one.

use super::*;

// A work that waits for the architect's reply holds no slot, and another work starts in the slot it left; once the
// reply came, the first goes on only when a slot is free again, so that the machine never runs more works than its
// slots, and then before any new work starts on a ready item; meanwhile the engine names it as waiting for a free slot,
// which the owner's page shows in place of ready. In the sixth trial a work answered went on at once, and the slots
// read three used of two. The architect's replies are written here by hand, each matter taken off the queue as it comes,
// so that no work of the architect starts and the test says when each reply comes.
#[test]
fn a_work_back_from_waiting_goes_on_in_a_free_slot_before_any_new_start() {
    let s = setup("back-to-a-slot");
    let mut agents = scripts();
    // The first work to research asks the architect; the first to implement, the second work, too.
    for cell in ["R1", "I1"] {
        agents.insert(
            cell,
            Box::new(|_: &Request, n: usize| {
                if n == 1 {
                    an_ask("architect")
                } else {
                    (Vec::new(), Vec::new())
                }
            }),
        );
    }
    let (mut e, said, first) = start(&s, agents);
    let queue = |e: &mut Engine, title: &str, position: f64| -> i64 {
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', ?1, 'x', 'y', 'change', 'l')",
                [title],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, ?2)", params![id, position])?;
            Ok(id)
        })
        .unwrap()
    };
    let second = queue(&mut e, "Add another", 2.0);
    let third = queue(&mut e, "Add a third", 3.0);
    // The question a work's item put to the architect, taken off the queue as if a work of the architect had it in hand.
    let held = |e: &mut Engine, item: i64| -> Option<i64> {
        let m: Option<i64> =
            e.db.conn()
                .query_row(
                    "SELECT id FROM architect_matters WHERE kind = 'engineer_question' AND item = ?1",
                    [item],
                    |r| r.get(0),
                )
                .ok();
        if let Some(m) = m {
            e.db.write(Actor::Engine, |tx| {
                tx.execute(
                    "UPDATE architect_matters SET handled_work = (SELECT min(id) FROM works WHERE graph = 'architect'),
                       handled_round = 1 WHERE id = ?1 AND handled_round IS NULL",
                    [m],
                )?;
                Ok(())
            })
            .unwrap();
        }
        m
    };
    assert!(
        steps_until(&mut e, 20, |e| held(e, first).is_some() && state(e, second).is_some()),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let asked = held(&mut e, first).unwrap();
    let (a, _, _) = state(&e, first).unwrap();
    // The architect replies while the second work holds the one slot: the first stays where it waited.
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "UPDATE architect_matters SET reply = ?2 WHERE id = ?1",
            params![asked, json!({ "text": "Go ahead.", "why": "It holds." }).to_string()],
        )?;
        Ok(())
    })
    .unwrap();
    let mut after = None;
    // While the second holds the one slot the first stays where it waited, and the engine names it, for the owner's
    // page, as waiting for a free slot; the second's question is taken off the queue as it comes.
    let mut named = false;
    for _ in 0..20 {
        if state(&e, first).is_some_and(|x| x.1 != "A2") {
            break;
        }
        let log = said.lock().unwrap().join("\n");
        assert!(e.free_slots().unwrap() >= 0, "{log}");
        let waiting = e.waiting_for_slot().unwrap();
        assert!(waiting.iter().all(|&w| w == a), "{waiting:?}\n{log}");
        named |= waiting == [a];
        after = e.step(after).unwrap().work;
        held(&mut e, second);
    }
    let log = said.lock().unwrap().join("\n");
    assert!(named, "{log}");
    // The first went on only once the second asked the architect in turn, holding no slot, and went on in the slot the
    // second left, to choose the way on with the reply, before a work started on the third item.
    assert_eq!(state(&e, first).map(|x| x.1).as_deref(), Some("A4"), "{log}");
    assert_eq!(state(&e, second).map(|x| x.1).as_deref(), Some("A2"), "{log}");
    assert_eq!(e.waiting_for_slot().unwrap(), Vec::<i64>::new(), "{log}");
    assert_eq!(state(&e, third), None, "{log}");
    assert_eq!(e.free_slots().unwrap(), 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Three works on one machine of two slots, the first two back from waiting for the architect's reply with one slot
/// free, as [`one_waits_for_a_slot`] makes them.
struct OneWaits {
    s: Setup,
    e: Engine,
    said: Arc<Mutex<Vec<String>>>,
    /// The items: the first, whose work was granted the free slot; the second, whose work waits on for one; the third,
    /// whose work holds the other slot.
    items: [i64; 3],
    /// Their works, in the same order.
    works: [i64; 3],
}

impl OneWaits {
    fn log(&self) -> String {
        self.said.lock().unwrap().join("\n")
    }

    /// The cell the work on an item stands at.
    fn at(&self, item: i64) -> Option<String> {
        state(&self.e, item).map(|w| w.1)
    }

    /// Each wait for a slot, in the order they began: its work and how it ended, none while it lasts, and whether it
    /// ended no sooner than it began.
    fn waits(&self) -> Vec<Value> {
        engine::query_json(
            self.e.db.conn(),
            "SELECT work, ended, until IS NULL OR until >= since AS ordered FROM slot_waits ORDER BY id",
            &[],
            10,
        )
        .unwrap()
    }
}

/// A wait for a slot of `work` that ended as `ended`, or lasts when none.
fn a_wait(work: i64, ended: Option<&str>) -> Value {
    json!({ "work": work, "ended": ended, "ordered": 1 })
}

/// With one slot free and two works whose wait for the architect's reply is over, only the first goes on, granted the
/// slot; the other waits on for a free slot, named so for the owner's page, while the third work holds the other slot.
/// The architect's replies are written by hand, each question taken off the queue as it comes, so that no work of the
/// architect starts.
fn one_waits_for_a_slot(name: &str) -> OneWaits {
    let asked: Arc<Mutex<BTreeSet<i64>>> = Arc::default();
    let who = asked.clone();
    let mut agents = scripts();
    // The first two works to research ask the architect, once each.
    agents.insert(
        "R1",
        Box::new(move |r: &Request, _: usize| {
            let mut who = who.lock().unwrap();
            if who.len() < 2 && who.insert(r.work) {
                an_ask("architect")
            } else {
                (Vec::new(), Vec::new())
            }
        }),
    );
    let (s, mut e, said, first, second) = two_works_with(name, agents, Hooks::default());
    let third =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 'Add a third', 'x', 'y', 'change', 'l')",
                [],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 3.0)", params![id])?;
            Ok(id)
        })
        .unwrap();
    // The questions put to the architect, taken off the queue as if a work of the architect had them in hand.
    let take = |e: &mut Engine| {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE architect_matters SET handled_work = (SELECT min(id) FROM works WHERE graph = 'architect'),
                   handled_round = 1 WHERE kind = 'engineer_question' AND handled_round IS NULL",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    };
    let at = |e: &Engine, item: i64| state(e, item).map(|w| w.1);
    for _ in 0..10 {
        if at(&e, first).as_deref() == Some("A2") && at(&e, second).as_deref() == Some("A2") && at(&e, third).is_some()
        {
            break;
        }
        e.tick().unwrap();
        take(&mut e);
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(at(&e, first).as_deref(), Some("A2"), "{log}");
    assert_eq!(at(&e, second).as_deref(), Some("A2"), "{log}");
    let (a, b, c) = (
        state(&e, first).unwrap().0,
        state(&e, second).unwrap().0,
        state(&e, third).expect(&log).0,
    );
    // The architect replies to both at once, while the third work holds one of the two slots.
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "UPDATE architect_matters SET reply = ?1 WHERE kind = 'engineer_question'",
            [json!({ "text": "Go ahead.", "why": "It holds." }).to_string()],
        )?;
        Ok(())
    })
    .unwrap();
    // A pass finds both replies come, granting no slot to a work before its step found its wait over; the next grants
    // the one free slot to the first.
    e.tick().unwrap();
    assert_eq!(at(&e, first).as_deref(), Some("A2"), "{log}");
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_ne!(at(&e, first).as_deref(), Some("A2"), "{log}");
    assert_eq!(at(&e, second).as_deref(), Some("A2"), "{log}");
    assert_eq!(e.waiting_for_slot().unwrap(), vec![b], "{log}");
    assert_eq!(e.free_slots().unwrap(), 0, "{log}");
    let one = OneWaits {
        s,
        e,
        said,
        items: [first, second, third],
        works: [a, b, c],
    };
    assert_eq!(one.waits(), [a_wait(a, Some("granted")), a_wait(b, None)], "{log}");
    one
}

// With one slot free and two works whose wait for the architect's reply is over, only the first goes on, granted the
// slot; the other waits on for a free slot, named so for the owner's page, until a slot frees, here when the owner
// pauses the work that holds the other slot. Each wait is kept, from when the step found the reply come until the loop
// granted the slot.
#[test]
fn of_two_works_back_from_waiting_one_takes_the_one_free_slot() {
    let mut one = one_waits_for_a_slot("one-free-slot");
    let [a, b, c] = one.works;
    // The wait is the database's, not this engine's memory: an engine started afresh on it names it too.
    let (mut afresh, _) = engine(&one.s, scripts());
    assert_eq!(afresh.waiting_for_slot().unwrap(), vec![b], "{}", one.log());
    drop(afresh);
    // The owner pauses the third, which frees its slot: the second goes on in it.
    api::pause(&mut one.e.db, c, None).unwrap();
    one.e.tick().unwrap();
    assert_ne!(one.at(one.items[1]).as_deref(), Some("A2"), "{}", one.log());
    assert_eq!(one.e.waiting_for_slot().unwrap(), Vec::<i64>::new(), "{}", one.log());
    assert_eq!(
        one.waits(),
        [a_wait(a, Some("granted")), a_wait(b, Some("granted"))],
        "{}",
        one.log()
    );
    let _ = std::fs::remove_dir_all(&one.s.root);
}

// After the engine starts again, a work that waited for a free slot as it stopped still does, and takes the first slot
// that frees, before any new work starts on a ready item: here the owner pauses the work that holds the other slot,
// and the first pass of the new engine grants it to the waiting work, while the item queued meanwhile waits.
#[test]
fn a_work_that_waits_for_a_slot_as_the_engine_stops_takes_the_first_free_one_after() {
    let one = one_waits_for_a_slot("slot-after-restart");
    let [a, b, c] = one.works;
    let OneWaits { s, e, items, .. } = one;
    drop(e);
    let (mut e, said) = engine(&s, scripts());
    let fourth = a_ready_item(&mut e, "Add a fourth");
    e.db.write(Actor::Owner, |tx| {
        tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 4.0)", [fourth])?;
        Ok(())
    })
    .unwrap();
    api::pause(&mut e.db, c, None).unwrap();
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_ne!(state(&e, items[1]).map(|w| w.1).as_deref(), Some("A2"), "{log}");
    assert_eq!(state(&e, fourth), None, "{log}");
    let waits = engine::query_json(e.db.conn(), "SELECT work, ended FROM slot_waits ORDER BY id", &[], 10).unwrap();
    assert_eq!(
        waits,
        [
            json!({ "work": a, "ended": "granted" }),
            json!({ "work": b, "ended": "granted" })
        ],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A wait for a free slot ends once its work is paused: the work waits for its owner then, and the engine names it as
// waiting for a slot no longer; the wait says how it ended. So it does in each test below, the way it ended told.
#[test]
fn a_wait_for_a_slot_ends_as_its_work_is_paused() {
    let mut one = one_waits_for_a_slot("slot-wait-paused");
    let [a, b, _] = one.works;
    api::pause(&mut one.e.db, b, None).unwrap();
    one.e.tick().unwrap();
    assert_eq!(one.e.waiting_for_slot().unwrap(), Vec::<i64>::new(), "{}", one.log());
    assert_eq!(
        one.waits(),
        [a_wait(a, Some("granted")), a_wait(b, Some("paused"))],
        "{}",
        one.log()
    );
    let _ = std::fs::remove_dir_all(&one.s.root);
}

// A wait for a free slot ends once its work ends, here as the owner cancels its item.
#[test]
fn a_wait_for_a_slot_ends_as_its_work_ends() {
    let mut one = one_waits_for_a_slot("slot-wait-ended");
    let [a, b, _] = one.works;
    cancel(&mut one.e, one.items[1], "owner");
    one.e.tick().unwrap();
    assert!(ended_why(&one.e, b).is_some(), "{}", one.log());
    assert_eq!(one.e.waiting_for_slot().unwrap(), Vec::<i64>::new(), "{}", one.log());
    assert_eq!(
        one.waits(),
        [a_wait(a, Some("granted")), a_wait(b, Some("ended"))],
        "{}",
        one.log()
    );
    let _ = std::fs::remove_dir_all(&one.s.root);
}

// A wait for a free slot ends once the owner's round asked of the work runs, which holds no slot: the work went on.
#[test]
fn a_wait_for_a_slot_ends_as_the_owner_steps_its_work() {
    let mut one = one_waits_for_a_slot("slot-wait-stepped");
    let [a, b, _] = one.works;
    let stepped = one.e.step_work(b).unwrap();
    assert!(!stepped.rounds.is_empty(), "{}", one.log());
    assert_ne!(one.at(one.items[1]).as_deref(), Some("A2"), "{}", one.log());
    assert_eq!(one.e.waiting_for_slot().unwrap(), Vec::<i64>::new(), "{}", one.log());
    assert_eq!(
        one.waits(),
        [a_wait(a, Some("granted")), a_wait(b, Some("gone_on"))],
        "{}",
        one.log()
    );
    let _ = std::fs::remove_dir_all(&one.s.root);
}

// A wait for a free slot ends once the work is brought to the flow version the engine runs, which cannot take it on
// where it waited: it no longer waits there, and is not kept from asking again where it is brought.
#[test]
fn a_wait_for_a_slot_ends_as_its_work_is_brought_to_a_newer_flow_version() {
    let mut one = one_waits_for_a_slot("slot-wait-brought");
    let [a, b, _] = one.works;
    // The waiting work stood, on an older version, at a cell the version the engine runs does not have.
    older(&mut one.e, &[b], |g| {
        let mut c = g.cell("A2").unwrap().clone();
        c.id = "Z2".into();
        g.cells.push(c);
    });
    one.e
        .db
        .write(Actor::Engine, |tx| {
            tx.execute("UPDATE works SET at = 'Z2' WHERE id = ?1", [b])?;
            Ok(())
        })
        .unwrap();
    one.e.tick().unwrap();
    let log = one.log();
    assert!(log.contains(&format!("work {b} comes from flow version")), "{log}");
    assert_eq!(one.e.waiting_for_slot().unwrap(), Vec::<i64>::new(), "{log}");
    assert_eq!(
        one.waits(),
        [a_wait(a, Some("granted")), a_wait(b, Some("gone_on"))],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&one.s.root);
}

// Of two works that wait, holding no slot, with one slot free, the one whose wait is over goes on, though the other,
// whose wait is not over, has the lower id: the loop grants slots only to works whose step found their wait over, so
// that a work still waiting never takes the slot a work back from waiting needs. Here the second work asked the
// architect and its reply came, while the first still waits for its own reply.
#[test]
fn a_work_back_from_waiting_takes_the_free_slot_before_one_still_waiting_with_a_lower_id() {
    let s = setup("still-waiting-lower-id");
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            if n <= 2 {
                an_ask("architect")
            } else {
                (Vec::new(), Vec::new())
            }
        }),
    );
    let (mut e, said, first) = start(&s, agents);
    let second =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 'Add another', 'x', 'y', 'change', 'l')",
                [],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 2.0)", params![id])?;
            Ok(id)
        })
        .unwrap();
    // The question each work put to the architect, taken off the queue as if a work of the architect had it in hand.
    let question = |e: &mut Engine, item: i64| -> Option<i64> {
        let m: Option<i64> =
            e.db.conn()
                .query_row(
                    "SELECT id FROM architect_matters WHERE kind = 'engineer_question' AND item = ?1",
                    [item],
                    |r| r.get(0),
                )
                .ok();
        if let Some(m) = m {
            e.db.write(Actor::Engine, |tx| {
                tx.execute(
                    "UPDATE architect_matters SET handled_work = (SELECT min(id) FROM works WHERE graph = 'architect'),
                       handled_round = 1 WHERE id = ?1 AND handled_round IS NULL",
                    [m],
                )?;
                Ok(())
            })
            .unwrap();
        }
        m
    };
    // With one slot, the first work asks and waits, holding none, and the second starts in it and asks in turn.
    assert!(
        steps_until(&mut e, 30, |e| question(e, first).is_some()
            && question(e, second).is_some()),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let asked = question(&mut e, second).unwrap();
    let (a, _, _) = state(&e, first).unwrap();
    let (b, _, _) = state(&e, second).unwrap();
    assert!(a < b);
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "UPDATE architect_matters SET reply = ?2 WHERE id = ?1",
            params![asked, json!({ "text": "Go ahead.", "why": "It holds." }).to_string()],
        )?;
        Ok(())
    })
    .unwrap();
    assert!(
        steps_until(&mut e, 10, |e| state(e, second).is_some_and(|x| x.1 == "A4")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(state(&e, first).map(|x| x.1).as_deref(), Some("A2"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
