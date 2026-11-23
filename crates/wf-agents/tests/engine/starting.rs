//! `wf-agents run` starting: what keeps it from starting, and going on from the store when started again.

use std::net::{Ipv4Addr, TcpListener};

use crate::{Engine, Place, SCRIPT, plain, plain_line, run, run_on, run_plain_on, trial};

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

// A directory that is no data directory is not made one by being named: with no store in it, nothing is made unless
// the engine is told the directory is new, whatever else the directory holds.
#[test]
fn a_directory_that_holds_no_store_is_refused_without_new_and_nothing_is_made_in_it() {
    let place = Place::new(SCRIPT);
    let data = place.data();
    // What a data directory holds before its first start (git in it, ignoring the worktrees), and what none holds.
    std::fs::create_dir(data.join(".git")).unwrap();
    std::fs::write(data.join(".gitignore"), "worktrees/\n").unwrap();
    std::fs::write(data.join("notes.txt"), "Mine.\n").unwrap();
    for held in [true, false] {
        if !held {
            std::fs::remove_dir_all(data.join(".git")).unwrap();
            std::fs::remove_file(data.join(".gitignore")).unwrap();
            std::fs::remove_file(data.join("notes.txt")).unwrap();
        }
        let out = run_plain_on(&data, &trial(), &place.script(), 0)
            .output()
            .expect("the command runs");
        assert!(!out.status.success());
        assert_eq!(
            String::from_utf8_lossy(&out.stderr),
            format!(
                "{}: the directory holds no store, and no engine makes one in a directory it was not told is new: \
                 start the engine with --new to make the data directory's store here\n",
                data.display()
            )
        );
        assert!(!data.join("store.sqlite").exists() && !data.join("engine.lock").exists());
    }
}

// Told that a directory is new where it holds a store, the engine refuses before it makes anything: it reads only
// whether the store's file is there, so an empty file stands for the store and no engine is started. That `--new` makes
// the store is shown by the first start of every other test of the engine, which gives it where the directory holds none
// (`run_on`), and that a directory with a store is run in without it by every start after the first.
#[test]
fn a_directory_that_holds_a_store_is_refused_new_and_nothing_is_made_in_it() {
    let place = Place::new(SCRIPT);
    let data = place.data();
    std::fs::write(data.join("store.sqlite"), "").unwrap();
    let out = run_plain_on(&data, &trial(), &place.script(), 0)
        .arg("--new")
        .output()
        .expect("the command runs");
    assert!(!out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        format!(
            "{}: the data directory holds a store already: start the engine without --new\n",
            data.display()
        )
    );
    assert!(!data.join("engine.lock").exists());
    assert_eq!(
        std::fs::metadata(data.join("store.sqlite")).unwrap().len(),
        0,
        "the store's file is left as it was"
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
                a round but to answer a note, until `wf-agents resume --all`";
    assert_eq!(
        plain(&engine.says(&["list"])),
        format!(
            "{held}; 0 of 2 slots taken.\n#1 (project script) \"Greet the user\" at level 1, W1 on trial: waits at plan for the owner to \
             answer call 2: its run reached its limit, 2 returns having been sent back\n"
        )
    );
    // `wait` says first that the engine waits for the owner.
    let said = engine.says(&["wait", "--for", "0"]);
    assert!(said.starts_with(&format!("{held}, for ")), "{said}");
    // The answer is taken, and the plan's next round waits for the owner to let everything on.
    engine.says(&["let-go", "1", "--call", "2"]);
    engine.until(&["list"], |line| {
        plain_line(line).ends_with("held back at plan by the pause of everything")
    });
    assert!(engine.says(&["resume", "--all"]).starts_with("Works go on; "));
    engine.until(&["list", "--all"], |line| plain_line(line).ends_with(": merged"));
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
        plain(&engine.says(&["list"])),
        "Works go on; 0 of 2 slots taken.\n#1 (project script) \"Greet the user\" at level 1, W1 on trial: paused at plan\n"
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
