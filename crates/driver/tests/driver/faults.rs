//! What the driver needs of a flow: each text it fills, given in the `writes` of the block it writes, so that nothing
//! it writes is empty. A flow that lacks one is not run, and the fault names the file that holds the block's words.

use std::fs;
use std::path::{Path, PathBuf};

use driver::DEPENDENCIES;
use flow::{Compiled, Has};
use tempfile::TempDir;

/// The repository's flows and what is beside them, copied into a directory of the test's own.
fn copied() -> TempDir {
    fn copy(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                copy(&path, &to.join(entry.file_name()));
            } else {
                fs::copy(&path, to.join(entry.file_name())).unwrap();
            }
        }
    }
    let dir = TempDir::new().unwrap();
    copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow"), dir.path());
    dir
}

/// The engineer's flow under this directory, compiled against what the engine has for it.
fn engineer(root: &Path) -> Compiled {
    let source = flow::read(&root.join("engineer")).expect("the engineer's flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = Has {
        sources: names(&[
            "answer",
            "asked",
            "words",
            DEPENDENCIES,
            "main",
            "session",
            "start",
            "head",
            "tree",
        ]),
        acts: names(&["start", "check", "merge", "deliver"]),
        tools: names(&["query", "run_tests"]),
        details: names(&["files"]),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the engineer's flow compiles")
}

/// The file beside the flows, with the line that begins so taken out.
fn without(root: &Path, file: &str, begins: &str) -> PathBuf {
    let path = root.join(file);
    let text = fs::read_to_string(&path).unwrap();
    assert!(
        text.lines().any(|l| l.starts_with(begins)),
        "{file} has a line {begins}"
    );
    let kept: Vec<&str> = text.lines().filter(|l| !l.starts_with(begins)).collect();
    fs::write(&path, kept.join("\n")).unwrap();
    path
}

#[test]
fn the_repository_s_flows_give_every_text_the_driver_fills() {
    let dir = copied();
    assert_eq!(driver::faults(&engineer(dir.path())), Vec::<String>::new());
    assert_eq!(driver::faults(&super::deliver()), Vec::<String>::new());
}

#[test]
fn a_kind_of_new_session_the_flow_gives_no_words_for_is_a_fault_on_the_file_that_holds_them() {
    let dir = copied();
    without(dir.path(), "triggers/of_a_line/on_session.toml", "lost = ");
    let faults = driver::faults(&engineer(dir.path()));
    // One for the block of each line a cell of which loads its session: the engineer's and the deep researcher's.
    assert_eq!(faults.len(), 2, "{faults:?}");
    for (fault, line) in faults.iter().zip(["deep_researcher", "engineer"]) {
        assert!(fault.contains("triggers/of_a_line/on_session.toml"), "{fault}");
        assert!(fault.contains(&format!("the block {line}_session ")), "{fault}");
        assert!(fault.ends_with("its writes must give lost"), "{fault}");
    }
}

#[test]
fn a_start_the_flow_gives_no_words_for_is_a_fault_on_the_file_that_holds_them() {
    let dir = copied();
    without(dir.path(), "triggers/on_start.toml", "text = ");
    let faults = driver::faults(&engineer(dir.path()));
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(faults[0].contains("triggers/on_start.toml"), "{}", faults[0]);
    assert!(faults[0].ends_with("its writes must give text"), "{}", faults[0]);
}

// The record of what is said, a block of the owner's words that no trigger stands on, gives the words of the cells'
// answers as well as of the owner's entries; the blocks of the owner's words the cells' triggers stand on need neither.
#[test]
fn a_record_of_what_is_said_that_gives_no_words_for_an_answer_is_a_fault_on_the_file_that_holds_them() {
    let dir = copied();
    without(dir.path(), "data/discussion.toml", "reply = ");
    let faults = driver::faults(&engineer(dir.path()));
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(faults[0].contains("data/discussion.toml"), "{}", faults[0]);
    assert!(faults[0].contains("the block discussion "), "{}", faults[0]);
    assert!(faults[0].ends_with("its writes must give reply"), "{}", faults[0]);
}

#[test]
fn a_flow_with_no_words_beside_it_for_the_owner_is_not_run() {
    let dir = copied();
    fs::remove_file(dir.path().join("agents/waits.toml")).unwrap();
    let faults = driver::faults(&engineer(dir.path()));
    // Every reason the engine says to the owner, and every reason it pauses a work with, would be empty.
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(faults[0].contains("../agents/waits.toml"), "{}", faults[0]);
    assert!(faults[0].contains("owner.paused"), "{}", faults[0]);
    assert!(faults[0].contains("paused.let_go"), "{}", faults[0]);
}

#[test]
fn a_flow_written_before_flows_were_laid_out_takes_the_words_beside_it_and_without_them_is_not_run() {
    let flow = "name = \"small\"\norder = [\"merge\"]\n[limits]\nruns = 1\nreturns = 1\ncalls = 1\n[blocks.go]\npeople = true\n";
    let merge = "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\ninputs = [\"go\"]\n";
    let has = Has {
        acts: ["merge".to_string()].into(),
        ..Default::default()
    };
    let source = |files: &[(String, String)]| flow::Source {
        flow: flow.to_string(),
        cells: [("merge".to_string(), merge.to_string())].into(),
        files: files.iter().cloned().collect(),
        ..Default::default()
    };
    let bare = flow::compile(&source(&[]), &has).expect("the flow compiles");
    assert_eq!(driver::faults(&bare).len(), 1, "{:?}", driver::faults(&bare));
    let words = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/agents/waits.toml")).unwrap();
    let beside = flow::compile(&source(&[(flow::waits_file(), words)]), &has).expect("the flow compiles");
    assert_eq!(driver::faults(&beside), Vec::<String>::new());
}
