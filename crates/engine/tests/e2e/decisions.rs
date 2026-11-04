//! Decisions: designs that depart from their item's, the rulings they wait for, and the decisions the engine
//! writes in items from the records.

use super::*;

/// An item's approach as it stands.
fn approach_of(e: &Engine, item: i64) -> String {
    e.db.conn()
        .query_row("SELECT coalesce(approach, '') FROM items WHERE id = ?1", [item], |r| {
            r.get(0)
        })
        .unwrap()
}

/// A decision as the cell that records decisions hands it in: what it settles, and the words it rests on named by their
/// record and the part that decides. Without an item it is one no item holds, with the title and tags of the observation
/// it is kept as.
fn decision_of(item: Option<&str>, what: &str, quotes: &[(i64, &str)]) -> Value {
    let mut d = json!({
        "what": what,
        "situation": "The parser's item is being written",
        "reason": "It keeps the item small",
        "reason_by": "said",
        "replaces": "none",
        "words": quotes
            .iter()
            .map(|(m, q)| json!({ "from": "matter", "id": m, "quote": q }))
            .collect::<Vec<_>>(),
    });
    match item {
        Some(i) => d["item"] = json!(i),
        None => {
            d["title"] = json!("Name stays");
            d["tags"] = json!([]);
        }
    }
    d
}

/// The number of the matter that holds the owner's words with `part` in them, read from the database at `path`.
fn matter_with(path: &Path, part: &str) -> i64 {
    let c = db::open_read(path).unwrap();
    c.query_row(
        "SELECT id FROM architect_matters WHERE kind = 'owner_words' AND json_extract(body, '$.text') LIKE ?1",
        [format!("%{part}%")],
        |r| r.get(0),
    )
    .unwrap()
}

/// The day a matter was queued, `YYYY-MM-DD`, as SQLite tells it from the matter's time.
fn day_of(e: &Engine, matter: i64) -> String {
    e.db.conn()
        .query_row(
            "SELECT date(at / 1000, 'unixepoch') FROM architect_matters WHERE id = ?1",
            [matter],
            |r| r.get(0),
        )
        .unwrap()
}

/// One line of a decision about the parser's item as the engine writes it, from the parts the test hands in.
fn decision_line(date: &str, who: &str, what: &str, words: &str) -> String {
    format!(
        "Decision ({date}, {who}): {what}. Situation: The parser's item is being written. Reason: It keeps the \
         item small. Replaces: none. Words: {words}"
    )
}

// The decisions the owner's words of a batch make are handed in by the cell that records them, as the records they quote
// and the part that decides, and are reviewed with the batch's other changes: the review is given the words whole and
// the line the engine made, the review drops one and the others are written under the head `Decisions:` of the item,
// after the item's text the batch changed and before `Learned:`, as the engine makes them (the date, who decided and the
// words from the records, not from what the cell typed), in the order handed in. Words said by the owner are the owner's,
// those of a delegate its own, and a decision on several records takes the least certain. One that names no item is kept
// as an observation, whose text is the line. The change the batch made to the item's text first is not written over.
#[test]
fn the_owner_s_decisions_are_written_by_the_engine_from_the_records_once_the_review_finds_them_sound() {
    let s = setup("decisions-written");
    let path = config::database(&s.data);
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let edit = json!({ "kind": "edit_item", "item": "1", "part": "approach",
                               "text": "Approach:\n- Parse it first.\n\nLearned:\nFact: parse first",
                               "reason": "Say it first." });
            (
                vec![("outcome.answer", "done")],
                vec![
                    ("outcome.answer.reply", json!("Noted.")),
                    ("outcome.effects", json!({ "item_changes": [edit] })),
                ],
            )
        }),
    );
    let at = path.clone();
    agents.insert(
        "B5",
        Box::new(move |_: &Request, _: usize| {
            let (a, b) = (matter_with(&at, "integers"), matter_with(&at, "old name"));
            let looked: Vec<Value> = [a, b]
                .iter()
                .map(|m| json!({ "matter": m, "verdict": "decision", "why": "It settles a choice." }))
                .collect();
            let decisions = json!([
                decision_of(
                    Some("1"),
                    "Integers come first",
                    &[(a, "the parser takes integers first")]
                ),
                decision_of(Some("1"), "The old name stays", &[(b, "keep the old name")]),
                decision_of(
                    Some("1"),
                    "No browser check at all",
                    &[(a, "No browser check"), (b, "Fine")]
                ),
                decision_of(None, "The name stays for good", &[(b, "Fine, keep")]),
            ]);
            (
                Vec::new(),
                vec![("outcome.looked", json!(looked)), ("outcome.decisions", decisions)],
            )
        }),
    );
    agents.insert(
        "B2",
        Box::new(|_: &Request, _: usize| {
            let row = |n: i64, v: &str| json!({ "change": n, "verdict": v, "why": "Judged against the words." });
            (
                Vec::new(),
                vec![(
                    "outcome.review",
                    json!([row(1, "ok"), row(2, "ok"), row(3, "drop"), row(4, "ok"), row(5, "ok")]),
                )],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE items SET approach = 'Approach:\n- Parse it.' WHERE id = 1", [])?;
        Ok(())
    })
    .unwrap();
    db::say_to_architect(
        &mut e.db,
        "p",
        None,
        None,
        "Yes, the parser takes integers first. No browser check.",
        &[],
        db::Speaker::Owner,
    )
    .unwrap();
    db::say_to_architect(
        &mut e.db,
        "p",
        None,
        None,
        "Fine, keep the old name.",
        &[],
        db::Speaker::Delegate,
    )
    .unwrap();
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let path_taken = path_of(&e, arch);
    let at = path_taken.iter().position(|c| c == "B5").expect(&log);
    assert_eq!(&path_taken[at..at + 3], ["B5", "B2", "B4"], "{log}");
    assert_eq!(rows_at(&e, arch, "B5"), ["decisions"], "{log}");
    let (a, b) = (matter_with(&path, "integers"), matter_with(&path, "old name"));
    let (da, dayb) = (day_of(&e, a), day_of(&e, b));
    let latest = std::cmp::max(da.clone(), dayb.clone());
    let first = decision_line(
        &da,
        "owner",
        "Integers come first",
        &format!("\"the parser takes integers first\" (matter {a}, {da})"),
    );
    let third = decision_line(
        &latest,
        "delegate",
        "No browser check at all",
        // The head is the delegate's, so the owner's own words say that they are the owner's.
        &format!("\"No browser check\" (matter {a}, {da}, owner); \"Fine\" (matter {b}, {dayb})"),
    );
    let approach: String =
        e.db.conn()
            .query_row("SELECT approach FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    assert_eq!(
        approach,
        format!("Approach:\n- Parse it first.\nDecisions:\n{first}\n{third}\n\nLearned:\nFact: parse first"),
        "{log}"
    );
    let states: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT state FROM architect_item_changes ORDER BY id",
        &[],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| r["state"].as_str().unwrap_or_default().to_string())
    .collect();
    assert_eq!(states, ["made", "made", "dropped", "made", "made"], "{log}");
    // The one that names no item is an observation, its text the line.
    let (title, what): (String, String) =
        e.db.conn()
            .query_row("SELECT title, what FROM observations", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!(title, "Name stays");
    assert_eq!(
        what,
        decision_line(
            &dayb,
            "delegate",
            "The name stays for good",
            &format!("\"Fine, keep\" (matter {b}, {dayb})")
        ),
        "{log}"
    );
    // The review was given the owner's words whole and the line to be written.
    let review = message_at(&e, arch, "B2", false);
    assert!(
        review.contains("Yes, the parser takes integers first. No browser check."),
        "{review}"
    );
    assert!(review.contains(&format!("(matter {a}, {da})")), "{review}");
    assert!(review.contains("delegate"), "{review}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A return of the cell that records decisions is sent back for each of these: a row of `looked` missing for some of the
// owner's words and another twice; a quotation that is not the record's letter for letter (the record's words come with
// the reason), words of another project's record, and a row that says decision with no decision quoting its words. Handed
// in sound, the decision goes to the review, which asks for a fix; the revision hands the decision in again, and the line
// written is the revised one, once, with the words from the record.
#[test]
fn a_decision_the_records_do_not_bear_out_is_sent_back_and_a_revised_one_is_written_once() {
    let s = setup("decisions-sent-back");
    let path = config::database(&s.data);
    let mut agents = scripts();
    let foreign = Arc::new(AtomicI64::new(0));
    let (at, other) = (path.clone(), foreign.clone());
    agents.insert(
        "B5",
        Box::new(move |_: &Request, n: usize| {
            let (a, b) = (matter_with(&at, "integers"), matter_with(&at, "old name"));
            let row = |m: i64, v: &str| json!({ "matter": m, "verdict": v, "why": "It is read." });
            match n {
                // One row for the words of `a` twice and none for those of `b`.
                1 => (
                    Vec::new(),
                    vec![("outcome.looked", json!([row(a, "nothing"), row(a, "nothing")]))],
                ),
                // A quotation in other letters, a record of another project, and a row for `b` no decision answers.
                2 => {
                    let wrong = decision_of(Some("1"), "Integers come first", &[(a, "THE PARSER TAKES")]);
                    let elsewhere = decision_of(
                        Some("1"),
                        "Elsewhere",
                        &[(a, "integers"), (other.load(Ordering::Relaxed), "Elsewhere.")],
                    );
                    (
                        Vec::new(),
                        vec![
                            ("outcome.looked", json!([row(a, "decision"), row(b, "decision")])),
                            ("outcome.decisions", json!([wrong, elsewhere])),
                        ],
                    )
                }
                _ => {
                    let sound = decision_of(
                        Some("1"),
                        "Integers come first",
                        &[(a, "the parser takes integers first")],
                    );
                    (
                        Vec::new(),
                        vec![
                            ("outcome.looked", json!([row(a, "decision"), row(b, "nothing")])),
                            ("outcome.decisions", json!([sound])),
                        ],
                    )
                }
            }
        }),
    );
    agents.insert(
        "B2",
        Box::new(|_: &Request, n: usize| {
            let row = if n == 1 {
                json!({ "change": 1, "verdict": "fix", "about": "constitution_and_policy",
                        "what": "It says what was done, not what is settled.", "how": "Say what stands.",
                        "cause": "rule_missing" })
            } else {
                json!({ "change": 1, "verdict": "ok", "why": "Now it says what stands." })
            };
            (Vec::new(), vec![("outcome.review", json!([row]))])
        }),
    );
    let at = path.clone();
    agents.insert(
        "B3",
        Box::new(move |_: &Request, _: usize| {
            let a = matter_with(&at, "integers");
            let revised = decision_of(
                Some("1"),
                "Integers come first, always",
                &[(a, "the parser takes integers first")],
            );
            (
                Vec::new(),
                vec![("outcome.changes", json!([{ "matter": a, "decision": revised }]))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE items SET approach = 'Approach:\n- Parse it.' WHERE id = 1", [])?;
        Ok(())
    })
    .unwrap();
    // Another project's words, handled already, which no batch of p's holds.
    let elsewhere =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO architect_matters (project, kind, detail, what, body, at, handled_work, handled_round)
                 VALUES ('q', 'owner_words', 'said', 'w',
                         '{\"text\":\"Elsewhere.\",\"where\":\"page\",\"speaker\":\"owner\"}', 1,
                         (SELECT id FROM works WHERE ended = 'over before the test'), 1)",
                [],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    foreign.store(elsewhere, Ordering::Relaxed);
    say(&mut e, "Yes, the parser takes integers first. No browser check.");
    say(&mut e, "Fine, keep the old name.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let sent_back = why_sent_back(&e, arch, "B5");
    assert_eq!(sent_back.len(), 2, "{sent_back:?}\n{log}");
    let (a, b) = (matter_with(&path, "integers"), matter_with(&path, "old name"));
    // A row twice and a row missing are both said, by their matters.
    for part in [
        format!("matter {a} has more than one row"),
        format!("matter {b}, which this batch handled, has no row"),
    ] {
        assert!(sent_back[0].contains(&part), "{part}\n{sent_back:?}");
    }
    for part in [
        "is not found as it is in the words of matter".to_string(),
        "Yes, the parser takes integers first. No browser check.".to_string(),
        format!("matter {elsewhere} is of q, and the decision is of p"),
        format!("matter {b}: its row says it holds a decision, and no decision quotes it"),
    ] {
        assert!(sent_back[1].contains(&part), "{part}\n{sent_back:?}");
    }
    let path_taken = path_of(&e, arch);
    let at = path_taken.iter().rposition(|c| c == "B5").expect(&log);
    assert_eq!(&path_taken[at..at + 5], ["B5", "B2", "B3", "B2", "B4"], "{log}");
    let da = day_of(&e, a);
    let approach: String =
        e.db.conn()
            .query_row("SELECT approach FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    assert_eq!(
        approach,
        format!(
            "Approach:\n- Parse it.\nDecisions:\n{}",
            decision_line(
                &da,
                "owner",
                "Integers come first, always",
                &format!("\"the parser takes integers first\" (matter {a}, {da})")
            )
        ),
        "{log}"
    );
    let states: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT state FROM architect_item_changes ORDER BY id",
        &[],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| r["state"].as_str().unwrap_or_default().to_string())
    .collect();
    assert_eq!(states, ["revised", "made"], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A return of the cell that records decisions that is wrong in two of its fields is sent back once, saying both: a row
// of `looked` missing for some of the owner's words, and a quotation that is not the record's letter for letter in
// `decisions`. Oracle: the project guide's rule that a check of a return reports every problem it can find in it at
// once, because each send-back costs a round.
#[test]
fn a_return_that_is_wrong_in_looked_and_in_decisions_is_sent_back_once_saying_both() {
    let s = setup("decisions-both-wrong");
    let path = config::database(&s.data);
    let mut agents = scripts();
    let at = path.clone();
    agents.insert(
        "B5",
        Box::new(move |_: &Request, n: usize| {
            let (a, b) = (matter_with(&at, "integers"), matter_with(&at, "old name"));
            let row = |m: i64, v: &str| json!({ "matter": m, "verdict": v, "why": "It is read." });
            if n == 1 {
                // No row for the words of `b`, and a quotation in other letters.
                let wrong = decision_of(Some("1"), "Integers come first", &[(a, "THE PARSER TAKES")]);
                return (
                    Vec::new(),
                    vec![
                        ("outcome.looked", json!([row(a, "decision")])),
                        ("outcome.decisions", json!([wrong])),
                    ],
                );
            }
            let sound = decision_of(
                Some("1"),
                "Integers come first",
                &[(a, "the parser takes integers first")],
            );
            (
                Vec::new(),
                vec![
                    ("outcome.looked", json!([row(a, "decision"), row(b, "nothing")])),
                    ("outcome.decisions", json!([sound])),
                ],
            )
        }),
    );
    agents.insert("B2", review_all_sound(path.clone()));
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE items SET approach = 'Approach:\n- Parse it.' WHERE id = 1", [])?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Yes, the parser takes integers first.");
    say(&mut e, "Fine, keep the old name.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let sent_back = why_sent_back(&e, arch, "B5");
    assert_eq!(sent_back.len(), 1, "{sent_back:?}\n{log}");
    let b = matter_with(&path, "old name");
    for part in [
        format!("matter {b}, which this batch handled, has no row"),
        "is not found as it is in the words of matter".to_string(),
    ] {
        assert!(sent_back[0].contains(&part), "{part}\n{sent_back:?}");
    }
    let approach: String =
        e.db.conn()
            .query_row("SELECT approach FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    assert!(approach.contains("Decisions:\nDecision ("), "{approach}\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The check that the rows of `looked` cover the owner's words of the batch says all of its problems at once: a matter
// that is none of them, one that has no row, and a row that says it makes a suggestion when the return holds no new
// observation to record it. Oracle: the project guide's rule that a check of a return reports every problem it can
// find in it at once.
#[test]
fn rows_that_do_not_cover_the_words_and_a_suggestion_nothing_records_are_sent_back_together() {
    let s = setup("looked-three-problems");
    let path = config::database(&s.data);
    let mut agents = scripts();
    let at = path.clone();
    agents.insert(
        "B5",
        Box::new(move |_: &Request, n: usize| {
            let (a, b) = (matter_with(&at, "integers"), matter_with(&at, "old name"));
            let row = |m: i64, v: &str| json!({ "matter": m, "verdict": v, "why": "It is read." });
            let rows = if n == 1 {
                // A matter of no words of the batch, none for `b`, and a suggestion with no observation.
                json!([row(a, "suggestion"), row(987_654, "nothing")])
            } else {
                json!([row(a, "nothing"), row(b, "nothing")])
            };
            (Vec::new(), vec![("outcome.looked", rows)])
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Yes, the parser takes integers first.");
    say(&mut e, "Fine, keep the old name.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let sent_back = why_sent_back(&e, arch, "B5");
    assert_eq!(sent_back.len(), 1, "{sent_back:?}\n{log}");
    let b = matter_with(&path, "old name");
    for part in [
        "matter 987654 is none of the owner's words this batch handled".to_string(),
        format!("matter {b}, which this batch handled, has no row"),
        "a row says it makes a suggestion, and observation_changes holds no new observation".to_string(),
    ] {
        assert!(sent_back[0].contains(&part), "{part}\n{sent_back:?}");
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// The rows of `looked` that name a matter twice and name matters that are none of the batch's are sent back once, each
// kind said with all its matters: two unknown matters in one sentence, the one given twice, and the one with no row.
// Oracle: the project guide's rule that several ids compared with a list the engine gave go through `id_misses`, which
// says each kind once with all its ids, and that a check says every problem at once.
#[test]
fn rows_that_name_a_matter_twice_and_two_that_are_not_the_batch_s_are_sent_back_with_each_kind_said_once() {
    let s = setup("looked-kinds");
    let path = config::database(&s.data);
    let mut agents = scripts();
    let at = path.clone();
    agents.insert(
        "B5",
        Box::new(move |_: &Request, n: usize| {
            let (a, b) = (matter_with(&at, "integers"), matter_with(&at, "old name"));
            let row = |m: i64, v: &str| json!({ "matter": m, "verdict": v, "why": "It is read." });
            let rows = if n == 1 {
                json!([
                    row(a, "nothing"),
                    row(a, "nothing"),
                    row(987_654, "nothing"),
                    row(987_655, "nothing")
                ])
            } else {
                json!([row(a, "nothing"), row(b, "nothing")])
            };
            (Vec::new(), vec![("outcome.looked", rows)])
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Yes, the parser takes integers first.");
    say(&mut e, "Fine, keep the old name.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let sent_back = why_sent_back(&e, arch, "B5");
    assert_eq!(sent_back.len(), 1, "{sent_back:?}\n{log}");
    let (a, b) = (matter_with(&path, "integers"), matter_with(&path, "old name"));
    for part in [
        "matters 987654, 987655 are none of the owner's words this batch handled".to_string(),
        format!("matter {a} has more than one row: one row for each"),
        format!("matter {b}, which this batch handled, has no row"),
    ] {
        assert!(sent_back[0].contains(&part), "{part}\n{sent_back:?}");
    }
    // The matter given twice is not also said to be none of the batch's, and no suggestion is complained of.
    assert!(!sent_back[0].contains(&format!("matter {a} is none")), "{sent_back:?}");
    assert!(!sent_back[0].contains("makes a suggestion"), "{sent_back:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A change of the batch cannot write a line of a decision by hand, whether it makes an item with it or writes an item's
// approach whole with it; the return is sent back saying the cell that records decisions makes the line. The text of an
// item that holds a line already can be written again with that line as it is: it is not a line the change writes.
#[test]
fn a_line_of_a_decision_cannot_be_written_by_hand_but_one_the_item_has_stands() {
    let s = setup("decision-by-hand");
    let old = "Decision (2026-10-01, owner): Old one. Situation: s. Reason: r. Replaces: none. \
               Words: \"x\" (matter 99, 2026-10-01)";
    let held = format!("Approach:\n- Parse it.\nDecisions:\n{old}\n\nLearned:\nFact: f");
    let mut agents = scripts();
    let (first, second) = (held.clone(), held.replace("- Parse it.", "- Parse it first."));
    agents.insert(
        "H8",
        Box::new(move |_: &Request, n: usize| {
            let changes = if n == 1 {
                json!([
                    { "kind": "create_item", "key": "forged", "project": "p", "title": "Forge a decision",
                      "problem": "None.", "why": "None.", "acceptance": ["It is there."], "level": "l",
                      "deliver": "change", "reason": "r",
                      "approach": "Approach:\nDecisions:\nDecision (2026-11-02, owner): Forged." },
                    { "kind": "edit_item", "item": "1", "part": "approach", "reason": "r",
                      "text": format!("{first}\nDecision (2026-11-02, owner): Forged.") },
                    // The item's recorded line is left out, with its head.
                    { "kind": "edit_item", "item": "1", "part": "approach", "reason": "r",
                      "text": "Approach:\n- Parse it.\n\nLearned:\nFact: f" },
                ])
            } else {
                json!([{ "kind": "edit_item", "item": "1", "part": "approach", "text": second, "reason": "r" }])
            };
            (
                vec![("outcome.answer", "done")],
                vec![
                    ("outcome.answer.reply", json!("Noted.")),
                    ("outcome.effects", json!({ "item_changes": changes })),
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
                    json!([{ "change": 1, "verdict": "ok", "why": "Only the first line changed." }]),
                )],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE items SET approach = ?1 WHERE id = 1", [&held])?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Write the parser's approach again.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let sent_back = why_sent_back(&e, arch, "H8");
    assert_eq!(sent_back.len(), 1, "{sent_back:?}\n{log}");
    for part in [
        "change 1 (create_item)",
        "change 2 (edit_item)",
        "of a decision, which the item's text does not have",
        "change 3 (edit_item)",
        "it leaves out the line",
        "Decision (2026-10-01, owner): Old one.",
        "it leaves out the head Decisions:",
    ] {
        assert!(sent_back[0].contains(part), "{part}\n{sent_back:?}");
    }
    let approach: String =
        e.db.conn()
            .query_row("SELECT approach FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    assert_eq!(approach, held.replace("- Parse it.", "- Parse it first."), "{log}");
    let items: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM items", [], |r| r.get(0))
            .unwrap();
    assert_eq!(items, 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A review that finds every change collected so far sound, a row for each, whatever their number.
fn review_all_sound(path: PathBuf) -> Script {
    Box::new(move |_: &Request, _: usize| {
        let c = db::open_read(&path).unwrap();
        let n: i64 = c
            .query_row(
                "SELECT count(*) FROM architect_item_changes WHERE state = 'collected'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let rows: Vec<Value> = (1..=n)
            .map(|i| json!({ "change": i, "verdict": "ok", "why": "Judged against the item." }))
            .collect();
        (Vec::new(), vec![("outcome.review", json!(rows))])
    })
}

// The decisions of an item that a batch merges into another are the kept item's from then on, whatever else the batch
// does to the kept item's `approach`. A batch that merges item 2 into item 1 and then writes item 1's approach whole,
// from the text item 1 had before the merge, must not lose the lines the merge carried: either the return is sent back
// or the line stands in item 1. Oracle: the acceptance rule (A4) that a recorded line is never lost or changed but by
// the engine's own writes, and the merge's rule that the absorbed item's lines go to the kept item.
#[test]
fn a_text_that_replaces_the_approach_of_the_item_kept_in_a_merge_keeps_the_decisions_the_merge_carried() {
    let s = setup("merge-then-edit");
    let path = config::database(&s.data);
    let old = "Decision (2026-10-01, owner): Old one. Situation: s. Reason: r. Replaces: none. \
               Words: \"x\" (matter 99, 2026-10-01)";
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            let merge = json!({ "kind": "merge", "keep": "1", "absorbed": ["2"], "reason": "The same work." });
            let edit = json!({ "kind": "edit_item", "item": "1", "part": "approach", "reason": "Say it first.",
                               "text": "Approach:\n- Parse it first." });
            let changes = match n {
                1 => json!([merge, edit]),
                2 => json!([merge]),
                _ => return (Vec::new(), Vec::new()),
            };
            (
                vec![("outcome.answer", "done")],
                vec![
                    ("outcome.answer.reply", json!("Noted.")),
                    ("outcome.effects", json!({ "item_changes": changes })),
                ],
            )
        }),
    );
    agents.insert("B2", review_all_sound(path.clone()));
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE items SET approach = 'Approach:\n- Parse it.' WHERE id = 1", [])?;
        tx.execute(
            "INSERT INTO items (project, title, problem, why, approach, deliver, level)
             VALUES ('p', 'Parse more', 'x', 'y', ?1, 'change', 'l')",
            [format!("Approach:\n- Other.\nDecisions:\n{old}")],
        )?;
        Ok(())
    })
    .unwrap();
    say(
        &mut e,
        "Merge the second item into the first, and write the first again.",
    );
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let approach: String =
        e.db.conn()
            .query_row("SELECT approach FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    // The line is in the kept item, whether the batch was sent back or not.
    assert!(
        approach.lines().any(|l| l == old),
        "the merge carried the line, and the text written over it lost it:\n{approach}\n{log}"
    );
    let cancelled: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM items WHERE id = 2 AND cancelled IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(cancelled, 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A decision the owner's words make is not left only in an item the same batch merges away: the batch merges item 2
// into item 1 and a decision is recorded in item 2 after it. The decision's line must be where a later agent reads it,
// in the item kept, or the return that records it is sent back. In the same batch a decision that names the key of an
// item the batch creates is written into that item, under its head and before `Learned:`. Oracle: the acceptance rule
// (A4) that the line is added under `Decisions:` of the item it is recorded for, which a cancelled item's reader never
// reads.
#[test]
fn a_decision_is_not_left_only_in_an_item_the_batch_merged_away_and_one_on_a_created_key_is_written() {
    let s = setup("decision-merged-away");
    let path = config::database(&s.data);
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let create = json!({ "kind": "create_item", "key": "lex", "project": "p", "title": "Lex it",
                "problem": "No lexer.", "why": "Needed.", "acceptance": ["It lexes."], "level": "l",
                "deliver": "change", "reason": "Split the lexer off.",
                "approach": "Approach:\n- Lex it.\nLearned:\nFact: f" });
            let merge = json!({ "kind": "merge", "keep": "1", "absorbed": ["2"], "reason": "The same work." });
            (
                vec![("outcome.answer", "done")],
                vec![
                    ("outcome.answer.reply", json!("Noted.")),
                    ("outcome.effects", json!({ "item_changes": [create, merge] })),
                ],
            )
        }),
    );
    let at = path.clone();
    agents.insert(
        "B5",
        Box::new(move |_: &Request, n: usize| {
            let (a, b) = (matter_with(&at, "integers"), matter_with(&at, "lexer simple"));
            let looked: Vec<Value> = [a, b]
                .iter()
                .map(|m| json!({ "matter": m, "verdict": "decision", "why": "It settles a choice." }))
                .collect();
            let mut decisions = Vec::new();
            // The first time, one in the item the batch merges away; sent back, in the item it is merged into, as the
            // reason it was sent back says. Every matter the rows call a decision is quoted by one, both times.
            let item = if n == 1 { "2" } else { "1" };
            decisions.push(decision_of(
                Some(item),
                "Integers come first",
                &[(a, "the parser takes integers first")],
            ));
            decisions.push(decision_of(
                Some("lex"),
                "The lexer stays simple",
                &[(b, "Keep the lexer simple")],
            ));
            (
                Vec::new(),
                vec![
                    ("outcome.looked", json!(looked)),
                    ("outcome.decisions", json!(decisions)),
                ],
            )
        }),
    );
    agents.insert("B2", review_all_sound(path.clone()));
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE items SET approach = 'Approach:\n- Parse it.' WHERE id = 1", [])?;
        tx.execute(
            "INSERT INTO items (project, title, problem, why, approach, deliver, level)
             VALUES ('p', 'Parse more', 'x', 'y', 'Approach:\n- Other.', 'change', 'l')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Yes, the parser takes integers first.");
    say(&mut e, "Keep the lexer simple.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 100, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let approach_of = |id: i64| -> String {
        e.db.conn()
            .query_row("SELECT coalesce(approach, '') FROM items WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap()
    };
    let (kept, gone) = (approach_of(1), approach_of(2));
    let has = |t: &str| t.lines().any(|l| l.contains("\"the parser takes integers first\""));
    // The created item is item 3: its decision is under the head, before `Learned:`, and nothing else moved.
    let b = matter_with(&path, "lexer simple");
    let day = day_of(&e, b);
    let line = decision_line(
        &day,
        "owner",
        "The lexer stays simple",
        &format!("\"Keep the lexer simple\" (matter {b}, {day})"),
    );
    // Both are judged before either fails the test, so that one failure does not hide the other.
    let mut wrong: Vec<String> = Vec::new();
    if has(&gone) && !has(&kept) {
        wrong.push(format!(
            "the decision stands only in the item merged away:\nkept: {kept}\nmerged away: {gone}"
        ));
    }
    let made = approach_of(3);
    let want = format!("Approach:\n- Lex it.\nDecisions:\n{line}\nLearned:\nFact: f");
    if made != want {
        wrong.push(format!("the created item's text is\n{made}\nand should be\n{want}"));
    }
    assert!(wrong.is_empty(), "{}\n{log}", wrong.join("\n"));
    let _ = std::fs::remove_dir_all(&s.root);
}

// A delegate that marks parts of its message as the owner's own words, and a decision that quotes them, go through the
// whole path: the cell that records decisions is given the marked parts with the words, and the lines are written as the
// engine tells who said each quotation. A decision that quotes only a marked part is the owner's, relayed by the delegate
// and not checked; one that quotes a marked part and an unmarked part of the same message is the delegate's, and says
// of the marked quotation that it is relayed; one that quotes the owner's own words and a marked part is told by the
// less certain of the two, the marked part, and says of the owner's own quotation that it is the owner's. Oracle:
// acceptance A3 and A6 (the order owner, relayed, delegate, unrecorded, architect; the head is the least certain
// source; a quotation of another source says its own in its brackets; `batch_words` carries `relayed`).
#[test]
fn a_decision_quoting_what_a_delegate_marked_as_the_owner_s_is_told_as_relayed_and_not_checked() {
    let s = setup("decisions-relayed");
    let path = config::database(&s.data);
    let relayed = "owner relayed by the delegate, not checked";
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let edit = json!({ "kind": "edit_item", "item": "1", "part": "title", "text": "Parse it first",
                               "reason": "Say it first." });
            (
                vec![("outcome.answer", "done")],
                vec![
                    ("outcome.answer.reply", json!("Noted.")),
                    ("outcome.effects", json!({ "item_changes": [edit] })),
                ],
            )
        }),
    );
    let given: Arc<Mutex<Vec<String>>> = Arc::default();
    let (at, seen) = (path.clone(), given.clone());
    agents.insert(
        "B5",
        Box::new(move |r: &Request, _: usize| {
            let (a, b) = (matter_with(&at, "No browser"), matter_with(&at, "old name"));
            if let Some(words) = given_value(&r.prompt, "·engine·batch_words") {
                seen.lock().unwrap().push(words.to_string());
            }
            let looked: Vec<Value> = [a, b]
                .iter()
                .map(|m| json!({ "matter": m, "verdict": "decision", "why": "It settles a choice." }))
                .collect();
            let decisions = json!([
                decision_of(Some("1"), "The old name stays", &[(b, "Keep the old name.")]),
                decision_of(
                    Some("1"),
                    "Integers are shipped",
                    &[(b, "Use integers."), (b, "Ship it.")]
                ),
                decision_of(
                    None,
                    "No browser check, old name",
                    &[(a, "No browser check."), (b, "Keep the old name.")]
                ),
            ]);
            (
                Vec::new(),
                vec![("outcome.looked", json!(looked)), ("outcome.decisions", decisions)],
            )
        }),
    );
    agents.insert("B2", review_all_sound(path.clone()));
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE items SET approach = 'Approach:\n- Parse it.' WHERE id = 1", [])?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "No browser check.");
    let marked = db::Declared {
        speaker: db::Speaker::Delegate,
        relayed: vec!["Keep the old name.".into(), "Use integers.".into()],
    };
    db::say_to_architect(
        &mut e.db,
        "p",
        None,
        None,
        "Fine. Keep the old name. Use integers. Ship it.",
        &[],
        marked,
    )
    .unwrap();
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 100, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let (a, b) = (matter_with(&path, "No browser"), matter_with(&path, "old name"));
    let (da, db_) = (day_of(&e, a), day_of(&e, b));
    let latest = std::cmp::max(da.clone(), db_.clone());
    let first = decision_line(
        &db_,
        relayed,
        "The old name stays",
        &format!("\"Keep the old name.\" (matter {b}, {db_})"),
    );
    let second = decision_line(
        &db_,
        "delegate",
        "Integers are shipped",
        &format!("\"Use integers.\" (matter {b}, {db_}, {relayed}); \"Ship it.\" (matter {b}, {db_})"),
    );
    let third = decision_line(
        &latest,
        relayed,
        "No browser check, old name",
        &format!("\"No browser check.\" (matter {a}, {da}, owner); \"Keep the old name.\" (matter {b}, {db_})"),
    );
    let approach: String =
        e.db.conn()
            .query_row("SELECT approach FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    let what: String =
        e.db.conn()
            .query_row("SELECT what FROM observations", [], |r| r.get(0))
            .unwrap();
    let mut wrong: Vec<String> = Vec::new();
    if approach != format!("Approach:\n- Parse it.\nDecisions:\n{first}\n{second}") {
        wrong.push(format!(
            "the item's text is\n{approach}\nwith the lines\n{first}\n{second}"
        ));
    }
    if what != third {
        wrong.push(format!("the observation is\n{what}\nand should be\n{third}"));
    }
    // The cell that records decisions was given the marked parts with the words.
    let marks = r#""relayed":["Keep the old name.","Use integers."]"#;
    if !given.lock().unwrap().iter().any(|g| g.contains(marks)) {
        wrong.push(format!(
            "the words given to B5 do not carry {marks}: {:?}",
            given.lock().unwrap()
        ));
    }
    assert!(wrong.is_empty(), "{}\n{log}", wrong.join("\n"));
    let _ = std::fs::remove_dir_all(&s.root);
}
