//! `wf-agents run` starting: what keeps it from starting, and going on from the store when started again.

use std::io::{BufRead, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::PathBuf;

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
fn a_flow_the_program_does_not_carry_keeps_the_engine_from_starting_and_changes_nothing() {
    let place = Place::new(SCRIPT);
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .arg("run")
        .arg("--data")
        .arg(place.data())
        .args(["--program-flow", "nowhere", "--port", "0"])
        .arg("--script")
        .arg(place.script())
        .output()
        .expect("the command runs");
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.starts_with("no flow nowhere is carried by this program, which carries "),
        "{said}"
    );
    assert!(out.stdout.is_empty());
    assert!(
        !place.data().join("flows").exists() && !place.data().join("flows.new").exists(),
        "the flows' directory was made"
    );
}

#[test]
fn flows_of_directories_and_the_programs_own_are_not_given_together() {
    let place = Place::new(SCRIPT);
    let out = run(&place.data(), &place.script())
        .args(["--program-flow", "anything"])
        .output()
        .expect("the command runs");
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("cannot be used with"), "{said}");
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
fn started_again_on_a_work_going_on_the_engine_holds_everything_until_the_owner_lets_it_on() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start(&place);
    engine.says(&[
        "add",
        "--title",
        "Greet the user",
        "--flow",
        "trial",
        "--project",
        "script",
        "--write",
        "request=Please.",
    ]);
    engine.says(&["resume", "1"]);
    engine.until(&["show", "1"], |line| {
        line.starts_with("Now: waits at plan for the owner")
    });
    drop(engine);

    let engine = Engine::start(&place);
    let held = "The engine started with works going on and holds every work until the owner has looked: none begins \
                a round but to answer a note, until `wf-agents resume --all`";
    assert_eq!(
        engine.says(&["list"]),
        format!(
            "{held}; 0 of 2 slots taken.\n#1 \"Greet the user\" at level 1, project script, W1 on trial: waits at plan for the owner to \
             answer call 2: its run reached its limit, 2 returns having been sent back\n"
        )
    );
    // `wait` says first that the engine waits for the owner.
    let said = engine.says(&["wait", "--for", "0"]);
    assert!(said.starts_with(&format!("{held}, for ")), "{said}");
    // The answer is taken, and the plan's next round waits for the owner to let everything on.
    engine.says(&["result", "1", "2", "Yes."]);
    engine.until(&["list"], |line| {
        line.ends_with("held back at plan by the pause of everything")
    });
    assert!(engine.says(&["resume", "--all"]).starts_with("Works go on; "));
    engine.until(&["list", "--all"], |line| line.ends_with(": merged"));
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
        "--project",
        "script",
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
        "Works go on; 0 of 2 slots taken.\n#1 \"Greet the user\" at level 1, project script, W1 on trial: paused at plan\n"
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
    std::fs::write(&merge, text.replacen("watches = [\"code\"]", "watches = [\"kode\"]", 1)).unwrap();
    let out = run_on(&place.data(), &flow, &place.script(), 0)
        .output()
        .expect("the command runs");
    assert!(!out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        format!(
            "{}/merge.toml watches[0]: the block kode is not declared in flow.toml\n",
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

/// The names of the flows the repository defines: each directory of `flow/` that holds a flow's file, in order.
fn defined_flows() -> Vec<String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    let mut names: Vec<String> = std::fs::read_dir(&root)
        .expect("flow/ is read")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.join("flow.toml").is_file())
        .map(|path| path.file_name().expect("a name").to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// What `GET path` on the engine's port answers: the JSON of its body.
fn get_json(port: u16, path: &str) -> serde_json::Value {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).expect("the engine is reached");
    write!(stream, "GET {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n\r\n").expect("the request is sent");
    let mut answer = String::new();
    stream.read_to_string(&mut answer).expect("the answer is read");
    let (head, body) = answer.split_once("\r\n\r\n").expect("an answer with a body");
    assert!(
        head.starts_with("HTTP/1.1 200") || head.starts_with("HTTP/1.0 200"),
        "{head}"
    );
    serde_json::from_str(body).expect("the body is JSON")
}

#[test]
fn the_engine_runs_the_flows_the_program_carries_in_the_order_named_and_says_its_version() {
    // Reference: the repository's `flow/` read from the disk by the test itself, and `wf-agents --version`, which says
    // the same version as the engine's start line.
    let place = Place::new("[took]\nms = 20\n");
    let mut flows = defined_flows();
    assert!(flows.len() >= 2, "the repository defines at least two flows: {flows:?}");
    // The last by name first, so that the flow shown is not the first the directory lists.
    flows.reverse();
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_wf-agents"));
    command.arg("run").arg("--data").arg(place.data());
    for name in &flows {
        command.args(["--program-flow", name]);
    }
    command.arg("--script").arg(place.script()).args(["--port", "0"]);
    let mut engine = Engine::start_with(command);
    let said_version = {
        let mut line = String::new();
        engine._output.read_line(&mut line).expect("the engine's second line");
        line
    };
    let version = std::process::Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .arg("--version")
        .output()
        .expect("the command runs");
    assert!(version.status.success());
    let version = String::from_utf8(version.stdout).unwrap();
    assert!(
        version.starts_with(&format!("wf-agents {} (", env!("CARGO_PKG_VERSION"))) && version.trim_end().ends_with(')'),
        "{version:?}"
    );
    assert_eq!(
        said_version, version,
        "the start line and `--version` say different things"
    );
    // The files the engine runs on are the program's, taken out of the repository's `flow/` of the build.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    for name in &flows {
        assert_eq!(
            std::fs::read(place.data().join("flows").join(name).join("flow.toml"))
                .expect("the flow's file is taken out"),
            std::fs::read(root.join(name).join("flow.toml")).unwrap(),
            "{name}"
        );
    }
    assert_eq!(
        std::fs::read(place.data().join("flows").join(flow::agents::RULES_FILE))
            .expect("what is said to every agent is taken out beside the flows"),
        std::fs::read(root.join(flow::agents::RULES_FILE)).unwrap()
    );
    // The flow page shows the first named.
    let shown = get_json(engine.port, "/api/v1/flow");
    assert_eq!(shown["state"], "compiled", "{shown}");
    assert_eq!(shown["flow"]["name"], flows[0].as_str(), "{shown}");
    // Every flow named runs: an item is added on each.
    for name in &flows {
        let said = engine.says(&[
            "add",
            "--title",
            "Try it",
            "--flow",
            name,
            "--project",
            "script",
            "--write",
            "background=What it is to try.",
        ]);
        assert!(said.starts_with("Added item"), "{name}: {said}");
    }
    drop(engine);
}

#[test]
fn the_version_the_program_says_is_its_own_and_there_is_no_version_command() {
    let said = std::process::Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .arg("--version")
        .output()
        .expect("the command runs");
    assert!(said.status.success());
    let said = String::from_utf8(said.stdout).unwrap();
    let inner = said
        .strip_prefix(&format!("wf-agents {} (", env!("CARGO_PKG_VERSION")))
        .and_then(|rest| rest.strip_suffix(")\n"))
        .unwrap_or_else(|| panic!("{said:?} is not `wf-agents <version> (<commit>)`"));
    assert!(!inner.is_empty() && !inner.contains('\n'), "{said:?}");
    let other = std::process::Command::new(env!("CARGO_BIN_EXE_wf-agents"))
        .arg("version")
        .output()
        .expect("the command runs");
    assert!(!other.status.success(), "a `version` command exists");
}
