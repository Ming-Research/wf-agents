//! What a work learned, and the history gathered for a line and looked up by its reader.

use super::*;

// What the work learned goes into its item's approach once it merged, with the commit it merged, through the one write
// path, so that the changes keep it.
#[test]
fn what_a_work_learned_goes_into_its_item_with_its_commits() {
    let s = setup("learned");
    let mut agents = scripts();
    agents.insert(
        "L2",
        Box::new(|_: &Request, _: usize| {
            (
                Vec::new(),
                vec![(
                    "outcome.learned",
                    json!("Fact (2026-11-02): One table is enough (R1-2)"),
                )],
            )
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let done = |e: &mut Engine| !ended_at(e, item, "L2").is_empty();
    assert!(steps_until(&mut e, 200, done), "{}", said.lock().unwrap().join("\n"));
    let log = said.lock().unwrap().join("\n");
    let approach: String =
        e.db.conn()
            .query_row("SELECT approach FROM items WHERE id = ?1", [item], |r| r.get(0))
            .unwrap();
    let merged: String =
        e.db.conn()
            .query_row(
                "SELECT json_extract(r.output, '$.result.merge_commit') FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.item = ?1 AND r.cell = 'M2' AND r.transition = 'merged'",
                [item],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(
        approach,
        format!("Learned:\nFact (2026-11-02): One table is enough (R1-2)\nCommits: {merged}"),
        "{log}"
    );
    let kept: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM changes WHERE table_name = 'items' AND json_extract(new_row, '$.approach') = ?1",
                [&approach],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(kept, 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The background is gathered before the research, in a round of the reader's line at the lowest level, and given to the
// research; a look-up goes to the reader and back to the cell that asked, in the same session, and is given to it.
#[test]
fn the_history_is_gathered_and_looked_up_by_the_reader_of_the_line_librarian() {
    let s = setup("history");
    let mut agents = scripts();
    agents.insert(
        "R0",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("outcome.brief", json!("The problem's history."))])),
    );
    agents.insert(
        "X5",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("outcome.brief", json!("What was looked up."))])),
    );
    agents.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                (
                    vec![("outcome", "history")],
                    vec![("outcome.history", json!("Why is it so?"))],
                )
            } else {
                (Vec::new(), Vec::new())
            }
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "I1");
    let (work, _, _) = state(&e, item).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_at(&e, item, "X5").len(), 1, "{log}");
    assert!(
        message_at(&e, work, "R1", false).contains("The problem's history."),
        "{log}"
    );
    assert!(
        message_at(&e, work, "R1", true).contains("What was looked up."),
        "{log}"
    );
    assert!(message_at(&e, work, "X5", false).contains("Why is it so?"), "{log}");
    let session = |cell: &str| session_of(&e, work, cell);
    assert_eq!(session("R0"), session("X5"));
    assert_ne!(session("R0"), session("R1"));
    let line: String =
        e.db.conn()
            .query_row("SELECT line FROM sessions WHERE id = ?1", [session("R0")], |r| r.get(0))
            .unwrap();
    assert_eq!(line, "librarian");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The gatherer reads the live database by the address it is given: the database is in WAL mode, with the engine's
// connection open, and what was committed to its log and not yet to the file is read too.
#[test]
fn the_history_s_address_reads_the_live_database_in_wal_mode() {
    let s = setup("history-wal");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "R1");
    let (work, _, _) = state(&e, item).unwrap();
    let log = said.lock().unwrap().join("\n");
    let sources = given_value(&message_at(&e, work, "R0", false), "·engine·history_sources").expect(&log);
    let database = sources["database"].as_str().unwrap().to_string();
    assert_eq!(database, config::database(&s.data).display().to_string());
    assert!(
        sources["read"]
            .as_str()
            .unwrap()
            .starts_with("sqlite3 -safe -readonly '"),
        "{sources}"
    );
    assert!(sources["checkout"].is_string(), "{sources}");
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE items SET title = 'Only in the log' WHERE id = ?1", [item])?;
        Ok(())
    })
    .unwrap();
    let wal = std::fs::metadata(format!("{database}-wal")).unwrap();
    assert!(wal.len() > 0);
    let out = Command::new("sqlite3")
        .args([
            "-safe",
            "-readonly",
            &database,
            &format!("SELECT title FROM items WHERE id = {item}"),
        ])
        .output()
        .expect("sqlite3 runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "Only in the log");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The review can have more looked up, as the research can: the look-up goes to the reader and back to the review, in the
// reader's session, which the gathering of the background started with the rule for gathering history, once; the review
// is given what was found, in the round after.
#[test]
fn the_review_has_the_history_looked_up_by_the_reader_in_its_session() {
    let s = setup("history-for-the-review");
    let mut agents = scripts();
    agents.insert(
        "X5",
        Box::new(|_: &Request, _: usize| {
            (
                Vec::new(),
                vec![("outcome.brief", json!("What was looked up for the review."))],
            )
        }),
    );
    // The first round asks instead of reviewing; the round after reviews.
    agents.insert(
        "T2",
        Box::new(move |_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            (
                vec![("outcome", "history")],
                vec![("outcome.history", json!("Was a second table dropped for src/lib.txt?"))],
            )
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the review answering", |e| {
        transitions(e, item, "T2").iter().any(|t| t == "done")
    });
    let (work, _, _) = state(&e, item).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(transitions(&e, item, "T2"), ["history", "done"], "{log}");
    assert!(
        message_at(&e, work, "X5", false).contains("Was a second table dropped for src/lib.txt?"),
        "{log}"
    );
    assert!(
        message_at(&e, work, "T2", true).contains("What was looked up for the review."),
        "{log}"
    );
    let session = |cell: &str| session_of(&e, work, cell);
    assert_eq!(session("R0"), session("X5"));
    assert_ne!(session("R0"), session("T2"));
    // The rule for gathering, with the check of paths, is in the system prompt the reader's session started with, once,
    // and every later round of it carries that prompt: no cell's own text says it again.
    let opening = "You gather the history an agent needs before it works.";
    let started = input_at(&e, work, "R0", false);
    assert!(in_system(&started, "rule:history"), "{}", started["system"]);
    let system = system_text(&started).expect("the reader's system prompt");
    assert_eq!(system.matches(opening).count(), 1, "{system}");
    assert!(
        system.contains("put `(gone)` after a path the files do not have"),
        "{system}"
    );
    for cell in ["R0", "X5"] {
        assert_eq!(
            system_text(&input_at(&e, work, cell, false)),
            Some(system.clone()),
            "{cell}"
        );
        assert!(!message_at(&e, work, cell, false).contains(opening), "{cell}");
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architect's look-up of the history is given the same rule for gathering history as the engineer's reader is, by
// name, since its line is the search of observations as well: the check of paths reaches every gatherer.
#[test]
fn the_architect_s_look_up_of_the_history_is_given_the_rule_for_gathering_it() {
    let s = setup("architect-history");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                return (
                    vec![("outcome", "history")],
                    vec![("outcome.history", json!("Why is the parser's item so?"))],
                );
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    db::say_to_architect(
        &mut e.db,
        "p",
        None,
        None,
        "Why is the parser's item so?",
        &[],
        db::Speaker::Owner,
    )
    .unwrap();
    let arch = e.step(None).unwrap().work.unwrap();
    let looked_up = |e: &mut Engine| -> bool {
        let n: i64 =
            e.db.conn()
                .query_row(
                    "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'X5' AND ended_at IS NOT NULL",
                    [arch],
                    |r| r.get(0),
                )
                .unwrap();
        n > 0
    };
    assert!(
        steps_until(&mut e, 40, looked_up),
        "{}",
        said.lock().unwrap().join("\n")
    );
    // The rule is in the system prompt the look-up's session starts with or in its message, as every line's rules are.
    let look_up = input_at(&e, arch, "X5", false);
    let text = format!(
        "{}\n{}",
        system_text(&look_up).unwrap_or_default(),
        look_up["message"].as_str().unwrap_or_default()
    );
    assert!(
        text.contains("put `(gone)` after a path the files do not have"),
        "{text}"
    );
    assert!(text.contains("(gone: moved to <path> in <commit>)"), "{text}");
    let _ = std::fs::remove_dir_all(&s.root);
}
