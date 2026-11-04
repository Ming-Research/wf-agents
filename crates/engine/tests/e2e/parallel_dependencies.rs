//! Works whose items depend on another by a parallel dependency, which holds a work only before it merges (`M1`): what
//! such a work waits for at the cells before, and what its keep-warm pings say it waits for.

use super::*;

/// A script that hands in what `script` hands in, with what the command line reports the round used, so that the
/// session the round ran in is kept warm while its work waits.
fn with_usage(script: Script) -> Script {
    Box::new(move |r: &Request, n: usize| {
        let (pick, mut set) = script(r, n);
        set.push(("@usage", used(1.0, 9_000)));
        (pick, set)
    })
}

/// The scripts of a work that is designed first, its research and its design reporting what they used.
fn designed_with_usage() -> BTreeMap<&'static str, Script> {
    let mut agents = designing();
    for cell in ["R1", "D2"] {
        let script = agents.remove(cell).unwrap();
        agents.insert(cell, with_usage(script));
    }
    agents
}

/// When the designer's latest round of a work ended: the last call of the session the review of its design comes back
/// to.
fn designed(e: &Engine, work: i64) -> i64 {
    e.db.conn()
        .query_row(
            "SELECT max(r.ended_at) FROM rounds r JOIN sessions s ON s.id = r.session
             WHERE s.work = ?1 AND s.line = 'designer'",
            [work],
            |r| r.get(0),
        )
        .unwrap()
}

/// Steps the work on an item to the review of its design, has the architect reply to the review's matter, and steps it
/// on until the owner is asked: gives the work.
fn asked_for_review(e: &mut Engine, item: i64, said: &Arc<Mutex<Vec<String>>>) -> i64 {
    step_to(e, item, "D3");
    let (work, _, _) = state(e, item).unwrap();
    step_until(e, item, said, "the review's matter", |e| {
        review_matter(e, work).is_some()
    });
    let matter = review_matter(e, work).unwrap();
    architect_replies(e, matter, "It fits the plan.");
    step_until(e, item, said, "the owner's ask", |e| open_ask(e, work).is_some());
    work
}

/// The keep-warm pings sent to the sessions of a work at `now`, the ones before it left out: where each says the work
/// waited and for what. None is a failure, as the work waits and its sessions are due.
fn pinged(e: &mut Engine, work: i64, now: i64, log: &dyn Fn() -> String) -> Vec<Value> {
    let before = pings_of(e, work).len();
    warm_now(e, now, None).unwrap();
    let after: Vec<Value> = pings_of(e, work).into_iter().skip(before).map(|p| p.0).collect();
    assert!(!after.is_empty(), "{}", log());
    after
}

// A work whose item depends on another by a parallel dependency waits for it only at the merge gate: at the review of
// its design it waits for the owner's answer alone, which its keep-warm pings say, and takes the answer in as any work
// does, the item it depends on unmerged: the step after the answer finds it come, and the loop grants the work the free
// slot at the next. In the trial of 2026-11-04 the pings of two such works said they waited for the item to merge.
#[test]
fn a_work_at_the_review_of_its_design_takes_the_owner_s_answer_whatever_its_item_depends_on() {
    let s = setup_priced("parallel-review");
    let (mut e, said, item) = start(&s, designed_with_usage());
    let log = || said.lock().unwrap().join("\n");
    // The item it depends on is held, so that no work starts on it: it stays unmerged.
    let other = a_ready_item(&mut e, "Check it in effect");
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE items SET held = 'Not now.' WHERE id = ?1", [other])?;
        db::add_dependency(tx, item, other, "its check", "parallel")
    })
    .unwrap();
    let work = asked_for_review(&mut e, item, &said);
    let (ask, _) = open_ask(&e, work).unwrap();
    let now = designed(&e, work) + 55 * 60_000;
    let waited = json!({ "cell": "D3", "on": "owner", "what": format!("the owner's answer to ask {ask}") });
    for p in pinged(&mut e, work, now, &log) {
        assert_eq!(p, waited, "{}", log());
    }
    owner_answers(&mut e, ask, "Approved; go ahead.");
    e.tick().unwrap();
    e.tick().unwrap();
    assert_eq!(
        transitions(&e, item, "D3").last().map(String::as_str),
        Some("reviewed"),
        "{}",
        log()
    );
    assert_eq!(state(&e, other), None, "{}", log());
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work whose answer comes while no slot is free waits on for one, holding none, and its keep-warm pings say so, not
// what its item depends on. So the trial of 2026-11-04 went: two works at the review of their design, whose items
// depended by a parallel dependency on an item whose work held a slot, found the owner's answers within seconds, but
// every slot was taken; the first went on only once that work merged and left its slot, while the pings said the works
// waited for the item to merge. Here the work of the item depended on takes the one slot the work at the review left
// as it waited, and holds it as the owner answers.
#[test]
fn a_work_answered_while_no_slot_is_free_is_said_to_wait_for_a_slot() {
    let s = setup_priced("answered-no-slot");
    let mut agents = designed_with_usage();
    // Only the first work to research is designed: the work of the item it depends on starts as it is.
    let design = agents.remove("R1").unwrap();
    let plain = scripts().remove("R1").unwrap();
    let first: Arc<Mutex<Option<i64>>> = Arc::default();
    agents.insert(
        "R1",
        Box::new(move |r: &Request, n: usize| {
            let designs = *first.lock().unwrap().get_or_insert(r.work) == r.work;
            if designs { design(r, n) } else { plain(r, n) }
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let log = || said.lock().unwrap().join("\n");
    let other = a_ready_item(&mut e, "Check it in effect");
    e.db.write(Actor::Owner, |tx| {
        db::add_dependency(tx, item, other, "its check", "parallel")
    })
    .unwrap();
    let work = asked_for_review(&mut e, item, &said);
    step_until(&mut e, item, &said, "the work of the item depended on", |e| {
        state(e, other).is_some()
    });
    let (ask, _) = open_ask(&e, work).unwrap();
    owner_answers(&mut e, ask, "Approved; go ahead.");
    e.tick().unwrap();
    assert_eq!(state(&e, item).map(|w| w.1).as_deref(), Some("D3"), "{}", log());
    assert_eq!(e.waiting_for_slot().unwrap(), vec![work], "{}", log());
    let now = designed(&e, work) + 55 * 60_000;
    let waited = json!({
        "cell": "D3", "on": "slot", "what": "a free slot to go on from cell D3, what it waited for come",
    });
    for p in pinged(&mut e, work, now, &log) {
        assert_eq!(p, waited, "{}", log());
    }
    step_until(&mut e, item, &said, "the review taken in", |e| {
        transitions(e, item, "D3").last().is_some_and(|t| t == "reviewed")
    });
    let waits = engine::query_json(
        e.db.conn(),
        "SELECT ended FROM slot_waits WHERE work = ?1",
        &[&work],
        10,
    )
    .unwrap();
    assert_eq!(waits, [json!({ "ended": "granted" })], "{}", log());
    let _ = std::fs::remove_dir_all(&s.root);
}
