//! The commands that manage items, against a running engine.

use crate::{Engine, Place, SCRIPT};

#[test]
fn an_item_is_added_let_on_answered_and_merged_from_the_command_line() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start(&place);
    assert_eq!(engine.says(&["list"]), "Works go on; 0 of 2 slots taken.\nNo items.\n");
    assert_eq!(
        engine.says(&[
            "add",
            "--title",
            "Greet the user",
            "--flow",
            "trial",
            "--project",
            "script",
            "--write",
            "request=Greet the user, please."
        ]),
        "Added item 1 with its work 1, paused: let it on with `wf-agents resume 1`.\n"
    );
    assert_eq!(
        engine.says(&["list"]),
        "Works go on; 0 of 2 slots taken.\n#1 \"Greet the user\" at level 1, W1 on trial: paused\n"
    );
    assert_eq!(engine.says(&["resume", "1"]), "Work 1 goes on.\n");
    let now = engine.until(&["show", "1"], |line| line.starts_with("Now: waits"));
    assert_eq!(
        now,
        "Now: waits at plan for the owner to answer call 2: \"Should the greeting name the user?\"."
    );
    let shown = engine.says(&["show", "1"]);
    assert!(
        shown.starts_with("Work 1 of item 1 \"Greet the user\" at level 1, on the flow trial.\n"),
        "{shown}"
    );
    // The plan's return asked, which ended its run: it is to run again once the owner answers.
    assert!(
        shown.contains("\n  plan: runs again, its record being run 1\n"),
        "{shown}"
    );
    assert!(
        shown.contains("\n  request: 1 version, the latest 23 characters, by a person\n"),
        "{shown}"
    );
    assert!(
        shown.contains("\n  run 1 of plan: 1 round, calls 2, finished"),
        "{shown}"
    );

    assert_eq!(
        engine.says(&["result", "1", "2", "Yes, by name."]),
        "Call 2 of work 1 has its result.\n"
    );
    engine.until(&["list", "--all"], |line| {
        line == "#1 \"Greet the user\" at level 1, W1 on trial: merged"
    });
    // What ended is left out of a list unless all are asked for, and counted.
    assert_eq!(
        engine.says(&["list"]),
        "Works go on; 0 of 2 slots taken.\nNo item that has not ended.\nAnd 1 item that ended, merged or cancelled: \
         wf-agents list --all\n"
    );
    assert_eq!(
        engine.refuses(&["result", "1", "2", "No."]),
        "Call 2 has its result already.\n"
    );
    // A work that merged is not cancelled, and is not said to be.
    assert_eq!(
        engine.refuses(&["cancel", "1", "--why", "No longer wanted."]),
        "Work 1 is not cancelled: it has merged.\n"
    );
}

#[test]
fn an_item_is_added_at_the_level_it_is_given_and_one_the_engine_lacks_is_refused() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start(&place);
    let add = |level: &'static str| {
        [
            "add",
            "--title",
            "Greet the user",
            "--flow",
            "trial",
            "--project",
            "script",
            "--level",
            level,
        ]
    };
    assert_eq!(
        engine.refuses(&add("2")),
        "The item's level: 2 is no level: the levels are 1.\n"
    );
    assert_eq!(engine.says(&["list"]), "Works go on; 0 of 2 slots taken.\nNo items.\n");
    engine.says(&add("1"));
    // Added with no request, nothing runs its first cell, and only the review makes the merge's precondition hold: the
    // work is stalled, and shown as the pause it was added with holds it.
    assert_eq!(
        engine.says(&["list"]),
        "Works go on; 0 of 2 slots taken.\n#1 \"Greet the user\" at level 1, W1 on trial: paused\n"
    );
    // An item is put at another level as it is added, and refused alike.
    assert_eq!(
        engine.says(&["set-level", "1", "1", "--why", "It is easy."]),
        "#1 was at level 1 already.\n"
    );
    assert_eq!(
        engine.refuses(&["set-level", "1", "2", "--why", "It is hard."]),
        "The item's level: 2 is no level: the levels are 1.\n"
    );
    assert_eq!(
        engine.refuses(&["set-level", "7", "1", "--why", "It is easy."]),
        "There is no item #7.\n"
    );
}

#[test]
fn a_work_is_paused_written_let_go_and_cancelled_from_the_command_line() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start(&place);
    let request = place.dir.path().join("request.md");
    std::fs::write(&request, "Greet the user, please.").expect("the request is written");
    let from_file = format!("request={}", request.display());
    engine.says(&[
        "add",
        "--title",
        "Greet the user",
        "--flow",
        "trial",
        "--project",
        "script",
        "--write-file",
        &from_file,
    ]);
    assert_eq!(
        engine.says(&[
            "add",
            "--title",
            "Say goodbye",
            "--flow",
            "trial",
            "--project",
            "script",
            "--depends-on",
            "1",
            "--write",
            "request=Bye."
        ]),
        "Added item 2 with its work 2, paused: let it on with `wf-agents resume 2`.\n"
    );
    // A precondition holds the work before its cell whether or not it is paused, and the items it depends on, not the
    // owner, can make it hold.
    let waits = "waits at plan for the outside (dependencies): Every item this one depends on has merged";
    let list = engine.says(&["list"]);
    assert!(
        list.ends_with(&format!(
            "#2 \"Say goodbye\" at level 1, W2 on trial, after #1: {waits}\n"
        )),
        "{list}"
    );
    assert_eq!(engine.says(&["resume", "2"]), "Work 2 goes on.\n");
    let shown = engine.says(&["show", "2"]);
    assert!(shown.contains(&format!("\nNow: {waits}.\nPaused: no.\n")), "{shown}");
    assert_eq!(engine.says(&["pause", "2"]), "Work 2 is paused.\n");
    assert_eq!(
        engine.says(&["write", "2", "opinions", "--text", "Keep it short."]),
        "Wrote opinions for work 2.\n"
    );
    assert_eq!(
        engine.says(&["let-go", "2", "build"]),
        "Cell build of work 2 goes on.\n"
    );
    assert_eq!(
        engine.says(&["cancel", "2", "--why", "No longer wanted."]),
        "Work 2 is cancelled.\n"
    );
    engine.until(&["list", "--all"], |line| {
        line == "#2 \"Say goodbye\" at level 1, W2 on trial, after #1: cancelled"
    });

    assert_eq!(
        engine.refuses(&["write", "2", "opinions"]),
        "Nothing is given to write: write takes --text, --file or --value.\n"
    );
    assert_eq!(engine.refuses(&["show", "9"]), "There is no work 9.\n");
    assert_eq!(
        engine.refuses(&["write", "2", "plan", "--text", "Mine."]),
        "People do not write the block plan.\n"
    );
    assert_eq!(
        engine.refuses(&[
            "add",
            "--title",
            "Other",
            "--flow",
            "trial",
            "--project",
            "script",
            "--write",
            "request"
        ]),
        "--write takes BLOCK=WHAT, and was given \"request\"\n"
    );
}
