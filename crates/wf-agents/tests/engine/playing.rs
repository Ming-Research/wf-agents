//! `wf-agents play`: a script played on the engine `run` makes, on a clock of its own, each item's work judged against
//! the path the script states; what a play refuses, in the words `run` refuses it in; the store it leaves, which `run`
//! serves; and every script kept beside the flows played, so that a change of a flow a script's case no longer takes
//! fails at that change (design document `testing`, 14).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

use store::{Storage, Store, WorkNumber};

use crate::{Engine, Place, run, run_on, trial};

/// The directory beside the flows of the tests, which holds the trial's script in `scripts/`.
fn laid_out() -> PathBuf {
    trial().join("..")
}

/// The directory that holds the repository's flows, and their scripts in `scripts/`.
fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../flow")
}

/// `wf-agents play` of the script on these flows over the data directory, as it ends.
fn play(data: &Path, flows: &[PathBuf], script: &Path) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wf-agents"));
    command.arg("play").arg("--data").arg(data).arg("--script").arg(script);
    for flow in flows {
        command.arg("--flow").arg(flow);
    }
    command.output().expect("the command runs")
}

/// What the command printed, and on its error stream.
fn said(out: &Output) -> (String, String) {
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// The trial's script, as its file is.
fn trial_script() -> String {
    std::fs::read_to_string(laid_out().join("scripts/trial.toml")).expect("the trial's script is read")
}

/// What a run says as it starts of a script that names items, which it checks and carries none of out.
fn none_carried_out(script: &Path) -> String {
    format!(
        "{}: its items, their acts and their paths are checked, and none is carried out: a run adds no item a script \
         names",
        script.display()
    )
}

// The trial's script plays its case, and the play says the work took its path, and nothing of a run's carrying none
// of its items out; served after by `run` with the same script, the work shows as it ended. Under a second alone; no
// other test plays a script through the command.
#[test]
fn the_trial_played_takes_its_path_and_run_serves_the_store_it_left() {
    let place = Place::new(&trial_script());
    let out = play(&place.data(), &[trial()], &place.script());
    let (printed, errors) = said(&out);
    assert!(out.status.success(), "{printed}{errors}");
    assert_eq!(printed, "items[0], W1: took its path\n");
    assert!(!errors.contains(&none_carried_out(&place.script())), "{errors}");
    let engine = Engine::start(&place);
    engine.until(&["list", "--all"], |line| {
        line == "#1 \"Greet the user\" at level 1, project script, W1 on trial: merged"
    });
}

// A path with one step wrong fails the play, which names that step with what the records hold there, within its bound.
#[test]
fn a_path_with_one_step_wrong_fails_the_play_naming_that_step() {
    let wrong = trial_script().replacen(
        "    { runs = \"review\" },\n    { runs = \"build\" },",
        "    { runs = \"review\" },\n    { runs = \"merge\" },",
        1,
    );
    assert_ne!(wrong, trial_script(), "the step is changed");
    let place = Place::new(&wrong);
    let out = play(&place.data(), &[trial()], &place.script());
    let (printed, errors) = said(&out);
    assert!(!out.status.success(), "{printed}");
    assert_eq!(
        printed, "items[0], W1: left its path at step 8, { runs = \"merge\" }: the records hold { runs = \"build\" }\n",
        "{errors}"
    );
}

// A round entry of the script that no round of a play that went as the script says took is named, where the script
// holds it, and the play still passes: whether the entry is left over is the script's to say.
#[test]
fn a_round_entry_no_round_took_is_named_by_a_play_that_passes() {
    let place = Place::new(&format!(
        "{}\n[[flows.trial.rounds]]\ncell = \"review\"\nrun = 3\n",
        trial_script()
    ));
    let out = play(&place.data(), &[trial()], &place.script());
    let (printed, errors) = said(&out);
    assert!(out.status.success(), "{printed}{errors}");
    assert_eq!(
        printed,
        "items[0], W1: took its path\n\
         flows.trial.rounds[5]: no round of the play took it, round 1 of the cell review's run 3\n"
    );
}

// What `run` refuses of a script, `play` refuses in the same words: one function reads and checks what a script
// carries out, and makes the engine for both.
#[test]
fn a_script_run_refuses_play_refuses_in_the_same_words() {
    let place = Place::new(&format!(
        "{}\n[[flows.trial.rounds]]\ncell = \"bild\"\n",
        trial_script()
    ));
    let refused = |out: Output| {
        assert!(!out.status.success());
        let (_, errors) = said(&out);
        (errors.lines())
            .filter(|line| line.contains("flows.trial.rounds"))
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    let by_run = refused(run(&place.data(), &place.script()).output().expect("the command runs"));
    let by_play = refused(play(&place.data(), &[trial()], &place.script()));
    assert_eq!(
        by_run,
        vec![format!(
            "{}: flows.trial.rounds[5]: the flow trial has no cell bild",
            place.script().display()
        )]
    );
    assert_eq!(by_play, by_run);
}

// A run of a script that names items checks them and carries none of them out, and says so once, as it starts.
#[test]
fn a_run_of_a_script_with_items_says_once_that_it_carries_none_of_them_out() {
    let place = Place::new(&trial_script());
    // On a port that is taken, the run stops once it has read the script.
    let taken = std::net::TcpListener::bind("127.0.0.1:0").expect("a port is taken");
    let port = taken.local_addr().expect("its address").port();
    let out = (run_on(&place.data(), &trial(), &place.script(), port).output()).expect("the command runs");
    assert!(!out.status.success());
    let (_, errors) = said(&out);
    let notice = none_carried_out(&place.script());
    assert_eq!(errors.lines().filter(|line| *line == notice).count(), 1, "{errors}");
}

// A play writes its store anew, into a data directory that holds nothing.
#[test]
fn a_play_refuses_a_data_directory_that_holds_anything() {
    let place = Place::new(&trial_script());
    std::fs::write(place.data().join("store.sqlite"), "").unwrap();
    let out = play(&place.data(), &[trial()], &place.script());
    let (_, errors) = said(&out);
    assert!(!out.status.success());
    assert_eq!(
        errors,
        format!(
            "{}: the data directory holds files already: a play writes its store into an empty one\n",
            place.data().display()
        )
    );
}

// The engineer's script plays its case on the engineer's flow and takes its path; played twice, on whatever day, its
// store holds the same times, which count from the script's start.
#[test]
fn the_engineers_case_takes_its_path_and_its_times_are_the_same_on_any_day() {
    let script = repository().join("scripts/engineer.toml");
    let times = || {
        let place = Place::new("");
        let out = play(&place.data(), &[repository().join("engineer")], &script);
        let (printed, errors) = said(&out);
        assert!(out.status.success(), "{printed}{errors}");
        let store = Store::open(place.data().join(store::FILE)).expect("the store opens");
        let kept = store.load(WorkNumber(1)).expect("the store has the work");
        kept.moments.iter().map(|m| m.at).collect::<Vec<_>>()
    };
    let first = times();
    assert_eq!(first.first().map(|at| at.0), Some(1_794_268_800_000));
    assert_eq!(times(), first);
}

/// Every script kept in `scripts/` beside the flows in this directory, with the directories of the flows it names.
fn scripts_beside(root: &Path) -> Vec<(PathBuf, Vec<PathBuf>)> {
    let mut scripts = Vec::new();
    for entry in std::fs::read_dir(root.join("scripts")).expect("the scripts are read") {
        let path = entry.expect("an entry of it").path();
        let text = std::fs::read_to_string(&path).expect("the script is read");
        let script = service::Script::read(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let mut named: Vec<&String> = (script.flows.keys())
            .chain(script.items.iter().map(|item| &item.flow))
            .collect();
        named.sort();
        named.dedup();
        let flows = (named.into_iter()).map(|name| root.join(name)).collect();
        scripts.push((path, flows));
    }
    scripts.sort();
    scripts
}

// Every script kept beside the repository's flows, and beside the flows of the tests, is played by the command on the
// flows it names, each item's work takes its path, and every round entry of the script is taken: a change of a flow
// that a script's case no longer takes fails at that change, naming the script, the item and the step, and so does a
// round the case no longer comes to. A second or so alone, the scripts' rounds taking none of the clock's time; the
// only test that plays the scripts kept beside the flows.
#[test]
fn every_script_beside_the_flows_is_played_and_takes_its_paths() {
    let started = Instant::now();
    let mut played = 0;
    for root in [repository(), laid_out()] {
        let scripts = scripts_beside(&root);
        assert!(
            !scripts.is_empty(),
            "no script is kept beside the flows of {}",
            root.display()
        );
        for (script, flows) in scripts {
            let place = Place::new("");
            let out = play(&place.data(), &flows, &script);
            let (printed, errors) = said(&out);
            assert!(out.status.success(), "{}: {printed}{errors}", script.display());
            let untaken: Vec<&str> = (printed.lines())
                .filter(|line| line.contains(": no round of the play took it"))
                .collect();
            assert!(untaken.is_empty(), "{}: {}", script.display(), untaken.join("\n"));
            played += 1;
        }
    }
    println!("{played} scripts played in {:?}", started.elapsed());
}
