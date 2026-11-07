//! What the hook after a tool use does with the interjections the round's message gave, as the owner's page and the
//! next round see it: given once, recorded as delivered in the round, and waiting again when the session ends.

use super::*;

/// The research of the default scripts, then two interjections for the engineer line, said before its first round: a
/// question of the owner and a word of the architect that needs no answer (`informs`).
fn research_then_said(data: &Path, scripts: &mut BTreeMap<&'static str, Script>) {
    let data = data.to_path_buf();
    let research = scripts.remove("R1").unwrap();
    scripts.insert(
        "R1",
        Box::new(move |r: &Request, n: usize| {
            let mut db = engine::database::open(&data).unwrap();
            db.write(Actor::Owner, |tx| {
                db::say_to_line(
                    tx,
                    &db::ToLine::new(db::Sender::Owner, r.work, "engineer", "Why this size?"),
                    None,
                )?;
                Ok(())
            })
            .unwrap();
            db.write(Actor::Architect, |tx| {
                db::say_to_line(
                    tx,
                    &db::ToLine {
                        informs: true,
                        ..db::ToLine::new(
                            db::Sender::Architect,
                            r.work,
                            "engineer",
                            "Your question became an item.",
                        )
                    },
                    None,
                )?;
                Ok(())
            })
            .unwrap();
            research(r, n)
        }),
    );
}

/// What the owner's interjection `text` is recorded as delivered in.
fn delivered_in(e: &Engine, text: &str) -> Option<i64> {
    e.db.conn()
        .query_row("SELECT taken_round FROM line_sayings WHERE text = ?1", [text], |r| {
            r.get(0)
        })
        .unwrap()
}

// The words the round's message gave are taken up from the first moment of the round, whichever kind they are: once
// the hook has run, the owner's page (which shows the words of an interjection not delivered in any round as not taken
// up yet) has nothing to show as waiting, though the round has not ended and the hook handed in nothing.
#[test]
fn what_the_message_gave_is_recorded_as_delivered_while_the_round_runs() {
    let s = setup("given-taken-up");
    let mut scripts = scripts();
    research_then_said(&s.data, &mut scripts);
    // What the database says in the middle of the round, after the hook: the output, the round, and how many
    // interjections still wait for the line and what each is recorded as delivered in.
    type Middle = (Option<String>, i64, i64, Vec<Option<i64>>);
    let middle: Arc<Mutex<Option<Middle>>> = Arc::default();
    let kept = middle.clone();
    let data = s.data.clone();
    scripts.insert(
        "I1",
        Box::new(move |r: &Request, _: usize| {
            let mut db = engine::database::open(&data).unwrap();
            let n: i64 = db
                .conn()
                .query_row("SELECT max(n) FROM rounds WHERE work = ?1", [r.work], |x| x.get(0))
                .unwrap();
            let out = sessions::interject::deliver(&mut db, r.work, n, "engineer").unwrap();
            let waiting: i64 = db
                .conn()
                .query_row(
                    "SELECT count(*) FROM waiting_interjections WHERE work = ?1",
                    [r.work],
                    |x| x.get(0),
                )
                .unwrap();
            let mut st = db
                .conn()
                .prepare("SELECT taken_round FROM line_sayings WHERE work = ?1 ORDER BY id")
                .unwrap();
            let recorded = st
                .query_map([r.work], |x| x.get::<_, Option<i64>>(0))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            *kept.lock().unwrap() = Some((out, n, waiting, recorded));
            (
                vec![("outcome", "answers")],
                vec![(
                    "outcome.answers",
                    json!([{ "to": "owner", "text": "Because the log says so." }]),
                )],
            )
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 2);
    let log = said.lock().unwrap().join("\n");
    let (out, round, waiting, recorded) = middle
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| panic!("the round at I1 did not run:\n{log}"));
    assert_eq!(out, None, "{log}");
    assert_eq!((waiting, recorded), (0, vec![Some(round); 2]), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// What the hook handed in during a round that the run guard stopped, and what the message of that round gave, wait
// again together, and the cell run again in a new session is given all of them by its whole message and records each as
// delivered in it.
#[test]
fn what_the_message_and_the_hook_gave_a_stopped_round_reaches_the_round_run_again() {
    let s = setup("given-stopped");
    let mut scripts = scripts();
    research_then_said(&s.data, &mut scripts);
    let data = s.data.clone();
    scripts.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            if n > 1 {
                return (
                    vec![("outcome", "answers")],
                    vec![(
                        "outcome.answers",
                        json!([{ "to": "owner", "text": "Because the log says so." }]),
                    )],
                );
            }
            // The first run says a word more while it runs, takes it from the hook, and then grows past the guard's
            // lines.
            let mut db = engine::database::open(&data).unwrap();
            db.write(Actor::Owner, |tx| {
                db::say_to_line(
                    tx,
                    &db::ToLine::new(db::Sender::Owner, r.work, "engineer", "Look at the log."),
                    None,
                )?;
                Ok(())
            })
            .unwrap();
            let round: i64 = db
                .conn()
                .query_row("SELECT max(n) FROM rounds WHERE work = ?1", [r.work], |x| x.get(0))
                .unwrap();
            sessions::interject::deliver(&mut db, r.work, round, "engineer")
                .unwrap()
                .unwrap();
            let calls: Vec<Value> = [10_000, 30_000]
                .iter()
                .enumerate()
                .map(|(i, p)| json!({ "call": format!("m{i}"), "prompt": p }))
                .collect();
            (Vec::new(), vec![("@stream", Value::Array(calls))])
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    history(&mut e, "I1", 19, &|i| (i * 100, 11, i * 100, 1000));
    for _ in 0..12 {
        if agent_rounds(&e, item).iter().filter(|r| r.1 == "I1").count() == 2 {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(failed_kinds(&e), ["guarded"], "{log}");
    let (work, _, _) = state(&e, item).unwrap();
    let again = input_at(&e, work, "I1", true);
    let told = again["message"].as_str().unwrap_or_default();
    for text in ["Why this size?", "Your question became an item.", "Look at the log."] {
        assert!(told.contains(text), "{text} is not in\n{told}");
    }
    let round: i64 = number(
        &e,
        &format!("SELECT max(n) FROM rounds WHERE work = {work} AND cell = 'I1'"),
    );
    let all = [
        delivered_in(&e, "Why this size?"),
        delivered_in(&e, "Your question became an item."),
        delivered_in(&e, "Look at the log."),
    ];
    assert_eq!(all, [Some(round); 3], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
