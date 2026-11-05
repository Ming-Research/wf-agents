//! Taking back the owner's words that no agent has taken up yet: which words the page offers to take back, what comes
//! back into the box, and what stays where it is.

use api::{ConversationEntry, Retract, TakeBack, Talks};
use db::attachments::{self, Image, Words};
use db::rusqlite::{Connection, ToSql};
use db::{Actor, Db};

/// A PNG's signature and a few bytes, which the images of the owner's words are checked against.
const PNG: &[u8] = b"\x89PNG\r\n\x1a\nbytes";

/// Words as the owner typed them, in a database with a work of the engineer's (1) at its first cell with a round running
/// (round 1, which was given interjection 2), and the architect's work (2) of the same project, at the cell that picks
/// what comes next, with no round running.
fn world() -> Db {
    let relations = db::relations(db::SCHEMA).unwrap();
    let compiled = flow::compile(
        flow::FLOW,
        &flow::Context {
            relations,
            tables: db::index_of(db::SCHEMA).unwrap(),
        },
    )
    .unwrap();
    let json = compiled.json();
    let mut db = Db::in_memory().unwrap();
    db.write(Actor::Engine, |tx| {
        let run = |sql: &str, args: &[&dyn ToSql]| tx.execute(sql, args).map(|_| ());
        run("INSERT INTO flow_versions (digest, compiled) VALUES ('d', ?1)", &[&json])?;
        run(
            "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 'First', 'x', 'y', 'change', '1')",
            &[],
        )?;
        run(
            "INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'R1')",
            &[],
        )?;
        run(
            "INSERT INTO works (item, flow_version, graph, at, project) VALUES (NULL, 1, 'architect', 'N0', 'p')",
            &[],
        )?;
        let given = r#"{"engine":{"interjections":[2]}}"#;
        run(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'R1', ?1, 'c', 50)",
            &[&given],
        )?;
        Ok(())
    })
    .unwrap();
    db
}

/// Says something as the owner would, with the rows the owner's write makes: an interjection to the engineer line.
fn said(db: &mut Db, id: i64, text: &str, informs: i64, matter: Option<i64>) {
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO interjections (id, work, line, speaker, text, informs, matter)
             VALUES (?1, 1, 'engineer', 'owner', ?2, ?3, ?4)",
            db::rusqlite::params![id, text, informs, matter],
        )?;
        Ok(())
    })
    .unwrap();
}

/// Words said to the architect, as its queue holds them, in a work's group (`work`) or in the one-to-one.
fn queued(db: &mut Db, id: i64, text: &str, place: &str) {
    let mut body = serde_json::json!({ "text": text, "where": place });
    if place == "work" {
        body["work"] = serde_json::json!(1);
    }
    let body = body.to_string();
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO architect_matters (id, project, kind, detail, what, body, at)
             VALUES (?1, 'p', 'owner_words', 'said', ?2, ?3, 400)",
            db::rusqlite::params![id, format!("The owner says: {text}"), body],
        )?;
        Ok(())
    })
    .unwrap();
}

fn entries(db: &Db, key: &str) -> Vec<ConversationEntry> {
    let t = Talks {
        projects: vec!["p".into(), "q".into()],
        sets: vec![vec!["p".into(), "q".into()]],
        reply_wait: 0,
    };
    api::conversation(db.conn(), &api::Flows::default(), &t, key, api::CONVERSATION_WINDOW)
        .unwrap()
        .entries
}

/// What the page is told of the words of an entry: whether they are shown as not taken up yet and whether they can be
/// taken back; none when the conversation has no such entry.
fn page(db: &Db, key: &str, source: &str, id: i64) -> Option<(bool, Retract)> {
    entries(db, key)
        .iter()
        .find(|e| e.source == source && e.id == id)
        .map(|e| (e.pending, e.retract))
}

fn take(db: &mut Db, source: &str, id: i64) -> Result<api::TakenBack, String> {
    api::take_back(
        db,
        &TakeBack {
            source: source.into(),
            id,
        },
    )
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

// Words no agent has come back whole into the box: the text, the images as they were sent and whom they were for; and no
// row of them is left for an agent, with the old rows kept in `changes` as the owner's. Before the words are taken, the
// page offers it.
#[test]
fn words_no_agent_has_come_back_whole_and_leave_no_row_for_an_agent() {
    let mut db = world();
    said(&mut db, 1, "Make it blue.", 0, None);
    db.write(Actor::Owner, |tx| {
        attachments::attach(
            tx,
            Words::Interjection(1),
            &[Image {
                media_type: "image/png".into(),
                bytes: PNG.to_vec(),
            }],
        )?;
        Ok(())
    })
    .unwrap();
    assert_eq!(page(&db, "work:1", "interjection", 1), Some((true, Retract::Yes)));
    assert_eq!(
        count(
            db.conn(),
            "SELECT count(*) FROM waiting_interjections WHERE interjection = 1"
        ),
        1
    );

    let back = take(&mut db, "interjection", 1).unwrap();

    assert_eq!(back.text, "Make it blue.");
    assert_eq!(back.to, ["engineer"]);
    assert_eq!(back.images.len(), 1);
    assert_eq!(back.images[0].media_type, "image/png");
    assert_eq!(attachments::from_base64(&back.images[0].data).unwrap(), PNG);
    assert_eq!(count(db.conn(), "SELECT count(*) FROM waiting_interjections"), 0);
    assert_eq!(count(db.conn(), "SELECT count(*) FROM interjections WHERE id = 1"), 0);
    assert_eq!(count(db.conn(), "SELECT count(*) FROM attachments"), 0);
    assert_eq!(page(&db, "work:1", "interjection", 1), None);
    assert_eq!(
        count(
            db.conn(),
            "SELECT count(*) FROM changes WHERE table_name = 'interjections' AND actor = 'owner'
               AND old_row IS NOT NULL AND new_row IS NULL"
        ),
        1
    );
    // Taken back once, there is nothing to take back again.
    assert!(take(&mut db, "interjection", 1).is_err());
}

// Words an agent has stay where they are, whatever way it has them: given by the message of a round that has not ended,
// recorded as handed in, or relayed from an answer to an ask, which the ask records; and what the owner did not say is
// not the owner's to take back. The page says which are which, and a refusal deletes nothing.
#[test]
fn words_an_agent_has_are_not_taken_back_and_nothing_is_deleted() {
    let mut db = world();
    said(&mut db, 2, "Given by the message.", 0, None);
    said(&mut db, 3, "Handed in.", 0, None);
    said(
        &mut db,
        4,
        "Asked to approve the change before it merges, the owner replied: fine",
        0,
        None,
    );
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO interjections (id, work, line, speaker, text) VALUES (5, 1, 'engineer', 'architect', 'Mine.')",
            [],
        )?;
        tx.execute("UPDATE interjections SET delivered_round = 1 WHERE id = 3", [])?;
        Ok(())
    })
    .unwrap();

    // Round 1 is running and its message gave interjection 2: the page still says not taken up, and says it is given.
    assert_eq!(page(&db, "work:1", "interjection", 2), Some((true, Retract::Given)));
    // Handed in: not shown as waiting any more. The relay of an answer is the owner's answer, not words to take back.
    assert_eq!(page(&db, "work:1", "interjection", 3).map(|p| p.1), Some(Retract::No));
    assert_eq!(page(&db, "work:1", "interjection", 4), Some((true, Retract::No)));
    for id in 2..=5 {
        assert!(take(&mut db, "interjection", id).is_err(), "interjection {id}");
    }
    assert_eq!(count(db.conn(), "SELECT count(*) FROM interjections"), 4);
    assert_eq!(
        count(
            db.conn(),
            "SELECT count(*) FROM changes WHERE table_name = 'interjections' AND new_row IS NULL"
        ),
        0
    );

    // The round ends: words its message gave are handed in and recorded so, and the round is over.
    db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE rounds SET ended_at = 60 WHERE work = 1 AND n = 1", [])?;
        tx.execute("UPDATE interjections SET delivered_round = 1 WHERE id = 2", [])?;
        Ok(())
    })
    .unwrap();
    assert!(take(&mut db, "interjection", 2).is_err());
}

// Words said to the architect in a work's group are two rows, an interjection that tells the engineer and the architect's
// matter; they are taken back together, and only while neither agent has them. Words to both come back as for both.
#[test]
fn words_said_in_a_work_s_group_to_the_architect_come_back_with_the_matter() {
    let mut db = world();
    queued(&mut db, 10, "Hold on.", "work");
    said(
        &mut db,
        6,
        "Said to the architect, for you to know: Hold on.",
        1,
        Some(10),
    );
    queued(&mut db, 11, "Both of you.", "work");
    said(&mut db, 7, "Both of you.", 0, Some(11));
    assert_eq!(page(&db, "work:1", "interjection", 6), Some((true, Retract::Yes)));
    assert_eq!(page(&db, "work:1", "interjection", 7), Some((true, Retract::Yes)));

    let back = take(&mut db, "interjection", 6).unwrap();
    assert_eq!(
        (back.text.as_str(), back.to.as_slice()),
        ("Hold on.", ["architect".to_string()].as_slice())
    );
    assert_eq!(
        count(db.conn(), "SELECT count(*) FROM architect_matters WHERE id = 10"),
        0
    );
    assert_eq!(count(db.conn(), "SELECT count(*) FROM interjections WHERE id = 6"), 0);
    assert_eq!(
        count(db.conn(), "SELECT count(*) FROM architect_queue WHERE matter = 10"),
        0
    );

    // The engineer has the words to both, though the architect has not picked them: the architect's half stays.
    db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE interjections SET delivered_round = 1 WHERE id = 7", [])?;
        Ok(())
    })
    .unwrap();
    assert_eq!(
        page(&db, "work:1", "interjection", 7).map(|p| p.1),
        Some(Retract::Given)
    );
    assert!(take(&mut db, "interjection", 7).is_err());
    assert_eq!(
        count(db.conn(), "SELECT count(*) FROM architect_matters WHERE id = 11"),
        1
    );

    // Words to both that nobody has come back for both.
    queued(&mut db, 12, "To you both.", "work");
    said(&mut db, 8, "To you both.", 0, Some(12));
    let back = take(&mut db, "interjection", 8).unwrap();
    assert_eq!(back.to, ["engineer", "architect"]);
    assert_eq!(
        count(db.conn(), "SELECT count(*) FROM architect_matters WHERE id = 12"),
        0
    );
}

// Words in the architect's queue come back while the architect has not picked them. Once a round of its work picked them,
// or handled them, or while a question to the owner holds them, they are the architect's; while it is picking the next
// matter, they wait a moment, and are taken back once it is done.
#[test]
fn words_in_the_architect_s_queue_come_back_until_it_picks_them() {
    let mut db = world();
    queued(&mut db, 20, "Look at the logs.", "page");
    db.write(Actor::Owner, |tx| {
        attachments::attach(
            tx,
            Words::Matter(20),
            &[Image {
                media_type: "image/png".into(),
                bytes: PNG.to_vec(),
            }],
        )?;
        Ok(())
    })
    .unwrap();
    queued(&mut db, 21, "Picked already.", "page");
    queued(&mut db, 22, "Handled already.", "page");
    queued(&mut db, 23, "Asked about.", "page");
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
             VALUES (2, 1, 'N0', '{}', '{\"next\":{\"verdict\":\"owner\",\"matter\":21}}', 'owner', 'c', 100, 101)",
            [],
        )?;
        tx.execute(
            "UPDATE architect_matters SET handled_work = 2, handled_round = 1 WHERE id = 22",
            [],
        )?;
        tx.execute(
            "INSERT INTO owner_asks (id, work, round, body) VALUES (1, 2, 1, '{}')",
            [],
        )?;
        tx.execute(
            "UPDATE architect_matters SET asked = 1, resume_cell = 'H8' WHERE id = 23",
            [],
        )?;
        Ok(())
    })
    .unwrap();

    assert_eq!(page(&db, "architect:p", "words", 20), Some((true, Retract::Yes)));
    for id in [21, 22, 23] {
        assert!(take(&mut db, "words", id).is_err(), "matter {id}");
    }
    assert_eq!(
        count(
            db.conn(),
            "SELECT count(*) FROM architect_matters WHERE id IN (21, 22, 23)"
        ),
        3
    );

    // The architect is picking what comes next: its round at the cell that does is not over, and the words wait.
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (2, 2, 'N0', '{}', 'c', 200)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let why = take(&mut db, "words", 20).unwrap_err();
    assert!(why.contains("try again"), "{why}");
    assert_eq!(
        count(db.conn(), "SELECT count(*) FROM architect_matters WHERE id = 20"),
        1
    );

    db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE rounds SET output = '{\"next\":{\"verdict\":\"none\"}}', transition = 'none', ended_at = 201
             WHERE work = 2 AND n = 2",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let back = take(&mut db, "words", 20).unwrap();
    assert_eq!(
        (back.text.as_str(), back.to.as_slice()),
        ("Look at the logs.", ["architect".to_string()].as_slice())
    );
    assert_eq!(back.images.len(), 1);
    assert_eq!(attachments::from_base64(&back.images[0].data).unwrap(), PNG);
    assert_eq!(
        count(db.conn(), "SELECT count(*) FROM architect_matters WHERE id = 20"),
        0
    );
    assert_eq!(count(db.conn(), "SELECT count(*) FROM attachments"), 0);
    assert_eq!(page(&db, "architect:p", "words", 20), None);
}

// Words said to several architects at once are one matter for each, taken up each on its own: the page keeps showing them
// as not taken up yet, offers no way back, and the write refuses.
#[test]
fn words_said_to_several_architects_at_once_are_not_taken_back() {
    let mut db = world();
    let body = serde_json::json!({
        "text": "To all.", "where": "projects", "group": "projects:p|q", "to": ["p", "q"]
    })
    .to_string();
    db.write(Actor::Owner, |tx| {
        for (id, project) in [(30, "p"), (31, "q")] {
            tx.execute(
                "INSERT INTO architect_matters (id, project, kind, detail, what, body, at)
                 VALUES (?1, ?2, 'owner_words', 'said', 'The owner says: To all.', ?3, 400)",
                db::rusqlite::params![id, project, body],
            )?;
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(page(&db, "projects:p|q", "words", 30), Some((true, Retract::No)));
    assert!(take(&mut db, "words", 30).is_err());
    assert_eq!(count(db.conn(), "SELECT count(*) FROM architect_matters"), 2);
}

// Words that are not what the owner typed are not the owner's to take back, whatever the page shows of them: a question
// about a decision card is asked in the card's own box, and the copy that tells the engineer what the architect was told
// has no words of its own without the matter it copies.
#[test]
fn words_about_a_decision_card_and_a_copy_without_its_matter_are_not_taken_back() {
    let mut db = world();
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO design_cards (work, round, key, body, status) VALUES (1, 1, 'card-1', '[]', 'open')",
            [],
        )?;
        tx.execute(
            "INSERT INTO interjections (id, work, line, speaker, text, design_card)
             VALUES (20, 1, 'engineer', 'owner', 'About the card.', 1)",
            [],
        )?;
        tx.execute(
            "INSERT INTO interjections (id, work, line, speaker, text, informs)
             VALUES (21, 1, 'engineer', 'owner', 'Said to the architect, for you to know: nothing.', 1)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    for id in [20, 21] {
        assert!(take(&mut db, "interjection", id).is_err(), "interjection {id}");
    }
    assert_eq!(
        count(db.conn(), "SELECT count(*) FROM interjections WHERE id IN (20, 21)"),
        2
    );
}
