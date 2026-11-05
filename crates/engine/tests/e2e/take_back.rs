//! What the engine does when the owner takes back words it was about to give an agent.

use super::*;

/// What the owner said to the engineer, and takes back.
const WORDS: &str = "Make it blue instead.";

/// A trigger standing for the owner's taking the words back at the one moment between the engine building a round's
/// message and recording the round: when the engineer line has no session going on, the engine opens one in a write of
/// its own in between.
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
    // The work is due at its first round of the engineer line, with words said to the engineer and no session going on
    // for the line, so that the engine opens one after it built the message.
    step_to(&mut e, item, "R1");
    let (work, _, _) = state(&e, item).unwrap();
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', 'owner', ?2)",
            params![work, WORDS],
        )?;
        tx.execute(
            "UPDATE sessions SET ended = 1 WHERE work = ?1 AND line = 'engineer' AND ended IS NULL",
            [work],
        )?;
        Ok(())
    })
    .unwrap();
    e.db.conn().execute_batch(TAKES_BACK).unwrap();
    let before = (rounds_of(&e, work), state(&e, item));

    // The pass whose step built the message with the words, and found them gone when it came to record the round.
    e.step(None).unwrap();

    let log = said.lock().unwrap().join("\n");
    let left: i64 = number(&e, "SELECT count(*) FROM interjections");
    assert_eq!(left, 0, "the words were not taken back in the gap:\n{log}");
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

/// The same moment, for words that came with an image: the owner's taking them back deletes the image too.
const TAKES_BACK_WITH_IMAGE: &str = "CREATE TRIGGER the_owner_takes_back_an_image AFTER INSERT ON sessions
    WHEN NEW.line = 'engineer'
    BEGIN
        DELETE FROM attachments WHERE interjection IN
            (SELECT id FROM interjections WHERE work = NEW.work AND text LIKE 'Make it blue instead.%');
        DELETE FROM interjections WHERE work = NEW.work AND text LIKE 'Make it blue instead.%';
    END;";

// Words that came with an image, taken back after the engine built the message and before it read the image, are the
// same case as words without one: the round does not start and the work is not stopped as a fault.
#[test]
fn words_with_an_image_taken_back_after_the_message_was_built_do_not_stop_the_work() {
    let s = setup("take-back-gap-image");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "R1");
    let (work, _, _) = state(&e, item).unwrap();
    let with_images = api::WorkSaid {
        text: WORDS.into(),
        speaker: "owner".into(),
        relayed: None,
        to: None,
        question: None,
        design_card: None,
        images: two_images(),
    };
    api::say_to_work(&mut e.db, work, &with_images).unwrap();
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "UPDATE sessions SET ended = 1 WHERE work = ?1 AND line = 'engineer' AND ended IS NULL",
            [work],
        )?;
        Ok(())
    })
    .unwrap();
    e.db.conn().execute_batch(TAKES_BACK_WITH_IMAGE).unwrap();
    let before = (rounds_of(&e, work), state(&e, item));

    // The pass that built the message with the words and their image, and found both gone.
    let stepped = e.step(None);
    let log = said.lock().unwrap().join("\n");
    let left: i64 = number(&e, "SELECT count(*) FROM interjections");
    assert_eq!(left, 0, "the words were not taken back in the gap:\n{log}");
    stepped.unwrap_or_else(|x| panic!("the pass failed on the image taken back with its words: {x}\n{log}"));
    assert_eq!((rounds_of(&e, work), state(&e, item)), before, "{log}");
    let paused: i64 = number(&e, &format!("SELECT paused IS NOT NULL FROM works WHERE id = {work}"));
    assert_eq!(paused, 0, "the work was stopped as a fault:\n{log}");

    // The next passes run the round.
    e.db.conn()
        .execute_batch("DROP TRIGGER the_owner_takes_back_an_image")
        .unwrap();
    step_until(&mut e, item, &said, "the round runs", |e| rounds_of(e, work) > before.0);
    let _ = std::fs::remove_dir_all(&s.root);
}
