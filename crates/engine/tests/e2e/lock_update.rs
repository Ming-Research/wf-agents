//! The tool `lock_update`: the lock files a project lists, the command each runs in the directory that holds it in the
//! work's worktree, and what the tool gives. The commands are `sh -c` stubs that write what the test reads, but for one
//! test, which runs the real cargo.
//!
//! That test (`cargo_locks_what_a_manifest_newly_asks_for_and_changes_nothing_else`) took 2.2 s alone in CI, against 2.0 s
//! for the stub test that only refuses (`a_project_that_lists_no_lock_file_refuses_and_says_what_to_do_instead`): nearly
//! all of it is the engine's setup that every test here has, and real cargo adds about 0.1 s. It is worth that because,
//! with the test of `lock_update_corners` that gives cargo a locked version it could move, it is the repository's only
//! proof that the tool works with a real package manager: a stub cannot tell whether a command a project writes locks
//! what the manifest asks for and leaves the rest alone. It cannot be smaller, since what it proves is the tool's run in
//! a work's worktree, and it cannot fold into a stub test, whose command it would replace.

use super::*;

/// What the project lists, as `[[lock_updates]]` tables. The first two commands write the directory they run in into
/// their lock file, relative to which they name it; the third changes nothing; the fourth writes a lock file that was
/// not there; the fifth changes its lock file and fails, as a resolve that finds no such package does; the sixth hangs,
/// with a process of its own, and has a time limit of one second; the seventh names a directory no worktree has.
const LISTED: &str = r#"
[[lock_updates]]
lock = "Cargo.lock"
command = "sh -c 'pwd -P > Cargo.lock; echo locked it; echo warned >&2'"

[[lock_updates]]
lock = "client/package-lock.json"
command = "sh -c 'pwd -P > package-lock.json'"

[[lock_updates]]
lock = "same.lock"
command = "sh -c 'echo nothing to do'"

[[lock_updates]]
lock = "new.lock"
command = "sh -c 'echo first > new.lock'"

[[lock_updates]]
lock = "fail.lock"
command = "sh -c 'echo half >> fail.lock; echo resolving; echo no such package >&2; exit 7'"

[[lock_updates]]
lock = "slow.lock"
timeout = 1
command = "sh -c 'sleep 120 & echo $! > child; echo started; wait'"

[[lock_updates]]
lock = "gone/other.lock"
command = "sh -c 'echo ran > ran.txt'"
"#;

/// What main holds as a lock file before any command.
const LOCKED: &str = "locked 1\n";

/// Puts on main the lock files the commands find.
fn main_has_lock_files(s: &Setup) {
    main_moves_on(
        &s.root,
        &[
            ("Cargo.lock", LOCKED),
            ("client/package-lock.json", LOCKED),
            ("same.lock", LOCKED),
            ("fail.lock", LOCKED),
        ],
        "Lock files",
    );
}

/// Adds these tables to the project's configuration, which the engine reads from its last commit.
fn lists(s: &Setup, tables: &str) {
    let project = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    write(&s.data, "projects/p.toml", &format!("{project}\n{tables}"));
    git(&s.data, &["commit", "-q", "-am", "the lock files a tool updates"]);
}

/// The agents as `scripts` makes them, but for the implementation, which first does `change` to its worktree, without
/// committing, and asks for these tools, and then answers as it did.
fn implementation_asks(
    requests: Value,
    change: impl Fn(&Path) + Send + Sync + 'static,
) -> BTreeMap<&'static str, Script> {
    let mut agents = scripts();
    let implement = agents.remove("I1").unwrap();
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            if n == 1 {
                change(&r.worktree);
                return (vec![("outcome", "tools")], vec![("outcome.tools", requests.clone())]);
            }
            implement(r, n - 1)
        }),
    );
    agents
}

fn ask(lock: &str) -> Value {
    json!({ "tool": "lock_update", "lock": lock })
}

/// What the tool's requests were, in order, as tool_runs records them.
fn asked(e: &Engine, item: i64) -> Vec<Value> {
    e.db.conn()
        .prepare(
            "SELECT t.args FROM tool_runs t JOIN works w ON w.id = t.work
             WHERE w.item = ?1 AND t.tool = 'lock_update' ORDER BY t.id",
        )
        .unwrap()
        .query_map([item], |r| r.get::<_, String>(0))
        .unwrap()
        .map(|x| serde_json::from_str(&x.unwrap()).unwrap())
        .collect()
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
fn fits_the_flow(e: &mut Engine, work: i64, output: &Value) -> bool {
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

// A lock file the project lists is brought in line by its command, run in the directory that holds it in the work's
// worktree: the command, which names the lock file relative to where it runs, writes that directory's absolute path into
// it, at the root and in a directory below, however the agent spells the path. The result gives what the command wrote,
// and says whether the lock file's content changed: it did for one the command rewrote and for one it wrote for the first
// time, and not for one it left as it was. Each request, with its arguments and its result, is a row of tool_runs. A lock
// file the project does not list and one whose directory the worktree lacks are refused with the reason, and no command
// runs. The tool cell's return, the refusals included, fits the flow.
#[test]
fn a_listed_lock_file_is_brought_in_line_in_its_directory_and_the_others_are_refused() {
    let s = setup("lock-update");
    main_has_lock_files(&s);
    lists(&s, LISTED);
    let requests = json!([
        ask("Cargo.lock"),
        ask("./client/package-lock.json"),
        ask("same.lock"),
        ask("new.lock"),
        ask("unlisted.lock"),
        ask("gone/other.lock"),
    ]);
    let (mut e, said, item) = start(&s, implementation_asks(requests.clone(), |_| {}));
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (work, _, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    let wt = worktree_of(&e, work);
    let runs = tool_runs(&e, item, "lock_update");
    assert_eq!(runs.len(), 6, "{runs:?}\n{log}");
    assert!(runs.iter().all(|r| r.0 == 1), "{runs:?}");
    assert_eq!(json!(asked(&e, item)), requests);
    let got = |i: usize| &runs[i].1["lock_update"];
    assert_eq!(
        *got(0),
        json!({ "verdict": "updated", "lock_changed": true, "output": "locked it\nwarned\n" })
    );
    assert_eq!(
        *got(1),
        json!({ "verdict": "updated", "lock_changed": true, "output": "" })
    );
    assert_eq!(
        *got(2),
        json!({ "verdict": "updated", "lock_changed": false, "output": "nothing to do\n" })
    );
    assert_eq!(
        *got(3),
        json!({ "verdict": "updated", "lock_changed": true, "output": "" })
    );
    let read = |path: &str| std::fs::read_to_string(wt.join(path)).unwrap();
    let real = |path: &Path| std::fs::canonicalize(path).unwrap().display().to_string();
    assert_eq!(read("Cargo.lock").trim(), real(&wt));
    assert_eq!(read("client/package-lock.json").trim(), real(&wt.join("client")));
    assert_eq!(read("same.lock"), LOCKED);
    assert_eq!(read("new.lock"), "first\n");
    // A lock file the project does not list: refused, with the ones it lists.
    assert_eq!(got(4)["verdict"], "refused", "{runs:?}");
    let why = |i: usize| got(i)["why"].as_str().unwrap().to_string();
    assert!(
        why(4).starts_with("\"unlisted.lock\" is not a lock file this project lists")
            && why(4).ends_with(
                "they are: Cargo.lock, client/package-lock.json, same.lock, new.lock, fail.lock, slow.lock, \
                 gone/other.lock"
            ),
        "{}",
        why(4)
    );
    // A directory the worktree lacks: refused, and the command did not run, nor did it come to be made.
    assert_eq!(got(5)["verdict"], "refused", "{runs:?}");
    assert!(why(5).starts_with("The worktree has no directory "), "{}", why(5));
    assert!(!wt.join("gone").exists(), "the refused command ran");
    assert!(!wt.join("ran.txt").exists(), "the refused command ran");
    let output = tool_cell_output(&e, item);
    assert!(fits_the_flow(&mut e, work, &output), "{output}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A project that lists no lock file runs no command, and the refusal says what the agent does instead: not write the lock
// file by hand, but use the lock commands the work's message names, or ask the architect. That text holds while those
// commands are there and after they are gone. The tool cell's return fits the flow.
#[test]
fn a_project_that_lists_no_lock_file_refuses_and_says_what_to_do_instead() {
    let s = setup("lock-update-none");
    main_has_lock_files(&s);
    let (mut e, said, item) = start(&s, implementation_asks(json!([ask("Cargo.lock")]), |_| {}));
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (work, _, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    let runs = tool_runs(&e, item, "lock_update");
    assert_eq!(runs.len(), 1, "{runs:?}\n{log}");
    assert_eq!(runs[0].1["lock_update"]["verdict"], "refused", "{runs:?}");
    let why = runs[0].1["lock_update"]["why"].as_str().unwrap();
    for says in [
        "Do not write the lock file by hand",
        "the lock commands your work's message names",
        "ask the architect",
    ] {
        assert!(why.contains(says), "{says}: {why}");
    }
    let wt = worktree_of(&e, work);
    assert_eq!(std::fs::read_to_string(wt.join("Cargo.lock")).unwrap(), LOCKED);
    let output = tool_cell_output(&e, item);
    assert!(fits_the_flow(&mut e, work, &output), "{output}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A command that does not exit with 0 gives a result that holds its exit code, what it wrote, and whether it changed the
// lock file before it failed; it is no failed tool, and is run once. A command that runs past its time limit is stopped
// with the process it started, which the result says (no exit code, stopped for time), and what it wrote before is kept.
// Both results fit the flow.
#[test]
fn a_command_that_fails_or_hangs_gives_a_result_and_a_hung_one_is_stopped_with_what_it_started() {
    let s = setup("lock-update-fails");
    main_has_lock_files(&s);
    lists(&s, LISTED);
    let requests = json!([ask("fail.lock"), ask("slow.lock")]);
    let (mut e, said, item) = start(&s, implementation_asks(requests, |_| {}));
    step_to(&mut e, item, "X1");
    let began = Instant::now();
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (work, _, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert!(began.elapsed() < Duration::from_secs(60), "{:?}", began.elapsed());
    let runs = tool_runs(&e, item, "lock_update");
    assert_eq!(runs.len(), 2, "{runs:?}\n{log}");
    assert!(runs.iter().all(|r| r.0 == 1), "{runs:?}");
    assert_eq!(
        runs[0].1["lock_update"],
        json!({
            "verdict": "failed",
            "exit": 7,
            "timed_out": false,
            "lock_changed": true,
            "output": "resolving\nno such package\n",
        })
    );
    assert_eq!(
        runs[1].1["lock_update"],
        json!({ "verdict": "failed", "timed_out": true, "lock_changed": false, "output": "started\n" })
    );
    // The process the hung command left behind is gone.
    let wt = worktree_of(&e, work);
    let pid = std::fs::read_to_string(wt.join("child")).unwrap().trim().to_string();
    let gone = (0..100).any(|_| {
        if alive(&pid) {
            std::thread::sleep(Duration::from_millis(50));
            return false;
        }
        true
    });
    assert!(gone, "process {pid} outlived the stop");
    let output = tool_cell_output(&e, item);
    assert!(fits_the_flow(&mut e, work, &output), "{output}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A command that fails while the engine finds the network down, as a package manager that cannot reach its registry
// does, is no failure of the work's: the tool fails for want of the network, no question goes to the architect, every
// work waits for the network, and the cell runs its tools again once it is back, which gives the command's result then.
#[test]
fn a_command_that_fails_while_the_network_is_down_waits_for_it_and_runs_again() {
    let s = setup("lock-update-offline");
    main_has_lock_files(&s);
    lists(&s, LISTED);
    let up = Arc::new(AtomicBool::new(true));
    let seen = up.clone();
    let hooks = Hooks {
        network: Some(Arc::new(move || seen.load(Ordering::SeqCst))),
        ..Hooks::default()
    };
    let (mut e, said, item) = start_with(&s, implementation_asks(json!([ask("fail.lock")]), |_| {}), hooks);
    step_to(&mut e, item, "X1");
    up.store(false, Ordering::SeqCst);
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("X1", None), "{log}");
    assert_eq!(e.halt().map(|h| h.kind).as_deref(), Some("offline"), "{log}");
    let runs = tool_runs(&e, item, "lock_update");
    assert_eq!(runs.len(), 1, "{runs:?}\n{log}");
    assert_eq!(runs[0].1["failed"]["kind"], "network_or_api", "{runs:?}");
    let matters: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM architect_matters WHERE work IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(matters, 0, "{log}");
    // Back: the cell runs the tool again, and the command fails as it does when the network is up.
    up.store(true, Ordering::SeqCst);
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "X1", "{log}");
    let runs = tool_runs(&e, item, "lock_update");
    assert_eq!(runs.len(), 2, "{runs:?}\n{log}");
    assert_eq!(runs[1].1["lock_update"]["verdict"], "failed", "{runs:?}");
    assert_eq!(runs[1].1["lock_update"]["exit"], 7, "{runs:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// What the item asks for, with a real package manager: the agent adds a dependency to a manifest without committing and
// asks for the tool, and the lock file locks it, with nothing else in it moved and nothing else in the worktree changed,
// so nothing was compiled (a `target` directory would show). The stubs of the other tests prove the tool; this is the one
// place that proves a real command does what the README tells a project to write. It is cargo because the machine that
// runs the tests has the cargo that compiled them; the dependency is a path, so it needs no network and no index.
#[test]
fn cargo_locks_what_a_manifest_newly_asks_for_and_changes_nothing_else() {
    const CARGO: &str = env!("CARGO");
    const WORKSPACE: &str = "[workspace]\nmembers = [\"a\", \"b\"]\nresolver = \"2\"\n";
    const A: &str = "[package]\nname = \"a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";
    const B: &str = "[package]\nname = \"b\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";
    let s = setup("lock-update-cargo");
    let files = [
        ("Cargo.toml", WORKSPACE),
        ("a/Cargo.toml", A),
        ("a/src/lib.rs", ""),
        ("b/Cargo.toml", B),
        ("b/src/lib.rs", ""),
    ];
    // Main's lock file is the one this cargo writes, so that the test does not pin the format of lock files.
    let fixture = s.root.join("fixture");
    for (path, text) in files {
        write(&fixture, path, text);
    }
    let made = Command::new(CARGO)
        .args(["generate-lockfile", "--offline"])
        .current_dir(&fixture)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    let lock = std::fs::read_to_string(fixture.join("Cargo.lock")).unwrap();
    let mut on_main = files.to_vec();
    on_main.push(("Cargo.lock", &lock));
    main_moves_on(&s.root, &on_main, "A workspace of two packages");
    lists(
        &s,
        &format!("[[lock_updates]]\nlock = \"Cargo.lock\"\ncommand = \"'{CARGO}' update --workspace --offline\"\n"),
    );
    let agents = implementation_asks(json!([ask("Cargo.lock")]), |wt: &Path| {
        write(
            wt,
            "a/Cargo.toml",
            &format!("{A}[dependencies]\nb = {{ path = \"../b\" }}\n"),
        );
    });
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (work, _, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    let runs = tool_runs(&e, item, "lock_update");
    assert_eq!(runs.len(), 1, "{runs:?}\n{log}");
    assert_eq!(runs[0].1["lock_update"]["verdict"], "updated", "{runs:?}");
    assert_eq!(runs[0].1["lock_update"]["lock_changed"], true, "{runs:?}");
    let wt = worktree_of(&e, work);
    let locked = "name = \"a\"\nversion = \"0.1.0\"\n";
    assert!(lock.contains(locked), "{lock}");
    let expected = lock.replace(locked, &format!("{locked}dependencies = [\n \"b\",\n]\n"));
    assert_eq!(std::fs::read_to_string(wt.join("Cargo.lock")).unwrap(), expected);
    let status = git(&wt, &["status", "--porcelain", "--untracked-files=all"]);
    let mut changed: Vec<&str> = status
        .lines()
        .filter_map(|l| l.trim_start().split_once(' ').map(|(_, path)| path))
        .collect();
    changed.sort_unstable();
    assert_eq!(changed, ["Cargo.lock", "a/Cargo.toml"], "{status}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The cells that change a manifest and commit are offered the tool, and no other cell of any graph is: research and design
// change only `research/`, and the others only read. The implementation's format names it, what the tool's input says
// holds the value of the setting it names (`none` where the project lists none, the lock files where it does), and an
// engineer is told by rule never to write a lock file by hand and what to do instead.
#[test]
fn only_the_cells_that_change_a_manifest_are_offered_the_tool_and_they_are_told_what_the_project_lists() {
    let s = setup("lock-update-offered");
    let described = |s: &Setup| -> String {
        let (e, _) = engine(s, scripts());
        let flow = e.flow(e.flow_now).unwrap();
        let offered: BTreeSet<(&str, &str)> = flow
            .graphs
            .iter()
            .flat_map(|g| {
                g.cells
                    .iter()
                    .filter(|c| c.tools.iter().any(|t| t == "lock_update"))
                    .map(move |c| (g.name.as_str(), c.id.as_str()))
            })
            .collect();
        let engineer = engine::ENGINEER;
        assert_eq!(
            offered,
            BTreeSet::from([(engineer, "I1"), (engineer, "T1"), (engineer, "T6")])
        );
        let g = flow.graphs.iter().find(|g| g.name == engineer).unwrap();
        let implement = g.cell("I1").unwrap();
        let format = engine::message::cell_format(g, implement);
        assert!(format.contains("lock_update"), "{format}");
        let parts = engine::message::line_parts(&e, g, implement, "p");
        let part = |address: &str| {
            parts
                .iter()
                .find(|(a, _)| a == address)
                .and_then(|(_, text)| text.as_str())
                .unwrap()
        };
        let rule = part("rule:engineering");
        assert!(
            rule.contains(
                "The repository: a lock file is never written by hand. Where the project lists it for the tool \
                 `lock_update`, change its manifest and have the tool bring the lock file in line with it; where it \
                 does not, use the lock commands the work's message names, and ask the architect when it names none."
            ),
            "{rule}"
        );
        // The schema is the last part of the text, which holds no blank line.
        let schema: Value = serde_json::from_str(part("format:common").rsplit("\n\n").next().unwrap()).unwrap();
        let form = tools::tool_form(&schema, "lock_update").expect("the common format has the tool");
        form["properties"]["lock"]["description"].as_str().unwrap().to_string()
    };
    let none = described(&s);
    assert!(none.contains("`project.lock_updates` (none)"), "{none}");
    lists(&s, LISTED);
    let told = described(&s);
    for listed in [
        r#"{"lock":"Cargo.lock","command":"sh -c 'pwd -P > Cargo.lock; echo locked it; echo warned >&2'"}"#,
        r#"{"lock":"slow.lock","command":"sh -c 'sleep 120 & echo $! > child; echo started; wait'","timeout":1}"#,
    ] {
        assert!(told.contains(listed), "{listed}\n{told}");
    }
    let _ = std::fs::remove_dir_all(&s.root);
}
