//! What the view `item_merge_conditions` works out from the answers to the merge gate's asks. The reference is the owner's
//! choice for decision point P3 of the design and the comment on the view: an answer declared as the owner's or as a
//! delegate's that chose an option judges a condition, and the view says who did. An answer of any other shape leaves it
//! open, and the work waits.

use db::rusqlite::params;
use db::{Actor, Db};

const OWNER_MET: &str = r#"{"by":"owner","at":1,"text":"","speaker":"owner","choice":"met"}"#;
const OWNER_WITHDRAWN: &str = r#"{"by":"owner","at":1,"text":"No.","speaker":"owner","choice":"withdrawn"}"#;
const OWNER_WORDS: &str = r#"{"by":"owner","at":1,"text":"met","speaker":"owner"}"#;
const OWNER_OTHER_OPTION: &str = r#"{"by":"owner","at":1,"text":"","speaker":"owner","choice":"open"}"#;
const DELEGATE_MET: &str = r#"{"by":"owner","at":1,"text":"","speaker":"delegate","choice":"met"}"#;
const DELEGATE_RELAYING: &str =
    r#"{"by":"owner","at":1,"text":"Met.","speaker":"delegate","relayed":["Met."],"choice":"withdrawn"}"#;
const DELEGATE_WORDS: &str = r#"{"by":"owner","at":1,"text":"met","speaker":"delegate"}"#;
const UNDECLARED_MET: &str = r#"{"by":"owner","at":1,"text":"","choice":"met"}"#;
const ENGINE_NOTE: &str = r#"{"by":"engine","at":1,"text":"No answer is needed: the work moved on."}"#;

// Each condition with the answers its asks had, in order, where it stands by them and who judged it. A condition with no
// ask, an ask that waits, words, an option that is neither of the two, an answer kept before the speaker was declared and
// the engine's note all leave it open; an option of the owner's or a delegate's judges it, also after any of those, and
// the view keeps which. An answer judges only the condition its ask is of.
#[test]
fn a_condition_stands_as_an_option_of_the_owner_or_a_delegate_says_and_no_other_answer_moves_it() {
    // The answers of a condition's asks, where it stands by them, and who judged it.
    type Case<'a> = (&'a [Option<&'a str>], &'a str, Option<&'a str>);
    let cases: [Case<'_>; 12] = [
        (&[], "open", None),
        (&[None], "open", None),
        (&[Some(OWNER_WORDS)], "open", None),
        (&[Some(OWNER_OTHER_OPTION)], "open", None),
        (&[Some(DELEGATE_WORDS)], "open", None),
        (&[Some(UNDECLARED_MET)], "open", None),
        (&[Some(ENGINE_NOTE)], "open", None),
        (&[Some(DELEGATE_MET)], "met", Some("delegate")),
        (&[Some(DELEGATE_RELAYING)], "withdrawn", Some("delegate")),
        (&[Some(OWNER_MET)], "met", Some("owner")),
        (&[Some(OWNER_WITHDRAWN)], "withdrawn", Some("owner")),
        (
            &[
                Some(ENGINE_NOTE),
                Some(DELEGATE_WORDS),
                Some(OWNER_WORDS),
                Some(OWNER_WITHDRAWN),
            ],
            "withdrawn",
            Some("owner"),
        ),
    ];
    let mut db = Db::in_memory().unwrap();
    db.write(Actor::Engine, |tx| {
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', 'change', '1')",
            [],
        )?;
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'M1')",
            [],
        )?;
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'M1', '{}', 'c', 1)",
            [],
        )?;
        // An ask of no condition, with the owner's own option: it judges none of them.
        tx.execute(
            "INSERT INTO owner_asks (work, round, body, answer) VALUES (1, 1, '{\"to\":\"owner\"}', ?1)",
            [OWNER_MET],
        )?;
        for (i, (answers, _, _)) in cases.iter().enumerate() {
            let condition = i as i64 + 1;
            tx.execute(
                "INSERT INTO merge_conditions
                   (id, item, what, source, source_id, quote, said_by, recorded_by, at)
                 VALUES (?1, 1, 'Run it.', 'ask', 1, 'OK', 'owner', 'owner', 1)",
                [condition],
            )?;
            for answer in *answers {
                tx.execute(
                    "INSERT INTO owner_asks (work, round, body, answer) VALUES (1, 1, json_object('condition', ?1), ?2)",
                    params![condition, answer],
                )?;
            }
        }
        Ok(())
    })
    .unwrap();
    for (i, (answers, stands, by)) in cases.iter().enumerate() {
        let condition = i as i64 + 1;
        let (status, judged, judged_by): (String, Option<i64>, Option<String>) = db
            .conn()
            .query_row(
                "SELECT status, judged, judged_by FROM item_merge_conditions WHERE item = 1 AND id = ?1",
                [condition],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(status, *stands, "condition {condition}, by the answers {answers:?}");
        assert_eq!(judged_by.as_deref(), *by, "condition {condition}");
        let last: Option<i64> = db
            .conn()
            .query_row(
                "SELECT max(id) FROM owner_asks WHERE json_extract(body, '$.condition') = ?1",
                [condition],
                |r| r.get(0),
            )
            .unwrap();
        let expected = if *stands == "open" { None } else { last };
        assert_eq!(judged, expected, "condition {condition}");
    }
}
