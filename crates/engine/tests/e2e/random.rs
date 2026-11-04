//! Random sequences of events, after each of which every chain of blockers is closed.

use super::*;

/// The same sequence of numbers for the same seed: the random tests' dice.
struct Dice(AtomicU64);

impl Dice {
    fn new(seed: u64) -> Dice {
        Dice(AtomicU64::new(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1))
    }

    /// A number below `n`.
    fn roll(&self, n: u64) -> u64 {
        let mut x = self.0.load(Ordering::Relaxed);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0.store(x, Ordering::Relaxed);
        x % n.max(1)
    }
}

/// What a round hands in, at random: an ask of the advisor or the architect, or its own answer.
fn random_ask(dice: &Dice) -> Picked {
    match dice.roll(10) {
        0 | 1 => an_ask("advisor"),
        2..=4 => an_ask("architect"),
        _ => (Vec::new(), Vec::new()),
    }
}

/// A work of the engineer's graph not ended, at random, if there is one.
fn a_work(e: &Engine, dice: &Dice) -> Option<i64> {
    let ids: Vec<i64> = engine::query_json(
        e.db.conn(),
        "SELECT id FROM works WHERE graph = 'engineer' AND ended IS NULL ORDER BY id",
        &[],
        1000,
    )
    .unwrap()
    .iter()
    .filter_map(|r| r["id"].as_i64())
    .collect();
    (!ids.is_empty()).then(|| ids[dice.roll(ids.len() as u64) as usize])
}

/// One random sequence of events over three items of project p, beside project q, related to it: steps of the engine,
/// whose rounds ask the advisor and the architect, hand questions on (the advisor to the architect, an architect to the
/// other project's architect), and reply, sometimes with an item to wait for, of either project, a way on chosen wrong
/// going to the architect as a flow anomaly; the architects' hours, past their limit; interjections; cancels; and
/// works put on an older flow version, which the engine brings to the one it runs. After each, every chain of blockers
/// closes.
fn random_events(seed: u64, events: usize) {
    let s = setup(&format!("blockers-{seed}"));
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(
        &s.data,
        "install.toml",
        &format!(
            "{}\n[agents.levels.m]\ndesc = \"one level up\"\ncli = \"claude\"\nmodel = \"m-up\"\n",
            install.replace("slots = 1", "slots = 2")
        ),
    );
    let q = s.root.join("q-checkout");
    git(
        &s.root,
        &["clone", "-q", &s.origin.to_string_lossy(), &q.to_string_lossy()],
    );
    let p_file = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    let checkout = s.root.join("checkout");
    write(
        &s.data,
        "projects/p.toml",
        &p_file.replace("rules = [\"ground\"]", "rules = [\"ground\"]\nother_projects = [\"q\"]"),
    );
    write(
        &s.data,
        "projects/q.toml",
        &p_file
            .replace("o/p", "o/q")
            .replace(&checkout.display().to_string(), &q.display().to_string())
            .replace("rules = [\"ground\"]", "rules = [\"ground\"]\nother_projects = [\"p\"]"),
    );
    git(&s.data, &["add", "-A"]);
    git(
        &s.data,
        &["commit", "-q", "-m", "a level above, two slots, and a related project"],
    );
    let dice = Arc::new(Dice::new(seed));
    let mut agents = scripts();
    // The research, the deep research and the review's details ask at random, and otherwise answer as usual.
    for cell in ["R1", "D1", "T3"] {
        let d = dice.clone();
        let usual = agents
            .remove(cell)
            .unwrap_or_else(|| Box::new(|_: &Request, _: usize| (Vec::new(), Vec::new())));
        agents.insert(
            cell,
            Box::new(move |r: &Request, n: usize| {
                let asked = random_ask(&d);
                if asked.0.is_empty() && asked.1.is_empty() {
                    usual(r, n)
                } else {
                    asked
                }
            }),
        );
    }
    // The advisor answers, or hands the question to the architect.
    let d = dice.clone();
    agents.insert(
        "A1",
        Box::new(move |_: &Request, _: usize| match d.roll(4) {
            0 | 1 => an_ask("architect"),
            _ => (Vec::new(), Vec::new()),
        }),
    );
    let path = config::database(&s.data);
    // An architect replies, now and then with an item to wait for, made after the work's own so that no cycle closes,
    // of either project; or hands the question to the owner, or to the other project's architect.
    let (d, p) = (dice.clone(), path.clone());
    agents.insert(
        "H3",
        Box::new(move |r: &Request, _: usize| {
            let c = db::open_read(&p).unwrap();
            let project = project_of(r);
            let asked: Option<i64> = c
                .query_row(
                    "SELECT m.item FROM architect_matters m
                     WHERE m.project = ?1 AND m.kind = 'engineer_question' AND m.handled_round IS NULL
                       AND m.withdrawn IS NULL
                     ORDER BY m.id LIMIT 1",
                    [&project],
                    |x| x.get(0),
                )
                .ok()
                .flatten();
            let order = if d.roll(2) == 0 { "ASC" } else { "DESC" };
            let later: Option<i64> = asked.and_then(|i| {
                c.query_row(
                    &format!("SELECT id FROM items WHERE id > ?1 AND cancelled IS NULL ORDER BY id {order} LIMIT 1"),
                    [i],
                    |x| x.get(0),
                )
                .ok()
            });
            let other = if project == "p" { "q" } else { "p" };
            let mut reply = json!({ "text": "Go on as you planned.", "why": "It holds." });
            match d.roll(6) {
                0 | 1 => reply["hand_over"] = json!({ "to": "architect", "project": other, "why": "It is theirs." }),
                2 => {
                    if let Some(i) = later {
                        reply["waits"] =
                            json!([{ "key": i.to_string(), "title": "the other", "delivers": "its part" }]);
                    }
                }
                _ => {}
            }
            (Vec::new(), vec![("outcome.reply", reply)])
        }),
    );
    // Choosing the way on waits when the reply says to and carries on otherwise, but now and then chooses wrong,
    // which goes to the architect as a flow anomaly.
    let (d, p) = (dice.clone(), path.clone());
    agents.insert(
        "A4",
        Box::new(move |r: &Request, _: usize| {
            let c = db::open_read(&p).unwrap();
            let waits: Option<String> = c
                .query_row(
                    "SELECT json_extract(output, '$.reply.waits') FROM rounds WHERE work = ?1 AND cell = 'A2'
                       AND transition IS NOT NULL ORDER BY n DESC LIMIT 1",
                    [r.work],
                    |x| x.get(0),
                )
                .ok()
                .flatten();
            let next = match (waits.is_some(), d.roll(8)) {
                (true, 0) | (false, 1..) => "carry_on",
                _ => "wait_for_items",
            };
            (vec![("outcome.next", next)], Vec::new())
        }),
    );
    // Whether to wait longer, at random.
    let d = dice.clone();
    agents.insert(
        "A7",
        Box::new(move |_: &Request, _: usize| {
            let v = ["wait_on", "withdraw"][d.roll(2) as usize];
            (vec![("outcome.longer", v)], Vec::new())
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let mut items: Vec<i64> = (0..3)
        .map(|i| {
            e.db.write(Actor::Owner, |tx| {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level)
                     VALUES ('p', ?1, 'It is missing.', 'People need it.', 'change', 'l')",
                    [format!("Add part {i}")],
                )?;
                let id = tx.last_insert_rowid();
                tx.execute(
                    "INSERT INTO dispatch_queue (item, position) VALUES (?1, ?2)",
                    params![id, i as f64],
                )?;
                Ok(id)
            })
            .unwrap()
        })
        .collect();
    // An item of q, which p's works may be had wait for; q starts no work of its own.
    let theirs =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level)
                 VALUES ('q', 'Their part', 'It is missing.', 'p needs it.', 'change', 'l')",
                [],
            )?;
            tx.execute(
                "INSERT INTO dispatch_holds (project, reason, since, why) VALUES ('q', 'architect', 1, 'Not now.')",
                [],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    items.push(theirs);
    let mut after = None;
    let mut versions = 0;
    let mut log = Vec::new();
    for n in 0..events {
        let event = match dice.roll(100) {
            0..=19 => match a_work(&e, &dice) {
                Some(w) => {
                    e.db.write(Actor::Owner, |tx| {
                        tx.execute(
                            "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', 'owner', ?2)",
                            params![w, "Mind the tests."],
                        )?;
                        Ok(())
                    })
                    .unwrap();
                    format!("the owner interjects into work {w}")
                }
                None => "no work to interject into".into(),
            },
            20..=22 if n > events / 3 => {
                let item = items[dice.roll(items.len() as u64) as usize];
                let projects = e.config.config.projects.clone();
                e.db.write(Actor::Owner, |tx| {
                    engine::ending::cancel_item(tx, item, "no longer needed", "owner", &projects)
                        .map_err(db::Error::Abort)
                })
                .unwrap();
                format!("the owner cancels item {item}")
            }
            23..=25 => match a_work(&e, &dice) {
                Some(w) => {
                    versions += 1;
                    let now = e.flow_now;
                    e.db.write(Actor::Engine, |tx| {
                        tx.execute(
                            "INSERT INTO flow_versions (digest, compiled)
                             SELECT ?2, compiled FROM flow_versions WHERE id = ?1",
                            params![now, format!("older {versions}")],
                        )?;
                        let v = tx.last_insert_rowid();
                        tx.execute("UPDATE works SET flow_version = ?2 WHERE id = ?1", params![w, v])?;
                        Ok(())
                    })
                    .unwrap();
                    format!("work {w} is on an older flow version")
                }
                None => "no work to put on an older flow version".into(),
            },
            // The architects had hours to act, in rounds of a work over by now: a question they hold is past its limit.
            26..=28 => {
                let v = e.flow_now;
                let now = db::now();
                e.db.write(Actor::Engine, |tx| {
                    for project in ["p", "q"] {
                        tx.execute(
                            "INSERT INTO works (item, flow_version, graph, at, ended, project)
                             VALUES (NULL, ?1, 'architect', 'E2', 'over before', ?2)",
                            params![v, project],
                        )?;
                        let arch = tx.last_insert_rowid();
                        tx.execute(
                            "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at,
                               ended_at)
                             VALUES (?1, 1, 'H10', '{}', '{}', 'done', 'c', ?2, ?3)",
                            params![arch, now - 3 * 3_600_000, now],
                        )?;
                    }
                    Ok(())
                })
                .unwrap();
                "the architects spent three hours".into()
            }
            _ => {
                let stepped = e.step(after).unwrap();
                after = stepped.work;
                format!("a step of work {:?}: {:?}", stepped.work, stepped.cell)
            }
        };
        log.push(event);
        let broken = engine::blockers::invariants(&mut e).unwrap();
        assert!(
            broken.is_empty(),
            "seed {seed}, event {n}: {broken:?}\nevents:\n{}\nengine:\n{}",
            log.join("\n"),
            said.lock().unwrap().join("\n")
        );
    }
    // Nothing the engine could not do paused a work.
    let paused = engine::query_json(
        e.db.conn(),
        "SELECT id, paused FROM works WHERE paused_by = 'engine'",
        &[],
        100,
    )
    .unwrap();
    assert!(
        paused.is_empty(),
        "seed {seed}: {paused:?}\nevents:\n{}\nengine:\n{}",
        log.join("\n"),
        said.lock().unwrap().join("\n")
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

macro_rules! random_events {
    ($($name:ident: $seed:expr),*) => {$(
        #[test]
        fn $name() {
            random_events($seed, 70);
        }
    )*};
}

// Random sequences of events (asks of the advisor and the architect, hand-overs between them and between two related
// projects' architects, the architects' replies, flow anomalies, interjections, cancels, ends and works brought to a new
// flow version) leave every chain of blockers closed after each event: every stopped task has
// exactly one open blocker, raised at the cell it stopped at and held by the holder of the cell it stands at; every
// open blocker's holder exists, in whichever project; no ended task has an open blocker; the items waited for form no
// cycle, across projects; and a chain of hand-overs between architects always has a current holder.
random_events!(
    every_chain_of_blockers_closes_under_random_events_1: 1,
    every_chain_of_blockers_closes_under_random_events_2: 2,
    every_chain_of_blockers_closes_under_random_events_3: 3,
    every_chain_of_blockers_closes_under_random_events_4: 4,
    every_chain_of_blockers_closes_under_random_events_5: 5
);
