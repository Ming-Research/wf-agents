//! `wf-agents run` starting: what keeps it from starting, and going on from the store when started again.

use std::io::{BufRead, BufReader};
use std::net::{Ipv4Addr, TcpListener};
use std::process::Stdio;

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

// A run with a script reads what is written beside its flows, as a run with settings does, and does not start where
// it cannot read it, saying why: the stand-ins find what is wrong with the questions a return asks in its words. Well
// under a second alone; no other test runs the command on flows with no words beside them.
#[test]
fn a_script_run_on_flows_with_no_words_beside_them_does_not_start() {
    let place = Place::new(
        "[[flows.trial.rounds]]\ncell = \"review\"\nhands_in = [{ block = \"verdict\", value = \"yes\" }]\n",
    );
    // The trial flow in a directory of its own, with nothing beside it but what the engine says to the owner.
    let flow = place.dir.path().join("flow");
    std::fs::create_dir(&flow).unwrap();
    for entry in std::fs::read_dir(trial()).unwrap() {
        let path = entry.unwrap().path();
        std::fs::copy(&path, flow.join(path.file_name().unwrap())).unwrap();
    }
    let agents = place.dir.path().join("agents");
    std::fs::create_dir(&agents).unwrap();
    std::fs::copy(trial().join("../agents/waits.toml"), agents.join("waits.toml")).unwrap();
    let mut child = run_on(&place.data(), &flow, &place.script(), 0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the command runs");
    let mut line = String::new();
    let stdout = child.stdout.take().expect("the command's output");
    BufReader::new(stdout)
        .read_line(&mut line)
        .expect("the command's first line");
    let _ = child.kill();
    let out = child.wait_with_output().expect("the command ends");
    assert_eq!(line, "", "the engine started");
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.contains(&format!("{}: ", agents.join("session.toml").display())),
        "{said}"
    );
    assert!(!place.data().join("store.sqlite").exists(), "the store was opened");
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
                a round but for the owner's words or answers, until `wf-agents resume --all`";
    assert_eq!(
        engine.says(&["list"]),
        format!(
            "{held}; 0 of 2 slots taken.\n#1 \"Greet the user\" at level 1, project script, W1 on trial: waits at plan for the owner to \
             answer the question named\n"
        )
    );
    // `wait` says first that the engine waits for the owner.
    let said = engine.says(&["wait", "--for", "0"]);
    assert!(said.starts_with(&format!("{held}, for ")), "{said}");
    // The answer is taken: the plan's run entered for it runs and finishes, as a run that answers the owner does, and
    // the build waits for the owner to let everything on.
    engine.says(&["answer", "1", "named", "9", "1"]);
    engine.until(&["list"], |line| line.ends_with("held back by the pause of everything"));
    let shown = engine.says(&["show", "1"]);
    assert!(shown.contains("\n  run 2 of plan: 1 round, finished"), "{shown}");
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
        "Works go on; 0 of 2 slots taken.\n#1 \"Greet the user\" at level 1, project script, W1 on trial: paused\n"
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
    // Every fault is said at once: the flow's, and that nothing is written beside it, which the flow copied alone lacks.
    let said = String::from_utf8_lossy(&out.stderr);
    let lines: Vec<&str> = said.lines().collect();
    assert_eq!(
        lines[0],
        format!(
            "{}/merge.toml inputs[0]: the block kode is not declared in flow.toml",
            flow.display()
        )
    );
    assert_eq!(lines.len(), 2, "{said}");
    assert!(lines[1].contains("agents/session.toml: "), "{said}");
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

/// A project's repository on its main branch, holding what its declaration names, as the installation's settings point
/// at it.
fn project_repository(repo: &std::path::Path) {
    std::fs::create_dir_all(repo.join("flow")).unwrap();
    std::fs::create_dir_all(repo.join("design")).unwrap();
    let declaration = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/project.toml");
    std::fs::copy(declaration, repo.join("flow/project.toml")).expect("the declaration is copied");
    for path in ["AGENTS.md", "design/constitution.md", "design/README.md"] {
        std::fs::write(repo.join(path), "A text.\n").unwrap();
    }
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args([
                "-c",
                "user.name=Tester",
                "-c",
                "user.email=tester@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "The project"]);
}

// An installation is new once: its data directory is empty, and the engine makes the store as it starts, before
// anything else opens it. The copies of the database, which the settings give the installation, open the store as the
// engine starts, so they meet the store made and migrated, and a store not yet made is no reason for the engine not to
// start. Run with the installation's own settings, as a script's run keeps no copies.
#[test]
fn an_installation_with_its_settings_starts_on_an_empty_data_directory() {
    let place = Place::new(SCRIPT);
    let repo = place.dir.path().join("project");
    project_repository(&repo);
    let sample = include_str!("../../../carry/install.sample.toml");
    let settings = place.dir.path().join("install.toml");
    std::fs::write(
        &settings,
        sample.replace("/path/to/the/repository", &repo.display().to_string()),
    )
    .unwrap();
    assert!(!place.data().join("store.sqlite").exists());
    let flows = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_wf-agents"));
    command
        .arg("run")
        .arg("--data")
        .arg(place.data())
        .arg("--flow")
        .arg(flows.join("engineer"))
        .arg("--flow")
        .arg(flows.join("deliver"))
        .arg("--settings")
        .arg(&settings)
        .args(["--port", "0"]);
    let engine = Engine::start_with(command);
    engine.says(&["list"]);
    assert!(place.data().join("store.sqlite").exists());
}
