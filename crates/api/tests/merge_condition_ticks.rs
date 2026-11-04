//! The tick of a merge condition, on each of the ways the owner's words come in. The reference is the design the owner
//! approved and acceptance A5: a tick records one condition resting on the record the words are kept in, saying who said
//! them as that record does; the table's comment on `quote` and `said_by`; and the item's aim, that what is ticked holds
//! the merge, so that a tick that can hold nothing is refused rather than kept.

use api::{Answer, WorkSaid, answer, reply_to_question_setting, say_to_work};
use db::{Actor, Db};

/// The matter of an engineer's question that an architect holds.
const QUESTION: i64 = 20;
/// An engineer's ask of the owner.
const ENGINEER_ASK: i64 = 1;
/// An ask of the architect, about the one item.
const ARCHITECT_ASK: i64 = 2;
/// The merge gate's ask of a merge condition.
const GATE_ASK: i64 = 3;

/// One item with its work at the implementation (work 1), a work of its project's architect (work 2), and what the owner
/// may answer.
fn database() -> Db {
    let mut db = Db::in_memory().unwrap();
    db.write(Actor::Engine, |tx| {
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', 'change', '1')",
            [],
        )?;
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'I1')",
            [],
        )?;
        tx.execute(
            "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'H8', 'p')",
            [],
        )?;
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES
             (1, 1, 'I1', '{}', 'c', 1), (2, 1, 'H8', '{}', 'c', 1)",
            [],
        )?;
        tx.execute(
            "INSERT INTO owner_asks (id, work, round, body) VALUES
             (1, 1, 1, '{\"to\":\"owner\"}'),
             (2, 2, 1, '{\"from\":\"architect\",\"project\":\"p\",\"kind\":\"question\",\"question\":\"Which?\",\"holds\":[10]}'),
             (3, 1, 1, '{\"to\":\"owner\",\"condition\":1}')",
            [],
        )?;
        tx.execute(
            "INSERT INTO architect_matters (id, project, kind, what, body, item, work, round, at) VALUES
             (10, 'p', 'owner_words', 'w', '{}', 1, NULL, NULL, 1),
             (20, 'p', 'engineer_question', 'q', '{}', 1, 1, 1, 1)",
            [],
        )?;
        tx.execute(
            "INSERT INTO blockers (work, cell, round, kind, what, holder, project, matter, at, since)
             VALUES (1, 'A2', 1, 'question', 'q', 'architect', 'p', 20, 1, 1)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    db
}

fn said(text: &str, to: &str, condition: Option<&str>) -> WorkSaid {
    WorkSaid {
        text: text.into(),
        speaker: "owner".into(),
        relayed: None,
        to: Some(to.into()),
        question: None,
        design_card: None,
        condition: condition.map(str::to_string),
        images: Vec::new(),
    }
}

fn answered(text: &str, condition: Option<&str>) -> Answer {
    Answer {
        text: text.into(),
        speaker: "owner".into(),
        relayed: None,
        why: None,
        reply: None,
        to: None,
        choice: None,
        condition: condition.map(str::to_string),
        images: Vec::new(),
    }
}

/// Each condition recorded: its item, sentence, the record's kind and number, the words, who said them and who recorded it.
type Row = (i64, String, String, i64, String, String, String);

fn rows(db: &Db) -> Vec<Row> {
    db.conn()
        .prepare("SELECT item, what, source, source_id, quote, said_by, recorded_by FROM merge_conditions ORDER BY id")
        .unwrap()
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn row(what: &str, source: &str, id: i64, quote: &str) -> Row {
    (
        1,
        what.into(),
        source.into(),
        id,
        quote.into(),
        "owner".into(),
        "owner".into(),
    )
}

// The answer in a work's group to an engineer's question rests on the question's matter; the answer to an ask of the
// architect about one item, through the ask's own way in, on the ask; words said to both the engineer and the architect
// are one condition, on the matter that keeps them.
#[test]
fn a_tick_on_an_answer_in_the_group_on_an_ask_of_the_architect_and_on_words_to_both_records_one_condition_each() {
    let mut db = database();
    let mut question = said("On real data.", "engineer", Some("Run it on real data."));
    question.question = Some(QUESTION);
    say_to_work(&mut db, 1, &question).unwrap();
    assert_eq!(
        rows(&db),
        [row("Run it on real data.", "matter", QUESTION, "On real data.")]
    );

    answer(&mut db, ARCHITECT_ASK, &answered("OK", Some("Keep the old path."))).unwrap();
    assert_eq!(rows(&db)[1], row("Keep the old path.", "ask", ARCHITECT_ASK, "OK"));

    say_to_work(&mut db, 1, &said("Do both.", "both", Some("Show me the output."))).unwrap();
    let all = rows(&db);
    assert_eq!(all.len(), 3, "{all:?}");
    let matter: i64 = db
        .conn()
        .query_row(
            "SELECT id FROM architect_matters WHERE json_extract(body, '$.text') = 'Do both.'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(all[2], row("Show me the output.", "matter", matter, "Do both."));
}

// What a delegate declares to be the owner's own words is recorded as relayed, on words said and on an answer alike, as
// the architect's record of the same words says (`Record::source_of`); words only partly relayed, as a whole, are the
// delegate's. Who recorded it is the delegate either way.
#[test]
fn a_tick_says_a_delegate_relayed_the_words_it_declared_the_owner_s() {
    let mut db = database();
    let mut relaying = said("Run it twice.", "engineer", Some("Run it twice before the merge."));
    relaying.speaker = "delegate".into();
    relaying.relayed = Some(vec!["Run it twice.".into()]);
    say_to_work(&mut db, 1, &relaying).unwrap();

    let mut part = said(
        "Run it twice. I agree.",
        "engineer",
        Some("Run it twice, the delegate agrees."),
    );
    part.speaker = "delegate".into();
    part.relayed = Some(vec!["Run it twice.".into()]);
    say_to_work(&mut db, 1, &part).unwrap();

    let mut reply = answered("Show me.", Some("Show the owner the output."));
    reply.speaker = "delegate".into();
    reply.relayed = Some(vec!["Show me.".into()]);
    answer(&mut db, ENGINEER_ASK, &reply).unwrap();

    let who: Vec<(String, String)> = rows(&db).into_iter().map(|r| (r.5, r.6)).collect();
    let pair = |said_by: &str| (said_by.to_string(), "delegate".to_string());
    assert_eq!(who, [pair("relayed"), pair("delegate"), pair("relayed")]);
}

// Once the item's merge stands on main a condition holds nothing: a tick on words or on an answer is refused, whichever
// way it comes, and keeps nothing, while the same words with no tick still go. When watching main reverted the merge, the
// work goes on and a tick holds again.
#[test]
fn a_tick_once_the_merge_stands_is_refused_and_again_taken_once_it_is_reverted() {
    let mut db = database();
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, transition, config_commit, started_at, ended_at)
             VALUES (1, 2, 'M2', '{}', 'merged', 'c', 2, 2)",
            [],
        )?;
        tx.execute("UPDATE works SET at = 'L1' WHERE id = 1", [])?;
        Ok(())
    })
    .unwrap();
    let late = Some("Run it twice before the merge.");
    for to in ["engineer", "architect", "both"] {
        let refused = say_to_work(&mut db, 1, &said("Run it twice.", to, late));
        assert!(refused.is_err(), "to {to}: {refused:?}");
    }
    let refused = answer(&mut db, ENGINEER_ASK, &answered("Run it twice.", late));
    assert!(refused.is_err(), "{refused:?}");
    let refused = reply_to_question_setting(&mut db, "p", ARCHITECT_ASK, "Yes.", &[], db::Speaker::Owner, late);
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(rows(&db), Vec::<Row>::new());
    say_to_work(&mut db, 1, &said("Run it twice.", "engineer", None)).unwrap();

    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, transition, config_commit, started_at, ended_at)
             VALUES (1, 3, 'L1', '{}', 'broken_by_merge', 'c', 3, 3)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    say_to_work(&mut db, 1, &said("Run it twice.", "engineer", late)).unwrap();
    assert_eq!(rows(&db).len(), 1);
}

// An option needs no words, and a condition rests on words: the table keeps an empty quotation only for an answer that
// was images alone, so an option with a tick and no words records none that reads "", which the gate's ask would give as
// the words the owner said.
#[test]
fn a_tick_on_an_option_with_no_words_keeps_no_empty_quotation() {
    let mut db = database();
    let mut chose = answered("", Some("Show me the output as well."));
    chose.choice = Some("met".into());
    let _ = answer(&mut db, GATE_ASK, &chose);
    let empty: Vec<Row> = rows(&db).into_iter().filter(|r| r.4.is_empty()).collect();
    assert_eq!(empty, Vec::<Row>::new());
}
