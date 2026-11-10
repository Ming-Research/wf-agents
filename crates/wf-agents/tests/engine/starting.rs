//! `wf-agents run` starting: what keeps it from starting, and going on from the store when started again.

use std::net::{Ipv4Addr, TcpListener};

use crate::{Engine, Place, SCRIPT, run, run_on, trial};

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

#[test]
fn a_second_engine_on_a_data_directory_one_drives_does_not_start() {
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
    let out = run(&place.data(), &place.script()).output().expect("the command runs");
    assert!(!out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        format!(
            "{}: an engine already runs on this data directory, and only one may\n",
            place.data().display()
        )
    );
    assert!(
        out.stdout.is_empty(),
        "the second engine served: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    // The first goes on as it was.
    assert_eq!(
        engine.says(&["list"]),
        "Item 1 \"Greet the user\", work 1 on trial: paused\n"
    );
}

#[test]
fn an_engine_whose_port_is_taken_opens_no_store() {
    let place = Place::new(SCRIPT);
    let taken = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = taken.local_addr().unwrap().port();
    let out = run_on(&place.data(), &trial(), &place.script(), port)
        .output()
        .expect("the command runs");
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.starts_with(&format!("127.0.0.1:{port}: ")), "{said}");
    assert!(!place.data().join("store.sqlite").exists(), "the store was opened");
}

#[test]
fn a_flow_that_does_not_compile_keeps_the_engine_from_starting() {
    let place = Place::new(SCRIPT);
    let flow = place.dir.path().join("flow");
    std::fs::create_dir(&flow).unwrap();
    for entry in std::fs::read_dir(trial()).unwrap() {
        let path = entry.unwrap().path();
        std::fs::copy(&path, flow.join(path.file_name().unwrap())).unwrap();
    }
    let merge = flow.join("merge.toml");
    let text = std::fs::read_to_string(&merge).unwrap();
    std::fs::write(&merge, text.replacen("inputs = [\"code\"]", "inputs = [\"kode\"]", 1)).unwrap();
    let out = run_on(&place.data(), &flow, &place.script(), 0)
        .output()
        .expect("the command runs");
    assert!(!out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        format!(
            "{}/merge.toml inputs[0]: the block kode is not declared in flow.toml\n",
            flow.display()
        )
    );
    assert!(!place.data().join("store.sqlite").exists(), "the store was opened");
}

#[test]
fn a_command_whose_engine_stops_answering_once_asked_says_it_may_have_been_taken() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    // Takes the connection and the request, and answers nothing.
    let taker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; 1024];
        let _ = std::io::Read::read(&mut stream, &mut request);
    });
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .args(["pause", "1", "--port", &port.to_string()])
        .output()
        .expect("the command runs");
    taker.join().unwrap();
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    let lost = format!("The engine at 127.0.0.1:{port} stopped answering after it was asked: ");
    assert!(said.starts_with(&lost), "{said}");
    assert!(said.contains("What was asked may have been taken"), "{said}");
}
