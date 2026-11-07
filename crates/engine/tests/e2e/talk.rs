//! What the architect's base, read anew, is told of the talk with the owner.

use super::*;

/// The message the first round of reading project p for its first base was sent, when the owner had said each of
/// `texts` to it, oldest first, and the project's notebook holds nothing yet. The words are no matters any longer, so
/// that no reply of the architect's comes into the talk before the reading: a planning for slots free starts the work.
fn first_understanding_after(name: &str, texts: &[String]) -> String {
    let s = setup(name);
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    for t in texts {
        say(&mut e, t);
    }
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE architect_matters SET withdrawn = 'only talk in this test' WHERE kind = 'owner_words'",
            [],
        )?;
        db::queue_matter(
            tx,
            &db::Matter {
                project: "p".into(),
                kind: "slots_free".into(),
                what: "1 slot is free, and no item of p is ready to start".into(),
                body: json!({ "free": 1, "ready": [] }),
                ..db::Matter::default()
            },
        )
        .map(|_| ())
    })
    .unwrap();
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| path_of(e, arch).iter().any(|c| c == "B1")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let u1 = input_at(&e, arch, "B1", false);
    assert_eq!(given_ending(&u1, "·engine·owner_talk"), Some(true), "{u1}");
    let m = u1["message"].as_str().unwrap().to_string();
    let _ = std::fs::remove_dir_all(&s.root);
    m
}

/// `n` characters of `fill` ending in `end`.
fn long_words(n: usize, fill: char, end: &str) -> String {
    format!("{}{end}", fill.to_string().repeat(n - end.len()))
}

/// The tail of a message, for a failure to show.
fn tail(m: &str) -> &str {
    &m[m.len().saturating_sub(2_000)..]
}

// The talk given to a new session is cut to the character: entries that together make 60,000 characters are all given,
// one character more and the earlier of them is left out whole, and an entry that is alone longer than that is given
// whole. The reference is the limit the design states (60,000 characters, whole entries, latest first, a line after
// them saying how many earlier ones there are).
#[test]
fn the_talk_given_is_cut_at_exactly_the_limit_and_never_inside_an_entry() {
    // Exactly the limit: both long entries are given, the short one before them is the one earlier entry.
    let m = first_understanding_after(
        "owner-talk-exact",
        &[
            "Alpha talk. ALPHAEND".to_string(),
            long_words(30_000, 'g', "GAMMAEND"),
            long_words(30_000, 'd', "DELTAEND"),
        ],
    );
    assert!(
        m.contains("GAMMAEND") && m.contains("DELTAEND"),
        "both fit the limit exactly: {}",
        tail(&m)
    );
    assert!(!m.contains("ALPHAEND"));
    assert!(m.contains("and 1 earlier entry before these"), "{}", tail(&m));

    // One character over: the earlier long entry is left out whole, with the short one.
    let m = first_understanding_after(
        "owner-talk-over",
        &[
            "Alpha talk. ALPHAEND".to_string(),
            long_words(30_001, 'g', "GAMMAEND"),
            long_words(30_000, 'd', "DELTAEND"),
        ],
    );
    assert!(m.contains("DELTAEND"), "{}", tail(&m));
    assert!(
        !m.contains("GAMMAEND") && !m.contains("ALPHAEND"),
        "an entry is left out whole"
    );
    assert!(m.contains("and 2 earlier entries before these"), "{}", tail(&m));

    // The very latest entry is given whole though it alone is over the limit.
    let m = first_understanding_after(
        "owner-talk-alone",
        &[
            "Alpha talk. ALPHAEND".to_string(),
            long_words(70_000, 'e', "EPSILONEND"),
        ],
    );
    assert!(m.contains("EPSILONEND"), "the latest entry is given whole");
    assert!(m.contains("and 1 earlier entry before these"), "{}", tail(&m));
    assert!(!m.contains("ALPHAEND"));
}

/// The owner says words to project p's architect with one image.
fn say_with_an_image(e: &mut Engine, text: &str) {
    say(e, text);
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO attachments (matter, position, media_type, size, hash, bytes)
             VALUES ((SELECT max(id) FROM architect_matters), 1, 'image/png', 4, 'h', zeroblob(4))",
            [],
        )?;
        Ok(())
    })
    .unwrap();
}

// Reading the project anew for a new base is told the talk of the one-to-one from the first round of the base before
// it, the session that read the project last, on: the moment itself counts, and what was said before it is not told. The
// owner's asks are told as the page shows them: an ask asked before that moment and answered at or after it with its
// question and answer, one asked at it, one asked and answered after it, one asked and answered before it not at all,
// one asked after it and not yet answered with its question; and how many images came with words or with an answer,
// not the images. The reference is the owner's page: the entries of the conversation `architect:p` from that moment.
#[test]
fn a_new_session_is_told_the_talk_from_the_start_of_the_session_before_with_its_asks_and_images() {
    let s = setup("owner-talk-window");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Alpha talk. ALPHAEND");
    let one = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, one)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    // The work's session began 100 minutes ago: its first round, the understanding's, started at `t - 30 seconds`, the
    // later rounds after it, and what was said before that earlier still. The notebook the work wrote plays no part.
    let t = db::now() - 100 * 60_000;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE rounds SET started_at = ?1 - 30000,
                    ended_at = ?1 - CASE WHEN cell = 'B1' THEN 20000 ELSE 0 END WHERE work = ?2",
            params![t, one],
        )?;
        tx.execute("UPDATE architect_matters SET at = ?1 - 100000", [t])?;
        Ok(())
    })
    .unwrap();
    let ask = |e: &mut Engine, question: &str, asked: i64, answer: Option<(&str, i64)>, images: i64| {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO owner_asks (work, round, body, answer) VALUES (?1, 1, ?2, ?3)",
                params![
                    one,
                    json!({ "question": question }).to_string(),
                    answer.map(|(text, at)| json!({ "by": "owner", "text": text, "at": at }).to_string())
                ],
            )?;
            let id = tx.last_insert_rowid();
            // An ask is asked when the engine's message of it says so.
            tx.execute(
                "INSERT INTO messages (sender, work, text, ask, at) VALUES ('engine', ?1, 'An ask.', ?3, ?2)",
                params![one, asked, id],
            )?;
            tx.execute(
                "INSERT INTO deliveries (message, recipient) VALUES (?1, 'owner')",
                [tx.last_insert_rowid()],
            )?;
            for position in 1..=images {
                tx.execute(
                    "INSERT INTO attachments (ask, position, media_type, size, hash, bytes)
                     VALUES (?1, ?2, 'image/png', 4, 'h', zeroblob(4))",
                    params![id, position],
                )?;
            }
            Ok(())
        })
        .unwrap();
    };
    // The cut is `t - 30 seconds`. Asked and answered before it: said before the session began. One answered at it, and
    // one asked at it and not answered: the moment itself counts. One asked before it and answered after it, and one
    // asked and answered after it, the answer with two images, and one asked after it and not answered.
    ask(&mut e, "QOLD?", t - 50_000, Some(("AOLD", t - 40_000)), 0);
    ask(&mut e, "QATCUT?", t - 30_000, None, 0);
    ask(&mut e, "QEDGE?", t - 31_000, Some(("AEDGE", t - 30_000)), 0);
    ask(&mut e, "QMID?", t - 15_000, Some(("AMID", t - 12_000)), 0);
    ask(&mut e, "QSTRADDLE?", t - 35_000, Some(("ASTRADDLE", t + 3_000)), 0);
    ask(&mut e, "QOPEN?", t + 5_000, None, 0);
    ask(&mut e, "QNEW?", t + 4_000, Some(("ANEW", t + 6_000)), 2);
    // The owner's words now, with one image, and a new base due.
    a_new_policy_takes_effect(&mut e);
    say_with_an_image(&mut e, "Beta talk. BETAEND");
    let two = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| path_of(e, two).iter().any(|c| c == "B1")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let sessions = architect_sessions(&e, one, two);
    assert_eq!(sessions.len(), 2, "{sessions:?}\n{log}");
    let u1 = input_at(&e, two, "B1", false);
    assert_eq!(given_ending(&u1, "·engine·owner_talk"), Some(true), "{u1}");
    let m = u1["message"].as_str().unwrap();
    for gone in ["ALPHAEND", "QOLD", "AOLD"] {
        assert!(
            !m.contains(gone),
            "{gone} was said before the session before began: {m}"
        );
    }
    for told in [
        "QATCUT",
        "QEDGE",
        "AEDGE",
        "QMID",
        "AMID",
        "QSTRADDLE",
        "ASTRADDLE",
        "QOPEN",
        "QNEW",
        "ANEW",
        "BETAEND",
    ] {
        assert!(m.contains(told), "{told} is told: {m}");
    }
    assert!(m.contains("2 images attached"), "{m}");
    assert!(m.contains("1 image attached"), "{m}");
    // In the order of the times an entry was made, an ask when it was answered if it was: the two at the cut, the one
    // answered 18 seconds after it, the one answered 33 seconds after it, the one asked at 35, the one answered at 36,
    // then the owner's words, which came now.
    let at = |x: &str| m.find(x).unwrap();
    let order = ["QATCUT", "QEDGE", "QMID", "QSTRADDLE", "QOPEN", "QNEW", "BETAEND"];
    assert!(order.windows(2).all(|w| at(w[0]) < at(w[1])), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// With no previous session there is no lower bound: the project's first understanding is told the talk from before its
// session began.
#[test]
fn the_first_understanding_is_told_the_talk_with_no_lower_bound() {
    let (s, e, _, works) = architect_works_in("owner-talk-first", scripts(), &[(0, "Alpha talk. ALPHAEND")], false);
    let m = message_at(&e, works[0], "B1", false);
    assert!(m.contains("ALPHAEND"), "said before the session began, and told: {m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Every kind of entry of the one-to-one is told from the first round of the session before on, and none from before it:
// the owner's interjection with its images counted, and an entry of a talk, which the page shows in the conversation.
// The reference is the owner's page, which shows these entries in the conversation `architect:p`, and the design: at or
// after the session before began.
#[test]
fn a_new_session_is_told_an_interjection_and_a_talk_entry_made_since_the_session_before_began() {
    let s = setup("owner-talk-kinds");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Alpha talk. ALPHAEND");
    // Said clearly before the work's first round.
    e.db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE architect_matters SET at = at - 5000", [])?;
        Ok(())
    })
    .unwrap();
    let one = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, one)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    age_architect(&mut e, 90 * 60_000);
    // The base began with its reading of the project.
    let cut: i64 =
        e.db.conn()
            .query_row(
                "SELECT min(started_at) FROM rounds WHERE work = ?1 AND cell = 'B1'",
                [one],
                |r| r.get(0),
            )
            .unwrap();
    e.db.write(Actor::Engine, |tx| {
        let matter: i64 = tx.query_row("SELECT min(id) FROM architect_matters", [], |r| r.get(0))?;
        // The entries of a talk a flow version before held: the owner's messages to the architect, naming the talk.
        for (text, at) in [("TALKOLD", cut - 1_000), ("TALKNEW", cut + 1_000)] {
            tx.execute(
                "INSERT INTO messages (sender, text, at) VALUES ('owner', ?1, ?2)",
                params![text, at],
            )?;
            tx.execute(
                "INSERT INTO deliveries (message, recipient, matter) VALUES (?1, 'architect', ?2)",
                params![tx.last_insert_rowid(), matter],
            )?;
        }
        let said = db::ToLine::new(db::Sender::Owner, one, "architect", "INTERJECTEND");
        let id = db::say_to_line(tx, &said, None)?;
        db::take(tx, id, one, 1)?;
        for position in 1..=3 {
            tx.execute(
                "INSERT INTO attachments (message, position, media_type, size, hash, bytes)
                 VALUES (?1, ?2, 'image/png', 4, 'h', zeroblob(4))",
                params![id, position],
            )?;
        }
        Ok(())
    })
    .unwrap();
    a_new_policy_takes_effect(&mut e);
    say(&mut e, "Beta talk. BETAEND");
    let two = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| path_of(e, two).iter().any(|c| c == "B1")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let u1 = input_at(&e, two, "B1", false);
    assert_eq!(given_ending(&u1, "·engine·owner_talk"), Some(true), "{u1}");
    let m = u1["message"].as_str().unwrap();
    assert!(!m.contains("TALKOLD"), "said before the session before began: {m}");
    assert!(!m.contains("ALPHAEND"), "{m}");
    for told in ["TALKNEW", "INTERJECTEND", "BETAEND"] {
        assert!(m.contains(told), "{told} is told: {m}");
    }
    assert!(m.contains("3 images attached"), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}
