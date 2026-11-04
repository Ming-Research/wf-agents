//! An item's way from the queue to main.

use super::*;

#[test]
fn an_item_goes_from_the_queue_to_main() {
    let s = setup("main");
    let (mut e, said, item) = start(&s, scripts());
    let mut done = false;
    for _ in 0..200 {
        e.tick().unwrap();
        if let Some((id, _, paused)) = &state(&e, item) {
            assert!(
                paused.is_none(),
                "the work paused: {paused:?}\n{}",
                said.lock().unwrap().join("\n")
            );
            if e.advance(*id).unwrap() == Advance::Stopped {
                done = true;
                break;
            }
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert!(done, "the work did not end:\n{log}");
    let (at, pr): (String, Option<i64>) =
        e.db.conn()
            .query_row("SELECT at, pr FROM works WHERE item = ?1", [item], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!(at, "E1", "{log}");
    assert_eq!(pr, Some(1));
    // Done at its end, the work let go of what it held: no session goes on, and its pull request, merged, is not
    // closed.
    let going: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM sessions s JOIN works w ON w.id = s.work WHERE w.item = ?1 AND s.ended IS NULL",
                [item],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(going, 0, "{log}");
    let closing: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM outbox WHERE kind = 'close_pr'", [], |r| r.get(0))
            .unwrap();
    assert_eq!(closing, 0, "{log}");
    let main = git(
        &s.root,
        &["--git-dir", &s.origin.to_string_lossy(), "show", "main:src/lib.txt"],
    );
    assert_eq!(main, "old\nfeature");
    let cells: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT cell, transition FROM rounds WHERE work = (SELECT id FROM works WHERE item = ?1) ORDER BY n",
        &[&item],
        1000,
    )
    .unwrap()
    .iter()
    .map(|r| {
        format!(
            "{}:{}",
            r["cell"].as_str().unwrap_or_default(),
            r["transition"].as_str().unwrap_or("-")
        )
    })
    .collect();
    for c in [
        "S1:started",
        "R0:done",
        "R1:start",
        "I1:tools",
        "X1:ran",
        "I1:done",
        "T2:done",
        "T3:clear",
        "M1:clear",
        "M2:merged",
        "L2:done",
        "E1:-",
    ] {
        assert!(cells.iter().any(|x| x == c), "no round {c}: {cells:?}\n{log}");
    }
    let rejected: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM rounds WHERE rejected IS NOT NULL", [], |r| {
                r.get(0)
            })
            .unwrap();
    assert_eq!(rejected, 0, "{log}");
    // The review is given what it reviews by commits the worktree has: main as fetched, the commit the branch was cut
    // from, the head under review and the command that reads the change between them; it only reads the worktree.
    let input: String =
        e.db.conn()
            .query_row(
                "SELECT input FROM rounds WHERE cell = 'T2' ORDER BY work, n LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
    let review = db::put_back(e.db.conn(), &serde_json::from_str(&input).unwrap());
    let message = review["message"].as_str().unwrap_or_default();
    let base = given_value(message, "·engine·review_base").expect(message);
    let fork = base["fork_point"].as_str().unwrap_or_default();
    assert_eq!(base["main"], "origin/main", "{base}");
    assert!(
        fork.len() == 40 && base["head"].as_str().is_some_and(|h| h.len() == 40),
        "{base}"
    );
    assert_eq!(
        base["change"],
        format!("git diff {fork}..{}", base["head"].as_str().unwrap())
    );
    let _ = std::fs::remove_dir_all(&s.root);
}
