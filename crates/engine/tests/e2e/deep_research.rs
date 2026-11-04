//! The way of a work through deep research and its review, the owner's approval at the merge gate, and the branch
//! brought to main before the cells that read what main changed.

use super::*;

/// The latest matter that asks the architect to review the work's design.
fn review_matter(e: &Engine, work: i64) -> Option<i64> {
    e.db.conn()
        .query_row(
            "SELECT id FROM architect_matters WHERE work = ?1 AND kind = 'engineer_question' AND detail = 'design_review'
             ORDER BY id DESC LIMIT 1",
            [work],
            |r| r.get(0),
        )
        .ok()
}

/// The architect's reply to a matter, taken off the queue as if a work of the architect had it in hand.
fn architect_replies(e: &mut Engine, matter: i64, text: &str) {
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "UPDATE architect_matters SET handled_work = (SELECT min(id) FROM works WHERE graph = 'architect'),
               handled_round = 1, reply = ?2 WHERE id = ?1",
            params![matter, json!({ "text": text, "why": "It fits the plan." }).to_string()],
        )?;
        Ok(())
    })
    .unwrap();
}

/// The line of the session a work's first round at a cell ran in.
fn line_at(e: &Engine, work: i64, cell: &str) -> String {
    e.db.conn()
        .query_row(
            "SELECT s.line FROM rounds r JOIN sessions s ON s.id = r.session WHERE r.work = ?1 AND r.cell = ?2
             ORDER BY r.n LIMIT 1",
            params![work, cell],
            |r| r.get(0),
        )
        .unwrap()
}

// A work whose research finds it must be designed goes to deep research, in a session of its own on the designer's
// line; its design stops at the review, first for the architect, whose matter asks it at the project's level, then for
// the owner, who is given the design with the architect's comments. Taking the review in, the designer is given both
// answers; the implementation starts a new session on the engineer's line, and the work goes on to merge. Its end
// records how long each stage took and what it cost.
#[test]
fn a_design_is_reviewed_by_the_architect_then_the_owner_and_implemented_in_a_new_session() {
    let s = setup("deep-research");
    let (mut e, said, item) = start(&s, designing());
    step_to(&mut e, item, "D3");
    let (work, _, _) = state(&e, item).unwrap();
    assert_eq!(line_at(&e, work, "D1"), "designer");
    assert_eq!(session_of(&e, work, "D1"), session_of(&e, work, "D2"));
    assert_ne!(session_of(&e, work, "D1"), session_of(&e, work, "R1"));
    // The architect is asked first, and the owner not yet.
    step_until(&mut e, item, &said, "the review's matter", |e| {
        review_matter(e, work).is_some()
    });
    let matter = review_matter(&e, work).unwrap();
    architect_replies(&mut e, matter, "It fits the plan; keep it small.");
    assert_eq!(open_ask(&e, work), None);
    step_until(&mut e, item, &said, "the owner's ask", |e| open_ask(e, work).is_some());
    let (ask, body) = open_ask(&e, work).unwrap();
    assert_eq!(
        body["asked"][1]["value"]["text"], "It fits the plan; keep it small.",
        "{body}"
    );
    assert!(body["asked"][0]["value"]["design"].is_object(), "{body}");
    owner_answers(&mut e, ask, "Approved; go ahead.");
    step_to(&mut e, item, "I1");
    let review = given_value(&message_at(&e, work, "D4", true), "·engine·design_review").unwrap();
    assert_eq!(
        review["architect"]["text"], "It fits the plan; keep it small.",
        "{review}"
    );
    assert_eq!(review["owner"]["text"], "Approved; go ahead.", "{review}");
    step_until(&mut e, item, &said, "the merge", |e| {
        transitions(e, item, "M2").iter().any(|t| t == "merged")
    });
    assert_eq!(line_at(&e, work, "I1"), "engineer");
    assert_ne!(session_of(&e, work, "I1"), session_of(&e, work, "R1"));
    let mut review = transitions(&e, item, "D3");
    review.dedup();
    assert_eq!(review, ["architect", "owner", "reviewed"]);
    step_until(&mut e, item, &said, "the end", |e| ended_at(e, item, "E1").len() == 1);
    let numbers = &output_at(&e, work, "E1")["numbers"];
    let stages: Vec<&str> = numbers["stages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|x| x["stage"].as_str())
        .collect();
    for stage in ["research", "design", "implement", "test", "merge"] {
        assert!(stages.contains(&stage), "{numbers}");
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The output of a work's last round at a cell.
fn output_at(e: &Engine, work: i64, cell: &str) -> Value {
    let output: String =
        e.db.conn()
            .query_row(
                "SELECT output FROM rounds WHERE work = ?1 AND cell = ?2 ORDER BY n DESC LIMIT 1",
                params![work, cell],
                |r| r.get(0),
            )
            .unwrap();
    serde_json::from_str(&output).unwrap()
}

// A change under a path the project has the owner approve waits at the merge gate for the owner: the gate asks, naming
// the files. An answer other than approved goes to the engineer as the owner's words, and the work chooses the way on;
// carried on, it comes back to the gate, which asks again, and the reply approved lets the work merge.
#[test]
fn a_change_the_owner_approves_waits_at_the_merge_gate_until_the_owner_replies_approved() {
    let s = setup("owner-approves");
    owner_approves_src(&s);
    let mut agents = scripts();
    agents.insert(
        "A4",
        Box::new(|_: &Request, _: usize| (vec![("outcome.next", "carry_on")], Vec::new())),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the gate's ask", |e| {
        state(e, item).is_some_and(|(w, at, _)| at == "M1" && open_ask(e, w).is_some())
    });
    let (work, _, _) = state(&e, item).unwrap();
    let (ask, body) = open_ask(&e, work).unwrap();
    assert_eq!(body["asked"][0]["value"]["files"], json!(["src/lib.txt"]), "{body}");
    owner_answers(&mut e, ask, "Keep the old line too.");
    step_until(&mut e, item, &said, "the gate's second ask", |e| {
        open_ask(e, work).is_some_and(|(id, _)| id != ask)
    });
    let relayed: String =
        e.db.conn()
            .query_row(
                "SELECT text FROM interjections WHERE work = ?1 ORDER BY id DESC LIMIT 1",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert!(
        relayed.ends_with("the owner replied: Keep the old line too."),
        "{relayed}"
    );
    let (again, _) = open_ask(&e, work).unwrap();
    owner_answers(&mut e, again, "approved");
    step_until(&mut e, item, &said, "the merge", |e| {
        transitions(e, item, "M2").iter().any(|t| t == "merged")
    });
    let mut gate = transitions(&e, item, "M1");
    gate.dedup();
    assert_eq!(gate, ["owner_approves", "owner_answered", "owner_approves", "clear"]);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work comes to the implementation after main moved on: the engine merges main into its branch before the round,
// and the round is told the files main changed.
#[test]
fn the_implementation_is_given_what_main_changed_since_the_branch_took_it() {
    let s = setup("catch-up");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "I1");
    main_moves_on(&s.root, &[("docs/new.md", "A new page.\n")], "Add a page");
    step_until(&mut e, item, &said, "the implementation", |e| {
        !transitions(e, item, "I1").is_empty()
    });
    let (work, _, _) = state(&e, item).unwrap();
    let brought = given_value(&message_at(&e, work, "I1", false), "·engine·main_changes").unwrap();
    assert_eq!(brought, json!({ "merged": true, "files": ["docs/new.md"] }));
    let wt = worktree_of(&e, work);
    assert!(wt.join("docs/new.md").exists());
    let _ = std::fs::remove_dir_all(&s.root);
}
