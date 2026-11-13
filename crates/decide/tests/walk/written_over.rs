//! What a run's round was told, for the checks of its return (`decide::told`): what ran it, which the return may name
//! as dealt with, and of each block it may write the version it was told of and the latest, so that a return that
//! would write over what the round never saw is sent back (plan, D4).

use decide::run::Standing;

use super::*;

#[test]
fn a_round_is_run_by_what_was_new_as_its_run_started_and_by_what_was_put_in_to_it() {
    let mut w = World::with_gate(vec![cell("answer", &["question", "plan", "notes"], &["reply"])]);
    w.person_writes("question");
    w.person_writes("plan");
    let run = w.start("answer");
    let notes = w.person_writes("notes");
    let at = w.tick();
    w.records.put_in.push(PutIn {
        run,
        block: "notes".into(),
        version: notes,
        at,
    });
    let told = decide::told(&w.flow, &w.records, run).expect("the run started");
    assert_eq!(
        told.ran_by,
        vec![BlockId::from("question"), "plan".into(), "notes".into()]
    );
}

#[test]
fn a_block_written_by_another_since_the_round_was_told_of_it_stands_at_a_later_version() {
    let (mut w, _, code) = judged();
    let review = w.start("review");
    let told = decide::told(&w.flow, &w.records, review).expect("the run started");
    let standing = Standing {
        told: None,
        latest: None,
        value: None,
    };
    assert_eq!(told.outputs[&BlockId::from("opinions")], standing);
    let opinion = w.person_writes("opinions");
    let told = decide::told(&w.flow, &w.records, review).expect("the run started");
    let standing = Standing {
        told: None,
        latest: Some(opinion),
        value: None,
    };
    assert_eq!(told.outputs[&BlockId::from("opinions")], standing);
    assert_eq!(told.ran_by, vec![BlockId::from("code")]);
    // Put in to the round, the version is one it was told of.
    let at = w.tick();
    w.records.put_in.push(PutIn {
        run: review,
        block: "opinions".into(),
        version: opinion,
        at,
    });
    let told = decide::told(&w.flow, &w.records, review).expect("the run started");
    assert_eq!(told.outputs[&BlockId::from("opinions")].told, Some(opinion));
    let _ = code;
}
