//! A return sent back pauses the engineer's work at once and asks the project's architect why, by a matter to set
//! straight that lists each problem by its check and field; the architect lets the work go on without the owner. The
//! tests here run the flow as built ([`sent_back_pause`]); the others run it with the owner's pause at three in a row.

use super::*;

/// The problems the first return of R1 is sent back with, by their check and field.
const OUTCOME: &str = "format (outcome): outcome must be an object, and is a string";
const PROGRESS: &str = "format (progress): progress must be a string, and is a number";

/// The engine over the flow as built, with one item queued, whose R1 first hands in a return that does not fit its
/// schema in two places and then the return of the default script, and whose architect's cell for a place to set
/// straight (`H1`) lets the work on item 1 go on when `lets_go`.
fn first_return_sent_back(name: &str, lets_go: bool) -> (Engine, Arc<Mutex<Vec<String>>>, i64, Setup) {
    let s = setup(name);
    let mut agents = scripts();
    let then = agents.remove("R1").unwrap();
    agents.insert(
        "R1",
        Box::new(move |r: &Request, n: usize| {
            if n == 1 {
                return (
                    Vec::new(),
                    vec![("outcome", json!("not an object")), ("progress", json!(7))],
                );
            }
            then(r, n)
        }),
    );
    if lets_go {
        agents.insert(
            "H1",
            Box::new(|_: &Request, _: usize| {
                let go = json!({ "kind": "resume_work", "item": 1, "reason": "The return was fixed by asking again." });
                (Vec::new(), vec![("outcome.effects", json!({ "actions": [go] }))])
            }),
        );
    }
    let (mut e, said, item) = start(&s, agents);
    sent_back_pause(&mut e, 1, true);
    (e, said, item, s)
}

/// The matters to set straight that ask why a return was sent back: each with its id, item, what, body, and whether
/// the architect handled it.
fn looks(e: &Engine) -> Vec<(i64, Option<i64>, String, Value, bool)> {
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT id, item, what, body, handled_round IS NOT NULL FROM architect_matters
                 WHERE kind = 'set_straight' AND detail = 'sent_back' ORDER BY id",
            )
            .unwrap();
    st.query_map([], |r| {
        let body: String = r.get(3)?;
        Ok((
            r.get(0)?,
            r.get(1)?,
            r.get(2)?,
            serde_json::from_str(&body).unwrap(),
            r.get(4)?,
        ))
    })
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

/// Who lifted each pause of a work, in order.
fn lifted_by(e: &Engine, work: i64) -> Vec<String> {
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT actor FROM changes
                 WHERE table_name = 'works' AND json_extract(row_key, '$.id') = ?1
                   AND json_extract(old_row, '$.paused') IS NOT NULL AND json_extract(new_row, '$.paused') IS NULL
                 ORDER BY id",
            )
            .unwrap();
    st.query_map([work], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

// A return sent back pauses the work at the cell at once, not after three, and the reason is a line for each problem
// by its check and field, with no JSON. The project's architect is asked in the same step, by one matter to set
// straight for the item, whose body holds each problem as `{check, field, text}` for counting and the same lines for
// reading.
#[test]
fn a_return_sent_back_pauses_the_work_with_each_problem_in_words_and_asks_the_architect_why() {
    let (mut e, said, item, s) = first_return_sent_back("look-pause", false);
    let paused = turns_until(&mut e, &Go::default(), |e| {
        state(e, item).is_some_and(|w| w.2.is_some())
    });
    let log = said.lock().unwrap().join("\n");
    assert!(paused, "{log}");
    let (work, at, why) = state(&e, item).unwrap();
    let why = why.unwrap();
    assert_eq!(
        (at.as_str(), paused_by(&e, work).as_deref()),
        ("R1", Some("engine")),
        "{log}"
    );
    assert!(
        why.starts_with(&format!("{} at cell R1", db::SENT_BACK_FOR_ARCHITECT)),
        "{why}\n{log}"
    );
    let mut lines: Vec<&str> = why.lines().skip(1).collect();
    lines.sort();
    assert_eq!(lines, [OUTCOME, PROGRESS], "{why}");
    assert!(!why.contains('{') && !why.contains("\"format\""), "{why}");
    assert_eq!(
        (counted(&e, work, "sent_back"), sent_back(&e)),
        (1, 1),
        "one return was sent back: {log}"
    );
    // One matter, for the item, whose problems are the return's.
    let all = looks(&e);
    assert_eq!(all.len(), 1, "{all:?}\n{log}");
    let (_, for_item, what, body, _) = &all[0];
    assert_eq!(*for_item, Some(item));
    assert!(what.contains("cell R1") && what.contains("format"), "{what}");
    assert_eq!((body["work"].as_i64(), body["cell"].as_str()), (Some(work), Some("R1")));
    let problems = body["problems"].as_array().unwrap();
    assert!(problems.iter().all(|p| p["check"] == "format"), "{body}");
    let mut fields: Vec<&str> = problems.iter().map(|p| p["field"].as_str().unwrap()).collect();
    fields.sort();
    assert_eq!(fields, ["outcome", "progress"], "{body}");
    let mut read: Vec<&str> = body["reason"].as_str().unwrap().lines().collect();
    read.sort();
    assert_eq!(read, [OUTCOME, PROGRESS], "{body}");
    // The architect is not the owner: the matter was not told as the owner's words.
    assert_eq!(
        number(&e, "SELECT count(*) FROM architect_matters WHERE kind = 'owner_words'"),
        0,
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The path the work takes: the return is sent back, the work pauses and the matter is queued; the architect's round
// on it lets the work go on, with no Start of the owner's; the matter is handled, and the cell runs again in the same
// session, the return it hands in then taken.
#[test]
fn the_architect_lets_a_work_go_on_that_was_paused_for_a_return_sent_back() {
    let (mut e, said, item, s) = first_return_sent_back("look-go-on", true);
    let paused = turns_until(&mut e, &Go::default(), |e| {
        state(e, item).is_some_and(|w| w.2.is_some())
    });
    let log = said.lock().unwrap().join("\n");
    assert!(paused, "{log}");
    let work = state(&e, item).unwrap().0;
    let on = turns_until(&mut e, &Go::default(), |e| {
        paused_by(e, work).is_none() && looks(e).iter().all(|l| l.4) && state(e, item).is_some_and(|w| w.1 != "R1")
    });
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(on, "{log}");
    // The pause was lifted once, by the architect.
    assert_eq!(lifted_by(&e, work), ["architect"], "{log}");
    assert_eq!(looks(&e).len(), 1, "{log}");
    // The cell ran again, and its second return was taken.
    let rows: Vec<Option<String>> = rows_taken(&e, work)
        .into_iter()
        .filter(|(cell, _)| cell == "R1")
        .map(|(_, row)| row)
        .collect();
    assert_eq!(rows.first(), Some(&Some("sent_back".to_string())), "{rows:?}\n{log}");
    assert!(
        rows.len() >= 2 && rows[1] != Some("sent_back".to_string()),
        "{rows:?}\n{log}"
    );
    // The architect's round on it is one on a place to set straight.
    let handled = number(
        &e,
        "SELECT count(*) FROM work_handled WHERE kind = 'set_straight' AND detail = 'sent_back'",
    );
    assert_eq!(handled, 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work of the architect's has no architect above it: its returns sent back pause it for the owner alone, after the
// flow's three in a row, with a line for each problem, and no matter is queued to look at why.
#[test]
fn the_architect_s_own_work_is_paused_for_the_owner_after_three_returns_sent_back_with_each_problem_in_words() {
    let s = setup("look-architect-own");
    let mut agents = scripts();
    // Every return of the owner's-words cell names an item with no work, which the check of its effects refuses.
    agents.insert(
        "H8",
        Box::new(|_: &Request, _: usize| {
            let over = json!({ "kind": "start_work_over", "item": 2, "reason": "Its design went the wrong way." });
            (Vec::new(), vec![("outcome.effects", json!({ "actions": [over] }))])
        }),
    );
    let (mut e, said, _item) = start(&s, agents);
    sent_back_pause(&mut e, 1, true);
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level, cancelled)
             VALUES ('p', 'Gone', 'x', 'y', 'change', 'l', 'not needed')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let work = e.step(None).unwrap().work.unwrap();
    say(&mut e, "Begin the feature again.");
    let paused = |e: &mut Engine| -> Option<(i64, String)> {
        e.db.conn()
            .query_row(
                "SELECT id, paused FROM works WHERE graph = 'architect' AND paused IS NOT NULL",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok()
    };
    assert!(
        steps_until(&mut e, 60, |e| paused(e).is_some()),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let (arch, why) = paused(&mut e).unwrap();
    assert_ne!(arch, work);
    assert!(
        why.starts_with("3 returns in a row were sent back, the last at cell H8, for:\n"),
        "{why}\n{log}"
    );
    assert!(why.contains("action 1: item 2 has no work running"), "{why}");
    assert!(!why.contains('{') && !why.contains("problems"), "{why}");
    assert_eq!(paused_by(&e, arch).as_deref(), Some("engine"), "{log}");
    assert!(looks(&e).is_empty(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
