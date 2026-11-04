//! What the hook after a tool use hands in to a running round: not what the round's message gave, and each interjection
//! under a lead that says whether it needs an answer (`informs`). The tests give a round the record the engine writes
//! when it starts one, `rounds.input`, with the interjections its message gave listed in `$.engine.interjections`.

use db::rusqlite::params;
use db::{Actor, Db};
use sessions::interject::deliver;

/// What the hook says before the interjections that may need an answer, and before those that need none.
const ASKS: &str = "answer in this round's return";
const TELLS: &str = "none of it needs an answer";

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

/// Something said to the work's engineer, by its text; `informs` for a word that needs no answer.
fn say(db: &mut Db, text: &str, informs: bool) {
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO interjections (work, line, speaker, text, informs) VALUES (1, 'engineer', 'owner', ?1, ?2)",
            params![text, informs],
        )?;
        Ok(())
    })
    .unwrap();
}

/// The round an interjection is recorded as delivered in.
fn delivered_in(db: &Db, id: i64) -> Option<i64> {
    db.conn()
        .query_row("SELECT delivered_round FROM interjections WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap()
}

// What the round's message gave is not handed in again by the hook, but it is recorded as delivered in the round (the
// agent has it, and the owner's page must not show it as not taken up while the round runs). What came after the
// message is handed in, once.
#[test]
fn the_hook_does_not_hand_in_what_the_rounds_message_gave() {
    let mut db = running_round(&[1]);
    say(&mut db, "Given by the message.", false);
    assert_eq!(deliver(&mut db, 1, 1, "engineer").unwrap(), None);
    assert_eq!(delivered_in(&db, 1), Some(1));

    say(&mut db, "Said while the round runs.", false);
    let out = deliver(&mut db, 1, 1, "engineer").unwrap().unwrap();
    assert!(out.contains("### ij:2") && !out.contains("ij:1"), "{out}");
    assert_eq!((delivered_in(&db, 1), delivered_in(&db, 2)), (Some(1), Some(1)));
    assert_eq!(deliver(&mut db, 1, 1, "engineer").unwrap(), None);
}

// An interjection that needs no answer is handed in under a lead that says so and asks for none; one that may need an
// answer under the lead that asks. Both at once come each under its own, the ones that may need an answer first.
#[test]
fn the_hook_asks_for_an_answer_only_where_one_may_be_needed() {
    let mut db = running_round(&[]);
    say(&mut db, "The question became an item.", true);
    let told = deliver(&mut db, 1, 1, "engineer").unwrap().unwrap();
    assert!(told.contains(TELLS) && told.contains("### ij:1"), "{told}");
    assert!(!told.contains(ASKS), "{told}");

    say(&mut db, "Why this size?", false);
    let asked = deliver(&mut db, 1, 1, "engineer").unwrap().unwrap();
    assert!(asked.contains(ASKS) && asked.contains("### ij:2"), "{asked}");
    assert!(!asked.contains(TELLS), "{asked}");

    say(&mut db, "Nothing changes for you.", true);
    say(&mut db, "Look at the log.", false);
    let both = deliver(&mut db, 1, 1, "engineer").unwrap().unwrap();
    let at = |text: &str| both.find(text).unwrap_or_else(|| panic!("no {text} in {both}"));
    assert!(
        at(ASKS) < at("### ij:4") && at("### ij:4") < at(TELLS) && at(TELLS) < at("### ij:3"),
        "{both}"
    );
}
