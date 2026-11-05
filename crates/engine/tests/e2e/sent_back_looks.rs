//! A return sent back queues a report for the project's architect, which lists each problem by its check and field, and
//! the engineer's work goes on at once; three returns in a row pause it, and the architect or the owner lets it go on.
//! The tests run the flow as built.

use super::*;

/// The problems the first return of R1 is sent back with, by their check and field, as the owner reads them.
pub(super) const OUTCOME: &str = "format (outcome): outcome must be an object, and is a string";
pub(super) const PROGRESS: &str = "format (progress): progress must be a string, and is a number";

/// What is written wrongly in a return of R1, by the path of the field, to send it back.
pub(super) type Wrong = Vec<(&'static str, Value)>;

/// Scripts whose R1 hands in the returns `wrong` says, one for each of its first rounds, each the default return with
/// those fields written as they say, and the default return after.
pub(super) fn returns_sent_back(wrong: Vec<Wrong>) -> BTreeMap<&'static str, Script> {
    let mut agents = scripts();
    let then = agents.remove("R1").unwrap();
    agents.insert(
        "R1",
        Box::new(move |r: &Request, n: usize| {
            let (picks, mut sets) = then(r, n);
            if let Some(w) = n.checked_sub(1).and_then(|i| wrong.get(i)) {
                sets.extend(w.iter().cloned());
            }
            (picks, sets)
        }),
    );
    agents
}

/// A return of R1 whose outcome and progress are both of the wrong type: two problems.
pub(super) fn both_wrong() -> Wrong {
    vec![("outcome", json!("not an object")), ("progress", json!(7))]
}

/// A return of R1 whose outcome alone is of the wrong type: one problem, `OUTCOME`.
pub(super) fn outcome_wrong() -> Wrong {
    vec![("outcome", json!("still not an object"))]
}

/// A return of R1 whose progress alone is of the wrong type: one problem, `PROGRESS`.
pub(super) fn progress_wrong() -> Wrong {
    vec![("progress", json!(8))]
}

/// The reports that ask why a return was sent back: each body and whether the architect handled it, oldest first.
pub(super) fn looks(e: &Engine) -> Vec<(Value, bool)> {
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT body, handled_round IS NOT NULL FROM architect_matters
                 WHERE kind = 'report' AND detail = 'sent_back' ORDER BY id",
            )
            .unwrap();
    st.query_map([], |r| {
        let body: String = r.get(0)?;
        Ok((serde_json::from_str(&body).unwrap(), r.get(1)?))
    })
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

/// What the queue of the architect's matters says each of the reports of returns sent back is waited for by, oldest
/// first.
pub(super) fn waited_for_by(e: &Engine) -> Vec<String> {
    let mut st =
        e.db.conn()
            .prepare("SELECT waits FROM architect_queue WHERE kind = 'report' AND detail = 'sent_back' ORDER BY matter")
            .unwrap();
    st.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
}

/// Who lifted each pause of a work, in order.
pub(super) fn lifted_by(e: &Engine, work: i64) -> Vec<String> {
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

/// The reasons a work was paused with, oldest first, and who paused it.
pub(super) fn pauses(e: &Engine, work: i64) -> Vec<(String, String)> {
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT json_extract(new_row, '$.paused'), json_extract(new_row, '$.paused_by') FROM changes
                 WHERE table_name = 'works' AND json_extract(row_key, '$.id') = ?1
                   AND json_extract(new_row, '$.paused') IS NOT NULL
                   AND json_extract(old_row, '$.paused') IS NULL
                 ORDER BY id",
            )
            .unwrap();
    st.query_map([work], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// The fields of the problems of a matter's body, in the order kept.
pub(super) fn fields(body: &Value) -> Vec<&str> {
    body["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["field"].as_str().unwrap())
        .collect()
}

/// The lines of the problems a pause or a matter says, sorted, with the line before them that says what was sent back
/// left out when `skip` is 1.
fn lines_of(text: &str, skip: usize) -> Vec<&str> {
    let mut lines: Vec<&str> = text.lines().skip(skip).collect();
    lines.sort();
    lines
}

// A return sent back does not pause the work: the cell runs again and its next return is taken. The project's architect
// is asked in the same write that ends the round, by one report for the item, whose body holds each problem as
// `{check, field, text}` for counting and the same lines for reading; nothing waits for it, and it is not the owner's
// words. When the work ends, the report stays in the queue.
#[test]
fn a_return_sent_back_queues_a_report_for_the_architect_and_the_work_goes_on() {
    let s = setup("look-queued");
    let (mut e, said, item) = start(&s, returns_sent_back(vec![both_wrong()]));
    let work = e.step(None).unwrap().work.unwrap();
    // Only this work runs, so that the architect does not take the matter before the test has looked at it.
    let held = Go {
        pause: held_but(&[work], &[]),
        ..Go::default()
    };
    // The work starts at the cell before R1 and comes to R1, so that the cell is done once a return of it is taken.
    let on = turns_until(&mut e, &held, |e| {
        rows_taken(e, work)
            .iter()
            .any(|(cell, row)| cell == "R1" && row.as_deref().is_some_and(|r| r != "sent_back"))
    });
    let log = said.lock().unwrap().join("\n");
    assert!(on, "{log}");
    // The work was never paused, and the cell ran again: the second return was taken.
    assert!(pauses(&e, work).is_empty(), "{log}");
    assert_eq!(paused_by(&e, work), None, "{log}");
    let rows: Vec<Option<String>> = rows_taken(&e, work)
        .into_iter()
        .filter(|(cell, _)| cell == "R1")
        .map(|(_, row)| row)
        .collect();
    assert_eq!(rows.first(), Some(&Some("sent_back".to_string())), "{rows:?}\n{log}");
    assert!(
        rows.len() >= 2 && rows[1].as_deref() != Some("sent_back"),
        "{rows:?}\n{log}"
    );
    // One report, for the item, whose problems are the return's.
    let all = looks(&e);
    assert_eq!(all.len(), 1, "{all:?}\n{log}");
    let (body, handled) = &all[0];
    assert!(!handled, "{log}");
    let (for_item, what): (Option<i64>, String) =
        e.db.conn()
            .query_row(
                "SELECT item, what FROM architect_matters WHERE kind = 'report' AND detail = 'sent_back'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!(for_item, Some(item));
    assert!(what.contains("cell R1") && what.contains("format"), "{what}");
    let round = number(&e, "SELECT n FROM rounds WHERE rejected IS NOT NULL");
    assert_eq!(
        (body["work"].as_i64(), body["round"].as_i64(), body["cell"].as_str()),
        (Some(work), Some(round), Some("R1")),
        "{body}"
    );
    let problems = body["problems"].as_array().unwrap();
    assert!(problems.iter().all(|p| p["check"] == "format"), "{body}");
    let mut kept = fields(body);
    kept.sort();
    assert_eq!(kept, ["outcome", "progress"], "{body}");
    let reason = body["reason"].as_str().unwrap();
    assert_eq!(lines_of(reason, 0), [OUTCOME, PROGRESS], "{body}");
    assert!(
        !reason.contains('{') && !reason.contains('}') && !reason.contains("problems"),
        "{reason}"
    );
    // Nothing waits for it, and the architect is not told it as the owner's words.
    assert_eq!(waited_for_by(&e), ["none"], "{log}");
    assert_eq!(
        number(&e, "SELECT count(*) FROM architect_matters WHERE kind = 'owner_words'"),
        0,
        "{log}"
    );
    // When the work ends, the report is not taken back: the cause of the return is still to be looked for.
    cancel(&mut e, item, "owner");
    let ended = turns_until(&mut e, &held, |e| ended_why(e, work).is_some());
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(ended, "{log}");
    assert_eq!(
        number(
            &e,
            "SELECT count(*) FROM architect_matters
             WHERE kind = 'report' AND detail = 'sent_back' AND withdrawn IS NULL AND handled_round IS NULL"
        ),
        1,
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// Three returns of the cell in a row, each sent back for its own problems, pause the work at the cell, not the first
// nor the second; each has its report, with its own round and problems. The pause says the last return's problems alone,
// one line each, and the architect may let the work go on. While the work is paused, its reports are waited for, and
// when it goes on they are not.
#[test]
fn three_returns_in_a_row_sent_back_pause_the_work_and_each_has_its_report() {
    let s = setup("look-three");
    let (mut e, said, item) = start(
        &s,
        returns_sent_back(vec![both_wrong(), outcome_wrong(), progress_wrong()]),
    );
    let work = e.step(None).unwrap().work.unwrap();
    let held = Go {
        pause: held_but(&[work], &[]),
        ..Go::default()
    };
    let paused = turns_until(&mut e, &held, |e| paused_by(e, work).is_some());
    let log = said.lock().unwrap().join("\n");
    assert!(paused, "{log}");
    assert_eq!(state(&e, item).unwrap().1, "R1", "{log}");
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"), "{log}");
    // The cell ran three times, each time sent back, and the pause came before a fourth.
    let rows: Vec<Option<String>> = rows_taken(&e, work)
        .into_iter()
        .filter(|(cell, _)| cell == "R1")
        .map(|(_, row)| row)
        .collect();
    assert_eq!(rows, vec![Some("sent_back".to_string()); 3], "{log}");
    // One pause, saying the last return's problem alone, in the words the architect may let go on.
    let made = pauses(&e, work);
    assert_eq!(made.len(), 1, "{made:?}\n{log}");
    let (why, by) = &made[0];
    assert_eq!(by, "engine");
    assert!(
        why.starts_with(
            "returns were sent back 3 times in a row, the last at cell R1, and its project's architect is asked to look \
             at why"
        ),
        "{why}"
    );
    assert_eq!(lines_of(why, 1), [PROGRESS], "{why}");
    assert!(!why.contains('{') && !why.contains("problems"), "{why}");
    // Three reports, one for each return, each with its own round and problems.
    let all = looks(&e);
    assert_eq!(all.len(), 3, "{log}");
    let mut first = fields(&all[0].0);
    first.sort();
    assert_eq!(first, ["outcome", "progress"], "{}", all[0].0);
    assert_eq!(fields(&all[1].0), ["outcome"], "{}", all[1].0);
    assert_eq!(fields(&all[2].0), ["progress"], "{}", all[2].0);
    let rounds: Vec<Option<i64>> = all.iter().map(|l| l.0["round"].as_i64()).collect();
    assert!(rounds[0] < rounds[1] && rounds[1] < rounds[2], "{rounds:?}");
    assert!(all.iter().all(|l| l.0["work"] == json!(work) && !l.1), "{all:?}");
    // The work is waited for while it is paused for them.
    assert_eq!(waited_for_by(&e), ["agent", "agent", "agent"], "{log}");
    // Once it goes on, the reports are the architect's to look at when it can, and nothing waits for them. The owner's
    // Start asks for it, and the engine's next turn lifts the pause, as its machine has a slot free.
    api::resume(&mut e.db, work).unwrap();
    let on = turns_until(&mut e, &held, |e| paused_by(e, work).is_none());
    let log = said.lock().unwrap().join("\n");
    assert!(on, "{log}");
    assert_eq!(waited_for_by(&e), ["none", "none", "none"], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architect's `resume_work`, handed in at the round on the report of a return sent back, lets the work go on that
// three returns in a row paused: the owner does not press Start, and the cell runs again in the same session and its
// return is taken. The pause is the architect's to lift and the owner's too.
#[test]
fn the_architect_lets_a_work_go_on_that_three_returns_sent_back_paused() {
    let s = setup("look-go-on");
    let mut agents = returns_sent_back(vec![both_wrong(), outcome_wrong(), progress_wrong()]);
    // The first report the architect looks at lets the work go on; the work is no longer paused for the others.
    agents.insert(
        "H1",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let go = json!({ "kind": "resume_work", "item": 1, "reason": "The return was fixed by asking again." });
            (Vec::new(), vec![("outcome.effects", json!({ "actions": [go] }))])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let work = e.step(None).unwrap().work.unwrap();
    let held = Go {
        pause: held_but(&[work], &[]),
        ..Go::default()
    };
    let paused = turns_until(&mut e, &held, |e| paused_by(e, work).is_some());
    let log = said.lock().unwrap().join("\n");
    assert!(paused, "{log}");
    // The architect is let go: its work looks at the reports, and the first of its rounds on one lets the work go on.
    let on = turns_until(&mut e, &Go::default(), |e| {
        paused_by(e, work).is_none() && state(e, item).is_some_and(|w| w.1 != "R1")
    });
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(on, "{log}");
    // The pause was lifted once, by the architect, never by the owner.
    assert_eq!(lifted_by(&e, work), ["architect"], "{log}");
    // The cell ran again, and its fourth return was taken.
    let rows: Vec<Option<String>> = rows_taken(&e, work)
        .into_iter()
        .filter(|(cell, _)| cell == "R1")
        .map(|(_, row)| row)
        .collect();
    assert_eq!(rows.len(), 4, "{rows:?}\n{log}");
    assert_ne!(rows[3].as_deref(), Some("sent_back"), "{rows:?}\n{log}");
    // The architect's round on it was one on a report of this kind.
    let handled = number(
        &e,
        "SELECT count(*) FROM work_handled WHERE kind = 'report' AND detail = 'sent_back'",
    );
    assert!(handled >= 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work of the architect's has no architect above it: its returns sent back queue no report, and three in a row pause
// it for the owner alone, with a line for each problem.
#[test]
fn the_architect_s_own_work_is_paused_for_the_owner_after_three_returns_sent_back_with_each_problem_in_words() {
    let s = setup("look-architect-own");
    let mut agents = scripts();
    // Every return of the cell that handles a matter names an item with no work, which the check of its effects refuses.
    agents.insert(
        "H1",
        Box::new(|_: &Request, _: usize| {
            let over = json!({ "kind": "start_work_over", "item": 2, "reason": "Its design went the wrong way." });
            (Vec::new(), vec![("outcome.effects", json!({ "actions": [over] }))])
        }),
    );
    let (mut e, said, _item) = start(&s, agents);
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
        why.starts_with("3 returns in a row were sent back, the last at cell H1, for:\n"),
        "{why}\n{log}"
    );
    assert!(why.contains("action 1: item 2 has no work running"), "{why}");
    assert!(!why.contains('{') && !why.contains("problems"), "{why}");
    // The reason does not start as the one the architect may lift, so the pause stays the owner's.
    assert!(!why.starts_with("returns were sent back"), "{why}");
    assert_eq!(paused_by(&e, arch).as_deref(), Some("engine"), "{log}");
    assert!(looks(&e).is_empty(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
