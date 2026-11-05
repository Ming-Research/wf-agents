//! What a round reads of the project's main: one commit fetched for it, and what comes of a fetch that fails.

use super::*;

/// Notes each fetch that reaches project p's remote from its checkout. The checkout's remote is set here to a server
/// of this test, which gives the origin's files over plain HTTP (git's "dumb" protocol) and is asked for the remote's
/// `info/refs` once by each fetch, whoever makes it. A program named by the checkout's configuration cannot be the
/// instrument: the engine's git runs none that the configuration names. The server can also push the commit that
/// [`commit_on_main`] made right after the next fetch is answered, as another work's push reaching main while a fetch
/// is under way.
struct Fetches {
    count: Arc<AtomicUsize>,
    push_after: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Fetches {
    fn watching(s: &Setup) -> Fetches {
        use std::io::{Read, Write};
        let root = s.root.clone();
        let origin = root.join("origin.git");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let count = Arc::new(AtomicUsize::new(0));
        let push_after = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (counted, pushing, stopped) = (Arc::clone(&count), Arc::clone(&push_after), Arc::clone(&stop));
        let thread = std::thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut head = Vec::new();
                let mut chunk = [0u8; 4096];
                while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => head.extend_from_slice(&chunk[..n]),
                    }
                }
                // The request line is `GET /<path>?<query> HTTP/1.1`.
                let head = String::from_utf8_lossy(&head).into_owned();
                let target = head.split_whitespace().nth(1).unwrap_or("/");
                let path = target
                    .split('?')
                    .next()
                    .unwrap_or("/")
                    .trim_start_matches('/')
                    .to_string();
                let refs = path == "info/refs";
                if refs {
                    counted.fetch_add(1, Ordering::Relaxed);
                    // The files that list the refs and the packs are made as the origin is at this moment.
                    let _ = Command::new("git")
                        .arg("-C")
                        .arg(&origin)
                        .arg("update-server-info")
                        .env_remove("GIT_DIR")
                        .status();
                }
                let body = if path.split('/').any(|p| p == "..") {
                    None
                } else {
                    std::fs::read(origin.join(&path)).ok()
                };
                let _ = match &body {
                    Some(b) => stream
                        .write_all(
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                b.len()
                            )
                            .as_bytes(),
                        )
                        .and_then(|()| stream.write_all(b)),
                    None => stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    ),
                };
                drop(stream);
                if refs && pushing.swap(false, Ordering::Relaxed) {
                    let _ = Command::new("git")
                        .arg("-C")
                        .arg(root.join("elsewhere"))
                        .args(["push", "-q", "origin", "HEAD:main"])
                        .env_remove("GIT_DIR")
                        .status();
                }
            }
        });
        git(
            &s.root.join("checkout"),
            &["config", "remote.origin.url", &format!("http://127.0.0.1:{port}")],
        );
        Fetches {
            count,
            push_after,
            stop,
            thread: Some(thread),
        }
    }

    /// How many fetches there have been since the watching began.
    fn so_far(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }

    /// Has the commit made with [`commit_on_main`] pushed to main right after the next fetch is answered.
    fn push_after_the_next(&self) {
        self.push_after.store(true, Ordering::Relaxed);
    }
}

impl Drop for Fetches {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// What a fetch must leave as it is in a project's checkout: the files it tracks and their state, the commit it stands
/// on and its branch.
fn checkout_state(s: &Setup) -> String {
    let dir = s.root.join("checkout");
    let files = git(&dir, &["ls-files", "-s"]);
    let status = git(&dir, &["status", "--porcelain", "--untracked-files=all"]);
    let head = git(&dir, &["rev-parse", "HEAD"]);
    let branch = git(&dir, &["branch", "--show-current"]);
    let lib = std::fs::read_to_string(dir.join("src/lib.txt")).unwrap_or_default();
    format!("{files}\n--\n{status}\n--\n{head}\n{branch}\n{lib}")
}

/// The commit of main the latest round of `work` at `cell` was pinned to, as its record keeps it.
fn pinned_main(e: &Engine, work: i64, cell: &str) -> Option<String> {
    e.db.conn()
        .query_row(
            "SELECT json_extract(input, '$.engine.main') FROM rounds WHERE work = ?1 AND cell = ?2
             ORDER BY n DESC LIMIT 1",
            params![work, cell],
            |r| r.get(0),
        )
        .unwrap()
}

/// Scripts that keep the prompt of each round of the cells named, each with its cell, and then hand in what `then`
/// does.
fn keeps_prompts(
    agents: &mut BTreeMap<&'static str, Script>,
    cells: &[&'static str],
    then: impl Fn(&Request, usize) -> Handed + Send + Sync + Clone + 'static,
) -> Arc<Mutex<Vec<(String, String)>>> {
    let kept = Arc::new(Mutex::new(Vec::new()));
    for &cell in cells {
        let (kept, then) = (kept.clone(), then.clone());
        agents.insert(
            cell,
            Box::new(move |r: &Request, n: usize| {
                kept.lock().unwrap().push((r.cell.clone(), r.prompt.clone()));
                then(r, n)
            }),
        );
    }
    kept
}

// A round of an engineer's work is given the project's constitution as the project's main has it when the round
// starts, which the engine fetches for it, though main moved on the remote since the last fetch and meanwhile; nothing
// in the project's checkout changes, no file and no commit, since the checkout is only where main is fetched into. The
// round's record keeps the commit of main it was given.
#[test]
fn an_engineer_s_round_is_given_the_constitution_main_has_and_the_checkout_stays_as_it_is() {
    let s = setup("main-engineer");
    names_a_constitution(&s);
    let small = "## Article 1. Small\n\nKeep it small.\n";
    let smaller = "## Article 1. Small\n\nKeep it smaller, always.\n";
    main_moves_on(&s.root, &[("CONSTITUTION.md", small)], "A constitution");
    let first = git(&s.origin, &["rev-parse", "main"]);
    let before = checkout_state(&s);
    let (root, mut agents) = (s.root.clone(), scripts());
    // Main moves on while the first round runs, after its message was made.
    let seen = keeps_prompts(&mut agents, &["R0", "R1"], move |r: &Request, _: usize| {
        if r.cell == "R0" {
            main_moves_on(&root, &[("CONSTITUTION.md", smaller)], "A stricter constitution");
        }
        (Vec::new(), Vec::new())
    });
    let (mut e, said, item) = start(&s, agents);
    for _ in 0..60 {
        if seen.lock().unwrap().len() >= 2 {
            break;
        }
        e.tick().unwrap();
        if let Some((_, _, paused)) = state(&e, item) {
            assert!(
                paused.is_none(),
                "paused: {paused:?}\n{}",
                said.lock().unwrap().join("\n")
            );
        }
    }
    let log = said.lock().unwrap().join("\n");
    let second = git(&s.origin, &["rev-parse", "main"]);
    let seen = seen.lock().unwrap();
    assert_eq!(
        seen.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(),
        ["R0", "R1"],
        "{log}"
    );
    assert!(seen[0].1.contains("Keep it small."), "{}", seen[0].1);
    assert!(seen[1].1.contains("Keep it smaller, always."), "{}", seen[1].1);
    let work = state(&e, item).unwrap().0;
    assert_eq!(pinned_main(&e, work, "R0").as_deref(), Some(first.as_str()), "{log}");
    assert_eq!(pinned_main(&e, work, "R1").as_deref(), Some(second.as_str()), "{log}");
    // The checkout fetched main, and nothing else of it changed: the constitution is in none of its files.
    let checkout = s.root.join("checkout");
    assert_eq!(git(&checkout, &["rev-parse", "origin/main"]), second);
    assert_eq!(checkout_state(&s), before);
    assert!(!checkout.join("CONSTITUTION.md").exists());
    let _ = std::fs::remove_dir_all(&s.root);
}

// A round of a project's architect reads the project at one commit of main, fetched for it: its constitution and its
// guide are of the same commit, though main moves on while it runs and another work's fetch brings that into the
// checkout.
#[test]
fn a_round_reads_one_commit_of_main_and_what_follows_it_reads_the_same() {
    let s = setup("main-pinned");
    names_a_constitution(&s);
    let article = |n: usize, what: &str| format!("## Article {n}. {what}\n\nKeep it {}.\n", what.to_lowercase());
    let two = format!("{}\n{}", article(1, "Small"), article(2, "Quick"));
    let three = format!("{two}\n{}", article(3, "Plain"));
    let four = format!("{three}\n{}", article(4, "Warm"));
    main_moves_on(
        &s.root,
        &[
            ("CONSTITUTION.md", two.as_str()),
            ("AGENTS.md", "Work the second way.\n"),
        ],
        "Two articles",
    );
    let (first, checkout) = (git(&s.origin, &["rev-parse", "main"]), s.root.join("checkout"));
    let (root, (to_three, to_four), mut agents) = (s.root.clone(), (three.clone(), four.clone()), scripts());
    let seen = keeps_prompts(&mut agents, &["B1", "P1"], move |r: &Request, _: usize| {
        // Main moves on while the round runs, and another work's fetch brings it into the checkout.
        let articles = if r.cell == "B1" { &to_three } else { &to_four };
        main_moves_on(
            &root,
            &[
                ("CONSTITUTION.md", articles.as_str()),
                ("AGENTS.md", "Work the third way.\n"),
            ],
            "More articles",
        );
        git(&checkout, &["fetch", "-q", "origin", "main"]);
        (Vec::new(), Vec::new())
    });
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    a_check_of_the_policy_is_queued(&mut e);
    let arch = architect_work_at(&mut e, "B1");
    for _ in 0..8 {
        if seen.lock().unwrap().len() >= 2 {
            break;
        }
        e.advance(arch).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    let second = git(&s.origin, &["rev-parse", "main~1"]);
    let seen = seen.lock().unwrap();
    assert_eq!(
        seen.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(),
        ["B1", "P1"],
        "{log}"
    );
    // The constitution and the guide it was given are those of the commit it was pinned to, not of the later ones.
    let (u1, p1) = (&seen[0].1, &seen[1].1);
    assert!(
        u1.contains("Keep it quick.") && u1.contains("Work the second way."),
        "{u1}"
    );
    assert!(!u1.contains("Keep it plain.") && !u1.contains("third way"), "{u1}");
    // The check of the policy, forked from the base, is not given the constitution main moved on to while it ran.
    assert!(!p1.contains("Keep it warm."), "{p1}");
    assert_eq!(pinned_main(&e, arch, "B1").as_deref(), Some(first.as_str()), "{log}");
    assert_eq!(pinned_main(&e, arch, "P1").as_deref(), Some(second.as_str()), "{log}");
    assert_eq!(rows_at(&e, arch, "P1"), ["holds"], "{log}");
    let sent_back: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'P1' AND rejected IS NOT NULL",
                [arch],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(sent_back, 0, "{log}");
    assert_eq!(
        git(&s.root.join("checkout"), &["rev-parse", "origin/main"]),
        git(&s.origin, &["rev-parse", "main"])
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A round of a project's architect on its line makes one fetch of main, and the worktree it reads files in, the inputs
// it is given and the commit its record keeps are of the commit that fetch brought, though main moves on the moment the
// fetch has been answered: a second fetch would bring the worktree past the commit the inputs were read at. The
// reference is the fetch count itself, which a second fetch adds one to, and the two commits compared with each other.
#[test]
fn an_architect_s_round_fetches_main_once_and_reads_one_commit_in_its_worktree_and_its_inputs() {
    let s = setup("main-one-fetch");
    main_moves_on(&s.root, &[("AGENTS.md", "Work the first way.\n")], "The first way");
    let first = git(&s.origin, &["rev-parse", "main"]);
    let fetches = Fetches::watching(&s);
    // The second way is committed now and reaches main right after the round's first fetch is answered.
    commit_on_main(&s.root, &[("AGENTS.md", "Work the second way.\n")], "The second way");
    let mut agents = scripts();
    let seen = keeps_prompts(&mut agents, &["B1"], |_: &Request, _: usize| (Vec::new(), Vec::new()));
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "B1");
    let before = fetches.so_far();
    fetches.push_after_the_next();
    e.advance(arch).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(fetches.so_far() - before, 1, "{log}");
    // Main did move on while the round was starting.
    let second = git(&s.origin, &["rev-parse", "main"]);
    assert_ne!(second, first, "the second way did not reach main\n{log}");
    // The worktree, the record and the text the round was given are all of the commit the one fetch brought.
    let worktree = s.data.join("worktrees").join("architect").join("p");
    assert_eq!(git(&worktree, &["rev-parse", "HEAD"]), first, "{log}");
    assert_eq!(
        std::fs::read_to_string(worktree.join("AGENTS.md")).unwrap(),
        "Work the first way.\n"
    );
    assert_eq!(pinned_main(&e, arch, "B1").as_deref(), Some(first.as_str()), "{log}");
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "{log}");
    assert!(seen[0].1.contains("Work the first way."), "{}", seen[0].1);
    assert!(!seen[0].1.contains("Work the second way."), "{}", seen[0].1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// An engineer's round makes one fetch of main, at the first cell (`R0`) and at the next (`R1`) alike: every agent's cell
// is given the constitution through the flow's common inputs, so each reads main, and the fetch is the round's own,
// made before its inputs are gathered. The reference is the fetch count of each step. This is new behaviour: an
// engineer's round read the checkout's files and made no fetch, so before the change both counts were 0 and the test
// fails there. (No agent's cell of the flow declares none of the inputs that read main, so a round that makes no fetch
// has no cell to be shown at here; that a cell reads main if and only if it declares one is held by the test of
// `reads_main` in `inputs.rs`.)
#[test]
fn an_engineer_s_round_now_fetches_main_once_at_each_agent_cell() {
    let s = setup("main-fetch-count");
    names_a_constitution(&s);
    main_moves_on(
        &s.root,
        &[("CONSTITUTION.md", "## Article 1. Small\n\nKeep it small.\n")],
        "A constitution",
    );
    let fetches = Fetches::watching(&s);
    let mut agents = scripts();
    keeps_prompts(&mut agents, &["R0", "R1"], |_: &Request, _: usize| {
        (Vec::new(), Vec::new())
    });
    let (mut e, said, item, _) = stands_at_r0(&s, agents, true);
    let work = state(&e, item).unwrap().0;
    let mut made: BTreeMap<String, usize> = BTreeMap::new();
    for _ in 0..12 {
        let Some((_, cell, _)) = state(&e, item) else { break };
        let before = fetches.so_far();
        e.advance(work).unwrap();
        if cell == "R0" || cell == "R1" {
            made.insert(cell, fetches.so_far() - before);
        }
        if made.len() == 2 {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        made,
        BTreeMap::from([("R0".to_string(), 1), ("R1".to_string(), 1)]),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A project's architect whose worktree cannot be brought to main does not start its round: with the network up its work
// is paused and says why, and with it down every work waits for it, and goes on from its cell once it is back (the
// round then starts: its prompt is built once). It holds before and after the fetch for the inputs became the same
// fetch as the worktree's.
#[test]
fn an_architect_whose_worktree_cannot_be_brought_to_main_is_paused_or_waits_and_goes_on_when_it_can() {
    for up in [true, false] {
        let s = setup(&format!("architect-no-main-{up}"));
        let net = Arc::new(std::sync::atomic::AtomicBool::new(up));
        let seen_net = net.clone();
        let hooks = Hooks {
            network: Some(Arc::new(move || seen_net.load(Ordering::SeqCst))),
            ..Hooks::default()
        };
        let mut agents = scripts();
        let seen = keeps_prompts(&mut agents, &["B1"], |_: &Request, _: usize| (Vec::new(), Vec::new()));
        let (mut e, said) = engine_with(&s, agents, hooks);
        quiet(&mut e);
        let arch = architect_work_at(&mut e, "B1");
        let away = s.root.join("origin-away");
        std::fs::rename(&s.origin, &away).unwrap();
        e.advance(arch).unwrap();
        let log = said.lock().unwrap().join("\n");
        let paused: Option<String> =
            e.db.conn()
                .query_row("SELECT paused FROM works WHERE id = ?1", [arch], |r| r.get(0))
                .unwrap();
        assert!(seen.lock().unwrap().is_empty(), "{log}");
        if up {
            let why = paused.unwrap_or_else(|| panic!("not paused\n{log}"));
            assert!(
                why.starts_with("cell B1: the architect's worktree ") && why.contains("could not be brought to main"),
                "{why}"
            );
            // Put right, the owner presses Start: the round starts, at the commit main has.
            std::fs::rename(&away, &s.origin).unwrap();
            api::resume(&mut e.db, arch).unwrap();
        } else {
            assert_eq!(paused, None, "{log}");
            assert_eq!(e.halt().map(|h| h.kind).as_deref(), Some("offline"), "{log}");
            std::fs::rename(&away, &s.origin).unwrap();
            net.store(true, Ordering::SeqCst);
            e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
        }
        e.advance(arch).unwrap();
        let log = said.lock().unwrap().join("\n");
        assert_eq!(seen.lock().unwrap().len(), 1, "{log}");
        let _ = std::fs::remove_dir_all(&s.root);
    }
}

// A constitution the configuration names that main does not have is no constitution to go without: no round of an
// engineer's work or a project's architect starts, and the work is paused with which project, which file and which
// commit, and how the owner puts it right.
#[test]
fn a_constitution_main_does_not_have_stops_the_rounds_and_says_how_to_put_it_right() {
    let s = setup("main-missing");
    names_a_constitution(&s);
    let main = git(&s.origin, &["rev-parse", "main"]);
    let mut agents = scripts();
    let seen = keeps_prompts(&mut agents, &["R0", "B1"], |_: &Request, _: usize| {
        (Vec::new(), Vec::new())
    });
    let (mut e, said, item) = start(&s, agents);
    let arch = architect_work_at(&mut e, "B1");
    for _ in 0..4 {
        e.tick().unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert!(seen.lock().unwrap().is_empty(), "{log}");
    let paused = |work: i64| -> Option<String> {
        e.db.conn()
            .query_row("SELECT paused FROM works WHERE id = ?1", [work], |r| r.get(0))
            .unwrap()
    };
    for (work, cell) in [(state(&e, item).unwrap().0, "R0"), (arch, "B1")] {
        let why = paused(work).unwrap_or_else(|| panic!("work {work} is not paused\n{log}"));
        assert!(
            why.starts_with(&format!("cell {cell}: project p: constitution: ")),
            "{why}"
        );
        assert!(why.contains("CONSTITUTION.md") && why.contains(&main), "{why}");
        assert!(
            why.contains("change the constitution's path") && why.contains("press Start"),
            "{why}"
        );
    }
    assert!(told(&s, "CONSTITUTION.md") >= 2);
    // The rounds did not start: none of the cells has one.
    let started: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM rounds WHERE cell IN ('R0', 'B1')", [], |r| {
                r.get(0)
            })
            .unwrap();
    assert_eq!(started, 0, "{log}");
    // The owner puts the file back on main and presses Start: both rounds start, and are given it.
    main_moves_on(
        &s.root,
        &[("CONSTITUTION.md", "## Article 1. Small\n\nKeep it small.\n")],
        "The constitution",
    );
    let engineer = state(&e, item).unwrap().0;
    api::resume(&mut e.db, engineer).unwrap();
    api::resume(&mut e.db, arch).unwrap();
    for _ in 0..6 {
        if seen.lock().unwrap().len() >= 2 {
            break;
        }
        e.tick().unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    let seen = seen.lock().unwrap();
    let mut cells: Vec<&str> = seen.iter().map(|x| x.0.as_str()).collect();
    cells.sort_unstable();
    assert_eq!(cells, ["B1", "R0"], "{log}");
    assert!(seen.iter().all(|x| x.1.contains("Keep it small.")), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// An engine whose network the test sets to up or down, over a setup with the scripts given, whose item's work stands at
/// the cell that gathers the history, before any round of it. Gives the engine, what it says, the item and the network.
fn stands_at_r0(
    s: &Setup,
    scripts: BTreeMap<&'static str, Script>,
    up: bool,
) -> (Engine, Arc<Mutex<Vec<String>>>, i64, Arc<std::sync::atomic::AtomicBool>) {
    let net = Arc::new(std::sync::atomic::AtomicBool::new(up));
    let seen = net.clone();
    let hooks = Hooks {
        network: Some(Arc::new(move || seen.load(Ordering::SeqCst))),
        ..Hooks::default()
    };
    let (mut e, said, item) = start_with(s, scripts, hooks);
    for _ in 0..6 {
        if state(&e, item).is_some_and(|x| x.1 == "R0") {
            break;
        }
        e.pass(&Go::default()).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(state(&e, item).map(|x| x.1).as_deref(), Some("R0"), "{log}");
    (e, said, item, net)
}

/// What the latest round of `work` at `cell` recorded of why main could not be fetched for it.
fn main_fetch_failed(e: &Engine, work: i64, cell: &str) -> Option<String> {
    e.db.conn()
        .query_row(
            "SELECT json_extract(input, '$.engine.main_fetch_failed') FROM rounds WHERE work = ?1 AND cell = ?2
             ORDER BY n DESC LIMIT 1",
            params![work, cell],
            |r| r.get(0),
        )
        .unwrap()
}

// Main is fetched for a round of a cell that reads it. A fetch that fails does not stop the round while the checkout has
// a commit of main from an earlier fetch, with the network up or down alike: the round goes on at that commit, is given
// the text there, and its record names the commit and says why the fetch failed; no work is paused or waits. The next
// round, with the remote back, is at the new commit and says nothing of a failure.
#[test]
fn a_failed_fetch_of_main_goes_on_at_the_commit_the_checkout_has_and_says_so() {
    for up in [true, false] {
        let s = setup(&format!("main-fetch-{up}"));
        names_a_constitution(&s);
        let old = "## Article 1. Small\n\nKeep it small.\n";
        let new = "## Article 1. Small\n\nKeep it smaller, always.\n";
        main_moves_on(&s.root, &[("CONSTITUTION.md", old)], "A constitution");
        let mut agents = scripts();
        let seen = keeps_prompts(&mut agents, &["R0", "R1"], |_: &Request, _: usize| {
            (Vec::new(), Vec::new())
        });
        let (mut e, said, item, net) = stands_at_r0(&s, agents, up);
        // The checkout has fetched the old constitution; main moves on, and the remote is then away.
        let checkout = s.root.join("checkout");
        let had = git(&checkout, &["rev-parse", "origin/main"]);
        main_moves_on(&s.root, &[("CONSTITUTION.md", new)], "A stricter constitution");
        let latest = git(&s.origin, &["rev-parse", "main"]);
        let away = s.root.join("origin-away");
        std::fs::rename(&s.origin, &away).unwrap();
        e.pass(&Go::default()).unwrap();
        let log = said.lock().unwrap().join("\n");
        let work = state(&e, item).unwrap().0;
        {
            let seen = seen.lock().unwrap();
            assert_eq!(seen.len(), 1, "{log}");
            assert!(seen[0].1.contains("Keep it small."), "{}", seen[0].1);
            assert!(!seen[0].1.contains("Keep it smaller, always."), "{}", seen[0].1);
        }
        assert_eq!(pinned_main(&e, work, "R0").as_deref(), Some(had.as_str()), "{log}");
        let why = main_fetch_failed(&e, work, "R0").unwrap_or_else(|| panic!("no failure recorded\n{log}"));
        assert!(why.starts_with("project p: main of o/p could not be fetched"), "{why}");
        assert!(log.contains(&format!("the round goes on at {had}")), "{log}");
        assert_eq!(state(&e, item).unwrap().2, None, "{log}");
        assert!(e.halt().is_none(), "{log}");
        // The remote is back: the next round is at the new commit, with the new text, and records no failure.
        std::fs::rename(&away, &s.origin).unwrap();
        net.store(true, Ordering::SeqCst);
        for _ in 0..6 {
            if seen.lock().unwrap().len() >= 2 {
                break;
            }
            e.pass(&Go::default()).unwrap();
        }
        let log = said.lock().unwrap().join("\n");
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2, "{log}");
        assert!(seen[1].1.contains("Keep it smaller, always."), "{}", seen[1].1);
        assert_eq!(pinned_main(&e, work, "R1").as_deref(), Some(latest.as_str()), "{log}");
        assert_eq!(main_fetch_failed(&e, work, "R1"), None, "{log}");
        let _ = std::fs::remove_dir_all(&s.root);
    }
}

// Where main cannot be fetched and the checkout has no commit of main either, the round does not start: with the network
// up the work is paused and says why, the failed fetch first, and with it down every work waits for it, and goes on
// from its cell once it is back.
#[test]
fn a_round_does_not_start_when_main_cannot_be_fetched_and_the_checkout_has_none() {
    for up in [true, false] {
        let s = setup(&format!("main-gone-{up}"));
        let (mut e, said, item, net) = stands_at_r0(&s, scripts(), up);
        git(
            &s.root.join("checkout"),
            &["update-ref", "-d", "refs/remotes/origin/main"],
        );
        let away = s.root.join("origin-away");
        std::fs::rename(&s.origin, &away).unwrap();
        e.pass(&Go::default()).unwrap();
        let log = said.lock().unwrap().join("\n");
        let work = state(&e, item).unwrap().0;
        let rounds_at_r0 = |e: &Engine| -> i64 {
            e.db.conn()
                .query_row(
                    "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'R0'",
                    [work],
                    |r| r.get(0),
                )
                .unwrap()
        };
        assert_eq!(rounds_at_r0(&e), 0, "{log}");
        let (_, _, paused) = state(&e, item).unwrap();
        if up {
            let why = paused.unwrap_or_else(|| panic!("not paused\n{log}"));
            assert!(
                why.starts_with("cell R0: project p: main of o/p could not be fetched"),
                "{why}"
            );
            assert!(why.contains("origin/main is not in the checkout"), "{why}");
        } else {
            assert_eq!(paused, None, "{log}");
            assert_eq!(e.halt().map(|h| h.kind).as_deref(), Some("offline"), "{log}");
            // Back: the round starts from its cell.
            std::fs::rename(&away, &s.origin).unwrap();
            net.store(true, Ordering::SeqCst);
            e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
            e.pass(&Go::default()).unwrap();
            let log = said.lock().unwrap().join("\n");
            assert_eq!(rounds_at_r0(&e), 1, "{log}");
            assert_eq!(state(&e, item).unwrap().2, None, "{log}");
        }
        let _ = std::fs::remove_dir_all(&s.root);
    }
}

// The message a work's next round would be sent is built with nothing run, recorded or fetched: it shows the project as
// main is in the checkout now, and leaves the checkout's refs as they were. The round itself fetches.
#[test]
fn the_preview_of_a_message_fetches_nothing_and_the_round_does() {
    let s = setup("main-preview");
    names_a_constitution(&s);
    let checkout = s.root.join("checkout");
    main_moves_on(
        &s.root,
        &[("CONSTITUTION.md", "## Article 1. Small\n\nKeep it small.\n")],
        "A constitution",
    );
    git(&checkout, &["fetch", "-q", "origin", "main"]);
    let fetched = git(&checkout, &["rev-parse", "origin/main"]);
    main_moves_on(
        &s.root,
        &[("CONSTITUTION.md", "## Article 1. Small\n\nKeep it smaller, always.\n")],
        "A stricter constitution",
    );
    let mut agents = scripts();
    let seen = keeps_prompts(&mut agents, &["B1"], |_: &Request, _: usize| (Vec::new(), Vec::new()));
    let (mut e, _) = engine(&s, agents);
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "B1");
    let preview = engine::next_message(&mut e, arch).unwrap().text;
    assert!(preview.contains("Keep it small."), "{preview}");
    assert!(!preview.contains("Keep it smaller, always."), "{preview}");
    assert_eq!(git(&checkout, &["rev-parse", "origin/main"]), fetched);
    // The round fetches main, and is given what it has.
    e.advance(arch).unwrap();
    let prompt = seen.lock().unwrap()[0].1.clone();
    assert!(prompt.contains("Keep it smaller, always."), "{prompt}");
    assert_eq!(
        git(&checkout, &["rev-parse", "origin/main"]),
        git(&s.origin, &["rev-parse", "main"])
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// Nothing of a project is read from its checkout's own branch or files: a checkout with no `origin/main` has no text to
// give, though its branch and its files are there, and the message says which ref is missing.
#[test]
fn a_checkout_without_the_remote_s_main_gives_no_text_from_its_own_branch() {
    let s = setup("main-no-remote");
    let checkout = s.root.join("checkout");
    git(&checkout, &["update-ref", "-d", "refs/remotes/origin/main"]);
    assert!(checkout.join("AGENTS.md").exists());
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "B1");
    let err = match engine::next_message(&mut e, arch) {
        Ok(m) => panic!("a message was built from the checkout's own files: {}", m.text),
        Err(err) => err,
    };
    assert!(err.contains("project p") && err.contains("origin/main"), "{err}");
    let _ = std::fs::remove_dir_all(&s.root);
}
