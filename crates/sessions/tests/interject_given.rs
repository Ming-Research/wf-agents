//! What the hook after a tool use does with the interjections the round's message already gave, those of both kinds
//! (`informs`) and those with images: the agent has them, so the hook hands in nothing of them; the owner's page must
//! not show them as not taken up while the round runs, so it records the ones the agent has as delivered in the round.
//! The round's record lists them as the engine writes it, in `rounds.input`, at `$.engine.interjections`.

use db::{Actor, Db};
use sessions::interject::{deliver, delivered_during};

/// A work with one round that runs, whose message gave the interjections listed in its record (`given`).
fn running_round(given: &[i64]) -> Db {
    let mut db = Db::in_memory().unwrap();
    db.write(Actor::Engine, |tx| {
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        tx.execute(
            "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'I1')",
            [],
        )?;
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'I1', ?1, 'c', 1)",
            [serde_json::json!({ "engine": { "interjections": given } }).to_string()],
        )?;
        Ok(())
    })
    .unwrap();
    db
}

/// Something said to the work's engineer, with images when `images` is true; the id it gets is the next.
fn say(db: &mut Db, text: &str, informs: bool, images: bool) {
    db.write(Actor::Owner, |tx| {
        let said = db::ToLine {
            informs,
            ..db::ToLine::new(db::Sender::Owner, 1, "engineer", text)
        };
        let id = db::say_to_line(tx, &said, None)?;
        if images {
            let image = db::attachments::Image {
                media_type: "image/png".into(),
                bytes: b"\x89PNG\r\n\x1a\n".to_vec(),
            };
            db::attachments::attach(tx, db::attachments::Words::Message(id), &[image])?;
        }
        Ok(())
    })
    .unwrap();
}

/// The round an interjection is recorded as delivered in.
fn delivered_in(db: &Db, id: i64) -> Option<i64> {
    db.conn()
        .query_row("SELECT taken_round FROM line_sayings WHERE id = ?1", [id], |r| r.get(0))
        .unwrap()
}

/// The numbers of the interjections `delivered_during` gives for the round, when its message gave `given`.
fn during(db: &Db, given: &[i64]) -> Vec<i64> {
    delivered_during(db, 1, 1, given)
        .unwrap()
        .iter()
        .map(|r| r["interjection"].as_i64().unwrap())
        .collect()
}

// Both kinds the message gave are recorded as delivered with nothing handed in, whether or not the hook has more to
// hand in; what the round handed in while it ran, and only that, is what the round's end adds to its record.
#[test]
fn the_hook_records_both_kinds_the_message_gave_and_hands_in_only_what_came_since() {
    let mut db = running_round(&[1, 2]);
    say(&mut db, "Why this size?", false, false);
    say(&mut db, "Your question became an item.", true, false);
    assert_eq!(deliver(&mut db, 1, 1, "engineer").unwrap(), None);
    assert_eq!((delivered_in(&db, 1), delivered_in(&db, 2)), (Some(1), Some(1)));
    assert_eq!(during(&db, &[1, 2]), Vec::<i64>::new());

    say(&mut db, "Look at the log.", false, false);
    let out = deliver(&mut db, 1, 1, "engineer").unwrap().unwrap();
    assert!(
        out.contains("### ij:3") && !out.contains("ij:1") && !out.contains("ij:2"),
        "{out}"
    );
    assert_eq!(during(&db, &[1, 2]), [3]);
    // Called again, it has nothing more to give and nothing more to record.
    assert_eq!(deliver(&mut db, 1, 1, "engineer").unwrap(), None);
    assert_eq!(
        (delivered_in(&db, 1), delivered_in(&db, 2), delivered_in(&db, 3)),
        (Some(1), Some(1), Some(1))
    );
}

// The words with images the message gave reached the agent with their images, so they too are recorded as delivered;
// those that came after the message wait for the line's next message, with their images, and are recorded by no hook.
#[test]
fn the_hook_records_words_with_images_the_message_gave_but_not_those_that_came_since() {
    let mut db = running_round(&[1]);
    say(&mut db, "See this.", false, true);
    assert_eq!(deliver(&mut db, 1, 1, "engineer").unwrap(), None);
    assert_eq!(delivered_in(&db, 1), Some(1));

    say(&mut db, "And this.", false, true);
    assert_eq!(deliver(&mut db, 1, 1, "engineer").unwrap(), None);
    assert_eq!(delivered_in(&db, 2), None);
    assert_eq!(during(&db, &[1]), Vec::<i64>::new());
}

// The record of another round of the work does not hide an interjection from this round's hook: only the round that
// runs says what its message gave.
#[test]
fn what_an_earlier_round_was_given_is_not_what_this_rounds_message_gave() {
    let mut db = running_round(&[1]);
    db.write(Actor::Engine, |tx| {
        // Round 1 ended, having been given interjection 1; round 2 runs, and its message gave nothing.
        tx.execute("UPDATE rounds SET ended_at = 2 WHERE work = 1 AND n = 1", [])?;
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at)
             VALUES (1, 2, 'I1', '{\"engine\":{\"interjections\":[]}}', 'c', 3)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    say(&mut db, "Waiting for the line.", false, false);
    let out = deliver(&mut db, 1, 2, "engineer").unwrap().unwrap();
    assert!(out.contains("### ij:1"), "{out}");
    assert_eq!(delivered_in(&db, 1), Some(2));
}
