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
// research.
#[test]
fn the_history_is_gathered_by_the_reader_of_the_line_librarian() {
    let s = setup("history");
    let mut agents = scripts();
    agents.insert(
        "R0",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("outcome.brief", json!("The problem's history."))])),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "I1");
    let (work, _, _) = state(&e, item).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(
        message_at(&e, work, "R1", false).contains("The problem's history."),
        "{log}"
    );
    let session = |cell: &str| session_of(&e, work, cell);
    assert_ne!(session("R0"), session("R1"));
    let line: String =
        e.db.conn()
            .query_row("SELECT line FROM sessions WHERE id = ?1", [session("R0")], |r| r.get(0))
            .unwrap();
    assert_eq!(line, "librarian");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The gatherer reads the live database by the address it is given: the database is in WAL mode, with the engine's
// connection open, and what was committed to its log and not yet to the file is read too. It is given for git the
// directory its own round runs in, where the permission check lets git run, and the project's main as git has it there,
// not the project's checkout (#250).
#[test]
fn the_history_s_address_reads_the_live_database_in_wal_mode() {
    let s = setup("history-wal");
    let ran_in: Arc<Mutex<Option<PathBuf>>> = Arc::default();
    let mut agents = scripts();
    let seen = ran_in.clone();
    agents.insert(
        "R0",
        Box::new(move |r: &Request, _: usize| {
            *seen.lock().unwrap() = Some(r.worktree.clone());
            (Vec::new(), vec![("outcome.brief", json!("The problem's history."))])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
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
    let ran_in = ran_in.lock().unwrap().clone().expect(&log);
    assert_eq!(sources["git"]["directory"].as_str(), ran_in.to_str(), "{sources}");
    assert!(sources.get("checkout").is_none(), "{sources}");
    // Git runs there: the main it names is a commit of the repository the directory is a worktree of, the project's.
    let main = sources["git"]["main"].as_str().unwrap();
    assert_eq!(main, "origin/main", "{sources}");
    assert_eq!(
        git(&ran_in, &["rev-parse", "--verify", &format!("{main}^{{commit}}")]),
        git(&s.origin, &["rev-parse", "main"]),
        "{sources}"
    );
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
