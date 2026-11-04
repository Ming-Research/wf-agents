//! Observations: the search of them, and those sent back.

use super::*;

/// A work of the architect on a side report, whose cell `H5` has the project's open observations searched, the
/// observer handing back `candidates`, and then merges the report into observation 1, run to its end. Gives the engine,
/// the architect's work, what the engine said and the setup.
fn searched(name: &str, candidates: Value) -> (Engine, i64, String, Setup) {
    let s = setup(name);
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    let levels = "[agents.levels.cheap]\ndesc = \"searching\"\ncli = \"claude\"\nmodel = \"m-cheap\"\n\
                  [agents]\nobserver = \"cheap\"\n";
    write(&s.data, "install.toml", &format!("{install}\n{levels}"));
    git(&s.data, &["commit", "-q", "-am", "the observer's level"]);
    let mut agents = scripts();
    agents.insert(
        "H5",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                let search = json!({ "look_for": "A slow parse of large files.", "tags": [], "purpose": "same" });
                return (vec![("outcome", "search")], vec![("outcome.search", search)]);
            }
            let merge = json!({ "kind": "merge", "into": 1, "seen": "Seen again in the parser.", "refs": [],
                                "reason": "The same slowness." });
            (
                vec![("outcome.take.way", "merged")],
                vec![("outcome.effects", json!({ "observation_changes": [merge] }))],
            )
        }),
    );
    agents.insert(
        "X2",
        Box::new(move |_: &Request, _: usize| (Vec::new(), vec![("outcome.candidates", candidates.clone())])),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "INSERT INTO observations (project, what, source) VALUES ('p', 'Parsing large files is slow.', 'A report.')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    // A side report of a work of the project, as the engine queues one.
    e.db.write(Actor::Engine, |tx| {
        let m = db::Matter {
            project: "p".into(),
            kind: "report".into(),
            detail: Some("other".into()),
            what: "A side report: the parser is slow on large files.".into(),
            body: json!({ "report": { "kind": "other", "what": "The parser is slow on large files.", "urgent": false } }),
            item: Some(1),
            raised: None,
        };
        db::queue_matter(tx, &m)
    })
    .unwrap();
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    (e, arch, log, s)
}

// A cell on a matter's path has the project's open observations searched, by the observer at its own level and line,
// and goes on with the candidates: here a report taken in is merged into the observation found, reported once more.
#[test]
fn the_search_of_observations_hands_back_candidates() {
    let (e, arch, log, s) = searched("search", json!([{ "id": 1, "why": "It records the same slowness." }]));
    let path = path_of(&e, arch);
    let at = path.iter().position(|c| c == "X2").expect(&log);
    assert_eq!(
        &path[at - 4..at + 3],
        ["H4", "Q0", "Q2", "H5", "X2", "H5", "B1"],
        "{log}"
    );
    let (line, model): (String, String) =
        e.db.conn()
            .query_row(
                "SELECT s.line, r.model FROM rounds r JOIN sessions s ON s.id = r.session
                 WHERE r.work = ?1 AND r.cell = 'X2'",
                [arch],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!((line.as_str(), model.as_str()), ("observer", "m-cheap"));
    let back = message_at(&e, arch, "H5", true);
    assert!(back.contains("It records the same slowness."), "{back}");
    // The observer writes what the owner reads in the owner's language, as every line does: its forms, in the system
    // prompt its session starts with or in its message, say which.
    let input = input_at(&e, arch, "X2", false);
    let search = format!(
        "{}\n{}",
        system_text(&input).unwrap_or_default(),
        input["message"].as_str().unwrap_or_default()
    );
    assert!(search.contains("`owner.language` (English)"), "{search}");
    let times: (i64, String) =
        e.db.conn()
            .query_row("SELECT times_reported, refs FROM observations WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!(times.0, 2, "{log}");
    assert!(times.1.contains("Seen again in the parser."), "{}", times.1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A search that found nothing comes back saying so: the cell that asked has the search's results listed as none,
// where before the search it had them not listed at all, and does not take them for not given.
#[test]
fn a_search_that_found_nothing_comes_back_as_none() {
    let (mut e, arch, log, s) = searched("search-none", json!([]));
    let w = e.work(arch).unwrap();
    let g = e.graph_of(&w).unwrap();
    let number = g
        .cell("H5")
        .unwrap()
        .inputs
        .iter()
        .filter(|x| !x.from.iter().all(|s| *s == flow::format::Src::Itself))
        .position(|x| x.name.as_deref() == Some("Search results"))
        .unwrap()
        + 1;
    let listed = |m: &str| work_from(m).into_iter().find(|l| l.starts_with(&format!("{number}. ")));
    let (asked, back) = (message_at(&e, arch, "H5", false), message_at(&e, arch, "H5", true));
    assert_eq!(listed(&asked), None, "{asked}");
    assert_eq!(listed(&back), Some(format!("{number}. none")), "{back}\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// An observation an item takes in is folded into that item, not closed: a close that says it was folded is sent back,
// saying to fold it.
#[test]
fn an_observation_closed_as_folded_is_sent_back_to_be_folded() {
    let s = setup("close-folded");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let close = json!({ "kind": "close", "id": 1, "why": "folded", "text": "The new item takes it in.",
                                "reason": "An item does it." });
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "observation_changes": [close] }))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "INSERT INTO observations (project, what, source) VALUES ('p', 'Parsing is slow.', 'A report.')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Fold the slowness into an item.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let rejected: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'H8' AND rejected IS NOT NULL",
        &[&arch],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| r["rejected"].to_string())
    .collect();
    assert_eq!(rejected.len(), 1, "{rejected:?}");
    assert!(rejected[0].contains("fold_observations"), "{rejected:?}");
    let closed: Option<String> =
        e.db.conn()
            .query_row("SELECT closed FROM observations WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    assert_eq!(closed, None);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A new observation of a project that has tags carries one of them at least, and a title as wide as an item's may be
// (here 24 at most, a Chinese character counting 2): one with no tag and a title of 13 Chinese characters is sent back,
// saying which tags there are and how wide the title is, and the return handed in again with a tag and a title of 12
// records it, its title with it.
#[test]
fn an_observation_with_no_tag_or_a_wide_title_is_sent_back() {
    let s = setup_titled("observation-untagged");
    let project = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    write(
        &s.data,
        "projects/p.toml",
        &project.replace(
            "[ci]",
            "observation_tags = [{ name = \"perf\", desc = \"How fast it runs\" }]\n[ci]",
        ),
    );
    git(&s.data, &["commit", "-q", "-am", "observation tags"]);
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            if n > 2 {
                return (Vec::new(), Vec::new());
            }
            let (tags, title) = if n == 1 {
                (json!([]), chinese(13))
            } else {
                (json!(["perf"]), chinese(12))
            };
            let new = json!({ "kind": "new", "title": title, "what": "Parsing large files is slow.", "tags": tags,
                              "source": "The owner's words.", "refs": [], "reason": "Worth keeping." });
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "observation_changes": [new] }))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Parsing large files is slow; keep it in mind.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let rejected: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'H8' AND rejected IS NOT NULL",
        &[&arch],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| r["rejected"].to_string())
    .collect();
    assert_eq!(rejected.len(), 1, "{rejected:?}");
    assert!(
        rejected[0].contains("observation change 1: it carries no tag") && rejected[0].contains("perf"),
        "{rejected:?}"
    );
    assert!(
        rejected[0].contains("observation change 1: the title is 26 wide and a title is 24 wide at most"),
        "{rejected:?}"
    );
    let recorded: Vec<(Value, Value)> =
        engine::query_json(e.db.conn(), "SELECT title, tags FROM observations", &[], 10)
            .unwrap()
            .iter()
            .map(|r| (r["title"].clone(), r["tags"].clone()))
            .collect();
    assert_eq!(recorded, [(json!(chinese(12)), json!(["perf"]))], "{recorded:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}
