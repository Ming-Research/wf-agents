//! The tool `dependency_sources` when the command ends but leaves a process it started running. The commands are
//! `sh -c` stubs, not cargo or npm.

use super::*;

/// Two lock files whose commands end at once and leave a `sleep` of their own behind, which holds the command's output:
/// the first ends with 0 after writing what it fetched, the second with 5.
const LISTED: &str = r#"
[[dependency_sources]]
lock = "left.lock"
into = "target/left"
command = "sh -c 'mkdir -p target/left; sleep 60 & echo $! > target/left/child; echo fetched > target/left/done'"

[[dependency_sources]]
lock = "left-failed.lock"
into = "target/left-failed"
command = "sh -c 'mkdir -p target/left-failed; sleep 60 & echo $! > target/left-failed/child; exit 5'"
"#;

/// Whether a process still stands, by the signal that only asks.
fn alive(pid: &str) -> bool {
    Command::new("kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// The agents as `scripts` makes them, but for the research, which asks for the tool for these lock files first.
fn research_asks(locks: &[&str]) -> BTreeMap<&'static str, Script> {
    let requests: Vec<Value> = locks
        .iter()
        .map(|l| json!({ "tool": "dependency_sources", "lock": l }))
        .collect();
    let mut agents = scripts();
    let research = agents.remove("R1").unwrap();
    agents.insert(
        "R1",
        Box::new(move |r: &Request, n: usize| {
            if n == 1 {
                return (vec![("outcome", "tools")], vec![("outcome.tools", json!(requests.clone()))]);
            }
            research(r, n - 1)
        }),
    );
    agents
}

// A command that ends leaves nothing of what it started running: a process that goes on after the tool gave its
// result would go on writing into the directory the agents read, or be there when the work is over, and the result
// would be read as the directory's final content. Whether the command ended with 0 or not, what it started is stopped
// when it ended (the output the process holds is given up after a wait, so the tool does not hang on it).
#[test]
fn a_process_a_command_leaves_behind_when_it_ends_is_stopped() {
    let s = setup("dependency-sources-left-behind");
    main_moves_on(
        &s.root,
        &[
            (".gitignore", "/target/\n"),
            ("left.lock", "locked\n"),
            ("left-failed.lock", "locked\n"),
        ],
        "Lock files, and what git ignores",
    );
    let project = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    write(&s.data, "projects/p.toml", &format!("{project}\n{LISTED}"));
    git(
        &s.data,
        &["commit", "-q", "-am", "the lock files whose dependencies' source is read"],
    );
    let (mut e, said, item) = start(&s, research_asks(&["left.lock", "left-failed.lock"]));
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (work, _, _) = state(&e, item).unwrap();
    let wt = worktree_of(&e, work);
    let runs = tool_runs(&e, item, "dependency_sources");
    let pids: Vec<String> = ["left", "left-failed"]
        .iter()
        .map(|d| {
            std::fs::read_to_string(wt.join(format!("target/{d}/child")))
                .unwrap_or_default()
                .trim()
                .to_string()
        })
        .collect();
    let standing: Vec<bool> = pids
        .iter()
        .map(|pid| {
            let gone = (0..60).any(|_| {
                if alive(pid) {
                    std::thread::sleep(Duration::from_millis(50));
                    return false;
                }
                true
            });
            !gone
        })
        .collect();
    // What the test started it stops itself, whatever the engine did.
    for pid in pids.iter().filter(|p| !p.is_empty()) {
        let _ = Command::new("kill")
            .args(["-KILL", pid])
            .stderr(std::process::Stdio::null())
            .status();
    }
    assert_eq!(runs.len(), 2, "{runs:?}\n{log}");
    assert_eq!(runs[0].1["sources"]["verdict"], "fetched", "{runs:?}");
    assert_eq!(runs[1].1["sources"]["verdict"], "failed", "{runs:?}");
    assert_eq!(runs[1].1["sources"]["exit"], 5, "{runs:?}");
    assert!(pids.iter().all(|p| !p.is_empty()), "{pids:?}\n{log}");
    assert_eq!(
        standing,
        [false, false],
        "a process the command started outlived it: {pids:?}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}
