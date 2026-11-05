//! The corners of the lines a return sent back is said with (`db::sent_back_lines`), of who may let go on a work paused
//! for returns sent back (`db::architect_may_resume`), and of the view of the architect's queue that says the work is
//! waited for, read through the public interface.

use serde_json::json;

// The reference is `db::problem_line`, which its own comment calls the only place the line is made: every line is that
// line for the entry's check, field and text, in the order the entries are kept, one for each entry, even for two
// entries that say the same, and a field that is null, missing or not a text is a problem about the return as a whole.
#[test]
fn every_kept_problem_is_one_line_in_its_order_whatever_its_field() {
    let rejected = json!({
        "check": "evidence_holds (the return): no evidence",
        "problems": [
            { "check": "tests_named", "field": "outcome.wrote", "text": "no such test" },
            { "check": "tests_named", "field": "outcome.wrote", "text": "no such test" },
            { "check": "evidence_holds", "text": "no evidence" },
            { "check": "evidence_holds", "field": 4, "text": "a number is no field" },
            { "check": "format", "field": "", "text": "an empty field" },
        ],
    });
    let want = [
        db::problem_line("tests_named", Some("outcome.wrote"), "no such test"),
        db::problem_line("tests_named", Some("outcome.wrote"), "no such test"),
        db::problem_line("evidence_holds", None, "no evidence"),
        db::problem_line("evidence_holds", None, "a number is no field"),
        db::problem_line("format", Some(""), "an empty field"),
    ];
    assert_eq!(db::sent_back_lines(&rejected), want);
}

// A record from before the problems were kept is said by the text it was sent back with, line by line; so is one whose
// `problems` is empty or holds nothing a line can be made of, so that a return sent back is never said by nothing while
// the column holds its reason. The check's text comes before the schema's, as `sent_back_text` reads it.
#[test]
fn a_record_without_usable_problems_is_said_by_the_text_it_was_sent_back_with() {
    let text = "first\nsecond\nthird";
    assert_eq!(
        db::sent_back_lines(&json!({ "check": text })),
        ["first", "second", "third"]
    );
    assert_eq!(
        db::sent_back_lines(&json!({ "check": text, "problems": [] })),
        ["first", "second", "third"]
    );
    let useless = json!({ "format": "the schema says no", "problems": [{ "check": 5 }, { "text": "x" }, "y"] });
    assert_eq!(db::sent_back_lines(&useless), ["the schema says no"]);
    let both = json!({ "check": "from the check", "format": "from the schema" });
    assert_eq!(db::sent_back_lines(&both), ["from the check"]);
    // A column that is not an object, or holds a text under neither name, says nothing.
    for nothing in [
        json!(null),
        json!("a text"),
        json!([]),
        json!({ "failed": { "kind": "offline" } }),
        json!({ "check": 3 }),
        json!({ "problems": "not a list" }),
    ] {
        assert!(db::sent_back_lines(&nothing).is_empty(), "{nothing}");
    }
}

// The architect may let go on only what it stopped itself or what the engine paused for returns in a row it was asked
// to look at, and only when the engine paused it. The reference is the two constants the reasons are made from: a
// reason that only resembles one, in another case, with something before it, or with the owner as the one who paused,
// is not one.
#[test]
fn the_architect_may_let_go_on_only_a_pause_the_engine_made_for_it() {
    let look = format!(
        "{} 3 times in a row, the last at cell R1, and its project's architect is asked to look at why",
        db::SENT_BACK_FOR_ARCHITECT
    );
    assert!(db::architect_may_resume(Some(&look), Some("engine")));
    for (reason, by) in [
        (look.clone(), Some("owner")),
        (look.clone(), Some("architect")),
        (look.clone(), None),
        (format!(" {look}"), Some("engine")),
        (look.to_uppercase(), Some("engine")),
        (look.replacen("returns", "return", 1), Some("engine")),
        // The pause the engine makes when no architect looks: the owner's alone, and worded to say so.
        (
            "3 returns in a row were sent back, the last at cell H8, for:\nx".to_string(),
            Some("engine"),
        ),
        (String::new(), Some("engine")),
    ] {
        assert!(!db::architect_may_resume(Some(&reason), by), "{reason:?} by {by:?}");
    }
    assert!(!db::architect_may_resume(None, Some("engine")));
}

// The view of the architect's queue says an engineer's work waits for the matters of its returns sent back while the
// engine has paused it for returns in a row, as the architect's leave to let it go on reads that pause: and only then.
// The reference is the words of the view, not the code that makes the reason: a work paused by the owner with the same
// words, one the engine paused for the owner alone, one that ended, one that runs, and a matter of another kind are
// not waited for.
#[test]
fn the_queue_says_a_paused_work_waits_for_the_matters_of_its_returns_sent_back_and_nothing_else_does() {
    let mut conn = db::Db::in_memory().unwrap();
    let look = format!(
        "{} 3 times in a row, the last at cell R1, and its project's architect is asked to look at why",
        db::SENT_BACK_FOR_ARCHITECT
    );
    let owners_alone = "3 returns in a row were sent back, the last at cell H8, for:\nx";
    // The work's pause, who paused it, why it ended, the detail of its matter, and what the queue says of the matter.
    let cases: [(Option<&str>, Option<&str>, Option<&str>, &str, &str); 6] = [
        (Some(look.as_str()), Some("engine"), None, "sent_back", "agent"),
        (Some(look.as_str()), Some("owner"), None, "sent_back", "none"),
        (Some(owners_alone), Some("engine"), None, "sent_back", "none"),
        (Some(look.as_str()), Some("engine"), Some("done"), "sent_back", "none"),
        (None, None, None, "sent_back", "none"),
        (Some(look.as_str()), Some("engine"), None, "order", "none"),
    ];
    conn.write(db::Actor::Engine, |tx| {
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        let version = tx.last_insert_rowid();
        for (paused, by, ended, detail, _) in cases {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, paused, paused_by, ended)
                 VALUES (NULL, ?1, 'engineer', 'R1', ?2, ?3, ?4)",
                db::rusqlite::params![version, paused, by, ended],
            )?;
            let work = tx.last_insert_rowid();
            db::queue_matter(
                tx,
                &db::Matter {
                    project: "p".into(),
                    kind: "set_straight".into(),
                    detail: Some(detail.into()),
                    what: "A return was sent back.".into(),
                    body: json!({ "work": work }),
                    ..db::Matter::default()
                },
            )?;
        }
        Ok(())
    })
    .unwrap();
    let mut st = conn
        .conn()
        .prepare("SELECT waits FROM architect_queue ORDER BY matter")
        .unwrap();
    let waits: Vec<String> = st.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
    let want: Vec<&str> = cases.iter().map(|c| c.4).collect();
    assert_eq!(waits, want);
}

// The view names the start of the reason by words of its own, which the constant that the architect's leave to let a
// work go on reads must be the same as: the two are written apart, so a change to one alone is caught here.
#[test]
fn the_view_of_the_queue_reads_the_start_of_the_reason_the_constant_names() {
    let like = format!("w.paused LIKE '{}%'", db::SENT_BACK_FOR_ARCHITECT);
    assert!(db::SCHEMA.contains(&like), "{like}");
}
