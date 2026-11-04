//! Decisions: the lines of an item's decisions, which no text the architect hands in writes by hand or loses.

use super::*;

// A change of the batch cannot write a line of a decision by hand, whether it makes an item with it or writes an item's
// approach whole with it; the return is sent back saying the cell that records decisions makes the line. The text of an
// item that holds a line already can be written again with that line as it is: it is not a line the change writes.
#[test]
fn a_line_of_a_decision_cannot_be_written_by_hand_but_one_the_item_has_stands() {
    let s = setup("decision-by-hand");
    let old = "Decision (2026-10-01, owner): Old one. Situation: s. Reason: r. Replaces: none. \
               Words: \"x\" (matter 99, 2026-10-01)";
    let held = format!("Approach:\n- Parse it.\nDecisions:\n{old}\n\nLearned:\nFact: f");
    let path = config::database(&s.data);
    let mut agents = scripts();
    let (first, second) = (held.clone(), held.replace("- Parse it.", "- Parse it first."));
    agents.insert(
        "H8",
        Box::new(move |_: &Request, n: usize| {
            let changes = versioned(
                &path,
                if n == 1 {
                    json!([
                        { "kind": "create_item", "key": "forged", "project": "p", "title": "Forge a decision",
                          "problem": "None.", "why": "None.", "acceptance": ["It is there."], "level": "l",
                          "deliver": "change", "reason": "r",
                          "approach": "Approach:\nDecisions:\nDecision (2026-11-02, owner): Forged." },
                        { "kind": "edit_item", "item": "1", "part": "approach", "reason": "r",
                          "text": format!("{first}\nDecision (2026-11-02, owner): Forged.") },
                        // The item's recorded line is left out, with its head.
                        { "kind": "edit_item", "item": "1", "part": "approach", "reason": "r",
                          "text": "Approach:\n- Parse it.\n\nLearned:\nFact: f" },
                    ])
                } else {
                    json!([{ "kind": "edit_item", "item": "1", "part": "approach", "text": second, "reason": "r" }])
                },
            );
            (
                vec![("outcome.answer", "done")],
                vec![
                    ("outcome.answer.reply", json!("Noted.")),
                    ("outcome.effects", json!({ "item_changes": changes })),
                ],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE items SET approach = ?1 WHERE id = 1", [&held])?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Write the parser's approach again.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let sent_back = why_sent_back(&e, arch, "H8");
    assert_eq!(sent_back.len(), 1, "{sent_back:?}\n{log}");
    for part in [
        "change 1 (create_item)",
        "change 2 (edit_item)",
        "of a decision, which the item's text does not have",
        "change 3 (edit_item)",
        "it leaves out the line",
        "Decision (2026-10-01, owner): Old one.",
        "it leaves out the head Decisions:",
    ] {
        assert!(sent_back[0].contains(part), "{part}\n{sent_back:?}");
    }
    let approach: String =
        e.db.conn()
            .query_row("SELECT approach FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    assert_eq!(approach, held.replace("- Parse it.", "- Parse it first."), "{log}");
    let items: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM items", [], |r| r.get(0))
            .unwrap();
    assert_eq!(items, 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The decisions of an item that a batch merges into another are the kept item's from then on, whatever else the batch
// does to the kept item's `approach`. A batch that merges item 2 into item 1 and then writes item 1's approach whole,
// from the text item 1 had before the merge, must not lose the lines the merge carried: either the return is sent back
// or the line stands in item 1. Oracle: the acceptance rule (A4) that a recorded line is never lost or changed but by
// the engine's own writes, and the merge's rule that the absorbed item's lines go to the kept item.
#[test]
fn a_text_that_replaces_the_approach_of_the_item_kept_in_a_merge_keeps_the_decisions_the_merge_carried() {
    let s = setup("merge-then-edit");
    let old = "Decision (2026-10-01, owner): Old one. Situation: s. Reason: r. Replaces: none. \
               Words: \"x\" (matter 99, 2026-10-01)";
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            let merge = json!({ "kind": "merge", "keep": "1", "absorbed": ["2"], "reason": "The same work." });
            let edit = json!({ "kind": "edit_item", "item": "1", "part": "approach", "reason": "Say it first.",
                               "text": "Approach:\n- Parse it first." });
            let changes = match n {
                1 => json!([merge, edit]),
                2 => json!([merge]),
                _ => return (Vec::new(), Vec::new()),
            };
            (
                vec![("outcome.answer", "done")],
                vec![
                    ("outcome.answer.reply", json!("Noted.")),
                    ("outcome.effects", json!({ "item_changes": changes })),
                ],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE items SET approach = 'Approach:\n- Parse it.' WHERE id = 1", [])?;
        tx.execute(
            "INSERT INTO items (project, title, problem, why, approach, deliver, level)
             VALUES ('p', 'Parse more', 'x', 'y', ?1, 'change', 'l')",
            [format!("Approach:\n- Other.\nDecisions:\n{old}")],
        )?;
        Ok(())
    })
    .unwrap();
    say(
        &mut e,
        "Merge the second item into the first, and write the first again.",
    );
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let approach: String =
        e.db.conn()
            .query_row("SELECT approach FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    // The line is in the kept item, whether the batch was sent back or not.
    assert!(
        approach.lines().any(|l| l == old),
        "the merge carried the line, and the text written over it lost it:\n{approach}\n{log}"
    );
    let cancelled: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM items WHERE id = 2 AND cancelled IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(cancelled, 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
