//! Returns the command line could not take as they were: closed, read from their text or repaired within the
//! round.

use super::*;

/// A work whose cell `R0` breaks its first return as `fault` says, the script `@repair` answering a repair of it as
/// `repair` says, if given, and every later return of `R0` as `later` says, if given.
fn unparsed_at_r0(
    name: &str,
    fault: &'static str,
    repair: Option<Script>,
    later: Option<&'static str>,
) -> (Setup, Engine, Arc<Mutex<Vec<String>>>, i64) {
    let s = setup(name);
    let mut agents = scripts();
    agents.insert(
        "R0",
        Box::new(move |_: &Request, n: usize| match (n, later) {
            (1, _) => (Vec::new(), vec![("@unparsed", json!(fault))]),
            (_, Some(f)) => (Vec::new(), vec![("@unparsed", json!(f))]),
            _ => (Vec::new(), Vec::new()),
        }),
    );
    if let Some(r) = repair {
        agents.insert("@repair", r);
    }
    let (e, said, item) = start(&s, agents);
    (s, e, said, item)
}

/// What the record of a work's first round at `R0` keeps of the repair of its return, with its contents put back.
fn repair_record(e: &Engine, work: i64) -> (Value, Value) {
    let (input, usage): (String, Option<String>) =
        e.db.conn()
            .query_row(
                "SELECT input, usage FROM rounds WHERE work = ?1 AND cell = 'R0' ORDER BY n LIMIT 1",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    let input = db::put_back(e.db.conn(), &serde_json::from_str(&input).unwrap());
    let usage = usage.map(|u| serde_json::from_str(&u).unwrap()).unwrap_or(Value::Null);
    (input["engine"]["unparsed"].clone(), usage)
}

/// Takes steps until the work on the item has left `R0` or paused.
fn past_r0(e: &mut Engine, item: i64) {
    for _ in 0..6 {
        e.step(None).unwrap();
        if state(e, item).is_some_and(|(_, at, paused)| at != "R0" || paused.is_some()) {
            break;
        }
    }
}

// A return that lacks only its last brace, its fields every return holds written inside `outcome`, is closed by the
// engine within its round, with those fields moved up beside `outcome`: the round takes its row, nothing is run again,
// and no repair is asked of a model.
#[test]
fn a_return_that_lacks_only_its_last_brace_is_closed_within_its_round() {
    let (s, mut e, said, item) = unparsed_at_r0("unparsed-close", "drop_last_brace", None, None);
    past_r0(&mut e, item);
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("R1", None), "{log}");
    assert_eq!(rows_at(&e, work, "R0"), ["done"], "{log}");
    assert!(failed_kinds(&e).is_empty(), "{log}");
    let (record, _) = repair_record(&e, work);
    assert_eq!(record["closed"]["added"], "}", "{record}");
    assert!(
        record.get("repair").is_none() && record.get("again").is_none(),
        "{record}"
    );
    assert!(
        record["attempts"][0]["text"]
            .as_str()
            .is_some_and(|t| t.contains("\"progress\"")),
        "{record}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// What the first round of a work at `R0` records of a form taken from the JSON text of it: the field it was.
fn taken_from_text(e: &Engine, work: i64) -> Option<String> {
    e.db.conn()
        .query_row(
            "SELECT json_extract(input, '$.engine.taken_from_text') FROM rounds
             WHERE work = ?1 AND cell = 'R0' ORDER BY n LIMIT 1",
            [work],
            |r| r.get(0),
        )
        .unwrap()
}

// A return whose outcome the agent wrote as a string holding its JSON is taken with the outcome that JSON holds: the
// round takes its row, and records that its outcome was taken from the text.
#[test]
fn an_outcome_handed_in_as_its_json_text_is_taken_as_the_object() {
    let s = setup("outcome-text");
    let mut agents = scripts();
    agents.insert(
        "R0",
        Box::new(|_: &Request, n: usize| match n {
            1 => (Vec::new(), vec![("@outcome_text", json!(true))]),
            _ => (Vec::new(), Vec::new()),
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    past_r0(&mut e, item);
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("R1", None), "{log}");
    assert_eq!(rows_at(&e, work, "R0"), ["done"], "{log}");
    assert_eq!(sent_back(&e), 0, "{log}");
    assert_eq!(taken_from_text(&e, work).as_deref(), Some("outcome"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// What the first round of a work at `R0` keeps under `engine` in its record.
fn engine_record(e: &Engine, work: i64) -> Value {
    let text: String =
        e.db.conn()
            .query_row(
                "SELECT json_extract(input, '$.engine') FROM rounds WHERE work = ?1 AND cell = 'R0'
                 ORDER BY n LIMIT 1",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    serde_json::from_str(&text).unwrap()
}

// A round records where its return was read from: the tool's input, which the command line checked, or the text of the
// run's result, which no check of it saw. A null a return holds in a field of another form than its own (`pause` beside
// the brief) is taken as left out, the round takes its row with nothing sent back, and the round records the
// path of the null; a return with no such null records none.
#[test]
fn a_round_records_where_its_return_came_from_and_the_nulls_it_took_as_left_out() {
    for (from, nulls) in [("text", true), ("tool", false)] {
        let s = setup(&format!("return-from-{from}"));
        let mut agents = scripts();
        agents.insert(
            "R0",
            Box::new(move |_: &Request, n: usize| match n {
                1 if nulls => (
                    Vec::new(),
                    vec![("@from", json!(from)), ("@foreign_null", json!("pause"))],
                ),
                1 => (Vec::new(), vec![("@from", json!(from))]),
                _ => (Vec::new(), Vec::new()),
            }),
        );
        let (mut e, said, item) = start(&s, agents);
        past_r0(&mut e, item);
        let log = said.lock().unwrap().join("\n");
        let (work, at, paused) = state(&e, item).unwrap();
        assert_eq!((at.as_str(), paused), ("R1", None), "{log}");
        assert_eq!(rows_at(&e, work, "R0"), ["done"], "{log}");
        assert_eq!(sent_back(&e), 0, "{log}");
        let record = engine_record(&e, work);
        assert_eq!(record["return_from"], from, "{record}");
        assert_eq!(
            record.get("foreign_nulls").cloned(),
            nulls.then(|| json!(["outcome.pause"])),
            "{record}"
        );
        let _ = std::fs::remove_dir_all(&s.root);
    }
}

// A run the command line gave up on because its last attempt held the outcome as a string of its JSON is taken from
// that attempt within its round, with the outcome its JSON holds: nothing is repaired or handed in again, and the
// round records both the attempt and that its outcome was taken from the text.
#[test]
fn an_attempt_refused_for_its_outcome_as_json_text_is_taken_within_its_round() {
    let (s, mut e, said, item) = unparsed_at_r0("unparsed-outcome-text", "outcome_text", None, None);
    past_r0(&mut e, item);
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("R1", None), "{log}");
    assert_eq!(rows_at(&e, work, "R0"), ["done"], "{log}");
    assert!(failed_kinds(&e).is_empty(), "{log}");
    let (record, _) = repair_record(&e, work);
    assert_eq!(record["as_it_is"], true, "{record}");
    assert!(
        record.get("repair").is_none() && record.get("again").is_none(),
        "{record}"
    );
    assert_eq!(taken_from_text(&e, work).as_deref(), Some("outcome"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// In the engineer's trial (R18-5, R18-10 and R18-11, cell D2) every attempt the command line refused for its outcome as
// a string held the JSON of the outcome with a closing brace too many after it: the model wrote the form as an object
// whose JSON did not parse, which reached the command line as text. A run the command line gave up on so is taken from
// its last attempt within its round, with the outcome that JSON holds, and the round records that its outcome was taken
// from the text, as for a string that holds the JSON alone.
#[test]
fn an_attempt_refused_for_its_outcome_as_json_text_with_a_brace_too_many_is_taken_within_its_round() {
    let (s, mut e, said, item) = unparsed_at_r0("unparsed-outcome-brace", "outcome_text_brace", None, None);
    past_r0(&mut e, item);
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("R1", None), "{log}");
    assert_eq!(rows_at(&e, work, "R0"), ["done"], "{log}");
    assert_eq!(sent_back(&e), 0, "{log}");
    let (record, _) = repair_record(&e, work);
    assert_eq!(record["as_it_is"], true, "{record}");
    assert_eq!(taken_from_text(&e, work).as_deref(), Some("outcome"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A repair is checked as the return it gives is taken: a repaired return whose outcome is a string holding its JSON is
// taken with that outcome, and the round records that it was taken from the text, instead of the repair being refused
// for an outcome that is no object and the agent asked to hand its return in again.
#[test]
fn a_repair_whose_outcome_is_its_json_text_is_taken_as_any_return() {
    let escapes: Script = Box::new(|r: &Request, _: usize| {
        let text = r.prompt.split("The text:\n").nth(1).unwrap_or_default().trim();
        let fixed = text.replacen(" \"at\" ", " \\\"at\\\" ", 1);
        (Vec::new(), vec![("@result", json!(fixed))])
    });
    let (s, mut e, said, item) = unparsed_at_r0(
        "unparsed-repair-text",
        "unescaped_quote_outcome_text",
        Some(escapes),
        None,
    );
    past_r0(&mut e, item);
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("R1", None), "{log}");
    assert_eq!(sent_back(&e), 0, "{log}");
    let (record, _) = repair_record(&e, work);
    assert_eq!(record["repair"]["taken"], true, "{record}");
    assert!(record.get("again").is_none(), "{record}");
    assert_eq!(taken_from_text(&e, work).as_deref(), Some("outcome"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The attempts the command line refused in a run that handed a return in at last are kept on its round, each by why,
// and the round takes its row as any: nothing is sent back.
#[test]
fn the_attempts_the_command_line_refused_before_it_took_a_return_are_recorded() {
    let s = setup("refused-before-taken");
    let mut agents = scripts();
    let why = "Output does not match required schema: /outcome: must NOT have additional properties ('tell_owner' is \
               not allowed)";
    agents.insert(
        "R0",
        Box::new(move |_: &Request, n: usize| match n {
            1 => (Vec::new(), vec![("@refused", json!([why]))]),
            _ => (Vec::new(), Vec::new()),
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    past_r0(&mut e, item);
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("R1", None), "{log}");
    assert_eq!(sent_back(&e), 0, "{log}");
    let sql = "SELECT json_extract(input, '$.engine.refused') FROM rounds WHERE work = ?1 AND cell = 'R0' ORDER BY n";
    let refused: Option<String> = e.db.conn().query_row(sql, [work], |r| r.get(0)).unwrap();
    let refused = refused.map(|r| serde_json::from_str::<Value>(&r).unwrap());
    assert_eq!(refused, Some(json!([why])), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A run the command line gave up on because its last attempt held the fields every return holds inside `outcome`,
// which it refused for one missing beside it, is taken from that attempt as it is within its round, with those fields
// moved up beside `outcome`: the round takes its row, and nothing is repaired, handed in again or sent back.
#[test]
fn an_attempt_refused_for_its_shape_is_taken_as_it_is_within_its_round() {
    let (s, mut e, said, item) = unparsed_at_r0("unparsed-inside", "inside", None, None);
    past_r0(&mut e, item);
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("R1", None), "{log}");
    assert_eq!(rows_at(&e, work, "R0"), ["done"], "{log}");
    assert!(failed_kinds(&e).is_empty(), "{log}");
    assert_eq!(sent_back(&e), 0, "{log}");
    let (record, _) = repair_record(&e, work);
    assert_eq!(record["as_it_is"], true, "{record}");
    assert!(
        record.get("repair").is_none() && record.get("again").is_none(),
        "{record}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A return with a bare quote in a text is repaired by the lowest level, once, and the repair is taken when it parses,
// fits and keeps every other character; the round's cost holds the repair's.
#[test]
fn a_repair_that_keeps_the_content_is_taken_within_the_round() {
    let escapes: Script = Box::new(|r: &Request, _: usize| {
        let text = r.prompt.split("The text:\n").nth(1).unwrap_or_default().trim();
        let fixed = text.replacen(" \"at\" ", " \\\"at\\\" ", 1);
        (
            Vec::new(),
            vec![
                ("@result", json!(fixed)),
                ("@usage", json!({ "session_cost_usd": 0.002 })),
            ],
        )
    });
    let (s, mut e, said, item) = unparsed_at_r0("unparsed-repair", "unescaped_quote", Some(escapes), None);
    past_r0(&mut e, item);
    let log = said.lock().unwrap().join("\n");
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "R1", "{log}");
    let (record, usage) = repair_record(&e, work);
    assert_eq!(record["repair"]["taken"], true, "{record}");
    assert_eq!(record["repair"]["cost_usd"], 0.002, "{record}");
    assert!(record.get("again").is_none(), "{record}");
    // The scripted round costs 0.01, and the repair beside it.
    let cost = usage["cost_usd"].as_f64().unwrap_or_default();
    assert!((cost - 0.012).abs() < 1e-9, "{usage}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A repair that changes the content is refused, and the agent hands its return in again in its own session, which is
// taken as any return; the round records both.
#[test]
fn a_repair_that_changes_the_content_is_refused_and_the_return_handed_in_again() {
    let rewords: Script = Box::new(|r: &Request, _: usize| {
        let text = r.prompt.split("The text:\n").nth(1).unwrap_or_default().trim();
        (
            Vec::new(),
            vec![("@result", json!(text.replacen(" \"at\" ", " in ", 1)))],
        )
    });
    let (s, mut e, said, item) = unparsed_at_r0("unparsed-again", "unescaped_quote", Some(rewords), None);
    past_r0(&mut e, item);
    let log = said.lock().unwrap().join("\n");
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "R1", "{log}");
    let (record, _) = repair_record(&e, work);
    assert!(
        record["repair"]["refused"]
            .as_str()
            .is_some_and(|w| w.contains("changed")),
        "{record}"
    );
    assert_eq!(record["again"]["handed_in"], true, "{record}");
    assert!(
        record["again"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("could not be parsed as JSON")),
        "{record}"
    );
    assert_eq!(rows_at(&e, work, "R0"), ["done"], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A return cut off is never repaired: the agent is asked to hand it in again, and when that cannot be parsed either,
// the work pauses, the owner is told, and the round is closed as a failed run of its kind.
#[test]
fn a_return_neither_repaired_nor_handed_in_again_pauses_the_work() {
    let (s, mut e, said, item) = unparsed_at_r0("unparsed-pause", "cut_off", None, Some("cut_off"));
    past_r0(&mut e, item);
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!(at, "R0", "{log}");
    assert!(
        paused.is_some_and(|p| p.contains("could not be parsed as JSON")),
        "{log}"
    );
    assert_eq!(failed_kinds(&e), ["unparsed"], "{log}");
    let (record, _) = repair_record(&e, work);
    assert_eq!(
        record["repair"]["not_tried"], "the last attempt's text was cut off",
        "{record}"
    );
    assert_eq!(record["again"]["handed_in"], false, "{record}");
    let _ = std::fs::remove_dir_all(&s.root);
}
