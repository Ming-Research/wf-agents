//! The tool `lock_update` at its edges: the ways a lock file's path can be written, what counts as the lock file having
//! changed, output that is not text or is long, a command that dies or cannot start, the network going down, and real
//! cargo with a locked version it could move. The tool's normal path is in `lock_update`.

use super::*;

/// Adds these tables to the project's configuration, which the engine reads from its last commit.
fn lists(s: &Setup, tables: &str) {
    let project = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    write(&s.data, "projects/p.toml", &format!("{project}\n{tables}"));
    git(&s.data, &["commit", "-q", "-am", "the lock files a tool updates"]);
}

fn ask(lock: &str) -> Value {
    json!({ "tool": "lock_update", "lock": lock })
}

/// The agents as `scripts` makes them, but for the implementation, whose rounds first do `steps` one at a time: each
/// changes its worktree, without committing, and asks for the tools it gives. Then it answers as it did.
fn implementation_steps(steps: Vec<Box<dyn Fn(&Path) -> Value + Send + Sync>>) -> BTreeMap<&'static str, Script> {
    let mut agents = scripts();
    let implement = agents.remove("I1").unwrap();
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| match steps.get(n - 1) {
            Some(step) => (vec![("outcome", "tools")], vec![("outcome.tools", step(&r.worktree))]),
            None => implement(r, n - steps.len()),
        }),
    );
    agents
}

/// The agents for one round of requests, after `change`.
fn implementation_asks(
    requests: Value,
    change: impl Fn(&Path) + Send + Sync + 'static,
) -> BTreeMap<&'static str, Script> {
    implementation_steps(vec![Box::new(move |wt: &Path| {
        change(wt);
        requests.clone()
    })])
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

/// Whether a process still stands, by the signal that only asks; it gives a process that was killed time to be taken in.
fn gone(pid: &str) -> bool {
    (0..100).any(|_| {
        let alive = Command::new("kill")
            .args(["-0", pid])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if alive {
            std::thread::sleep(Duration::from_millis(50));
        }
        !alive
    })
}

// However a path is spelled, it names one lock file when it names the same parts, in the configuration as in a request:
// `parts` says `a/b`, `a/b/` and `./a/b` are one, and a request is read with the white space around it taken away. The
// command runs in the directory of those parts, once for each such request. A path with other parts names nothing the
// project lists, though it may reach the same file on the disk: going up and coming back, going above the worktree, from
// the root, the directory alone, the file's name alone, another case. Those are refused and no command runs, so an agent
// cannot make the command run anywhere the configuration does not say. A lock file below a plain file has no directory to
// run in: refused too. The tool cell's return, the refusals included, fits the flow.
#[test]
fn a_lock_file_is_one_however_its_path_is_spelled_and_no_other_path_runs_its_command() {
    let s = setup("lock-update-spelled");
    main_moves_on(&s.root, &[("nested/dir/spelled.lock", "")], "A lock file below");
    lists(
        &s,
        r#"
[[lock_updates]]
lock = "./nested/./dir/spelled.lock/"
command = "sh -c 'pwd -P >> spelled.lock; echo ran'"

[[lock_updates]]
lock = "src/lib.txt/below-a-file.lock"
command = "sh -c 'echo ran > ran.txt'"
"#,
    );
    let names = [
        "nested/dir/spelled.lock",
        "./nested/dir/spelled.lock",
        "nested//dir/./spelled.lock",
        "nested/dir/spelled.lock/",
        "  nested/dir/spelled.lock\n",
    ];
    let others = [
        "nested/dir/../dir/spelled.lock",
        "../nested/dir/spelled.lock",
        "/nested/dir/spelled.lock",
        "",
        "nested/dir",
        "spelled.lock",
        "NESTED/dir/spelled.lock",
    ];
    let mut requests: Vec<Value> = names.iter().chain(&others).map(|l| ask(l)).collect();
    requests.push(ask("src/lib.txt/below-a-file.lock"));
    let (mut e, said, item) = start(&s, implementation_asks(json!(requests), |_| {}));
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (work, _, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    let runs = tool_runs(&e, item, "lock_update");
    assert_eq!(runs.len(), names.len() + others.len() + 1, "{runs:?}\n{log}");
    for (i, name) in names.iter().enumerate() {
        assert_eq!(
            runs[i].1["lock_update"],
            json!({ "verdict": "updated", "lock_changed": true, "output": "ran\n" }),
            "{name:?}"
        );
    }
    for (i, name) in others.iter().enumerate() {
        let got = &runs[names.len() + i].1["lock_update"];
        assert_eq!(got["verdict"], "refused", "{name:?}: {got}");
        let why = got["why"].as_str().unwrap();
        assert!(why.contains("is not a lock file this project lists"), "{name:?}: {why}");
    }
    let last = &runs[names.len() + others.len()].1["lock_update"];
    assert_eq!(last["verdict"], "refused", "{last}");
    let wt = worktree_of(&e, work);
    assert!(!wt.join("ran.txt").exists() && !wt.join("src/ran.txt").exists());
    // Each request that named the lock file ran its command there, and no other did.
    let dir = std::fs::canonicalize(wt.join("nested/dir"))
        .unwrap()
        .display()
        .to_string();
    let ran = std::fs::read_to_string(wt.join("nested/dir/spelled.lock")).unwrap();
    assert_eq!(ran, format!("{dir}\n").repeat(names.len()));
    let output = tool_cell_output(&e, item);
    assert!(fits_the_flow(&mut e, work, &output), "{output}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// `lock_changed` says whether the bytes the lock file's path gives differ from what they were before the command, as the
// flow's description of the field says, and nothing else: a lock file the command deleted has changed; one it rewrote
// with the same bytes has not, though its time and inode did; one that is there neither before nor after has not; one
// the agent changed and the command left has not, though git sees a change (the design's reason to compare content); two
// contents that are not text and read alike once made into text differ. Through a symlink, what counts is what the path
// reads: its target moved, it changed; the link replaced by a file of the same bytes, it did not; a link to nothing that
// the command gives a target, it did.
#[test]
fn lock_changed_is_what_the_path_reads_before_and_after_and_nothing_else() {
    let s = setup("lock-update-changed");
    main_moves_on(
        &s.root,
        &[
            ("deleted.lock", "locked\n"),
            ("rewritten.lock", "locked\n"),
            ("dirty.lock", "locked\n"),
            ("target.lock", "locked\n"),
            ("same-target.lock", "locked\n"),
        ],
        "Lock files",
    );
    lists(
        &s,
        r#"
[[lock_updates]]
lock = "deleted.lock"
command = "rm deleted.lock"

[[lock_updates]]
lock = "rewritten.lock"
command = "sh -c 'cat rewritten.lock > t; rm rewritten.lock; mv t rewritten.lock'"

[[lock_updates]]
lock = "never.lock"
command = "true"

[[lock_updates]]
lock = "dirty.lock"
command = "true"

[[lock_updates]]
lock = "bytes.lock"
command = "sh -c 'printf \"\\377\" > bytes.lock'"

[[lock_updates]]
lock = "linked.lock"
command = "sh -c 'echo moved > target.lock'"

[[lock_updates]]
lock = "same-linked.lock"
command = "sh -c 'cat same-linked.lock > t; rm same-linked.lock; mv t same-linked.lock'"

[[lock_updates]]
lock = "dangling.lock"
command = "sh -c 'echo made > nothing-yet.lock'"
"#,
    );
    let requests = json!([
        ask("deleted.lock"),
        ask("rewritten.lock"),
        ask("never.lock"),
        ask("dirty.lock"),
        ask("bytes.lock"),
        ask("linked.lock"),
        ask("same-linked.lock"),
        ask("dangling.lock"),
    ]);
    let change = |wt: &Path| {
        use std::os::unix::fs::symlink;
        std::fs::write(wt.join("dirty.lock"), "changed by the agent\n").unwrap();
        std::fs::write(wt.join("bytes.lock"), [0xfe_u8]).unwrap();
        symlink("target.lock", wt.join("linked.lock")).unwrap();
        symlink("same-target.lock", wt.join("same-linked.lock")).unwrap();
        symlink("nothing-yet.lock", wt.join("dangling.lock")).unwrap();
    };
    let (mut e, said, item) = start(&s, implementation_asks(requests, change));
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (work, _, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    let runs = tool_runs(&e, item, "lock_update");
    let told: Vec<(&str, &Value)> = runs
        .iter()
        .map(|r| {
            let got = &r.1["lock_update"];
            (got["verdict"].as_str().unwrap_or_default(), &got["lock_changed"])
        })
        .collect();
    let (yes, no) = (json!(true), json!(false));
    assert_eq!(
        told,
        [
            ("updated", &yes),
            ("updated", &no),
            ("updated", &no),
            ("updated", &no),
            ("updated", &yes),
            ("updated", &yes),
            ("updated", &no),
            ("updated", &yes),
        ],
        "{runs:?}\n{log}"
    );
    let wt = worktree_of(&e, work);
    assert!(!wt.join("deleted.lock").exists());
    assert_eq!(std::fs::read(wt.join("bytes.lock")).unwrap(), [0xff_u8]);
    assert_eq!(
        std::fs::read_to_string(wt.join("dirty.lock")).unwrap(),
        "changed by the agent\n"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// What a command writes reaches the agent however it is: bytes that are not text come as the replacement character and
// the rest stands; output past what a result holds is saved whole outside the worktree, where it changes nothing the
// agent commits, with its path in the result, each of a round's in a file of its own and from a command that failed too.
// A command that reads its standard input ends at once rather than running to its limit, as a package manager's prompt
// would. A command that dies by a signal has no exit code and was not stopped for time. A command past its limit that
// changed the lock file first says so, and what it started is gone. The tool cell's return fits the flow throughout.
#[test]
fn what_a_command_writes_and_how_it_ends_reach_the_agent_whatever_they_are() {
    let s = setup("lock-update-output");
    lists(
        &s,
        r#"
[[lock_updates]]
lock = "bad-bytes.lock"
command = "sh -c 'printf \"a\\377b\\n\"'"

[[lock_updates]]
lock = "long.lock"
command = "sh -c 'yes x | head -c 30000'"

[[lock_updates]]
lock = "long-failed.lock"
command = "sh -c 'yes y | head -c 30000; exit 3'"

[[lock_updates]]
lock = "stdin.lock"
command = "sh -c 'cat; echo read to its end'"

[[lock_updates]]
lock = "killed.lock"
command = "sh -c 'echo before; kill -9 $$'"

[[lock_updates]]
lock = "half-hung.lock"
timeout = 1
command = "sh -c 'echo half > half-hung.lock; sleep 120 & echo $! > child; echo started; wait'"
"#,
    );
    let requests = json!([
        ask("bad-bytes.lock"),
        ask("long.lock"),
        ask("long-failed.lock"),
        ask("stdin.lock"),
        ask("killed.lock"),
        ask("half-hung.lock"),
    ]);
    let (mut e, said, item) = start(&s, implementation_asks(requests, |_| {}));
    step_to(&mut e, item, "X1");
    let began = Instant::now();
    e.step(None).unwrap();
    assert!(began.elapsed() < Duration::from_secs(45), "{:?}", began.elapsed());
    let log = said.lock().unwrap().join("\n");
    let (work, _, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    let runs = tool_runs(&e, item, "lock_update");
    assert_eq!(runs.len(), 6, "{runs:?}\n{log}");
    let got = |i: usize| &runs[i].1["lock_update"];
    assert_eq!(
        *got(0),
        json!({ "verdict": "updated", "lock_changed": false, "output": "a\u{fffd}b\n" })
    );
    let wt = worktree_of(&e, work);
    let saved = |i: usize, what: &str| {
        let output = got(i)["output"].as_str().unwrap();
        let (said, path) = output.rsplit_once("saved to ").expect(output);
        assert!(said.contains("30000 bytes"), "{output}");
        let path = Path::new(path);
        assert!(!path.starts_with(&wt), "{output}");
        assert_eq!(std::fs::read_to_string(path).unwrap(), what.repeat(15_000), "{output}");
    };
    assert_eq!(got(1)["verdict"], "updated", "{runs:?}");
    saved(1, "x\n");
    assert_eq!((&got(2)["verdict"], &got(2)["exit"]), (&json!("failed"), &json!(3)));
    saved(2, "y\n");
    assert_eq!(
        *got(3),
        json!({ "verdict": "updated", "lock_changed": false, "output": "read to its end\n" })
    );
    assert_eq!(
        *got(4),
        json!({ "verdict": "failed", "timed_out": false, "lock_changed": false, "output": "before\n" })
    );
    assert_eq!(
        *got(5),
        json!({ "verdict": "failed", "timed_out": true, "lock_changed": true, "output": "started\n" })
    );
    let pid = std::fs::read_to_string(wt.join("child")).unwrap().trim().to_string();
    assert!(gone(&pid), "process {pid} outlived the stop");
    let output = tool_cell_output(&e, item);
    assert!(fits_the_flow(&mut e, work, &output), "{output}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A lock file that cannot be read says nothing of what a command would do with it: where the configuration names a
// directory, as `config check` cannot tell without a worktree, the tool fails three times and asks the architect, and the
// command does not run, rather than running with `lock_changed` meaning nothing.
#[test]
fn a_lock_file_that_is_a_directory_fails_the_tool_and_no_command_runs() {
    let s = setup("lock-update-directory");
    main_moves_on(&s.root, &[("client/package.json", "{}\n")], "A directory");
    lists(
        &s,
        "[[lock_updates]]\nlock = \"client\"\ncommand = \"sh -c 'echo ran > ran.txt'\"\n",
    );
    let (mut e, said, item) = start(&s, implementation_asks(json!([ask("client/")]), |_| {}));
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("A2", None), "{log}");
    let runs = tool_runs(&e, item, "lock_update");
    assert_eq!(runs.len(), 3, "{runs:?}\n{log}");
    for run in &runs {
        assert_eq!(run.1["failed"]["kind"], "other", "{runs:?}");
        assert!(run.1["failed"]["what"].as_str().unwrap().contains("client"), "{runs:?}");
    }
    assert!(!worktree_of(&e, work).join("ran.txt").exists(), "the command ran");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A command that cannot be started, as where the engine's PATH lacks the program, is a fault of the configuration and no
// result of the command: as the design says, the tool fails, is tried three times and goes to the architect, whose
// question names the program, and the agent is not told the lock file was brought in line.
#[test]
fn a_command_that_cannot_be_started_fails_the_tool_and_goes_to_the_architect() {
    let s = setup("lock-update-no-program");
    lists(
        &s,
        "[[lock_updates]]\nlock = \"Cargo.lock\"\ncommand = \"wf-agents-no-such-program update\"\n",
    );
    let (mut e, said, item) = start(&s, implementation_asks(json!([ask("Cargo.lock")]), |_| {}));
    step_to(&mut e, item, "X1");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("A2", None), "{log}");
    assert_eq!(e.halt().map(|h| h.kind), None, "{log}");
    let runs = tool_runs(&e, item, "lock_update");
    assert_eq!(runs.len(), 3, "{runs:?}\n{log}");
    assert_eq!(runs.iter().map(|r| r.0).collect::<Vec<_>>(), [1, 2, 3], "{runs:?}");
    for run in &runs {
        assert_eq!(run.1["failed"]["kind"], "other", "{runs:?}");
        let what = run.1["failed"]["what"].as_str().unwrap();
        assert!(what.starts_with("wf-agents-no-such-program: "), "{what}");
    }
    let asked: String =
        e.db.conn()
            .query_row(
                "SELECT what FROM blockers WHERE work = ?1 AND ended IS NULL",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert!(asked.contains("wf-agents-no-such-program"), "{asked}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The network being down is what makes a failed command wait, and nothing else: a command that ends with 0 needs nothing
// more of the network, whatever the engine finds, and a refusal runs nothing. Both are results the agent gets, and no work
// waits.
#[test]
fn a_command_that_succeeds_and_a_refusal_are_results_while_the_network_is_down() {
    let s = setup("lock-update-offline-fine");
    lists(
        &s,
        "[[lock_updates]]\nlock = \"fine.lock\"\ncommand = \"sh -c 'echo locked > fine.lock'\"\n",
    );
    let up = Arc::new(AtomicBool::new(true));
    let seen = up.clone();
    let hooks = Hooks {
        network: Some(Arc::new(move || seen.load(Ordering::SeqCst))),
        ..Hooks::default()
    };
    let requests = json!([ask("fine.lock"), ask("unlisted.lock")]);
    let (mut e, said, item) = start_with(&s, implementation_asks(requests, |_| {}), hooks);
    step_to(&mut e, item, "X1");
    up.store(false, Ordering::SeqCst);
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(e.halt().map(|h| h.kind), None, "{log}");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "X1", "{log}");
    let runs = tool_runs(&e, item, "lock_update");
    assert_eq!(runs.len(), 2, "{runs:?}\n{log}");
    assert_eq!(
        runs[0].1["lock_update"],
        json!({ "verdict": "updated", "lock_changed": true, "output": "" })
    );
    assert_eq!(runs[1].1["lock_update"]["verdict"], "refused", "{runs:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// When one request of a round fails for want of the network, the cell runs again once it is back, every request of it:
// the agent is given one result for each request, those of the run that counted, and what the run before wrote to the
// worktree stays there for the next.
#[test]
fn after_the_network_comes_back_the_agent_has_one_result_for_each_request() {
    let s = setup("lock-update-offline-rerun");
    lists(
        &s,
        r#"
[[lock_updates]]
lock = "fine.lock"
command = "sh -c 'echo locked >> fine.lock'"

[[lock_updates]]
lock = "fails.lock"
command = "sh -c 'echo no such package; exit 7'"
"#,
    );
    let up = Arc::new(AtomicBool::new(true));
    let seen = up.clone();
    let hooks = Hooks {
        network: Some(Arc::new(move || seen.load(Ordering::SeqCst))),
        ..Hooks::default()
    };
    let requests = json!([ask("fine.lock"), ask("fails.lock")]);
    let (mut e, said, item) = start_with(&s, implementation_asks(requests, |_| {}), hooks);
    step_to(&mut e, item, "X1");
    up.store(false, Ordering::SeqCst);
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("X1", None), "{log}");
    assert_eq!(e.halt().map(|h| h.kind).as_deref(), Some("offline"), "{log}");
    up.store(true, Ordering::SeqCst);
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "X1", "{log}");
    let output = tool_cell_output(&e, item);
    assert_eq!(output["failed"], json!([]), "{output}");
    let results = output["results"].as_array().unwrap();
    assert_eq!(results.len(), 2, "{output}");
    assert_eq!(results[0]["lock_update"]["verdict"], "updated", "{output}");
    assert_eq!(results[1]["lock_update"]["verdict"], "failed", "{output}");
    assert_eq!(results[1]["lock_update"]["exit"], 7, "{output}");
    let wt = worktree_of(&e, work);
    assert_eq!(
        std::fs::read_to_string(wt.join("fine.lock")).unwrap(),
        "locked\nlocked\n"
    );
    assert!(fits_the_flow(&mut e, work, &output), "{output}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A package of the directory the fixture's cargo reads as its registry, with a checksum of its own version.
fn vendored(fixture: &Path, name: &str, version: &str, sum: char) {
    let dir = format!("vendor/{name}-{version}");
    let manifest = format!("[package]\nname = \"{name}\"\nversion = \"{version}\"\nedition = \"2021\"\n");
    let checksum = format!("{{\"files\":{{}},\"package\":\"{}\"}}", sum.to_string().repeat(64));
    write(fixture, &format!("{dir}/Cargo.toml"), &manifest);
    write(fixture, &format!("{dir}/src/lib.rs"), "");
    write(fixture, &format!("{dir}/.cargo-checksum.json"), &checksum);
}

// What the README tells a project to write, with real cargo and a version it could move, in a workspace below the root:
// the package a manifest newly asks for is locked and a package already locked stays at its version though a newer one
// is there. A control shows the fixture can tell: the bare `cargo update` the README warns of does move it (cargo's
// documentation says both: with no package named it updates all; `--workspace` only the workspace's). The command goes
// through `env`, as the README's does, and runs in the lock file's directory, or cargo finds neither workspace nor
// registry. What the tool's description says of a merge conflict holds: a lock file with conflict markers fails with
// what cargo says and is left as it was; main's taken instead, the tool locks what the manifest newly asks for; asked
// again, it changes nothing. Nothing else in the worktree changes, so nothing was compiled. It needs no network, since
// the registry is a directory.
#[test]
fn a_package_already_locked_stays_where_cargo_could_move_it_and_a_conflict_is_mended_as_told() {
    const CARGO: &str = env!("CARGO");
    const WORKSPACE: &str = "[workspace]\nmembers = [\"a\"]\nresolver = \"2\"\n";
    const A: &str = "[package]\nname = \"a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[dependencies]\nc = \"1\"\n";
    const CONFIG: &str = "[source.crates-io]\nreplace-with = \"vendored\"\n[source.vendored]\ndirectory = \"vendor\"\n";
    let s = setup("lock-update-cargo-locked");
    let fixture = s.root.join("fixture/rust");
    write(&fixture, "Cargo.toml", WORKSPACE);
    write(&fixture, ".cargo/config.toml", CONFIG);
    write(&fixture, "a/Cargo.toml", A);
    write(&fixture, "a/src/lib.rs", "");
    vendored(&fixture, "c", "1.0.0", 'a');
    vendored(&fixture, "d", "1.0.0", 'c');
    let cargo = |args: &[&str]| {
        let made = Command::new(CARGO).args(args).current_dir(&fixture).output().unwrap();
        assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
        std::fs::read_to_string(fixture.join("Cargo.lock")).unwrap()
    };
    // Locked while 1.0.0 is the newest there is; then 1.0.1 comes to be.
    let lock = cargo(&["generate-lockfile", "--offline"]);
    assert!(lock.contains("name = \"c\"\nversion = \"1.0.0\""), "{lock}");
    assert!(!lock.contains("name = \"d\""), "{lock}");
    vendored(&fixture, "c", "1.0.1", 'b');
    let moved = cargo(&["update", "--offline"]);
    assert!(moved.contains("name = \"c\"\nversion = \"1.0.1\""), "{moved}");
    std::fs::write(fixture.join("Cargo.lock"), &lock).unwrap();
    let mut on_main = Vec::new();
    let mut below = vec![fixture.clone()];
    while let Some(dir) = below.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                below.push(path);
            } else {
                let name = format!("rust/{}", path.strip_prefix(&fixture).unwrap().display());
                on_main.push((name, std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    let files: Vec<(&str, &str)> = on_main.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
    main_moves_on(
        &s.root,
        &files,
        "A workspace below the root, and the directory it reads as a registry",
    );
    lists(
        &s,
        &format!(
            "[[lock_updates]]\nlock = \"rust/Cargo.lock\"\n\
             command = \"env RUSTUP_AUTO_INSTALL=0 '{CARGO}' update --workspace --offline\"\n"
        ),
    );
    let requests = json!([ask("rust/Cargo.lock")]);
    let (first, again, last) = (requests.clone(), requests.clone(), requests);
    let conflicted = format!("<<<<<<< HEAD\n{lock}=======\n{lock}>>>>>>> other\n");
    let held = conflicted.clone();
    let agents = implementation_steps(vec![
        Box::new(move |wt: &Path| {
            write(wt, "rust/a/Cargo.toml", &format!("{A}d = \"1\"\n"));
            write(wt, "rust/Cargo.lock", &held);
            first.clone()
        }),
        Box::new(move |wt: &Path| {
            git(wt, &["checkout", "origin/main", "--", "rust/Cargo.lock"]);
            again.clone()
        }),
        Box::new(move |_: &Path| last.clone()),
    ]);
    let (mut e, said, item) = start(&s, agents);
    let mut read_after = Vec::new();
    for _ in 0..3 {
        step_to(&mut e, item, "X1");
        e.step(None).unwrap();
        let (work, _, _) = state(&e, item).unwrap();
        read_after.push(std::fs::read_to_string(worktree_of(&e, work).join("rust/Cargo.lock")).unwrap());
    }
    let log = said.lock().unwrap().join("\n");
    let (work, _, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    let runs = tool_runs(&e, item, "lock_update");
    assert_eq!(runs.len(), 3, "{runs:?}\n{log}");
    let got = |i: usize| &runs[i].1["lock_update"];
    // The markers: cargo cannot read the file, says so, and leaves it.
    assert_eq!(got(0)["verdict"], "failed", "{runs:?}");
    assert_eq!(got(0)["lock_changed"], false, "{runs:?}");
    assert!(got(0)["exit"].as_i64().is_some(), "{runs:?}");
    assert!(got(0)["output"].as_str().unwrap().contains("Cargo.lock"), "{runs:?}");
    assert_eq!(read_after[0], conflicted);
    // Main's: `d` is locked, `a` asks for it, and every line main's held stands.
    assert_eq!(got(1)["verdict"], "updated", "{runs:?}");
    assert_eq!(got(1)["lock_changed"], true, "{runs:?}");
    let now = &read_after[1];
    assert!(now.contains("name = \"d\"\nversion = \"1.0.0\""), "{now}");
    assert!(now.contains(" \"c\",\n \"d\",\n"), "{now}");
    assert!(now.contains("name = \"c\"\nversion = \"1.0.0\""), "{now}");
    assert!(!now.contains("1.0.1"), "{now}");
    for line in lock.lines() {
        assert!(now.lines().any(|l| l == line), "{line:?} went: {now}");
    }
    // Again: nothing to do.
    assert_eq!(got(2)["verdict"], "updated", "{runs:?}");
    assert_eq!(got(2)["lock_changed"], false, "{runs:?}");
    assert_eq!(read_after[2], *now);
    let wt = worktree_of(&e, work);
    let status = git(&wt, &["status", "--porcelain", "--untracked-files=all"]);
    let mut changed: Vec<&str> = status
        .lines()
        .filter_map(|l| l.trim_start().split_once(' ').map(|(_, path)| path))
        .collect();
    changed.sort_unstable();
    assert_eq!(changed, ["rust/Cargo.lock", "rust/a/Cargo.toml"], "{status}");
    let _ = std::fs::remove_dir_all(&s.root);
}
