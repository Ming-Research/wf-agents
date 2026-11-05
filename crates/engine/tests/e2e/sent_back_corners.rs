//! The corners of the pause at a return sent back that the architect looks at: a work sent back again after it went on,
//! the owner's Start and Pause beside the architect's `resume_work`. The tests run the flow as built
//! ([`sent_back_pause`]).

use super::*;

const OUTCOME: &str = "format (outcome): outcome must be an object, and is a string";
const PROGRESS: &str = "format (progress): progress must be a string, and is a number";

/// The matters to set straight that ask why a return was sent back: each with its body and whether the architect
/// handled it, oldest first.
fn looks(e: &Engine) -> Vec<(Value, bool)> {
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT body, handled_round IS NOT NULL FROM architect_matters
                 WHERE kind = 'set_straight' AND detail = 'sent_back' ORDER BY id",
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

/// The reasons a work was paused with, oldest first, and who paused it.
fn pauses(e: &Engine, work: i64) -> Vec<(String, String)> {
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
fn fields(body: &Value) -> Vec<&str> {
    body["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["field"].as_str().unwrap())
        .collect()
}

/// Scripts whose R1 hands in a return that does not fit its schema in two places the first time, in one place the
/// second time, and the return of the default script after.
fn sent_back_twice() -> BTreeMap<&'static str, Script> {
    let mut agents = scripts();
    let then = agents.remove("R1").unwrap();
    agents.insert(
        "R1",
        Box::new(move |r: &Request, n: usize| match n {
            1 => (
                Vec::new(),
                vec![("outcome", json!("not an object")), ("progress", json!(7))],
            ),
            2 => (
                Vec::new(),
                vec![
                    ("outcome", json!("still not an object")),
                    ("progress", json!("one sentence")),
                ],
            ),
            _ => then(r, n),
        }),
    );
    agents
}

// A work the architect let go on is sent back again by its next return: it pauses again at once, for the second return's
// problems alone, with a matter of its own, and the architect lets it go on again. The count of returns in a row starts
// afresh at each Start, so that the first pause does not make the second come early or late, nor the second reason
// repeat the first's lines, and every pause of the work is the one the architect may lift.
#[test]
fn a_work_sent_back_again_after_it_went_on_is_paused_again_with_a_matter_of_its_own() {
    let s = setup("look-twice");
    let mut agents = sent_back_twice();
    agents.insert(
        "H1",
        Box::new(|_: &Request, _: usize| {
            let go = json!({ "kind": "resume_work", "item": 1, "reason": "Asked again." });
            (Vec::new(), vec![("outcome.effects", json!({ "actions": [go] }))])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    sent_back_pause(&mut e, 1, true);
    let done = turns_until(&mut e, &Go::default(), |e| {
        let all = looks(e);
        state(e, item).is_some_and(|w| w.1 != "R1" && w.2.is_none()) && all.len() == 2 && all.iter().all(|l| l.1)
    });
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(done, "{log}");
    let work = state(&e, item).unwrap().0;
    // Two pauses, each lifted by the architect, never by the owner.
    assert_eq!(lifted_by(&e, work), ["architect", "architect"], "{log}");
    let paused = pauses(&e, work);
    assert_eq!(paused.len(), 2, "{paused:?}\n{log}");
    assert!(paused.iter().all(|p| p.1 == "engine"), "{paused:?}");
    for (why, by) in &paused {
        assert!(db::architect_may_resume(Some(why), Some(by)), "{why}");
    }
    // Each pause says its own return's problems alone.
    let mut first: Vec<&str> = paused[0].0.lines().skip(1).collect();
    first.sort();
    assert_eq!(first, [OUTCOME, PROGRESS], "{}", paused[0].0);
    let second: Vec<&str> = paused[1].0.lines().skip(1).collect();
    assert_eq!(
        second,
        ["format (outcome): outcome must be an object, and is a string"],
        "{}",
        paused[1].0
    );
    // Each pause has a matter of its own, with the problems of its own return and its own round.
    let all = looks(&e);
    assert_eq!(all.len(), 2, "{log}");
    let mut first_fields = fields(&all[0].0);
    first_fields.sort();
    assert_eq!(first_fields, ["outcome", "progress"], "{}", all[0].0);
    assert_eq!(fields(&all[1].0), ["outcome"], "{}", all[1].0);
    assert!(all[0].0["round"].as_i64() < all[1].0["round"].as_i64(), "{all:?}");
    assert_eq!(all[0].0["work"], all[1].0["work"]);
    // The cell ran three times: two returns sent back, and the third taken.
    let rows: Vec<Option<String>> = rows_taken(&e, work)
        .into_iter()
        .filter(|(cell, _)| cell == "R1")
        .map(|(_, row)| row)
        .collect();
    let sent: Vec<bool> = rows.iter().map(|r| r.as_deref() == Some("sent_back")).collect();
    assert_eq!(sent, [true, true, false], "{rows:?}\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's Start lifts a pause the architect was asked to look at as it lifts any other: the cell runs again in the
// same session and its return is taken, the pause is lifted by the owner and by no one else, and the matter is still
// there for the architect, who is not told as the owner's words that a work it never stopped was started. The
// architect is held here, so that the owner's Start is the one that lifts the pause.
#[test]
fn the_owner_s_start_lifts_a_pause_the_architect_was_asked_to_look_at() {
    let s = setup("look-owner-start");
    let (mut e, said, item) = start(&s, sent_back_twice());
    sent_back_pause(&mut e, 1, true);
    let work = e.step(None).unwrap().work.unwrap();
    let held = Go {
        pause: held_but(&[work], &[]),
        ..Go::default()
    };
    let paused = turns_until(&mut e, &held, |e| paused_by(e, work).is_some());
    let log = said.lock().unwrap().join("\n");
    assert!(paused, "{log}");
    assert_eq!(state(&e, item).unwrap().1, "R1", "{log}");
    assert_eq!(looks(&e).len(), 1, "{log}");
    api::resume(&mut e.db, work).unwrap();
    // The second return is sent back too, so the work pauses again at the same cell, after the owner let it go on.
    let again = turns_until(&mut e, &held, |e| looks(e).len() == 2);
    let log = said.lock().unwrap().join("\n");
    assert!(again, "{log}");
    assert_eq!(lifted_by(&e, work), ["owner"], "{log}");
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"), "{log}");
    // Both matters wait for the architect, unhandled, and none is the owner's words.
    assert!(looks(&e).iter().all(|l| !l.1), "{log}");
    assert_eq!(
        number(&e, "SELECT count(*) FROM architect_matters WHERE kind = 'owner_words'"),
        0,
        "{log}"
    );
    // The owner lifts the second pause too, and the cell's third return is taken.
    api::resume(&mut e.db, work).unwrap();
    let on = turns_until(&mut e, &held, |e| state(e, item).is_some_and(|w| w.1 != "R1"));
    let log = said.lock().unwrap().join("\n");
    assert!(on, "{log}");
    assert_eq!(lifted_by(&e, work), ["owner", "owner"], "{log}");
    assert_eq!(looks(&e).len(), 2, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architect's `resume_work` does not reach a pause the owner made, whatever the work had been sent back for: the
// check of its effects sends the round back, saying the work is not one it may let go on, and the work stays paused by
// the owner. The reference is the words of the item: the owner's pause is not the architect's to lift.
#[test]
fn the_architect_cannot_let_go_on_a_work_the_owner_paused() {
    let s = setup("look-owner-paused");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, _: usize| {
            let go = json!({ "kind": "resume_work", "item": 1, "reason": "It looks well." });
            (Vec::new(), vec![("outcome.effects", json!({ "actions": [go] }))])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let work = e.step(None).unwrap().work.unwrap();
    api::pause(&mut e.db, work, None).unwrap();
    say(&mut e, "Let the feature go on.");
    let refused = steps_until(&mut e, 60, |e| {
        number(
            e,
            "SELECT count(*) FROM rounds WHERE cell = 'H8' AND rejected LIKE '%nor for a return sent back%'",
        ) > 0
    });
    let log = said.lock().unwrap().join("\n");
    assert!(refused, "{log}");
    assert_eq!(state(&e, item).unwrap().0, work, "{log}");
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"), "{log}");
    assert!(lifted_by(&e, work).is_empty(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
