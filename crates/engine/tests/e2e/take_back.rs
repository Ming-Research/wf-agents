//! What the engine does when the owner takes back words it was about to give an agent.

use super::*;

/// What the owner said to the engineer, and takes back.
const WORDS: &str = "Make it blue instead.";

/// A trigger standing for the owner's taking the words back at the one moment between the engine building a round's
/// message and recording the round: the engine opens the engineer line's session in between, in a write of its own.
const TAKES_BACK: &str = "CREATE TRIGGER the_owner_takes_back AFTER INSERT ON sessions WHEN NEW.line = 'engineer'
    BEGIN DELETE FROM interjections WHERE work = NEW.work AND text = 'Make it blue instead.'; END;";

/// How many rounds a work has had.
fn rounds_of(e: &Engine, work: i64) -> i64 {
    number(e, &format!("SELECT count(*) FROM rounds WHERE work = {work}"))
}

// Words the owner takes back after the engine built the message that gives them and before it recorded the round do not
// reach the agent: the round is not recorded, the work stays where it is, and the next pass builds the message again
// without them and runs the round, so that the work goes on.
#[test]
fn words_taken_back_after_the_message_was_built_do_not_start_the_round() {
    let s = setup("take-back-gap");
    let (mut e, said, item) = start(&s, scripts());
    step_until(&mut e, item, &said, "the work starts", |e| state(e, item).is_some());
    let (work, _, _) = state(&e, item).unwrap();
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', 'owner', ?2)",
            params![work, WORDS],
        )?;
        Ok(())
    })
    .unwrap();
    e.db.conn().execute_batch(TAKES_BACK).unwrap();

    // The pass whose step built the message with the words, and found them gone when it came to record the round.
    let mut gap = None;
    for _ in 0..30 {
        let before = (rounds_of(&e, work), state(&e, item));
        e.step(None).unwrap();
        let left: i64 = number(&e, "SELECT count(*) FROM interjections");
        if left == 0 {
            gap = Some(before);
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    let before = gap.unwrap_or_else(|| panic!("no round was given the words:\n{log}"));
    assert_eq!((rounds_of(&e, work), state(&e, item)), before, "{log}");

    // The next passes run the round, and it was given nothing of the words.
    e.db.conn().execute_batch("DROP TRIGGER the_owner_takes_back").unwrap();
    step_until(&mut e, item, &said, "the round runs", |e| rounds_of(e, work) > before.0);
    let given: i64 = number(
        &e,
        &format!("SELECT count(*) FROM rounds r, json_each(r.input, '$.engine.interjections') WHERE r.work = {work}"),
    );
    let told: i64 = number(
        &e,
        &format!("SELECT count(*) FROM rounds WHERE work = {work} AND instr(input, '{WORDS}') > 0"),
    );
    assert_eq!((given, told), (0, 0), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
