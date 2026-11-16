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
        added: false,
    };
    assert_eq!(told.outputs[&BlockId::from("opinions")], standing);
    let opinion = w.person_writes("opinions");
    let told = decide::told(&w.flow, &w.records, review).expect("the run started");
    let standing = Standing {
        told: None,
        latest: Some(opinion),
        value: None,
        added: false,
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
    // A block the cell writes, put in so that the round does not write over it, runs nothing: the return need not
    // name it as dealt with to be held to what the cell must write (design document `data-and-context`, 12).
    assert_eq!(told.ran_by, vec![BlockId::from("code")]);
    let _ = code;
}

/// A maker of the dialogue `opinions`: run by the plan and by the dialogue, it writes the code and its answers.
fn maker(id: &str) -> Cell {
    let mut c = cell(id, &["plan", "opinions"], &["code", "opinions"]);
    c.dialogues = BTreeMap::from([("opinions".into(), vec!["code".into()])]);
    c
}

// The next word is owed for a point (design document `data-and-context`, 41): a version of the dialogue the other side
// wrote, new to the maker. Another maker's answer is none; and a doubt, which has no verdict, is open while its latest
// version is the other side's.
#[test]
fn a_round_is_told_a_point_in_its_dialogue_only_where_the_other_side_wrote_one_new_to_it() {
    let doubt = cell("doubt", &["code"], &["opinions"]);
    let mut w = World::with_gate(vec![maker("implement"), maker("deep"), doubt]);
    w.person_writes("plan");
    w.run("implement", &[("code", Matters), ("opinions", Matters)]);
    // The other maker's answer runs the deep one, and is no point to it.
    let deep = w.start("deep");
    let told = decide::told(&w.flow, &w.records, deep).expect("the run started");
    assert!(told.ran_by.contains(&BlockId::from("opinions")), "{:?}", told.ran_by);
    assert_eq!(told.points, Vec::<BlockId>::new());
    w.finish("deep", &[("code", Matters)]);
    // The doubter's point is one to the maker it is new to.
    w.run("doubt", &[("opinions", Matters)]);
    let implement = w.start("implement");
    let told = decide::told(&w.flow, &w.records, implement).expect("the run started");
    assert_eq!(told.points, vec![BlockId::from("opinions")]);
}

// A judge's dialogue is open while its verdict does not pass (design document `data-and-context`, 41): a point the
// judge has passed over since is none the maker owes a word for.
#[test]
fn a_judge_s_point_is_one_to_its_maker_while_the_verdict_fails_and_none_once_it_passes() {
    let mut review = cell("review", &["plan", "code"], &["verdict", "opinions"]);
    let verdict = decide::Verdict {
        fails: "no".to_string(),
        dialogue: "opinions".into(),
    };
    review.verdicts = BTreeMap::from([("verdict".into(), verdict)]);
    let mut w = World::with_gate(vec![review, maker("implement")]);
    w.person_writes("plan");
    w.run("review", &[("verdict", Value("no")), ("opinions", Matters)]);
    let mut open = w.clone();
    let run = open.start("implement");
    let told = decide::told(&open.flow, &open.records, run).expect("the run started");
    assert_eq!(told.points, vec![BlockId::from("opinions")]);
    // The plan changes before the maker runs, and the judge passes it.
    w.person_writes("plan");
    w.run("review", &[("verdict", Value("yes"))]);
    let run = w.start("implement");
    let told = decide::told(&w.flow, &w.records, run).expect("the run started");
    assert!(told.ran_by.contains(&BlockId::from("opinions")), "{:?}", told.ran_by);
    assert_eq!(told.points, Vec::<BlockId>::new());
}
