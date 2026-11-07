//! Words said after other words were taken back: the number the deleted words had is free again, and the new words must
//! not inherit anything the deleted words left under it.

use api::TakeBack;
use db::{Actor, Db};

/// A database with one work of the engineer's (1) and its flow, as the conversation reads them.
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
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', ?1)", [&json])?;
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 'First', 'x', 'y', 'change', '1')",
            [],
        )?;
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'R1')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    db
}

/// Says something to the work's engineer as the owner would: the message the owner's write makes; gives its number.
fn say(db: &mut Db, text: &str) -> i64 {
    db.write(Actor::Owner, |tx| {
        db::say_to_line(tx, &db::ToLine::new(db::Sender::Owner, 1, "engineer", text), None)
    })
    .unwrap()
}

// The time the owner said words is the time their message keeps: words said after other words were taken back are
// shown from the time they were said, not from the time of the deleted words that had the same number.
#[test]
fn words_said_after_others_were_taken_back_are_shown_from_the_time_they_were_said() {
    let mut db = world();
    let first = say(&mut db, "Make it blue.");
    // That was long ago.
    db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE messages SET at = 1000 WHERE id = ?1", [first])?;
        Ok(())
    })
    .unwrap();
    api::take_back(
        &mut db,
        &TakeBack {
            source: "interjection".into(),
            id: first,
        },
    )
    .unwrap();

    let second = say(&mut db, "Make it red.");

    let t = api::Talks {
        projects: vec!["p".into()],
        sets: vec![],
        reply_wait: 0,
        wf_agents: None,
    };
    let page = api::conversation(
        db.conn(),
        &api::Flows::default(),
        &t,
        "work:1",
        api::CONVERSATION_WINDOW,
    )
    .unwrap();
    let entry = page
        .entries
        .iter()
        .find(|e| e.source == "interjection" && e.id == second)
        .expect("the new words are in the conversation");
    assert!(
        entry.at > 1000,
        "the new words (number {second}) are shown from the time {} of the words taken back (number {first})",
        entry.at
    );
}
