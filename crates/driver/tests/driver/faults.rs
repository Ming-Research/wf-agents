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
        sources: names(&["answer", "asked", DEPENDENCIES, "main", "session", "start", "tree"]),
        acts: names(&["start", "check", "merge", "deliver"]),
        tools: names(&["query", "run_tests"]),
        fields: names(&["questions", "reply"]),
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
