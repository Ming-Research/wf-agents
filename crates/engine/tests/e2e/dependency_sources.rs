//! The tool `dependency_sources`: the lock files a project lists, the command each runs in the work's worktree, and what
//! the tool gives. The commands are `sh -c` stubs that read the lock file and write what a real one would put in the
//! directory, not cargo or npm.

use super::*;

/// What the project lists, as `[[dependency_sources]]` tables. The first two commands copy the lock file into the
/// directory, and the first counts its runs; the third writes into a directory git does not ignore; the fourth fails,
/// as a fetch that finds no such package does; the fifth hangs, with a process of its own that it leaves behind, and has
/// a time limit of one second.
const LISTED: &str = r#"
[[dependency_sources]]
lock = "Cargo.lock"
into = "target/dependency-sources"
command = "sh -c 'mkdir -p target/dependency-sources && cp Cargo.lock target/dependency-sources/copy.txt && echo ran >> target/dependency-sources/runs.txt'"

[[dependency_sources]]
lock = "client/package-lock.json"
into = "client/node_modules"
command = "sh -c 'mkdir -p client/node_modules && cp client/package-lock.json client/node_modules/copy.txt'"

[[dependency_sources]]
lock = "other.lock"
into = "vendored"
command = "sh -c 'mkdir -p vendored && echo ran > vendored/ran.txt'"

[[dependency_sources]]
lock = "fail.lock"
into = "target/fail"
command = "sh -c 'echo fetching; echo no such package >&2; exit 7'"

[[dependency_sources]]
lock = "slow.lock"
into = "target/slow"
timeout = 1
command = "sh -c 'mkdir -p target/slow; sleep 120 & echo $! > target/slow/child; echo started; wait'"
"#;

/// What a lock file says in the worktree, written by the research without committing it.
const NEWER: &str = "locked 2, not committed\n";

/// Puts on main what the tool's checks read: the lock files, and a `.gitignore` whose rules each name a directory, as
/// this repository's own do (`/target/`, `/client/node_modules/`), so that a path is ignored only when taken as one.
fn main_has_lock_files(s: &Setup) {
    main_moves_on(
        &s.root,
        &[
            (".gitignore", "/target/\n/client/node_modules/\n"),
            ("Cargo.lock", "locked 1\n"),
            ("client/package-lock.json", "locked 1\n"),
            ("other.lock", "locked 1\n"),
        ],
        "Lock files, and what git ignores",
    );
}

/// Lists the lock files in the project's configuration, which the engine reads from its last commit.
fn lists_lock_files(s: &Setup) {
    let project = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    write(&s.data, "projects/p.toml", &format!("{project}\n{LISTED}"));
    git(
        &s.data,
        &[
            "commit",
            "-q",
            "-am",
            "the lock files whose dependencies' source is read",
        ],
    );
}

/// The agents as `scripts` makes them, but for the research, which asks for these tools in its first round, after
/// changing two lock files in its worktree without committing, and then answers as it did.
fn research_asks(requests: Value) -> BTreeMap<&'static str, Script> {
    let mut agents = scripts();
    let research = agents.remove("R1").unwrap();
    agents.insert(
        "R1",
        Box::new(move |r: &Request, n: usize| {
            if n == 1 {
                write(&r.worktree, "Cargo.lock", NEWER);
                write(&r.worktree, "client/package-lock.json", NEWER);
                return (vec![("outcome", "tools")], vec![("outcome.tools", requests.clone())]);
            }
            research(r, n - 1)
        }),
    );
    agents
}

fn ask(lock: &str) -> Value {
    json!({ "tool": "dependency_sources", "lock": lock })
}

/// The output of the tool cell's first round that ended, which the engine checked against the flow before it took it.
fn tool_cell_output(e: &Engine, item: i64) -> Value {
    let text: String =
        e.db.conn()
            .query_row(
                "SELECT r.output FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.item = ?1 AND r.cell = 'X1' AND r.transition IS NOT NULL ORDER BY r.work, r.n LIMIT 1",
                [item],
                |r| r.get(0),
            )
            .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// Whether the output fits what the compiled flow says cell `X1` returns.
fn fits_the_flow(e: &Engine, work: i64, output: &Value) -> bool {
    let w = e.work(work).unwrap();
    let g = e.graph_of(&w).unwrap();
    let schema = g.cell("X1").and_then(|c| c.schema.clone()).unwrap();
    flow::valid(&schema, output)
}

/// Whether a process still stands, by the signal that only asks.
fn alive(pid: &str) -> bool {
    Command::new("kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

// A lock file the project lists is fetched by its command, run in the work's worktree, from the lock file as the
// worktree has it now, what is not committed included; the result gives the directory it filled, and the request is a
// row of tool_runs. A directory a rule for a directory ignores is ignored when it is not there yet, though the rule
// reads it only as a directory. A lock file the project does not list, and a directory git does not ignore, are
// refused with the reason, and no command runs: what the refused command would have written is not in the worktree.
// The tool cell's return, the refusals included, fits the flow.
#[test]
fn a_listed_lock_file_is_fetched_as_the_worktree_has_it_and_the_others_are_refused() {
    let s = setup("dependency-sources");
    main_has_lock_files(&s);
    lists_lock_files(&s);
    let requests = json!([
        ask("Cargo.lock"),
        ask("./client/package-lock.json"),
        ask("other.lock"),
        ask("unlisted.lock")
    ]);
    let (mut e, said, item) = start(&s, research_asks(requests));
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (work, _, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_eq!(ended_at(&e, item, "X1"), [(Some("ran".to_string()), None)], "{log}");
    let wt = worktree_of(&e, work);
    let runs = tool_runs(&e, item, "dependency_sources");
    assert_eq!(runs.len(), 4, "{runs:?}\n{log}");
    assert!(runs.iter().all(|r| r.0 == 1), "{runs:?}");
    let verdicts: Vec<&str> = runs
        .iter()
        .map(|r| r.1["sources"]["verdict"].as_str().unwrap())
        .collect();
    assert_eq!(verdicts, ["fetched", "fetched", "refused", "refused"], "{runs:?}");
    // Each command ran once, in the worktree, over the lock file as the worktree has it, not as main has it.
    let sources = wt.join("target/dependency-sources");
    assert_eq!(runs[0].1["sources"]["path"], sources.display().to_string());
    assert_eq!(std::fs::read_to_string(sources.join("copy.txt")).unwrap(), NEWER);
    assert_eq!(std::fs::read_to_string(sources.join("runs.txt")).unwrap(), "ran\n");
    // The directory a rule for a directory (`/client/node_modules/`) ignores, not there before the command.
    let modules = wt.join("client/node_modules");
    assert_eq!(runs[1].1["sources"]["path"], modules.display().to_string());
    assert_eq!(std::fs::read_to_string(modules.join("copy.txt")).unwrap(), NEWER);
    // Nothing the fetched commands wrote shows as a change of the worktree: git ignores it.
    let status = git(&wt, &["status", "--porcelain"]);
    assert!(
        status.contains("Cargo.lock") && status.contains("package-lock.json"),
        "{status}"
    );
    assert!(
        !status.contains("target") && !status.contains("node_modules"),
        "{status}"
    );
    // A directory git does not ignore: refused, and the command did not run.
    let why = |i: usize| runs[i].1["sources"]["why"].as_str().unwrap().to_string();
    assert!(
        why(2).starts_with("Git does not ignore vendored in the worktree"),
        "{}",
        why(2)
    );
    assert!(!wt.join("vendored").exists(), "the refused command ran");
    // A lock file the project does not list: refused, with the ones it lists.
    assert!(
        why(3).starts_with("\"unlisted.lock\" is not a lock file this project lists")
            && why(3).ends_with("they are: Cargo.lock, client/package-lock.json, other.lock, fail.lock, slow.lock"),
        "{}",
        why(3)
    );
    let output = tool_cell_output(&e, item);
    assert!(fits_the_flow(&e, work, &output), "{output}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A command that does not exit with 0 gives a result that holds its exit code and what it wrote, to its standard output
// and error as it wrote them; it is no failed tool, and is run once. A command that runs past its time limit is stopped
// with the process it started, which the result says (no exit code, stopped for time) and what it wrote before is kept.
// Both results fit the flow.
#[test]
fn a_command_that_fails_or_hangs_gives_a_result_and_a_hung_one_is_stopped_with_what_it_started() {
    let s = setup("dependency-sources-fails");
    main_has_lock_files(&s);
    lists_lock_files(&s);
    let (mut e, said, item) = start(&s, research_asks(json!([ask("fail.lock"), ask("slow.lock")])));
    step_to(&mut e, item, "X1");
    let began = Instant::now();
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (work, _, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert!(began.elapsed() < Duration::from_secs(60), "{:?}", began.elapsed());
    let runs = tool_runs(&e, item, "dependency_sources");
    assert_eq!(runs.len(), 2, "{runs:?}\n{log}");
    assert!(runs.iter().all(|r| r.0 == 1), "{runs:?}");
    assert_eq!(
        runs[0].1["sources"],
        json!({ "verdict": "failed", "exit": 7, "timed_out": false, "output": "fetching\nno such package\n" })
    );
    assert_eq!(
        runs[1].1["sources"],
        json!({ "verdict": "failed", "timed_out": true, "output": "started\n" })
    );
    // The process the hung command left behind is gone.
    let wt = worktree_of(&e, work);
    let pid = std::fs::read_to_string(wt.join("target/slow/child"))
        .unwrap()
        .trim()
        .to_string();
    let gone = (0..100).any(|_| {
        if alive(&pid) {
            std::thread::sleep(Duration::from_millis(50));
            return false;
        }
        true
    });
    assert!(gone, "process {pid} outlived the stop");
    let output = tool_cell_output(&e, item);
    assert!(fits_the_flow(&e, work, &output), "{output}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A command that fails while the engine finds the network down is no failure of the work's: the tool fails for want of
// the network, no question goes to the architect, every work waits for the network, and the cell runs its tools again
// once it is back, which gives the result of the command as it fails then.
#[test]
fn a_fetch_that_fails_while_the_network_is_down_waits_for_it_and_runs_again() {
    let s = setup("dependency-sources-offline");
    main_has_lock_files(&s);
    lists_lock_files(&s);
    let up = Arc::new(AtomicBool::new(true));
    let seen = up.clone();
    let hooks = Hooks {
        network: Some(Arc::new(move || seen.load(Ordering::SeqCst))),
        ..Hooks::default()
    };
    let (mut e, said, item) = start_with(&s, research_asks(json!([ask("fail.lock")])), hooks);
    step_to(&mut e, item, "X1");
    up.store(false, Ordering::SeqCst);
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("X1", None), "{log}");
    assert_eq!(e.halt().map(|h| h.kind).as_deref(), Some("offline"), "{log}");
    let runs = tool_runs(&e, item, "dependency_sources");
    assert_eq!(runs.len(), 1, "{runs:?}\n{log}");
    assert_eq!(runs[0].1["failed"]["kind"], "network_or_api", "{runs:?}");
    let asked: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM architect_matters WHERE work IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(asked, 0, "{log}");
    // Back: the cell runs the tool again, and the command fails as it does when the network is up.
    up.store(true, Ordering::SeqCst);
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "X1", "{log}");
    let runs = tool_runs(&e, item, "dependency_sources");
    assert_eq!(runs.len(), 2, "{runs:?}\n{log}");
    assert_eq!(runs[1].1["sources"]["verdict"], "failed", "{runs:?}");
    assert_eq!(runs[1].1["sources"]["exit"], 7, "{runs:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The cells that research, design, implement, review, test and advise are offered the tool, and what each is told of the
// lock file to ask for holds the value of the setting it names, as the project sets it now: `none` where it lists none,
// the lock files with their directories and commands where it does. A design that does not know the project lists a
// lock file would take what a dependency does for something to infer.
#[test]
fn the_cells_are_offered_the_tool_and_told_the_lock_files_the_project_lists() {
    let s = setup("dependency-sources-described");
    let described = |s: &Setup| -> String {
        let (e, _) = engine(s, scripts());
        let flow = e.flow(e.flow_now).unwrap();
        let g = flow.graphs.iter().find(|g| g.name == engine::ENGINEER).unwrap();
        for cell in ["R1", "D1", "D2", "I1", "T1", "T2", "T3", "T4", "T6", "A1"] {
            let offered = &g.cell(cell).unwrap().tools;
            assert!(offered.iter().any(|t| t == "dependency_sources"), "{cell}: {offered:?}");
        }
        let parts = engine::message::line_parts(&e, g, g.cell("R1").unwrap(), "p");
        let common = parts
            .iter()
            .find(|(address, _)| address == "format:common")
            .and_then(|(_, text)| text.as_str())
            .unwrap();
        // The schema is the last part of the text, which holds no blank line.
        let schema: Value = serde_json::from_str(common.rsplit("\n\n").next().unwrap()).unwrap();
        let form = tools::tool_form(&schema, "dependency_sources").expect("the common format has the tool");
        form["properties"]["lock"]["description"].as_str().unwrap().to_string()
    };
    let none = described(&s);
    assert!(none.contains("`project.dependency_sources` (none)"), "{none}");
    lists_lock_files(&s);
    let told = described(&s);
    for listed in [
        r#"{"lock":"Cargo.lock","into":"target/dependency-sources","command":"sh -c 'mkdir -p target/dependency-sources"#,
        r#"{"lock":"slow.lock","into":"target/slow","command":"sh -c 'mkdir -p target/slow; sleep 120 & echo $! > target/slow/child; echo started; wait'","timeout":1}"#,
    ] {
        assert!(told.contains(listed), "{listed}\n{told}");
    }
    let _ = std::fs::remove_dir_all(&s.root);
}
