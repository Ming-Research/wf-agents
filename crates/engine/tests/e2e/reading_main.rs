//! What a round reads of the project's main: one commit fetched for it, and what comes of a fetch that fails.

use super::*;

/// Notes each fetch that reaches project p's remote from its checkout. A fetch runs `git upload-pack` at the remote,
/// which the checkout's remote is set here to run as a script that notes the run and then does it; so every fetch is
/// noted once, whoever makes it. The script can also push the commit that [`commit_on_main`] made right after the next
/// fetch is answered, as another work's push reaching main while a fetch is under way.
struct Fetches {
    root: PathBuf,
}

impl Fetches {
    fn watching(s: &Setup) -> Fetches {
        let root = s.root.clone();
        let script = root.join("upload-pack.sh");
        let r = root.display();
        write(
            &root,
            "upload-pack.sh",
            &format!(
                "#!/bin/sh\n\
                 echo fetch >> '{r}/fetches'\n\
                 git upload-pack \"$@\"\n\
                 status=$?\n\
                 if [ -f '{r}/push-after' ]; then\n\
                 \x20 rm '{r}/push-after'\n\
                 \x20 (unset GIT_DIR; git -C '{r}/elsewhere' push -q origin HEAD:main)\n\
                 fi\n\
                 exit $status\n"
            ),
        );
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        git(
            &root.join("checkout"),
            &["config", "remote.origin.uploadpack", &script.to_string_lossy()],
        );
        Fetches { root }
    }

    /// How many fetches there have been since the watching began.
    fn so_far(&self) -> usize {
        std::fs::read_to_string(self.root.join("fetches")).map_or(0, |t| t.lines().count())
    }

    /// Has the commit made with [`commit_on_main`] pushed to main right after the next fetch is answered.
    fn push_after_the_next(&self) {
        write(&self.root, "push-after", "");
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
    let seen = keeps_prompts(&mut agents, &["G1", "R1"], move |r: &Request, _: usize| {
        if r.cell == "G1" {
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
        ["G1", "R1"],
        "{log}"
    );
    assert!(seen[0].1.contains("Keep it small."), "{}", seen[0].1);
    assert!(seen[1].1.contains("Keep it smaller, always."), "{}", seen[1].1);
    let work = state(&e, item).unwrap().0;
    assert_eq!(pinned_main(&e, work, "G1").as_deref(), Some(first.as_str()), "{log}");
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
// checkout. What its return is checked against, the articles of the constitution it was given, and the hash of the
// constitution a check of the policy it queues keeps, are of that commit too.
#[test]
fn a_round_reads_one_commit_of_main_and_what_follows_it_reads_the_same() {
    let s = setup("main-pinned");
    no_reply_wait(&s);
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
    let seen = keeps_prompts(&mut agents, &["U1", "P1"], move |r: &Request, _: usize| {
        // Main moves on while the round runs, and another work's fetch brings it into the checkout.
        let (articles, rows) = if r.cell == "U1" { (&to_three, 0) } else { (&to_four, 3) };
        main_moves_on(
            &root,
            &[
                ("CONSTITUTION.md", articles.as_str()),
                ("AGENTS.md", "Work the third way.\n"),
            ],
            "More articles",
        );
        git(&checkout, &["fetch", "-q", "origin", "main"]);
        // The check of the policy was given the articles of the commit it read: three, then.
        let against: Vec<Value> = ["Small", "Quick", "Plain"]
            .iter()
            .enumerate()
            .take(rows)
            .map(|(i, what)| {
                json!({ "article": format!("Article {}. {what}", i + 1), "status": "kept", "where": "all", "grounds": "seen" })
            })
            .collect();
        (
            Vec::new(),
            if rows == 0 {
                Vec::new()
            } else {
                vec![("outcome.against", json!(against))]
            },
        )
    });
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "U1");
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
        ["U1", "P1"],
        "{log}"
    );
    // The constitution and the guide it was given are those of the commit it was pinned to, not of the later ones.
    let (u1, p1) = (&seen[0].1, &seen[1].1);
    assert!(
        u1.contains("Keep it quick.") && u1.contains("Work the second way."),
        "{u1}"
    );
    assert!(!u1.contains("Keep it plain.") && !u1.contains("third way"), "{u1}");
    assert!(p1.contains("Keep it plain.") && !p1.contains("Keep it warm."), "{p1}");
    assert_eq!(pinned_main(&e, arch, "U1").as_deref(), Some(first.as_str()), "{log}");
    assert_eq!(pinned_main(&e, arch, "P1").as_deref(), Some(second.as_str()), "{log}");
    // Its check of the policy was queued under the hash of the constitution of that commit.
    let kept: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT json_extract(body, '$.constitution') FROM architect_matters
                 WHERE project = 'p' AND kind = 'check_policy' ORDER BY id LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(kept, Some(db::content_hash(&two)), "{log}");
    // The articles the check counted are those of the commit its round read: three, though main has four by now.
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
    no_reply_wait(&s);
    main_moves_on(&s.root, &[("AGENTS.md", "Work the first way.\n")], "The first way");
    let first = git(&s.origin, &["rev-parse", "main"]);
    let fetches = Fetches::watching(&s);
    // The second way is committed now and reaches main right after the round's first fetch is answered.
    commit_on_main(&s.root, &[("AGENTS.md", "Work the second way.\n")], "The second way");
    let mut agents = scripts();
    let seen = keeps_prompts(&mut agents, &["U1"], |_: &Request, _: usize| (Vec::new(), Vec::new()));
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "U1");
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
    assert_eq!(pinned_main(&e, arch, "U1").as_deref(), Some(first.as_str()), "{log}");
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "{log}");
    assert!(seen[0].1.contains("Work the first way."), "{}", seen[0].1);
    assert!(!seen[0].1.contains("Work the second way."), "{}", seen[0].1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// An engineer's round makes one fetch of main, at the first cell (`G1`) and at the next (`R1`) alike: every agent's cell
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
    keeps_prompts(&mut agents, &["G1", "R1"], |_: &Request, _: usize| {
        (Vec::new(), Vec::new())
    });
    let (mut e, said, item, _) = stands_at_g1(&s, agents, true);
    let work = state(&e, item).unwrap().0;
    let mut made: BTreeMap<String, usize> = BTreeMap::new();
    for _ in 0..12 {
        let Some((_, cell, _)) = state(&e, item) else { break };
        let before = fetches.so_far();
        e.advance(work).unwrap();
        if cell == "G1" || cell == "R1" {
            made.insert(cell, fetches.so_far() - before);
        }
        if made.len() == 2 {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        made,
        BTreeMap::from([("G1".to_string(), 1), ("R1".to_string(), 1)]),
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
        no_reply_wait(&s);
        let net = Arc::new(std::sync::atomic::AtomicBool::new(up));
        let seen_net = net.clone();
        let hooks = Hooks {
            network: Some(Arc::new(move || seen_net.load(Ordering::SeqCst))),
            ..Hooks::default()
        };
        let mut agents = scripts();
        let seen = keeps_prompts(&mut agents, &["U1"], |_: &Request, _: usize| (Vec::new(), Vec::new()));
        let (mut e, said) = engine_with(&s, agents, hooks);
        quiet(&mut e);
        let arch = architect_work_at(&mut e, "U1");
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
                why.starts_with("cell U1: the architect's worktree ") && why.contains("could not be brought to main"),
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
    let seen = keeps_prompts(&mut agents, &["G1", "U1"], |_: &Request, _: usize| {
        (Vec::new(), Vec::new())
    });
    let (mut e, said, item) = start(&s, agents);
    let arch = architect_work_at(&mut e, "U1");
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
    for (work, cell) in [(state(&e, item).unwrap().0, "G1"), (arch, "U1")] {
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
            .query_row("SELECT count(*) FROM rounds WHERE cell IN ('G1', 'U1')", [], |r| {
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
    assert_eq!(cells, ["G1", "U1"], "{log}");
    assert!(seen.iter().all(|x| x.1.contains("Keep it small.")), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// An engine whose network the test sets to up or down, over a setup with the scripts given, whose item's work stands at
/// the cell that gathers the history, before any round of it. Gives the engine, what it says, the item and the network.
fn stands_at_g1(
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
        if state(&e, item).is_some_and(|x| x.1 == "G1") {
            break;
        }
        e.pass(&Go::default()).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(state(&e, item).map(|x| x.1).as_deref(), Some("G1"), "{log}");
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
        let seen = keeps_prompts(&mut agents, &["G1", "R1"], |_: &Request, _: usize| {
            (Vec::new(), Vec::new())
        });
        let (mut e, said, item, net) = stands_at_g1(&s, agents, up);
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
        assert_eq!(pinned_main(&e, work, "G1").as_deref(), Some(had.as_str()), "{log}");
        let why = main_fetch_failed(&e, work, "G1").unwrap_or_else(|| panic!("no failure recorded\n{log}"));
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
        let (mut e, said, item, net) = stands_at_g1(&s, scripts(), up);
        git(
            &s.root.join("checkout"),
            &["update-ref", "-d", "refs/remotes/origin/main"],
        );
        let away = s.root.join("origin-away");
        std::fs::rename(&s.origin, &away).unwrap();
        e.pass(&Go::default()).unwrap();
        let log = said.lock().unwrap().join("\n");
        let work = state(&e, item).unwrap().0;
        let rounds_at_g1 = |e: &Engine| -> i64 {
            e.db.conn()
                .query_row(
                    "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'G1'",
                    [work],
                    |r| r.get(0),
                )
                .unwrap()
        };
        assert_eq!(rounds_at_g1(&e), 0, "{log}");
        let (_, _, paused) = state(&e, item).unwrap();
        if up {
            let why = paused.unwrap_or_else(|| panic!("not paused\n{log}"));
            assert!(
                why.starts_with("cell G1: project p: main of o/p could not be fetched"),
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
            assert_eq!(rounds_at_g1(&e), 1, "{log}");
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
    no_reply_wait(&s);
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
    let seen = keeps_prompts(&mut agents, &["U1"], |_: &Request, _: usize| (Vec::new(), Vec::new()));
    let (mut e, _) = engine(&s, agents);
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "U1");
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
    no_reply_wait(&s);
    let checkout = s.root.join("checkout");
    git(&checkout, &["update-ref", "-d", "refs/remotes/origin/main"]);
    assert!(checkout.join("AGENTS.md").exists());
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "U1");
    let err = match engine::next_message(&mut e, arch) {
        Ok(m) => panic!("a message was built from the checkout's own files: {}", m.text),
        Err(err) => err,
    };
    assert!(err.contains("project p") && err.contains("origin/main"), "{err}");
    let _ = std::fs::remove_dir_all(&s.root);
}
