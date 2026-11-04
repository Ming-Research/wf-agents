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

// The history is gathered before the research and before the design, each in a round of the reader's line at the lowest
// level, and given to the cell after it; a look-up goes to the reader and back to the cell that asked, in the same
// session, and is given to it.
#[test]
fn the_history_is_gathered_and_looked_up_by_the_reader_of_the_line_librarian() {
    let s = setup("history");
    let mut agents = scripts();
    agents.insert(
        "G1",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("outcome.brief", json!("The problem's history."))])),
    );
    agents.insert(
        "G2",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("outcome.brief", json!("The code's history."))])),
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
    step_to(&mut e, item, "D2");
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
    assert!(
        message_at(&e, work, "D1", false).contains("The code's history."),
        "{log}"
    );
    assert!(message_at(&e, work, "X5", false).contains("Why is it so?"), "{log}");
    let session = |cell: &str| session_of(&e, work, cell);
    assert_eq!(session("G1"), session("G2"));
    assert_eq!(session("G1"), session("X5"));
    assert_ne!(session("G1"), session("R1"));
    let line: String =
        e.db.conn()
            .query_row("SELECT line FROM sessions WHERE id = ?1", [session("G1")], |r| r.get(0))
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
    let sources = given_value(&message_at(&e, work, "G1", false), "·engine·history_sources").expect(&log);
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

// The history of the files a change touches is gathered once, by the reader of the history, between the blind review
// and the first table review, from the change's commits and where the history is. The table review is given it, and
// is still given it at the re-review after a fix, which does not gather it again. The blind review is given neither
// the brief nor what the table review is told to do with it.
#[test]
fn the_history_of_the_changed_files_is_gathered_once_and_the_table_review_is_given_it() {
    let s = setup("history-of-the-change");
    let mut agents = scripts();
    let brief = "Dropped: a second table, for lack of room (#12), bears on src/lib.txt.";
    agents.insert(
        "G3",
        Box::new(move |_: &Request, _: usize| (Vec::new(), vec![("outcome.brief", json!(brief))])),
    );
    agents.insert(
        "T4",
        review_with_a_finding(vec![json!([{ "fid": "R1", "verdict": "closed" }])]),
    );
    agents.insert(
        "T6",
        fix_answering(vec![
            json!([{ "answer": "fixed_outside_commit", "id": "F1", "where": "The pull request description." }]),
        ]),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the review clearing the work", |e| {
        transitions(e, item, "M1").iter().any(|t| t == "clear")
    });
    let (work, _, _) = state(&e, item).unwrap();
    let log = said.lock().unwrap().join("\n");
    // One gathering, between the blind review and the first table review; two table reviews, the second after a fix.
    assert_eq!(transitions(&e, item, "G3"), ["done"], "{log}");
    assert_eq!(transitions(&e, item, "T4"), ["done", "done"], "{log}");
    let path = path_of(&e, work);
    let blind = path.iter().position(|c| c == "T3").unwrap();
    assert_eq!(path[blind..blind + 3].to_vec(), ["T3", "G3", "T4"], "{path:?}");
    // The table review is given the brief and told what to do with it; the blind review neither.
    let first = message_at(&e, work, "T4", false);
    assert!(first.contains(brief), "{first}");
    assert!(first.contains("records as dropped"), "{first}");
    assert!(first.contains("`history`"), "{first}");
    let blind = message_at(&e, work, "T3", false);
    assert!(
        !blind.contains(brief) && !blind.contains("records as dropped"),
        "{blind}"
    );
    // The re-review has the brief still, which its session holds: it is referred to, not given again.
    let given_to = |last: bool| given_ending(&input_at(&e, work, "T4", last), "·G3·brief");
    assert_eq!((given_to(false), given_to(true)), (Some(true), Some(false)), "{log}");
    // The gatherer is given the change by commits the worktree has. Where the history is, it is given as the reader's
    // session was at the first gathering, which that session holds: the round refers to it.
    let message = message_at(&e, work, "G3", false);
    let base = given_value(&message, "·engine·review_base").expect(&message);
    let fork = base["fork_point"].as_str().unwrap_or_default();
    let head = base["head"].as_str().unwrap_or_default();
    assert!(fork.len() == 40 && head.len() == 40, "{base}");
    assert_eq!(base["change"], format!("git diff {fork}..{head}"));
    assert!(base.get("last_reviewed").is_none(), "{base}");
    let sources = given_value(&message_at(&e, work, "G1", false), "·engine·history_sources").expect(&log);
    assert_eq!(sources["database"], config::database(&s.data).display().to_string());
    assert!(sources["checkout"].is_string(), "{sources}");
    let at_g3 = given_ending(&input_at(&e, work, "G3", false), "·engine·history_sources");
    assert_eq!(at_g3, Some(false), "{message}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// What the sessions hold decides what a round is given. When the reader's session was compacted before the gathering
// of the change's history, that round is given where the history is, and when the review's session was compacted
// before the re-review, that round is given the brief again: the review and the gathering do not rest on a session
// that holds what they need, they are given it.
#[test]
fn the_gathering_and_the_re_review_are_given_what_a_compacted_session_no_longer_holds() {
    let s = setup("history-after-compaction");
    let mut agents = scripts();
    let brief = "Dropped: a second table, for lack of room (#12), bears on src/lib.txt.";
    agents.insert(
        "G2",
        Box::new(|_: &Request, _: usize| {
            (
                Vec::new(),
                vec![
                    ("outcome.brief", json!("The code's history.")),
                    ("@compactions", json!(1)),
                ],
            )
        }),
    );
    agents.insert(
        "G3",
        Box::new(move |_: &Request, _: usize| (Vec::new(), vec![("outcome.brief", json!(brief))])),
    );
    let review = review_with_a_finding(vec![json!([{ "fid": "R1", "verdict": "closed" }])]);
    agents.insert(
        "T4",
        Box::new(move |r: &Request, n: usize| {
            let (set, mut put) = review(r, n);
            if n == 1 {
                put.push(("@compactions", json!(1)));
            }
            (set, put)
        }),
    );
    agents.insert(
        "T6",
        fix_answering(vec![
            json!([{ "answer": "fixed_outside_commit", "id": "F1", "where": "The pull request description." }]),
        ]),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the review clearing the work", |e| {
        transitions(e, item, "M1").iter().any(|t| t == "clear")
    });
    let (work, _, _) = state(&e, item).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(transitions(&e, item, "G3"), ["done"], "{log}");
    assert_eq!(transitions(&e, item, "T4"), ["done", "done"], "{log}");
    // The gathering is given the change by the engine's review base, and where the history is, as the first gathering
    // gave it, the value the same.
    let at_g3 = input_at(&e, work, "G3", false);
    assert_eq!(given_ending(&at_g3, "·engine·history_sources"), Some(true), "{log}");
    let message = message_at(&e, work, "G3", false);
    let base = given_value(&message, "·engine·review_base").expect(&message);
    assert!(
        base["change"].as_str().is_some_and(|c| c.starts_with("git diff ")),
        "{base}"
    );
    let sources = given_value(&message, "·engine·history_sources").expect(&message);
    assert_eq!(sources["database"], config::database(&s.data).display().to_string());
    assert!(sources["checkout"].is_string(), "{sources}");
    let first = given_value(&message_at(&e, work, "G1", false), "·engine·history_sources").expect(&log);
    assert_eq!(sources, first);
    // The re-review is given the brief again, in its text.
    let given_to = |last: bool| given_ending(&input_at(&e, work, "T4", last), "·G3·brief");
    assert_eq!((given_to(false), given_to(true)), (Some(true), Some(true)), "{log}");
    assert!(message_at(&e, work, "T4", true).contains(brief), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The table review can have more looked up, as the research can: the look-up goes to the reader and back to the table
// review, in the reader's session, which the first gathering started with the rule for gathering history, once; the
// table review is given what was found, in the round after.
#[test]
fn the_table_review_has_the_history_looked_up_by_the_reader_in_its_session() {
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
    // The first round reads the table, so that the later round's answers know its columns, and asks instead.
    let table = answers_columns(|_| vec![("outcome.no_adversarial", "stands")]);
    agents.insert(
        "T4",
        Box::new(move |r: &Request, n: usize| {
            let answered = table(r, n);
            if n > 1 {
                return answered;
            }
            (
                vec![("outcome", "history")],
                vec![("outcome.history", json!("Was a second table dropped for src/lib.txt?"))],
            )
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the table review answering", |e| {
        transitions(e, item, "T4").iter().any(|t| t == "done")
    });
    let (work, _, _) = state(&e, item).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(transitions(&e, item, "T4"), ["history", "done"], "{log}");
    assert!(
        message_at(&e, work, "X5", false).contains("Was a second table dropped for src/lib.txt?"),
        "{log}"
    );
    assert!(
        message_at(&e, work, "T4", true).contains("What was looked up for the review."),
        "{log}"
    );
    let session = |cell: &str| session_of(&e, work, cell);
    for cell in ["G2", "G3", "X5"] {
        assert_eq!(session("G1"), session(cell), "{cell}");
    }
    assert_ne!(session("G1"), session("T4"));
    // The rule for gathering, with the check of paths, is in the system prompt the reader's session started with, once,
    // and every later round of it carries that prompt: no cell's own text says it again.
    let opening = "You gather the history an agent needs before it works.";
    let started = input_at(&e, work, "G1", false);
    assert!(in_system(&started, "rule:history"), "{}", started["system"]);
    let system = system_text(&started).expect("the reader's system prompt");
    assert_eq!(system.matches(opening).count(), 1, "{system}");
    assert!(
        system.contains("put `(gone)` after a path the files do not have"),
        "{system}"
    );
    for cell in ["G1", "G2", "G3", "X5"] {
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
