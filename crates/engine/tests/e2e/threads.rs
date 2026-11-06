//! Steps on threads of their own: works side by side, halts, and the waits a step sets.

use super::*;

// A step taken on another thread, on a context forked from the engine with a connection of its own, writes the same
// rounds and changes as the same step taken on the engine's own thread from the same database, the numbers and times
// of the transactions and the ids drawn at random aside; and the one commit listener the engine was given hears of
// what that connection wrote, as the owner's page hears of every change.
#[test]
fn a_step_on_another_thread_writes_what_it_writes_on_the_engines_own() {
    let s = setup("fork_step");
    let (mut e, said, item) = start(&s, scripts());
    // The first step starts the work and sets up its worktree; the next is the round of its first agent's cell.
    e.step(None).unwrap();
    let at = state(&e, item).unwrap().1;
    let before = s.root.join("before.db");
    e.db.conn()
        .execute("VACUUM INTO ?1", [before.to_string_lossy().into_owned()])
        .unwrap();
    let base = written(&e);
    let on_main = e.step(None).unwrap();
    let main = written(&e);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(on_main.cell.as_deref(), Some(at.as_str()), "{on_main:?}\n{log}");
    assert!(main.len() > base.len(), "{log}");
    drop(e);
    // The database as it was before that step, and the same step taken on another thread.
    let path = config::database(&s.data);
    for end in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{end}", path.display()));
    }
    std::fs::copy(&before, &path).unwrap();
    let heard = Arc::new(Mutex::new(Vec::<db::Changed>::new()));
    let h = heard.clone();
    let hooks = Hooks {
        commit: Some(Arc::new(move |_: &db::rusqlite::Connection, rows: &[db::Changed]| {
            h.lock().unwrap().extend(rows.iter().cloned());
        })),
        ..Hooks::default()
    };
    let (e, said) = engine_with(&s, scripts(), hooks);
    let mut t = e.fork().unwrap();
    let on_thread = std::thread::spawn(move || t.step(None)).join().unwrap().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(on_thread, on_main, "{log}");
    assert_eq!(written(&e), main, "{log}");
    let round = on_thread.rounds.first().unwrap_or_else(|| panic!("no round:\n{log}"));
    let rowid: i64 =
        e.db.conn()
            .query_row(
                "SELECT rowid FROM rounds WHERE work = ?1 AND n = ?2",
                [round.work, round.n],
                |r| r.get(0),
            )
            .unwrap();
    let heard = heard.lock().unwrap().clone();
    let told = db::Changed {
        table: "rounds".into(),
        rowid,
    };
    assert!(heard.contains(&told), "{heard:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A step reports the rounds its own connection wrote: the round of the work it took, and a round it cut off before
// that, as one an engine that stopped left open; never a round another connection wrote meanwhile, as another
// thread's step would.
#[test]
fn a_step_reports_the_rounds_its_own_connection_wrote() {
    let s = setup("own_rounds");
    let path = config::database(&s.data);
    let mut scripts = scripts();
    // While the round of cell `R0` runs, another connection writes a round of the architect's work over before.
    scripts.insert(
        "R0",
        Box::new(move |_: &Request, _: usize| {
            let mut db = db::Db::open(&path).unwrap();
            db.write(Actor::Engine, |tx| {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                     SELECT w.id, (SELECT max(r.n) FROM rounds r WHERE r.work = w.id) + 1, 'H10', '{}', '{}', 'done',
                            'c', ?1, ?1
                     FROM works w WHERE w.graph = 'architect'",
                    [db::now()],
                )?;
                Ok(())
            })
            .unwrap();
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    let first = e.step(None).unwrap();
    let work = first.work.unwrap();
    let r1 = e.step(first.work).unwrap();
    let log = said.lock().unwrap().join("\n");
    let n: i64 =
        e.db.conn()
            .query_row("SELECT max(n) FROM rounds WHERE work = ?1", [work], |r| r.get(0))
            .unwrap();
    let architects: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds r JOIN works w ON w.id = r.work WHERE w.graph = 'architect'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(r1.cell.as_deref(), Some("R0"), "{r1:?}\n{log}");
    assert_eq!(architects, 2, "{log}");
    assert_eq!(r1.rounds, [engine::RoundId::new(work, n)], "{log}");
    // A round of the work left open, as by an engine that stopped while it ran: the next step cuts it off, and reports
    // it with the round it runs in its place.
    let at = state(&e, item).unwrap().1;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (?1, ?2, ?3, '{}', 'c', ?4)",
            params![work, n + 1, at, db::now()],
        )?;
        Ok(())
    })
    .unwrap();
    let again = e.step(Some(work)).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(again.work, Some(work), "{again:?}\n{log}");
    assert!(
        again.rounds.contains(&engine::RoundId::new(work, n + 1))
            && again.rounds.contains(&engine::RoundId::new(work, n + 2)),
        "{again:?}\n{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// What lasts from one step to the next is the engine's, whichever thread's context takes the step: a work taken over on
// a forked context's thread is among those the engine's own context reads as replaced, which is how the owner's Start
// of the old work goes on in the new one; a configuration one context reads is the one every context goes on with,
// told once; and a halt one context sets stops a step on another.
#[test]
fn a_forked_context_shares_what_lasts_between_steps() {
    let s = setup("fork_shared");
    let (mut e, said, item) = start(&s, scripts());
    let first = e.step(None).unwrap();
    let work = first.work.unwrap();
    e.step(first.work).unwrap();
    api::start_over(&mut e.db, work).unwrap();
    let mut t = e.fork().unwrap();
    let (mut t, over) = std::thread::spawn(move || {
        let over = t.step(Some(work));
        (t, over)
    })
    .join()
    .unwrap();
    let over = over.unwrap();
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 2, "{works:?}\n{over:?}\n{log}");
    let new = works[1].0;
    assert_eq!(e.replaced(), [(work, new)], "{log}");
    assert_eq!(t.replaced(), Vec::<(i64, i64)>::new(), "{log}");
    // A new configuration, read on the engine's own context, is the forked context's at its next read.
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(&s.data, "install.toml", &format!("{install}\n# read once\n"));
    git(&s.data, &["commit", "-q", "-am", "a configuration read once"]);
    e.reload_config();
    let commit = e.config.commit.clone();
    t.reload_config();
    assert_eq!(t.config.commit, commit);
    let log = said.lock().unwrap().join("\n");
    let in_use = format!("configuration {commit} in use");
    assert_eq!(log.matches(&in_use).count(), 1, "{log}");
    // A halt set on the engine's own context stops the forked context's step.
    e.set_halt(Some(engine::Halt {
        kind: "usage_limit".into(),
        until: db::now() + 3_600_000,
        why: "a usage limit".into(),
    }));
    let rounds: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM rounds", [], |r| r.get(0))
            .unwrap();
    let halted = std::thread::spawn(move || t.step(Some(new))).join().unwrap().unwrap();
    assert_eq!(halted, engine::Stepped::default(), "{log}");
    let after: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM rounds", [], |r| r.get(0))
            .unwrap();
    assert_eq!(after, rounds);
    let _ = std::fs::remove_dir_all(&s.root);
}

// Two works on two ready items of one project, with two slots free on the machine, run their agents' rounds side by
// side: each round of `R0` waits until the other's has come, or ten seconds, so that rounds taken one after the other
// do not overlap. While both run, the engine says both works' rounds run, which the owner's page shows as running.
#[test]
fn two_works_run_their_agents_rounds_side_by_side() {
    let met = Gate::default();
    let at = met.clone();
    let mut agents = scripts();
    agents.insert(
        "R0",
        Box::new(move |_: &Request, _: usize| {
            at.meet(2);
            (Vec::new(), Vec::new())
        }),
    );
    let busy = Arc::new(Mutex::new(Vec::new()));
    let seen = busy.clone();
    let hooks = Hooks {
        busy: Arc::new(move |w: &[i64]| seen.lock().unwrap().push(w.to_vec())),
        ..Hooks::default()
    };
    let (s, mut e, said, first, second) = two_works_with("side-by-side", agents, hooks);
    // The first pass starts both works; the second runs the round of `R0` of each.
    e.tick().unwrap();
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let span = |item: i64| -> (i64, i64) {
        e.db.conn()
            .query_row(
                "SELECT r.started_at, r.ended_at FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.item = ?1 AND r.cell = 'R0'",
                [item],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or_else(|x| panic!("{x}\n{log}"))
    };
    let (a, b) = (span(first), span(second));
    assert!(a.0 < b.1 && b.0 < a.1, "{a:?} {b:?}\n{log}");
    let works = vec![state(&e, first).unwrap().0, state(&e, second).unwrap().0];
    let told = busy.lock().unwrap().clone();
    assert!(told.contains(&works), "{told:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// While one work's CI has not come back, another work of the same project goes on: the fake GitHub holds every check
// until the test lets it through, and the owner's global pause holds the second work until the first's CI is under
// way.
#[test]
fn a_work_goes_on_while_another_waits_for_its_ci() {
    let (s, mut e, said, first, second) = two_works_with("beside-ci", scripts(), Hooks::default());
    s.ci.shut();
    e.tick().unwrap();
    let (a, b) = (state(&e, first).unwrap().0, state(&e, second).unwrap().0);
    let only_first = Go {
        pause: held_but(&[a], &[]),
        ..Go::default()
    };
    let ci = turns_until(&mut e, &only_first, |_| s.ci.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(ci, "{log}");
    let since = tool_round_open(&e, a).expect(&log);
    let went_on = turns_until(&mut e, &Go::default(), |e| rounds_after(e, b, since).1 > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(went_on, "{log}");
    // The first's CI is still under way: its step in flight, its round open.
    assert!(e.in_flight().contains(&a), "{log}");
    assert_eq!(tool_round_open(&e, a), Some(since), "{log}");
    s.ci.open();
    e.drain().unwrap();
    let _ = std::fs::remove_dir_all(&s.root);
}

// Two works that run side by side each merge into main through the flow, in the order they came into the project's
// merge queue, the work with the larger number coming first: while its merge runs, held at its CI, the other comes,
// reads that it is behind and records a round saying so, and merges only after the first's merge round ended. The order
// they came in is read from the times their rounds before the queue ended, the merges' order from main's log and the
// rounds' times; the `merge_queue` view, whose order is what is tested, is read nowhere here. Every change to their
// state is one transaction recorded in `changes`, the rows of a transaction written at one time by one writer, one
// after the other; each work's rounds are numbered from one, none twice or missing, and none is left open.
#[test]
fn two_works_side_by_side_merge_in_turn_and_record_every_change_once() {
    let mut agents = scripts();
    // Each work changes a file of its own, so that the second has something to merge after the first.
    agents.insert(
        "I1",
        Box::new(|r: &Request, _: usize| {
            let wt = &r.worktree;
            let file = format!("src/work-{}.txt", r.work);
            if !wt.join(&file).exists() {
                write(wt, &file, "feature\n");
                git(wt, &["add", "-A"]);
                git(wt, &["commit", "-q", "-m", "Add the feature"]);
                let head = git(wt, &["rev-parse", "HEAD"]);
                let tools = json!([{ "tool": "ci", "head": head, "tests": ["feature"], "pr": { "title": "Add the feature", "body": "It adds it." } }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            let head = git(wt, &["rev-parse", "HEAD"]);
            (
                Vec::new(),
                vec![
                    ("outcome.head", json!(head)),
                    ("outcome.conditions", json!([{ "how": "tested", "id": "A1", "tests": ["feature"] }])),
                ],
            )
        }),
    );
    let (s, mut e, said, first, second) = two_works_with("both-merge", agents, Hooks::default());
    let over = |e: &Engine, item: i64| state(e, item).is_some_and(|w| w.1 == "E1");
    let said_so_far = || said.lock().unwrap().join("\n");
    let cell = |e: &Engine, work: i64| -> String {
        e.db.conn()
            .query_row("SELECT at FROM works WHERE id = ?1", [work], |r| r.get(0))
            .unwrap()
    };
    // The rounds of a work's merge cell, oldest first: the row each took, when it started and when it ended.
    let merge_rounds = |e: &Engine, work: i64| -> Vec<(Option<String>, i64, Option<i64>)> {
        let sql = "SELECT transition, started_at, ended_at FROM rounds WHERE work = ?1 AND cell = 'M2' ORDER BY n";
        let mut st = e.db.conn().prepare(sql).unwrap();
        st.query_map([work], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    // A work goes on alone until it stands at `until`, not having taken that cell's step.
    let goes_on_to = |e: &mut Engine, work: i64, until: &str| {
        for _ in 0..300 {
            if cell(&*e, work) == until {
                return;
            }
            e.advance(work).unwrap();
        }
    };
    // The first pass starts both works; `a` began first and has the smaller number.
    e.tick().unwrap();
    let (a, b) = (state(&e, first).unwrap().0, state(&e, second).unwrap().0);
    assert!(a < b);
    // `a` stops short of the merge queue; `b`, which began after it, comes into the queue first.
    goes_on_to(&mut e, a, "M1");
    goes_on_to(&mut e, b, "M2");
    let (at_a, at_b) = (cell(&e, a), cell(&e, b));
    assert_eq!((at_a.as_str(), at_b.as_str()), ("M1", "M2"), "{}", said_so_far());
    // `b` merges with its CI held, its merge round open. `a` comes into the queue meanwhile.
    s.ci.shut();
    let held = s.ci.came();
    let only_b = Go {
        pause: held_but(&[b], &[]),
        ..Go::default()
    };
    let merging = turns_until(&mut e, &only_b, |_| s.ci.came() > held);
    assert!(merging, "{}", said_so_far());
    // A step's round is written when it starts and its row when it ends, in two transactions: wait for `a`'s first
    // merge round to end, or the row is read before it is written. A merge round of `a` would not end, held at the CI.
    let in_the_queue = turns_until(&mut e, &Go::default(), |e| {
        merge_rounds(&*e, a).first().is_some_and(|r| r.2.is_some())
    });
    let (a_rounds, b_rounds) = (merge_rounds(&e, a), merge_rounds(&e, b));
    s.ci.open();
    assert!(in_the_queue, "{}", said_so_far());
    // `a` read that it is behind and recorded a round saying so, while `b`'s merge round is open still.
    assert_eq!(
        a_rounds.iter().map(|r| r.0.as_deref()).collect::<Vec<_>>(),
        [Some("behind")],
        "{}",
        said_so_far()
    );
    assert!(
        b_rounds.len() == 1 && b_rounds[0].0.is_none() && b_rounds[0].2.is_none(),
        "{b_rounds:?}\n{}",
        said_so_far()
    );
    let finished = turns_until(&mut e, &Go::default(), |e| over(&*e, first) && over(&*e, second));
    let log = said_so_far();
    assert!(finished, "{log}");
    for item in [first, second] {
        let paused = state(&e, item).and_then(|w| w.2);
        assert!(paused.is_none(), "{paused:?}\n{log}");
    }
    // The order they came into the queue in: when each one's round before it ended, `b`'s the earlier.
    let entered = |work: i64| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT max(ended_at) FROM rounds WHERE work = ?1 AND cell = 'M1'",
                [work],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert!(entered(b) < entered(a), "{} {}\n{log}", entered(b), entered(a));
    let queued = vec![b, a];
    // Each merged once, `a` after `b`'s merge round ended; the round in which `a` said it was behind lay within it.
    let merged_round = |work: i64| -> Vec<(i64, i64)> {
        merge_rounds(&e, work)
            .into_iter()
            .filter(|r| r.0.as_deref() == Some("merged"))
            .map(|r| (r.1, r.2.unwrap()))
            .collect()
    };
    let (a_merged, b_merged) = (merged_round(a), merged_round(b));
    assert!(
        a_merged.len() == 1 && b_merged.len() == 1,
        "{a_merged:?} {b_merged:?}\n{log}"
    );
    let (a_merged, b_merged) = (a_merged[0], b_merged[0]);
    assert!(a_merged.0 >= b_merged.1, "{a_merged:?} {b_merged:?}\n{log}");
    assert!(
        a_rounds[0].1 >= b_merged.0 && a_rounds[0].2.is_some_and(|ended| ended <= b_merged.1),
        "{a_rounds:?} {b_merged:?}\n{log}"
    );
    // Merged onto main in the queue's order: the merges, oldest first, by the works whose pull requests they merged.
    let subjects = git(
        &s.root,
        &["--git-dir", &s.origin.to_string_lossy(), "log", "--format=%s", "main"],
    );
    let merged: Vec<i64> = subjects
        .lines()
        .rev()
        .filter_map(|l| l.strip_prefix("Merge PR "))
        .map(|pr| {
            let pr: i64 = pr.parse().unwrap();
            e.db.conn()
                .query_row("SELECT id FROM works WHERE pr = ?1", [pr], |r| r.get(0))
                .unwrap()
        })
        .collect();
    assert_eq!(merged.len(), 2, "{subjects}\n{log}");
    assert_eq!(merged, queued, "{log}");
    // Each work's rounds numbered from one, none twice or missing, none open.
    for w in [a, b] {
        let mut st =
            e.db.conn()
                .prepare("SELECT n, ended_at IS NULL FROM rounds WHERE work = ?1 ORDER BY n")
                .unwrap();
        let rounds: Vec<(i64, bool)> = st
            .query_map([w], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        let numbers: Vec<i64> = rounds.iter().map(|r| r.0).collect();
        assert_eq!(numbers, (1..=numbers.len() as i64).collect::<Vec<_>>(), "work {w}");
        assert!(rounds.iter().all(|r| !r.1), "work {w}: {rounds:?}");
    }
    // No transaction's number taken twice: the rows that share one were written at one time, by one writer, one
    // after the other.
    let shared: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM (SELECT tx FROM changes GROUP BY tx
                 HAVING count(DISTINCT at) > 1 OR count(DISTINCT actor) > 1 OR max(id) - min(id) + 1 <> count(*))",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(shared, 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner pauses one of two works side by side while its round runs: that round ends as usual and is recorded, the
// work then takes no step, and the other goes on.
#[test]
fn a_work_paused_beside_another_stops_after_its_round_and_the_other_goes_on() {
    let pausing = Arc::new(AtomicI64::new(0));
    let hold = Gate::default();
    let (p, h) = (pausing.clone(), hold.clone());
    let mut agents = scripts();
    // The round of `R0` of the work to pause waits at the gate while the test pauses the work.
    agents.insert(
        "R0",
        Box::new(move |r: &Request, _: usize| {
            if r.work == p.load(Ordering::SeqCst) {
                h.pass();
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (s, mut e, said, first, second) = two_works_with("pause-beside", agents, Hooks::default());
    e.tick().unwrap();
    let (a, b) = (state(&e, first).unwrap().0, state(&e, second).unwrap().0);
    pausing.store(a, Ordering::SeqCst);
    hold.shut();
    let came = turns_until(&mut e, &Go::default(), |_| hold.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(came, "{log}");
    api::pause(&mut e.db, a, None).unwrap();
    hold.open();
    let ended = turns_until(&mut e, &Go::default(), |e| !e.in_flight().contains(&a));
    let log = said.lock().unwrap().join("\n");
    assert!(ended, "{log}");
    let took: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT transition FROM rounds WHERE work = ?1 AND cell = 'R0'",
                [a],
                |r| r.get(0),
            )
            .unwrap();
    assert!(took.is_some(), "{log}");
    assert_eq!(paused_by(&e, a).as_deref(), Some("owner"), "{log}");
    let (mine, _) = rounds_after(&e, a, 0);
    let (theirs, _) = rounds_after(&e, b, 0);
    let went_on = turns_until(&mut e, &Go::default(), |e| rounds_after(e, b, 0).0 >= theirs + 2);
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(went_on, "{log}");
    assert_eq!(rounds_after(&e, a, 0).0, mine, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Two items, the second depending on the first by a blocking dependency, run one after the other though two slots are
// free: no work starts on the second until the first has merged into main, and then one does.
#[test]
fn a_blocking_dependency_starts_the_second_item_only_once_the_first_merged() {
    let (s, mut e, said, first, second) = two_works_with("after-the-first", scripts(), Hooks::default());
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO item_links (item, depends_on, needs, kind) VALUES (?1, ?2, 'the feature', 'blocking')",
            params![second, first],
        )?;
        Ok(())
    })
    .unwrap();
    let merged = |s: &Setup| {
        git(
            &s.root,
            &["--git-dir", &s.origin.to_string_lossy(), "show", "main:src/lib.txt"],
        ) == "old\nfeature"
    };
    // Whether main held the first's change when the pass that started the second began.
    let mut after_merge = None;
    for _ in 0..300 {
        let was = merged(&s);
        e.tick().unwrap();
        if state(&e, second).is_some() {
            after_merge = Some(was);
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(after_merge, Some(true), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// No keep-warm ping goes to a work's sessions while a step of it runs: here its CI has not come back while the owner
// has paused it, and its engineer's session, due a ping, gets one only once the step has ended. The session has no
// round open meanwhile, so that only the step in flight keeps the ping off.
#[test]
fn no_ping_goes_to_a_work_while_its_step_runs() {
    let s = setup_priced("no-ping-in-a-step");
    s.ci.shut();
    let mut agents = scripts();
    let implement = agents.remove("I1").unwrap();
    // The engineer's round that asks for CI says what it used, as the CLI does, which a ping needs to run it again.
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            let (pick, mut set) = implement(r, n);
            set.push(("@usage", used(1.0, 9_000)));
            (pick, set)
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let ci = turns_until(&mut e, &Go::default(), |_| s.ci.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(ci, "{log}");
    let work = state(&e, item).unwrap().0;
    pause_by_owner(&mut e, work, "to look at it");
    let now = last_call(&e, work) + 55 * 60_000 + 1_000;
    assert_eq!(e.keep_warm(now, None).unwrap(), 0, "{log}");
    s.ci.open();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(warm_now(&mut e, now, None).unwrap() > 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A failed run whose cause stops every work, here a usage limit, keeps the loop from dispatching any step until the
// wait is over, while the step of the other work under way ends as usual and is recorded; then both go on.
#[test]
fn a_halt_dispatches_no_step_and_lets_the_step_under_way_end() {
    let reset = db::now() / 1000 + 3600;
    let failing = Arc::new(AtomicI64::new(0));
    let hold = Gate::default();
    let (f, h) = (failing.clone(), hold.clone());
    let mut agents = scripts();
    // The round of `R0` of the work to fail fails once, when the other's round has come to the gate, where it waits.
    agents.insert(
        "R0",
        Box::new(move |r: &Request, _: usize| {
            if f.compare_exchange(r.work, 0, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                let until = Instant::now() + Duration::from_secs(10);
                while h.came() == 0 && Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(5));
                }
                let report = format!("Claude AI usage limit reached|{reset}");
                return (Vec::new(), vec![("@error", json!(report))]);
            }
            h.pass();
            (Vec::new(), Vec::new())
        }),
    );
    let (s, mut e, said, first, second) = two_works_with("halt-beside", agents, Hooks::default());
    e.tick().unwrap();
    let (a, b) = (state(&e, first).unwrap().0, state(&e, second).unwrap().0);
    failing.store(a, Ordering::SeqCst);
    hold.shut();
    let halted = turns_until(&mut e, &Go::default(), |e| {
        e.halt().is_some() && !e.in_flight().contains(&a) && hold.came() > 0
    });
    let log = said.lock().unwrap().join("\n");
    assert!(halted, "{log}");
    assert_eq!(e.in_flight(), BTreeSet::from([b]), "{log}");
    // While the halt holds, a turn of the loop dispatches nothing.
    let n = rounds_count(&e);
    e.dispatch(&Go::default()).unwrap();
    assert_eq!(e.in_flight(), BTreeSet::from([b]), "{log}");
    hold.open();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    let took: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT transition FROM rounds WHERE work = ?1 AND cell = 'R0'",
                [b],
                |r| r.get(0),
            )
            .unwrap();
    assert!(took.is_some(), "{log}");
    assert_eq!(rounds_count(&e), n, "{log}");
    e.dispatch(&Go::default()).unwrap();
    assert!(e.in_flight().is_empty(), "{log}");
    assert_eq!(rounds_count(&e), n, "{log}");
    // Once the wait is over, both go on.
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    let p = e.pass(&Go::default()).unwrap();
    let log = said.lock().unwrap().join("\n");
    let moved: BTreeSet<i64> = p
        .jobs
        .iter()
        .filter(|j| !j.rounds.is_empty())
        .filter_map(|j| j.work)
        .collect();
    assert_eq!(moved, BTreeSet::from([a, b]), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A step whose thread panics, here in its agent's round, leaves neither a job in flight nor a round open: the loop
// pauses its work with why, so that the slot it held is free, while the other work's round beside it ends as usual.
// Once the owner lets it go on, the work is dispatched again: a job left in flight would keep it from any step for good.
#[test]
fn a_step_whose_thread_panics_pauses_its_work_and_lets_it_go_on_later() {
    let panicking = Arc::new(AtomicI64::new(0));
    let p = panicking.clone();
    let mut agents = scripts();
    agents.insert(
        "R0",
        Box::new(move |r: &Request, _: usize| {
            if r.work == p.load(Ordering::SeqCst) {
                panic!("a scripted fault in the agent's round");
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (s, mut e, said, first, second) = two_works_with("panic-in-a-step", agents, Hooks::default());
    e.tick().unwrap();
    let (a, b) = (state(&e, first).unwrap().0, state(&e, second).unwrap().0);
    panicking.store(a, Ordering::SeqCst);
    let paused = turns_until(&mut e, &Go::default(), |e| {
        paused_by(e, a).is_some()
            && !e.in_flight().contains(&a)
            && transitions_at(e, b, "R0").iter().any(Option::is_some)
    });
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(paused, "{log}");
    let why: Option<String> =
        e.db.conn()
            .query_row("SELECT paused FROM works WHERE id = ?1", [a], |r| r.get(0))
            .unwrap();
    assert_eq!(paused_by(&e, a).as_deref(), Some("engine"), "{log}");
    assert!(
        why.as_deref()
            .is_some_and(|w| w.contains("a scripted fault in the agent's round")),
        "{why:?}\n{log}"
    );
    assert_eq!(open_rounds(&e, a), 0, "{log}");
    assert!(e.in_flight().is_empty(), "{log}");
    // The paused work holds no slot: the other holds one of the two at most.
    assert!(e.free_slots().unwrap() >= 1, "{log}");
    // The owner lets it go on: its next round of `R0` runs and ends.
    panicking.store(0, Ordering::SeqCst);
    api::resume(&mut e.db, a).unwrap();
    let again = turns_until(&mut e, &Go::default(), |e| {
        transitions_at(e, a, "R0").iter().any(Option::is_some)
    });
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(again, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Each start of the loop writes a row of `engine_runs` with the build, the configuration commit and the flow version;
// the stop completes it with the time and why; a start that finds a row never completed (the process was killed) marks
// it lost and leaves the time of its stop unknown.
#[test]
fn each_run_of_the_loop_is_recorded_from_its_start_to_its_stop() {
    type Run = (i64, String, String, i64, Option<i64>, Option<String>);
    let row = |e: &Engine, id: i64| -> Run {
        e.db.conn()
            .query_row(
                "SELECT started_at, built_from, config_commit, flow_version, stopped_at, stop_reason
                 FROM engine_runs WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .unwrap()
    };
    let s = setup("engine-runs");
    let (mut e, _) = engine(&s, scripts());
    let first = e.record_start().unwrap();
    let (at, built, config, flow, stopped, why) = row(&e, first);
    assert!(at <= db::now() && at > db::now() - 60_000, "{at}");
    assert_eq!(built, engine::VERSION);
    assert_eq!(config, e.config.commit);
    assert_eq!(flow, e.flow_now);
    assert_eq!((stopped, why), (None, None));
    e.record_stop(first, "stop file").unwrap();
    let (.., stopped, why) = row(&e, first);
    assert!(stopped.is_some_and(|t| t >= at));
    assert_eq!(why.as_deref(), Some("stop file"));
    // A run that ends in an error says so.
    let second = e.record_start().unwrap();
    e.record_stop(second, "error: it went wrong").unwrap();
    assert_eq!(row(&e, second).5.as_deref(), Some("error: it went wrong"));
    // A run never completed is lost when the next starts; the runs completed are left as they were.
    let third = e.record_start().unwrap();
    let fourth = e.record_start().unwrap();
    let (.., stopped, why) = row(&e, third);
    assert_eq!((stopped, why.as_deref()), (None, Some("lost")));
    assert_eq!(row(&e, first).5.as_deref(), Some("stop file"));
    assert_eq!(row(&e, second).5.as_deref(), Some("error: it went wrong"));
    assert_eq!(row(&e, fourth).5, None);
    let _ = std::fs::remove_dir_all(&s.root);
}

/// When a run of the loop stopped and why, as its row of `engine_runs` says.
fn stop_of(e: &Engine, run: i64) -> (Option<i64>, Option<String>) {
    e.db.conn()
        .query_row(
            "SELECT stopped_at, stop_reason FROM engine_runs WHERE id = ?1",
            [run],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
}

// A run's stop is written once: a later stop of the same run, as the loop's two ways out would make if both ran, leaves
// the first time and reason. A run found lost stays lost over later starts, with the time of its stop unknown.
#[test]
fn a_stop_of_a_run_is_written_once_and_a_lost_run_stays_lost() {
    let s = setup("engine-runs-corners");
    let (mut e, _) = engine(&s, scripts());
    let first = e.record_start().unwrap();
    e.record_stop(first, "stop file").unwrap();
    let stopped = stop_of(&e, first);
    assert!(stopped.0.is_some());
    e.record_stop(first, "error: a later one").unwrap();
    assert_eq!(stop_of(&e, first), stopped);
    assert_eq!(stopped.1.as_deref(), Some("stop file"));
    let second = e.record_start().unwrap();
    let third = e.record_start().unwrap();
    let fourth = e.record_start().unwrap();
    assert_eq!(stop_of(&e, second), (None, Some("lost".to_string())));
    assert_eq!(stop_of(&e, third), (None, Some("lost".to_string())));
    assert_eq!(stop_of(&e, fourth), (None, None));
    assert_eq!(stop_of(&e, first), stopped);
    let rows: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM engine_runs", [], |r| r.get(0))
            .unwrap();
    assert_eq!(rows, 4);
    let _ = std::fs::remove_dir_all(&s.root);
}

// The engine's process killed leaves its run open; the next engine over the same data marks it lost when it starts,
// and its own run is open.
#[test]
fn an_engine_started_over_the_data_of_a_killed_one_marks_its_run_lost() {
    let s = setup("engine-runs-restart");
    let (mut killed, _) = engine(&s, scripts());
    let left = killed.record_start().unwrap();
    drop(killed);
    let (mut next, _) = engine(&s, scripts());
    let now = next.record_start().unwrap();
    assert_ne!(left, now);
    assert_eq!(stop_of(&next, left), (None, Some("lost".to_string())));
    assert_eq!(stop_of(&next, now), (None, None));
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's Step of a work whose step is under way waits until that step has ended: the loop's turn meanwhile
// dispatches no second step of the work and leaves the Step asked, so that the step under way ends as a step of its
// own and the work is not paused by it; the turn after takes the owner's round, after which the owner holds the work
// paused.
#[test]
fn the_owner_s_step_of_a_work_whose_step_runs_waits_until_that_step_ends() {
    let s = setup("step-beside-a-step");
    let hold = Gate::default();
    let h = hold.clone();
    let mut agents = scripts();
    agents.insert(
        "R0",
        Box::new(move |_: &Request, n: usize| {
            if n == 1 {
                h.pass();
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    hold.shut();
    let came = turns_until(&mut e, &Go::default(), |_| hold.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(came, "{log}");
    let work = state(&e, item).unwrap().0;
    // The round of `R0` under way is among them.
    let rounds = rounds_after(&e, work, 0).0;
    let step = Go {
        pause: held_but(&[], &[]),
        steps: vec![work],
        ..Go::default()
    };
    let taken = e.dispatch(&step).unwrap();
    assert!(taken.is_empty(), "{taken:?}\n{log}");
    assert!(e.in_flight().contains(&work), "{log}");
    hold.open();
    let reaped = e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    let mine: Vec<&Stepped> = reaped.iter().filter(|s| s.work == Some(work)).collect();
    assert_eq!(mine.len(), 1, "{reaped:?}\n{log}");
    assert_eq!(mine[0].asked, None, "{reaped:?}\n{log}");
    assert_eq!(transitions_at(&e, work, "R0").len(), 1, "{log}");
    assert_eq!(rounds_after(&e, work, 0).0, rounds, "{log}");
    assert_eq!(paused_by(&e, work), None, "{log}");
    // The Step, still asked, is the next turn's: one round, and the work is the owner's, paused.
    let p = e.pass(&step).unwrap();
    let log = said.lock().unwrap().join("\n");
    let asked = asked_in(&p, &Ask::Work(work));
    assert_eq!(asked.work, Some(work), "{p:?}\n{log}");
    assert_eq!(rounds_after(&e, work, 0).0, rounds + 1, "{p:?}\n{log}");
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// While the loop looks at the network as a wait for no network ends, a step on another thread may stop every work for
// another cause, here a usage limit that resets in an hour: the look ends only the wait it looked at, and the one set
// meanwhile still stops every work, so that no step is dispatched before the limit resets.
#[test]
fn a_halt_a_step_sets_while_the_loop_looks_at_the_network_holds() {
    let s = setup("halt-during-a-look");
    let look = Gate::default();
    let l = look.clone();
    // The network is up, once the test lets the look through.
    let hooks = Hooks {
        network: Some(Arc::new(move || {
            l.pass();
            true
        })),
        ..Hooks::default()
    };
    let (e, said) = engine_with(&s, scripts(), hooks);
    e.set_halt(Some(engine::Halt {
        kind: "offline".into(),
        until: 0,
        why: "no network".into(),
    }));
    look.shut();
    let t = e.fork().unwrap();
    let looking = std::thread::spawn(move || t.halted());
    let until = Instant::now() + Duration::from_secs(10);
    while look.came() == 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(look.came(), 1);
    // A step's run meets a usage limit while the look is under way.
    let limit = engine::Halt {
        kind: "usage_limit".into(),
        until: db::now() + 3_600_000,
        why: "a usage limit".into(),
    };
    e.set_halt(Some(limit.clone()));
    look.open();
    looking.join().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(e.halt(), Some(limit), "{log}");
    assert!(e.halted(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The rounds of two works side by side fail together, for two causes that stop every work: whichever of the two waits
// ends later stands, in either order, so that a wait of half a minute for the network that came after a usage limit
// resetting in an hour does not take its place, and no step is dispatched while the limit holds; the owner is told of
// each cause once.
#[test]
fn of_two_waits_set_together_the_one_that_ends_later_holds() {
    let reset = db::now() / 1000 + 3600;
    let limit = format!("Claude AI usage limit reached|{reset}");
    let offline = "API Error: Connection error. (getaddrinfo ENOTFOUND api.anthropic.com)".to_string();
    two_waits_set_together("later-wait-second", limit.clone(), offline.clone(), reset);
    two_waits_set_together("later-wait-first", offline, limit, reset);
}

/// Two works whose rounds of `R0` run side by side and fail, the first to come with `first` and the other, once that
/// wait stands, with `second`; the wait for the usage limit that resets at `reset`, in seconds, is the one that holds.
fn two_waits_set_together(name: &str, first: String, second: String, reset: i64) {
    let at = Gate::default();
    let then = Gate::default();
    let after = then.clone();
    let mut agents = scripts();
    agents.insert(
        "R0",
        Box::new(move |_: &Request, n: usize| {
            let report = match n {
                1 => first.clone(),
                2 => second.clone(),
                _ => return (Vec::new(), Vec::new()),
            };
            at.meet(2);
            // The second fails only once the test saw the first's wait set.
            if n == 2 {
                after.pass();
            }
            (Vec::new(), vec![("@error", json!(report))])
        }),
    );
    let (s, mut e, said, a, b) = two_works_with(name, agents, Hooks::default());
    // The first pass starts both works; the next dispatches the round of `R0` of each.
    e.tick().unwrap();
    then.shut();
    e.dispatch(&Go::default()).unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while e.halt().is_none() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    let log = said.lock().unwrap().join("\n");
    assert!(e.halt().is_some(), "{log}");
    then.open();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    let h = e.halt().unwrap_or_else(|| panic!("no wait:\n{log}"));
    assert_eq!((h.kind.as_str(), h.until), ("usage_limit", reset * 1000), "{log}");
    let mut kinds = failed_kinds(&e);
    kinds.sort();
    assert_eq!(kinds, ["offline", "usage_limit"], "{log}");
    let before = rounds_count(&e);
    e.tick().unwrap();
    assert_eq!(rounds_count(&e), before, "{log}");
    assert_eq!((told(&s, "usage_limit"), told(&s, "offline")), (1, 1), "{log}");
    // Once the limit resets, both works go on from their cell.
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    for item in [a, b] {
        let (_, at, paused) = state(&e, item).unwrap();
        assert_eq!(paused, None, "{log}");
        assert_ne!(at, "R0", "{log}");
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// The rounds of three works side by side fail one after another while the first wait stands: a usage limit, then no
// network, then the usage limit again. The owner is told of each cause once, as the first time it stops every work:
// the third round's limit is the one the owner was told of first, and the wait for it still stands, so no second
// notice of it comes because a notice of another cause came between.
#[test]
fn a_cause_told_once_is_not_told_again_after_another_cause_between() {
    let reset = db::now() / 1000 + 3600;
    let limit = format!("Claude AI usage limit reached|{reset}");
    let offline = "API Error: Connection error. (getaddrinfo ENOTFOUND api.anthropic.com)".to_string();
    let met = Gate::default();
    let (second, third) = (Gate::default(), Gate::default());
    let (g2, g3) = (second.clone(), third.clone());
    let mut agents = scripts();
    agents.insert(
        "R0",
        Box::new(move |_: &Request, n: usize| {
            let report = match n {
                1 | 3 => limit.clone(),
                2 => offline.clone(),
                _ => return (Vec::new(), Vec::new()),
            };
            met.meet(3);
            // The second and the third fail only once the test let each through, one after the other.
            match n {
                2 => g2.pass(),
                3 => g3.pass(),
                _ => {}
            }
            (Vec::new(), vec![("@error", json!(report))])
        }),
    );
    let s = setup("told-once-between");
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(&s.data, "install.toml", &install.replace("slots = 1", "slots = 3"));
    git(&s.data, &["commit", "-q", "-am", "three slots"]);
    let (mut e, said, _) = start_with(&s, agents, Hooks::default());
    e.db.write(Actor::Owner, |tx| {
        for (title, place) in [("Add another", 2), ("Add a third", 3)] {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', ?1, 'x', 'y', 'change', 'l')",
                params![title],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO start_order (item, place) VALUES (?1, ?2)",
                params![id, place],
            )?;
        }
        Ok(())
    })
    .unwrap();
    // The first pass starts the three works; the next dispatches the round of `R0` of each.
    e.tick().unwrap();
    second.shut();
    third.shut();
    e.dispatch(&Go::default()).unwrap();
    let failed = |e: &Engine, n: usize| {
        let until = Instant::now() + Duration::from_secs(10);
        while failed_kinds(e).len() < n && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
        failed_kinds(e).len()
    };
    let log = said.lock().unwrap().join("\n");
    assert_eq!(failed(&e, 1), 1, "{log}");
    second.open();
    assert_eq!(failed(&e, 2), 2, "{log}");
    third.open();
    e.drain().unwrap();
    // The notices the steps queued are written out by the loop's next turn, which dispatches nothing while the wait
    // stands.
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(failed_kinds(&e).len(), 3, "{log}");
    let h = e.halt().unwrap_or_else(|| panic!("no wait:\n{log}"));
    assert_eq!((h.kind.as_str(), h.until), ("usage_limit", reset * 1000), "{log}");
    assert_eq!((told(&s, "usage_limit"), told(&s, "offline")), (1, 1), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A failed run's cause that another work's round meets after the loop dispatched a step stops that step's round too,
// as the owner is told no agent round starts: the second work's step is held at its first write, the session it
// opens, before its round starts, until the first work's round failed for a usage limit; its round does not start, and
// it goes on from its cell once the wait is over.
#[test]
fn a_round_whose_step_was_dispatched_before_a_halt_does_not_start() {
    let reset = db::now() / 1000 + 3600;
    let mut agents = scripts();
    agents.insert(
        "R0",
        Box::new(move |_: &Request, n: usize| {
            if n == 1 {
                let report = format!("Claude AI usage limit reached|{reset}");
                return (Vec::new(), vec![("@error", json!(report))]);
            }
            (Vec::new(), Vec::new())
        }),
    );
    let stop = Gate::default();
    let held = Arc::new(AtomicI64::new(0));
    let (g, which) = (stop.clone(), held.clone());
    // The first write on the thread of the work held waits at the gate, once.
    let hooks = Hooks {
        commit: Some(Arc::new(move |_: &db::rusqlite::Connection, _: &[db::Changed]| {
            let w = which.load(Ordering::SeqCst);
            if w != 0 && std::thread::current().name() == Some(format!("work {w}").as_str()) {
                which.store(0, Ordering::SeqCst);
                g.pass();
            }
        })),
        ..Hooks::default()
    };
    let (s, mut e, said, first, second) = two_works_with("halt-before-a-round", agents, hooks);
    // The first pass starts both works; the next dispatches the round of `R0` of each.
    e.tick().unwrap();
    let b = state(&e, second).unwrap().0;
    stop.shut();
    held.store(b, Ordering::SeqCst);
    // The second's round starts a new session, as the owner may ask, whose opening is its step's first write.
    e.sessions().ask_new(api::Fresh::Work(b));
    e.dispatch(&Go::default()).unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while !(stop.came() > 0 && e.halt().is_some()) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    let log = said.lock().unwrap().join("\n");
    assert!(stop.came() > 0 && e.halt().is_some(), "{log}");
    stop.open();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    let rounds_at = |e: &Engine, work: i64| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'R0'",
                [work],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(rounds_at(&e, b), 0, "{log}");
    let (_, at, paused) = state(&e, second).unwrap();
    assert_eq!((at.as_str(), paused), ("R0", None), "{log}");
    assert_eq!(failed_kinds(&e), ["usage_limit"], "{log}");
    // Once the limit resets, both works go on from their cell.
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    for item in [first, second] {
        let (_, at, paused) = state(&e, item).unwrap();
        assert_eq!(paused, None, "{log}");
        assert_ne!(at, "R0", "{log}");
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's Step of a work whose round a failed run's cause stopped before it started, the cause met by another
// work's round after the loop dispatched the Step, is not taken: the work is not paused and the ask stands; once the
// wait is over, the Step runs exactly one round and pauses the work after it. Every work is held by the owner's global
// pause but the one that fails; the stepped work's step is held at its first write, the new session it opens.
#[test]
fn the_owner_s_step_stopped_by_a_halt_before_its_round_stands_until_the_wait_is_over() {
    let reset = db::now() / 1000 + 3600;
    let mut agents = scripts();
    agents.insert(
        "R0",
        Box::new(move |_: &Request, n: usize| {
            if n == 1 {
                let report = format!("Claude AI usage limit reached|{reset}");
                return (Vec::new(), vec![("@error", json!(report))]);
            }
            (Vec::new(), Vec::new())
        }),
    );
    let stop = Gate::default();
    let held = Arc::new(AtomicI64::new(0));
    let (g, which) = (stop.clone(), held.clone());
    // The first write on the thread of the work held waits at the gate, once.
    let hooks = Hooks {
        commit: Some(Arc::new(move |_: &db::rusqlite::Connection, _: &[db::Changed]| {
            let w = which.load(Ordering::SeqCst);
            if w != 0 && std::thread::current().name() == Some(format!("work {w}").as_str()) {
                which.store(0, Ordering::SeqCst);
                g.pass();
            }
        })),
        ..Hooks::default()
    };
    let (s, mut e, said, first, second) = two_works_with("step-before-a-halt", agents, hooks);
    // The first pass starts both works; then the first goes on and the owner steps the second.
    e.tick().unwrap();
    let (a, b) = (state(&e, first).unwrap().0, state(&e, second).unwrap().0);
    let go = Go {
        pause: held_but(&[a], &[]),
        steps: vec![b],
        architects: Vec::new(),
    };
    stop.shut();
    held.store(b, Ordering::SeqCst);
    e.sessions().ask_new(api::Fresh::Work(b));
    e.dispatch(&go).unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while !(stop.came() > 0 && e.halt().is_some()) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    let log = said.lock().unwrap().join("\n");
    assert!(stop.came() > 0 && e.halt().is_some(), "{log}");
    stop.open();
    let jobs = e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    let ask = Ask::Work(b);
    assert!(jobs.iter().all(|j| j.asked.as_ref() != Some(&ask)), "{jobs:?}\n{log}");
    let rounds_at = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'R0'",
                [b],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(rounds_at(&e), 0, "{log}");
    assert_eq!(state(&e, second).unwrap().2, None, "{log}");
    // Once the limit resets, the ask that stood runs one round, and the work is paused after it.
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    let p = e.pass(&go).unwrap();
    let log = said.lock().unwrap().join("\n");
    let stepped = asked_in(&p, &ask);
    assert_eq!(
        stepped.rounds.iter().filter(|r| r.work == b).count(),
        1,
        "{stepped:?}\n{log}"
    );
    assert_eq!(rounds_at(&e), 1, "{log}");
    assert!(state(&e, second).unwrap().2.is_some(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
