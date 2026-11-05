//! Observations: those sent back.

use super::*;

// An observation an item takes in is folded into that item, not closed: a close that says it was folded is sent back,
// saying to fold it.
#[test]
fn an_observation_closed_as_folded_is_sent_back_to_be_folded() {
    let s = setup("close-folded");
    let mut agents = scripts();
    agents.insert(
        "H1",
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
        "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'H1' AND rejected IS NOT NULL",
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
        "H1",
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
        "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'H1' AND rejected IS NOT NULL",
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
