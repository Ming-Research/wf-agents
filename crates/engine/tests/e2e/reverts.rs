//! A merge the project's architect has reverted, from main's red CI to the record of the revert.

use super::*;

// Item 231, A8: main is red by a merge. The engine's check of main finds that merge made it red, the architect asks in
// its answer for the revert, and the engine opens the pull request and merges it. The revert is recorded with the
// architect's work, the merge reverted and the commit on main; the health numbers count it as a merge reverted and as a
// merge that broke main, and it is not among the commits that came onto main without the flow, where every other commit
// of main is, none of them merged by the flow here.
#[test]
fn the_revert_the_architect_asks_for_is_recorded_and_counted() {
    let s = setup("revert");
    main_moves_on(
        &s.root,
        &[(&format!("tests/{REGRESSION}.txt"), "It fails.\n")],
        "Add a test that fails",
    );
    let broken = git(&s.origin, &["rev-parse", "main"]);
    let mut agents = scripts();
    let asked = broken.clone();
    agents.insert(
        "L2",
        Box::new(move |_: &Request, _: usize| {
            let action = json!({ "kind": "revert_merge", "merge_commit": asked });
            (
                Vec::new(),
                vec![
                    ("outcome.red.way", json!("revert")),
                    ("outcome.effects", json!({ "actions": [action] })),
                ],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    s.watch_main.store(true, Ordering::SeqCst);
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let checked: Value =
        e.db.conn()
            .query_row(
                "SELECT output FROM rounds WHERE work = ?1 AND cell = 'L1' AND transition IS NOT NULL",
                [arch],
                |r| r.get::<_, String>(0),
            )
            .map(|o| serde_json::from_str(&o).unwrap())
            .unwrap();
    assert_eq!(checked["main"]["kind"], "broken_by_merge", "{checked}\n{log}");
    let left: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM outbox WHERE kind = 'revert_merge'", [], |r| {
                r.get(0)
            })
            .unwrap();
    assert_eq!(left, 0, "{log}");
    let on_main = git(&s.origin, &["rev-parse", "main"]);
    assert_eq!(git(&s.origin, &["rev-parse", "main^1"]), broken, "{log}");
    let recorded: (i64, String, String) =
        e.db.conn()
            .query_row("SELECT work, merge_commit, revert_commit FROM reverts", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
    assert_eq!(recorded, (arch, broken, on_main), "{log}");
    let mut read = health::MainLog::GIT_LOG.to_vec();
    read.push("main");
    let main = health::MainLog::parse(&git(&s.origin, &read)).unwrap();
    let scope = health::Scope {
        project: Some("p"),
        subject: None,
        main: Some(&main),
    };
    let (from, to) = (db::now() - 3_600_000, db::now() + 60_000);
    let whole = |name: &str| {
        let m = health::measure(name).unwrap();
        health::series(e.db.conn(), m, &scope, from, to, to - from)
            .unwrap()
            .points[0]
            .value
    };
    assert_eq!(whole("merge.reverted"), Some(1.0), "{log}");
    assert_eq!(whole("merge.broken"), Some(1.0), "{log}");
    let commits: f64 = git(&s.origin, &["rev-list", "--count", "main"]).parse().unwrap();
    assert_eq!(whole("main.outside_flow"), Some(commits - 1.0), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
