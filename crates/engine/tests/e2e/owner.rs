//! The owner's own controls of works: pause, step, start, start over, a new session, and keeping sessions warm.

use super::*;

// A single step runs exactly one round: starting the work is the first, and each later one is the next round of the
// work that has a step to take.
#[test]
fn a_single_step_runs_exactly_one_round() {
    let s = setup("step");
    let (mut e, said, item) = start(&s, scripts());
    // The rounds of works on items: the architect work `quiet` recorded before the test is not counted.
    let rounds = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.graph = 'engineer' AND r.cell NOT IN ('G1', 'G2', 'G3')",
                [],
                |r| r.get(0),
            )
            .unwrap()
    };
    let first = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (work, at, _) = state(&e, item).unwrap_or_else(|| panic!("no work started:\n{log}"));
    assert_eq!(first.work, Some(work), "{log}");
    assert_eq!(first.started_item, Some(item));
    assert_eq!(first.cell.as_deref(), Some("S1"));
    assert_eq!(rounds(&e), 1, "{log}");
    assert_ne!(at, "S1", "{log}");
    let mut last = first.work;
    for n in 2..=4 {
        let next = e.step(last).unwrap();
        let log = said.lock().unwrap().join("\n");
        assert_eq!(next.work, Some(work), "{log}");
        assert_eq!(next.started_item, None);
        assert_eq!(next.rounds.len(), 1, "{next:?}\n{log}");
        assert_eq!(rounds(&e), n, "{log}");
        last = next.work;
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work the engine pauses because a step cannot be taken closes the round the step left open, with the reason, and
// tells the owner once. A round an earlier engine left open when it paused is closed with the pause once the owner
// lets the work go on, and the cell runs again.
#[test]
fn a_paused_work_closes_its_open_round_and_tells_the_owner() {
    let s = setup("paused");
    let (mut e, said, item) = start(&s, scripts());
    for _ in 0..2 {
        e.step(None).unwrap();
    }
    let (work, at, _) = state(&e, item).unwrap();
    let open_round = |e: &mut Engine| -> i64 {
        e.db.write(Actor::Engine, |tx| {
            let n = next_round(tx, work)?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (?1, ?2, ?3, '{}', 'c', 1)",
                params![work, n, at],
            )?;
            Ok(n)
        })
        .unwrap()
    };
    let rejected = |e: &Engine, round: i64| -> (Option<i64>, Value) {
        let (ended, rejected): (Option<i64>, Option<String>) =
            e.db.conn()
                .query_row(
                    "SELECT ended_at, rejected FROM rounds WHERE work = ?1 AND n = ?2",
                    [work, round],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
        (
            ended,
            rejected
                .map(|x| serde_json::from_str(&x).unwrap())
                .unwrap_or(Value::Null),
        )
    };
    let left = open_round(&mut e);
    e.pause(work, "cell R1: the return does not fit").unwrap();
    let (ended, why) = rejected(&e, left);
    assert!(ended.is_some());
    assert_eq!(why, json!({ "error": "cell R1: the return does not fit" }));
    let told: String =
        e.db.conn()
            .query_row("SELECT text FROM engine_notices WHERE work = ?1", [work], |r| r.get(0))
            .unwrap();
    assert!(told.ends_with("is paused: cell R1: the return does not fit"), "{told}");
    // An earlier engine paused without closing the round; the owner lets the work go on.
    let earlier = open_round(&mut e);
    api::resume(&mut e.db, work).unwrap();
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (ended, why) = rejected(&e, earlier);
    assert!(ended.is_some(), "{log}");
    assert_eq!(
        why,
        json!({ "error": "the work was paused: cell R1: the return does not fit" }),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work the owner pauses while its round runs stops at the end of that round: the round ends as usual and the work
// moves on, then takes no step by itself until the owner lets it go on. While the round runs, the engine says whose
// round it is.
#[test]
fn a_work_the_owner_pauses_stops_at_the_end_of_its_round() {
    let s = setup("pause_round");
    let mut scripts = scripts();
    let path = config::database(&s.data);
    // The owner pauses the work on the page while the round of cell `R1` runs.
    scripts.insert(
        "R0",
        Box::new(move |_: &Request, _: usize| {
            let mut db = db::Db::open(&path).unwrap();
            let work: i64 = db
                .conn()
                .query_row("SELECT id FROM works WHERE ended IS NULL", [], |r| r.get(0))
                .unwrap();
            api::pause(&mut db, work, None).unwrap();
            (Vec::new(), Vec::new())
        }),
    );
    let busy = Arc::new(Mutex::new(Vec::new()));
    let seen = busy.clone();
    let hooks = Hooks {
        busy: Arc::new(move |w: &[i64]| seen.lock().unwrap().push(w.to_vec())),
        ..Hooks::default()
    };
    let (mut e, said, item) = start_with(&s, scripts, hooks);
    let first = e.step(None).unwrap();
    e.step(first.work).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!(at, "R1", "{log}");
    assert_eq!(paused.as_deref(), Some(db::PAUSED_BY_OWNER), "{log}");
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"));
    let took: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT transition FROM rounds WHERE work = ?1 AND cell = 'R0'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert!(took.is_some(), "{log}");
    assert_eq!(
        *busy.lock().unwrap(),
        [vec![work], vec![], vec![work], vec![], vec![work], vec![]]
    );
    // Paused, it takes no step by itself.
    let n = rounds_count(&e);
    assert_eq!(e.step(Some(work)).unwrap().work, None);
    assert_eq!(rounds_count(&e), n);
    // Once the owner lets it go on, it does.
    api::resume(&mut e.db, work).unwrap();
    assert_eq!(e.step(Some(work)).unwrap().work, Some(work));
    assert_eq!(rounds_count(&e), n + 1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's Start all lifts no work's own pause: a work the owner paused, and one the engine paused for a problem,
// stay paused until the owner lets each go on by itself.
#[test]
fn start_all_lifts_no_work_s_own_pause() {
    let s = setup("start_all");
    let (mut e, said, item) = start(&s, scripts());
    let work = e.step(None).unwrap().work.unwrap();
    api::pause(&mut e.db, work, None).unwrap();
    let c = api::Control::default();
    c.set_paused(false);
    let n = rounds_count(&e);
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new());
    assert_eq!(rounds_count(&e), n);
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"));
    api::resume(&mut e.db, work).unwrap();
    assert_eq!(e.step(Some(work)).unwrap().work, Some(work));
    assert_eq!(rounds_count(&e), n + 1);
    e.pause(work, "cell R2: the return does not fit").unwrap();
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"));
    let log = said.lock().unwrap().join("\n");
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new(), "{log}");
    assert_eq!(
        state(&e, item).unwrap().2.as_deref(),
        Some("cell R2: the return does not fit")
    );
    api::resume(&mut e.db, work).unwrap();
    assert_eq!(e.step(Some(work)).unwrap().work, Some(work));
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's step runs exactly one round of a paused work, which stays paused in the owner's hands. A work the
// engine paused is taken into them: its cell runs again, counting afresh as once the owner lets a work go on, and it
// stays paused by the owner.
#[test]
fn a_step_runs_one_round_of_a_paused_work_which_stays_paused() {
    let s = setup("step_paused");
    let (mut e, said, item) = start(&s, scripts());
    let work = e.step(None).unwrap().work.unwrap();
    api::pause(&mut e.db, work, None).unwrap();
    let (_, before, _) = state(&e, item).unwrap();
    let stepped = e.step_work(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        (stepped.work, stepped.rounds.len()),
        (Some(work), 1),
        "{stepped:?}\n{log}"
    );
    let (_, at, paused) = state(&e, item).unwrap();
    assert_ne!(at, before, "{log}");
    assert_eq!(paused.as_deref(), Some(db::PAUSED_BY_OWNER));
    assert_eq!(e.step(Some(work)).unwrap().work, None);
    e.pause(work, "cell R2: the return does not fit").unwrap();
    let stepped = e.step_work(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(stepped.rounds.len(), 1, "{stepped:?}\n{log}");
    assert_eq!(state(&e, item).unwrap().2.as_deref(), Some(db::PAUSED_BY_OWNER));
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"));
    let lifted: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM changes WHERE table_name = 'works' AND actor = 'owner'
             AND json_extract(old_row, '$.paused_by') = 'engine' AND json_extract(new_row, '$.paused') IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(lifted, 1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work's pause, and who paused it, are kept when the engine stops and starts again: a paused work takes no step by
// itself after the restart.
#[test]
fn a_work_s_pause_is_kept_across_a_restart() {
    let s = setup("pause_restart");
    let (mut e, _, item) = start(&s, scripts());
    let work = e.step(None).unwrap().work.unwrap();
    api::pause(&mut e.db, work, Some("Wait for the release.")).unwrap();
    drop(e);
    let (mut e, _) = engine(&s, scripts());
    assert_eq!(state(&e, item).unwrap().2.as_deref(), Some("Wait for the release."));
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"));
    assert_eq!(e.step(None).unwrap().work, None);
    api::resume(&mut e.db, work).unwrap();
    e.pause(work, "cell R1: the return does not fit").unwrap();
    drop(e);
    let (mut e, _) = engine(&s, scripts());
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"));
    assert_eq!(e.step(None).unwrap().work, None);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A paused work on an older flow version is not brought up while nothing makes it go on; the owner's step brings it
// up, the same work moving where it stands, runs its round there, and leaves it paused in the owner's hands.
#[test]
fn a_step_brings_a_work_on_an_older_flow_version_up_and_keeps_it_paused() {
    let s = setup("step_takeover");
    let (mut e, said, item) = start(&s, scripts());
    let first = e.step(None).unwrap();
    let old = first.work.unwrap();
    e.step(first.work).unwrap();
    api::pause(&mut e.db, old, None).unwrap();
    let at = state(&e, item).unwrap().1;
    let v = older(&mut e, &[old], |g| renamed(g, &at));
    drop(e);
    let (mut e, _) = engine(&s, scripts());
    assert_eq!(e.step(None).unwrap().work, None);
    assert_eq!(versions_of(&e, old), (v, None));
    let before = transitions(&e, item, &at).len();
    let stepped = e.step_work(old).unwrap();
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 1, "{works:?}\n{log}");
    assert_eq!(stepped.work, Some(old), "{log}");
    assert_eq!(versions_of(&e, old), (e.flow_now, Some(v)), "{log}");
    assert_eq!(transitions(&e, item, &at).len(), before + 1, "{log}");
    assert_eq!(paused_by(&e, old).as_deref(), Some("owner"));
    assert_eq!(e.step(Some(old)).unwrap().work, None);
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner starts a work over while a round of it runs, after pausing it: what the round hands in is not taken, and the
// takeover cuts the round off with the owner's reason, what it cost kept on it. The work ends as taken over, saying the
// owner started it over, as the group's notice says too; its question to the architect leaves the queue, its ask the
// owner answered keeps the owner's answer and the other needs none, and its decision card is replaced with the owner's
// ruling kept. A new work on the item starts at the first cell, paused as the owner had paused the old one: it takes
// over the branch, the worktree with what the cut-off round left uncommitted in it, and the pull request, and its cell
// `R1` is told what exists, the old work's decision cards and asks with their rulings and answers included. The work
// over and a work of the architect cannot be started over.
#[test]
fn the_owner_starts_a_work_over_in_the_middle_of_a_round() {
    let s = setup("start-over");
    let mut agents = scripts();
    let implement = agents.remove("I1").unwrap();
    let path = config::database(&s.data);
    // The owner pauses the work and starts it over while its second round at cell `I1` runs, which leaves a file it
    // does not commit.
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            if n == 2 {
                write(&r.worktree, "notes.txt", "half done\n");
                let mut db = db::Db::open(&path).unwrap();
                let work: i64 = db
                    .conn()
                    .query_row(
                        "SELECT id FROM works WHERE graph = 'engineer' AND ended IS NULL",
                        [],
                        |x| x.get(0),
                    )
                    .unwrap();
                api::pause(&mut db, work, None).unwrap();
                assert_eq!(
                    api::start_over(&mut db, work).unwrap(),
                    format!("work {work} is started over")
                );
            }
            implement(r, n)
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    for _ in 0..80 {
        e.step(None).unwrap();
        if works_of(&e, item)[0].4.is_some() {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    let (old, _, worktree, branch, pr, _) = works_of(&e, item)[0].clone();
    assert!(pr.is_some() && worktree.is_some() && branch.is_some(), "{log}");
    // It put two asks to the owner, a decision card and a question to the architect; the owner answers one ask and rules
    // on the card.
    let (asks, card) =
        e.db.write(Actor::Engine, |tx| {
            let round: i64 = tx.query_row("SELECT max(n) FROM rounds WHERE work = ?1", [old], |r| r.get(0))?;
            let mut asks = Vec::new();
            for _ in 0..2 {
                tx.execute(
                    "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, '{\"to\": \"owner\"}')",
                    [old, round],
                )?;
                asks.push(tx.last_insert_rowid());
            }
            tx.execute(
                "INSERT INTO design_cards (work, round, key, body, status) VALUES (?1, ?2, 'name', ?3, 'open')",
                params![
                    old,
                    round,
                    json!({ "key": "name", "question": "Which name?", "options": [{ "name": "Old" }, { "name": "New" }],
                            "recommended": "Old" })
                    .to_string()
                ],
            )?;
            let card = tx.last_insert_rowid();
            db::queue_matter(
                tx,
                &db::Matter {
                    project: "p".into(),
                    kind: "engineer_question".into(),
                    detail: Some("other_question".into()),
                    what: format!("Work {old} on item {item} asks which name to keep"),
                    body: json!({ "to": "architect", "work": old }),
                    item: Some(item),
                    raised: Some((old, round)),
                },
            )?;
            Ok((asks, card))
        })
        .unwrap();
    let answer = api::Answer {
        text: "Keep the old name.".into(),
        speaker: "owner".into(),
        relayed: None,
        why: None,
        reply: None,
        to: None,
        images: Vec::new(),
    };
    api::answer(&mut e.db, asks[0], &answer).unwrap();
    let ruling = api::Ruling {
        verdict: "disagree".into(),
        option: None,
        text: Some("Name it after the feature.".into()),
    };
    api::rule(&mut e.db, card, &ruling).unwrap();
    // The owner's step of the work runs its round, in which the owner starts it over.
    let stepped = e.step_work(old).unwrap();
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 2, "{works:?}\n{log}");
    let why = "stopped: the owner started it over, and a new work on it takes the item over";
    assert_eq!(works[0].5.as_deref(), Some(why), "{log}");
    let (n, cut, usage): (i64, String, Option<String>) =
        e.db.conn()
            .query_row(
                "SELECT n, json_extract(rejected, '$.interrupted'), usage FROM rounds
                 WHERE work = ?1 AND cell = 'I1' AND transition IS NULL",
                [old],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
    assert_eq!(stepped.rounds, [engine::RoundId::new(old, n)], "{log}");
    assert_eq!(cut, why);
    assert!(usage.is_some_and(|u| u.contains("cost_usd")));
    let notice: String =
        e.db.conn()
            .query_row(
                "SELECT text FROM engine_notices WHERE work = ?1 ORDER BY id DESC LIMIT 1",
                [old],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(
        notice,
        format!("Work {old} stopped: the owner started it over, and a new work on it takes the item over.")
    );
    let withdrawn: String =
        e.db.conn()
            .query_row("SELECT withdrawn FROM architect_matters WHERE work = ?1", [old], |r| {
                r.get(0)
            })
            .unwrap();
    assert_eq!(withdrawn, format!("the work that raised it ended ({why})"));
    let by = |e: &Engine, ask: i64| -> String {
        e.db.conn()
            .query_row(
                "SELECT json_extract(answer, '$.by') FROM owner_asks WHERE id = ?1",
                [ask],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!((by(&e, asks[0]), by(&e, asks[1])), ("owner".into(), "engine".into()));
    let (status, rulings): (String, String) =
        e.db.conn()
            .query_row(
                "SELECT status, rulings FROM item_design_cards WHERE work = ?1",
                [old],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!(status, "replaced");
    assert!(rulings.contains("Name it after the feature."), "{rulings}");
    // The new work is at the first cell, paused as the old one was.
    let new = works[1].0;
    assert_eq!(works[1].1, "S1");
    assert_eq!(paused_by(&e, new).as_deref(), Some("owner"));
    // Stepped, it takes over the branch, the worktree with what was left in it, and the pull request, and is told what
    // exists.
    e.step_work(new).unwrap();
    e.step_work(new).unwrap();
    e.step_work(new).unwrap();
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(
        (&works[1].2, &works[1].3, works[1].4),
        (&worktree, &branch, pr),
        "{log}"
    );
    let left = std::fs::read_to_string(PathBuf::from(worktree.unwrap()).join("notes.txt")).unwrap();
    assert_eq!(left, "half done\n");
    let input: String =
        e.db.conn()
            .query_row("SELECT input FROM rounds WHERE work = ?1 AND cell = 'R1'", [new], |r| {
                r.get(0)
            })
            .unwrap();
    let input: Value = db::put_back(e.db.conn(), &serde_json::from_str(&input).unwrap());
    let m = input["message"].as_str().unwrap();
    for want in [
        "This item was worked on before.".to_string(),
        format!("- Work {old} "),
        format!("when it ended: {why}."),
        format!("the views `item_design_cards` and `item_asks` with `work = {old}`"),
        "what is not committed is `git status`".to_string(),
        format!("pr view {} --comments", pr.unwrap()),
        format!("- Work {old} asked the architect, and had no answer when it ended, so the question was withdrawn"),
    ] {
        assert!(m.contains(&want), "{want}\n{m}");
    }
    // The work over cannot be started over again, nor can a work of the architect.
    assert_eq!(api::start_over(&mut e.db, old), Err(format!("work {old} has ended")));
    let v = e.flow_now;
    let arch: i64 =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (flow_version, graph, at, project) VALUES (?1, 'architect', 'Q0', 'p')",
                [v],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    assert_eq!(
        api::start_over(&mut e.db, arch),
        Err(format!(
            "work {arch} is the architect's: only a work on an item can be started over"
        ))
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// While every work is paused, the owner's start over of a work the owner started is carried out at once, and the
// owner's Start goes on in the new work, which goes on as the old one did.
#[test]
fn the_owner_s_start_goes_on_in_the_work_that_starts_a_work_over() {
    let s = setup("start-over-started");
    let (mut e, said, item) = start(&s, scripts());
    let c = api::Control::default();
    let work = e.create_work(item).unwrap();
    c.start(api::Fresh::Work(work), false);
    assert_eq!(
        turns(&mut e, &c, 2),
        [work, work],
        "{}",
        said.lock().unwrap().join("\n")
    );
    api::start_over(&mut e.db, work).unwrap();
    let moved = turns(&mut e, &c, 1);
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 2, "{works:?}\n{log}");
    let new = works[1].0;
    assert!(works[0].5.is_some(), "{log}");
    assert_eq!(moved, [new], "{log}");
    let asked = c.asked();
    assert!(
        asked.started.contains(&new) && !asked.started.contains(&work),
        "{asked:?}"
    );
    assert_eq!(paused_by(&e, new), None);
    let _ = std::fs::remove_dir_all(&s.root);
}

// With every work paused, the owner steps a project's architect: the first step starts a work of the architect, paused
// by the owner, and runs its first round; each later step runs the work's next round, as a step of the work by its id
// does, and the work stays paused in the owner's hands. With nothing to start a work for, a step runs nothing.
#[test]
fn the_owner_steps_the_architect_one_round_at_a_time_with_every_work_paused() {
    let s = setup("step-architect");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    assert_eq!(e.step_architect("p").unwrap().work, None);
    say(&mut e, "Look at the queue.");
    let first = e.step_architect("p").unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!((first.cell.as_deref(), first.rounds.len()), (Some("S1"), 1), "{log}");
    let arch = first.work.unwrap();
    assert_eq!(paused_by(&e, arch).as_deref(), Some("owner"));
    assert_eq!(e.step(None).unwrap().work, None);
    let second = e.step_work(arch).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!((second.cell.as_deref(), second.rounds.len()), (Some("U1"), 1), "{log}");
    let third = e.step_architect("p").unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        (third.work, third.cell.as_deref(), third.rounds.len()),
        (Some(arch), Some("Q0"), 1),
        "{log}"
    );
    assert_eq!(paused_by(&e, arch).as_deref(), Some("owner"));
    assert_eq!(architect_works(&e), [arch]);
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The owner's Pause of a work, as the page sends it: the work's own pause written as the owner's, its Start over.
fn owner_pauses(e: &mut Engine, c: &api::Control, work: i64) {
    c.pause(&api::Fresh::Work(work));
    api::pause(&mut e.db, work, None).unwrap();
}

/// A setup whose one machine has two slots, and an engine over it with two items in the queue.
fn two_works(name: &str) -> (Setup, Engine, Arc<Mutex<Vec<String>>>) {
    let (s, e, said, _, _) = two_works_with(name, scripts(), Hooks::default());
    (s, e, said)
}

// Each work follows the global pause and start unless the owner started or paused it, and the later action wins: a
// work the owner starts goes on while every work is paused, and follows the global pause again once every work is
// paused after; a work the owner pauses stays paused through Start all, until the owner starts it; a work the owner
// takes one round of is paused by the owner after it.
#[test]
fn the_owner_s_later_start_or_pause_of_a_work_wins_over_the_global_state() {
    let (s, mut e, said) = two_works("own-start");
    let c = api::Control::default();
    // Every work paused, as the engine starts: nothing moves.
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new());
    // Started all, one turn starts both works, side by side.
    c.set_paused(false);
    let moved = turns(&mut e, &c, 1);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(moved.len(), 2, "{log}");
    let (one, two) = (moved[0], moved[1]);
    assert_ne!(one, two, "{log}");
    // Paused again: nothing moves, until the owner starts one work, which alone goes on.
    c.set_paused(true);
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new());
    c.start(api::Fresh::Work(one), false);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(turns(&mut e, &c, 3), [one, one, one], "{log}");
    // A later global pause makes it follow the global state again.
    c.set_paused(true);
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new());
    // The owner pauses the other work: Start all runs the first and leaves the one the owner paused.
    owner_pauses(&mut e, &c, two);
    c.set_paused(false);
    let moved = turns(&mut e, &c, 3);
    assert!(!moved.is_empty() && !moved.contains(&two), "{moved:?}");
    assert_eq!(paused_by(&e, two).as_deref(), Some("owner"));
    // Its Start lifts its pause, as the owner, and it goes on.
    c.start(api::Fresh::Work(two), false);
    let moved = turns(&mut e, &c, 4);
    let log = said.lock().unwrap().join("\n");
    assert!(moved.contains(&two), "{moved:?}\n{log}");
    assert_eq!(paused_by(&e, two), None);
    assert!(c.asked().releasing.is_empty());
    let lifted: String = e
        .db
        .conn()
        .query_row(
            "SELECT actor FROM changes WHERE table_name = 'works' AND json_extract(old_row, '$.paused_by') = 'owner'
                 AND json_extract(new_row, '$.paused') IS NULL ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(lifted, "owner");
    // One round the owner asks for runs whatever the global state, and leaves the work paused by the owner, which
    // Start all does not lift.
    c.set_paused(true);
    c.ask_step(one, false).unwrap();
    let stepped = turn(&mut e, &c);
    let log = said.lock().unwrap().join("\n");
    assert_eq!((stepped.work, stepped.rounds.len()), (Some(one), 1), "{stepped:?}");
    assert_eq!(paused_by(&e, one).as_deref(), Some("owner"), "{stepped:?}\n{log}");
    c.set_paused(false);
    let moved = turns(&mut e, &c, 3);
    assert!(!moved.contains(&one), "{moved:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work the owner starts that is paused holds no slot: its pause is lifted only once its machine has a slot free, and
// until then it stays paused, its Start waiting.
#[test]
fn a_work_the_owner_starts_still_needs_a_free_slot() {
    let s = setup("own-start-slot");
    let (mut e, said, _) = start(&s, scripts());
    let c = api::Control::default();
    c.set_paused(false);
    let first = turn(&mut e, &c).work.unwrap();
    owner_pauses(&mut e, &c, first);
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 'Add another', 'x', 'y', 'change', 'l')",
            [],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 2.0)", params![id])?;
        Ok(())
    })
    .unwrap();
    let second = turn(&mut e, &c).work.unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_ne!(second, first, "{log}");
    c.set_paused(true);
    c.start(api::Fresh::Work(first), false);
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new());
    assert_eq!(paused_by(&e, first).as_deref(), Some("owner"));
    assert!(c.asked().releasing.contains(&first));
    // The other work paused frees its slot: the first goes on.
    owner_pauses(&mut e, &c, second);
    let moved = turns(&mut e, &c, 2);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(moved.first(), Some(&first), "{log}");
    assert_eq!(paused_by(&e, first), None);
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's Start of a work the architect stopped is the owner's release of it: its pause is lifted as the owner's,
// and the architect is told in the owner's words.
#[test]
fn the_owner_s_start_of_a_work_the_architect_stopped_tells_the_architect() {
    let s = setup("own-release");
    let (mut e, said, item) = start(&s, scripts());
    let c = api::Control::default();
    c.set_paused(false);
    let work = turn(&mut e, &c).work.unwrap();
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE works SET paused = ?2, paused_by = 'engine' WHERE id = ?1",
            params![work, format!("{}: it waits for the parser", db::STOPPED_BY_ARCHITECT)],
        )?;
        Ok(())
    })
    .unwrap();
    c.set_paused(true);
    c.start(api::Fresh::Work(work), false);
    let moved = turns(&mut e, &c, 1);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(moved, [work], "{log}");
    assert_eq!(paused_by(&e, work), None);
    let told: (String, Option<i64>) =
        e.db.conn()
            .query_row(
                "SELECT body ->> 'text', item FROM architect_matters WHERE kind = 'owner_words'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!(
        told,
        (
            format!("I started work {work}, which you had stopped (it waits for the parser): it goes on."),
            Some(item)
        )
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architect's works are taken the same way: started while every work is paused, a work of the architect starts when
// a matter is due and goes on; the owner's Pause of the architect pauses the work, and its Step leaves the work paused
// by the owner.
#[test]
fn the_architect_the_owner_starts_works_and_goes_on_while_every_work_is_paused() {
    let s = setup("own-start-architect");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let c = api::Control::default();
    say(&mut e, "Look at the queue.");
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new());
    c.start(api::Fresh::Architect("p".into()), false);
    let moved = turns(&mut e, &c, 3);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(moved.len(), 3, "{log}");
    let arch = moved[0];
    assert!(moved.iter().all(|w| *w == arch), "{moved:?}");
    assert_eq!(paused_by(&e, arch), None);
    // Paused, the work takes no step, and no other work of the architect starts.
    c.pause(&api::Fresh::Architect("p".into()));
    api::pause_architect(&mut e.db, "p").unwrap();
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new());
    assert_eq!(paused_by(&e, arch).as_deref(), Some("owner"));
    // Started again, its pause is lifted; a round asked for leaves it paused by the owner.
    c.start(api::Fresh::Architect("p".into()), false);
    assert_eq!(turns(&mut e, &c, 1), [arch]);
    assert_eq!(paused_by(&e, arch), None);
    c.ask_step_architect("p", false).unwrap();
    let stepped = turn(&mut e, &c);
    assert_eq!(stepped.work, Some(arch));
    assert_eq!(paused_by(&e, arch).as_deref(), Some("owner"));
    assert!(c.asked().architects_started.is_empty());
    assert_eq!(architect_works(&e), [arch]);
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's Step or Start can ask for a new session: the round starts one for its cell's line, recorded as the
// owner's, given everything its cell needs; the session before ends and is otherwise left as it is, and the line's
// later rounds go on in the new one. For the architect, the new session is its work's.
#[test]
fn the_owner_asks_for_a_new_session_with_a_step_or_a_start() {
    let s = setup("own-new-session");
    let (mut e, said, item) = start(&s, scripts());
    agent_rounds_to(&mut e, item, 1);
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "I1", "{}", said.lock().unwrap().join("\n"));
    let c = api::Control::default();
    c.ask_step(work, true).unwrap();
    turn(&mut e, &c);
    let log = said.lock().unwrap().join("\n");
    let lines = line_sessions(&e, work, "engineer");
    assert_eq!(
        lines,
        [
            ("line".to_string(), true, "R1".to_string()),
            ("owner".to_string(), false, "I1".to_string())
        ],
        "{log}"
    );
    let rounds = agent_rounds(&e, item);
    let two = &rounds[1].2;
    assert_eq!(given(two, "format:I1"), Some(true));
    for a in ["format:common", "rule:ground"] {
        assert!(in_system(two, a), "{a}: {two}");
    }
    assert!(two["call"]["session"]["started"].is_string(), "{}", two["call"]);
    // Started as it is, the work's next round goes on in that session; started in a new session, the next starts one.
    c.start(api::Fresh::Work(work), false);
    let n = agent_rounds(&e, item).len();
    for _ in 0..10 {
        if agent_rounds(&e, item).len() > n {
            break;
        }
        turn(&mut e, &c);
    }
    let lines = line_sessions(&e, work, "engineer");
    assert_eq!(lines.len(), 2, "{lines:?}\n{}", said.lock().unwrap().join("\n"));
    owner_pauses(&mut e, &c, work);
    c.start(api::Fresh::Work(work), true);
    let n = agent_rounds(&e, item).len();
    for _ in 0..10 {
        if agent_rounds(&e, item).len() > n {
            break;
        }
        turn(&mut e, &c);
    }
    let lines = line_sessions(&e, work, "engineer");
    let log = said.lock().unwrap().join("\n");
    assert_eq!(lines.len(), 3, "{lines:?}\n{log}");
    assert!(lines[1].1, "{lines:?}");
    assert_eq!((lines[2].0.as_str(), lines[2].1), ("owner", false), "{lines:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architect's Step in a new session starts one for its work, recorded as the owner's; the work's next rounds go on
// in it, the understanding of the project first, since the session is new in the middle of the work: the cell it would
// have started at takes the row back to understanding.
#[test]
fn the_owner_asks_for_a_new_session_for_the_architect_s_work() {
    let s = setup("own-new-architect-work-session");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let c = api::Control::default();
    say(&mut e, "Look at the queue.");
    c.start(api::Fresh::Architect("p".into()), false);
    // Starting takes the engine's own step, which finds the session has not understood the project; then the first
    // round.
    let arch = turns(&mut e, &c, 2)[0];
    let log = said.lock().unwrap().join("\n");
    let lines = line_sessions(&e, arch, "architect");
    assert_eq!(lines.len(), 1, "{lines:?}\n{log}");
    c.pause(&api::Fresh::Architect("p".into()));
    c.ask_step_architect("p", true).unwrap();
    turn(&mut e, &c);
    for _ in 0..5 {
        c.ask_step_architect("p", false).unwrap();
        turn(&mut e, &c);
    }
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, arch);
    assert_eq!(path[..7], ["S1", "U1", "Q0", "Q2", "H8", "S1", "U1"], "{log}");
    let lines = line_sessions(&e, arch, "architect");
    assert_eq!(lines.len(), 2, "{lines:?}\n{log}");
    assert!(lines[0].1, "{lines:?}");
    assert_eq!((lines[1].0.as_str(), lines[1].1), ("owner", false), "{lines:?}");
    assert!(lines[1].2.starts_with("U1"), "{lines:?}\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// While every work is paused, a work the pause holds waits on it: its session is kept warm as the session manager's
// rules say, unless the owner switched keeping warm off, which holds across restarts; a work the owner started runs,
// and is kept warm as any work that goes on is, for its going on.
#[test]
fn sessions_are_kept_warm_while_every_work_is_paused_unless_the_owner_switches_it_off() {
    let s = setup_priced("warm-paused");
    let mut scripts = scripts();
    scripts.insert("R1", adding("R1", |_| vec![("@usage", used(1.0, 9_000))]));
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 1);
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "I1", "{}", said.lock().unwrap().join("\n"));
    let from = last_call(&e, work);
    let due = from + 55 * 60_000 + 1_000;
    let paused = |warm: bool, runs: &[i64]| engine::GlobalPause {
        warm,
        runs: runs.iter().copied().collect(),
        architects: Default::default(),
    };
    assert_eq!(warm_now(&mut e, due, Some(&paused(false, &[]))).unwrap(), 0);
    assert_eq!(
        warm_now(&mut e, due, Some(&paused(true, &[]))).unwrap(),
        1,
        "{}",
        said.lock().unwrap().join("\n")
    );
    assert_eq!(
        pings_of(&e, work)[0].0,
        json!({ "cell": "I1", "on": "pause", "what": "the owner paused every work" })
    );
    // A work the owner started runs on, as every work does with no pause: the line it comes back to is kept warm
    // for its going on.
    let later = due + 55 * 60_000 + 1_000;
    assert_eq!(warm_now(&mut e, later, Some(&paused(true, &[work]))).unwrap(), 1);
    let going_on = json!({ "cell": "I1", "on": "work", "what": "the work goes on" });
    assert_eq!(pings_of(&e, work)[1].0, going_on);
    assert_eq!(warm_now(&mut e, later + 55 * 60_000 + 1_000, None).unwrap(), 1);
    assert_eq!(pings_of(&e, work)[2].0, going_on);
    // The owner's switch is kept in the database, as the owner's, across a restart.
    assert!(api::keep_warm_paused(e.db.conn()).unwrap());
    api::set_keep_warm_paused(&mut e.db, false).unwrap();
    drop(e);
    let (e, _) = engine(&s, BTreeMap::new());
    assert!(!api::keep_warm_paused(e.db.conn()).unwrap());
    let _ = std::fs::remove_dir_all(&s.root);
}
