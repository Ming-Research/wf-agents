//! `wf-agents run` starting: what keeps it from starting, and going on from the store when started again.

use std::net::{Ipv4Addr, TcpListener};

use crate::{Engine, Place, SCRIPT, run};

#[test]
fn a_script_that_names_what_the_flow_lacks_keeps_the_engine_from_starting() {
    let place = Place::new("[[flows.trial.rounds]]\ncell = \"bild\"\n");
    let out = run(&place.data(), &place.script()).output().expect("the command runs");
    assert!(!out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        format!(
            "{}: flows.trial.rounds[0]: the flow trial has no cell bild\n",
            place.script().display()
        )
    );
}

#[test]
fn the_engine_does_not_start_without_its_data_directory() {
    let place = Place::new(SCRIPT);
    let data = place.dir.path().join("nowhere");
    let out = run(&data, &place.script()).output().expect("the command runs");
    assert!(!out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        format!(
            "{}: no such directory: the data directory is made before the engine runs in it\n",
            data.display()
        )
    );
}

#[test]
fn a_command_with_no_engine_to_talk_to_says_so() {
    let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .args(["list", "--port", &port.to_string()])
        .output()
        .expect("the command runs");
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.starts_with(&format!("No engine answers at 127.0.0.1:{port}: ")),
        "{said}"
    );
}

#[test]
fn started_again_the_engine_goes_on_from_the_store() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start(&place);
    engine.says(&[
        "add",
        "--title",
        "Greet the user",
        "--flow",
        "trial",
        "--write",
        "request=Please.",
    ]);
    engine.says(&["resume", "1"]);
    engine.until(&["show", "1"], |line| {
        line.starts_with("Now: waits at plan for the owner")
    });
    drop(engine);

    let engine = Engine::start(&place);
    assert_eq!(
        engine.says(&["list"]),
        "Item 1 \"Greet the user\", work 1 on trial: waits at plan for the owner to answer call 2: \"Should the greeting \
         name the user?\"\n"
    );
    engine.says(&["answer", "1", "2", "Yes."]);
    engine.until(&["list"], |line| line.ends_with(": merged"));
}
